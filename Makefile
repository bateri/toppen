# toppen is one native file: the program (src/program/, Rust, on the pencere crate) with the page
# (src/view/, a page of pencere's kit) and the pictures (src/assets/) carried in it by build.rs. The
# gate and `make dev` are here.
.PHONY: pin fmt lint test test-page test-tools check-timing shots check build dist sums release-prepare rehearse pencere-src pencere fake-host pencere-ref dev

# pencere is pinned in one place, Cargo.lock (Cargo.toml names the commit): the crate the program is
# built on is of it, and so are the `pencere` command `make dev` runs toppen under and the stand-in
# terminal the program's tests run it in, which are built from it in .cache/pencere. To work against
# a checkout you are changing, say so; one that is not at Cargo.lock's ref is said to be so, and used:
#
#   make dev PENCERE_SRC=../pencere
#
# `node tools/release.mjs pencere-ref` reads the ref from Cargo.lock: the commit, or the tag of a release
# on crates.io.
PENCERE_REF = $(shell node tools/release.mjs pencere-ref)
PENCERE_CACHE = .cache/pencere
PENCERE_SRC ?= $(PENCERE_CACHE)
PENCERE ?= $(PENCERE_SRC)/target/release/pencere
PENCERE_GIT = https://github.com/bateri/pencere

# The stand-in terminal the program's tests run toppen in (FAKE_HOST): the one `make fake-host`
# builds at Cargo.lock's pencere.
FAKE_HOST ?= $(PENCERE_CACHE)/target/debug/pencere-fake-host

# pin: main is built on a pencere anyone can fetch, the commit Cargo.lock names. A branch that waits
# for a pencere not pushed yet takes it from a checkout on this machine with a [patch] of Cargo.toml;
# that section is refused on main and on what is to be merged into it.
pin:
	@if grep -q '^\[patch' Cargo.toml; then \
		branch="$$(git rev-parse --abbrev-ref HEAD 2>/dev/null)"; \
		if [ "$$branch" = main ] || [ "$${GITHUB_REF:-}" = refs/heads/main ] || [ "$${GITHUB_BASE_REF:-}" = main ]; then \
			echo "pin: Cargo.toml takes pencere from a [patch]: main is built on a pushed pencere (move the rev in Cargo.toml, drop the patch)"; exit 1; \
		fi; \
		echo "pin: Cargo.toml takes pencere from a [patch] ($$branch): main will refuse it"; \
	fi

fmt:
	cargo fmt

lint:
	cargo fmt -- --check
	cargo clippy --locked --all-targets -- -D warnings

# The program's tests: its own, and the ones that run it in a pane under the stand-in terminal. Nothing
# in them is skipped; a stand-in that is not there stops the run and says which.
test:
	FAKE_HOST="$(FAKE_HOST)" cargo test --locked

# The page's tests: src/view/ run as a terminal runs it, on pencere's host with no screen
# (pencere-drive), with the program's own handlers in the same process (src/program/page_tests.rs),
# over a fixture machine. Each compiles and runs the kit's interpreter, so they are `cargo test`'s
# ignored ones, run here; a few at a time. A call of the page is held to 10 s here, not to the kit's
# 250 ms, which is the wall clock's: a runner under load takes many times what one at rest does, and
# a test of what the page does should not fail for that. check-timing is the strict one (below).
test-page:
	cargo test --locked --bin toppen -- page_tests:: --ignored --test-threads=2

# check-timing runs the page's tests with every call of the page held to the kit's 250 ms
# (CALL_MS=125 for half of it), as a terminal holds it. Not part of the gate: it measures the
# machine as much as the page, so run it on one at rest.
CALL_MS ?= 250
check-timing:
	TOPPEN_CALL_MS=$(CALL_MS) cargo test --locked --bin toppen -- page_tests:: --ignored --test-threads=2

# shots draws the pictures of docs/ from the page as it is, over a fixture machine (never the
# machine's own processes), in the fonts of this machine (macOS: SF Mono, or Menlo, and SF): not part
# of the gate, which needs no font of the system.
shots:
	cargo test --locked --bin toppen page_shots:: -- --ignored

# test-tools runs the tests of the installer (install.sh) against a directory that stands in for
# GitHub: nothing in them reaches the network.
test-tools:
	node --test test/install.test.mjs

# check is the gate: the pin of pencere, formatting, lints, the program's tests, the page's tests, the
# installer's tests.
check: pin lint test test-page test-tools

# build makes target/release/toppen, the program for this machine, with the page in it.
build:
	cargo build --locked --release

# dist makes the program for one target (TARGET, a Rust target triple) into dist/release, as
# toppen-<version>-<target>. The release workflow runs it on a machine of each of the three targets
# toppen ships (Apple silicon Macs and both Linux architectures, the Linux ones linked statically
# with musl; bateri, the terminal that shows the page, runs on Apple silicon only), then `make sums`
# where the three files are together.
TARGETS = aarch64-apple-darwin aarch64-unknown-linux-musl x86_64-unknown-linux-musl
PACKAGE_VERSION = $(shell node tools/release.mjs version)

dist:
	@test -n "$(TARGET)" || { echo "usage: make dist TARGET=<one of: $(TARGETS)>"; exit 2; }
	cargo build --locked --release --target $(TARGET)
	mkdir -p dist/release
	cp target/$(TARGET)/release/toppen dist/release/toppen-$(PACKAGE_VERSION)-$(TARGET)

# sums writes the SHA256SUMS of the files of dist/release, and checks them.
sums:
	cd dist/release && { sha256sum toppen-* 2>/dev/null || shasum -a 256 toppen-*; } > SHA256SUMS
	@cat dist/release/SHA256SUMS
	node tools/release.mjs dist dist/release

# release-prepare sets the version and dates the changelog's Unreleased section.
release-prepare:
	@test -n "$(VERSION)" || { echo "usage: make release-prepare VERSION=<x.y.z>"; exit 2; }
	node tools/release.mjs prepare $(VERSION)

# rehearse runs the checks of a release of the prepared version, on any branch. It changes nothing:
# a release is the commit of release-prepare pushed to main, which the release workflow tags,
# builds and drafts once the gate is green, and publishes when a maintainer approves (RELEASING.md).
rehearse:
	node tools/release.mjs rehearse

# pencere-src makes PENCERE_SRC ready: fetched at Cargo.lock's ref when it is the cache (one checkout,
# moved to another ref when Cargo.lock moves, so that cargo builds again only what changed).
pencere-src:
	@command -v cargo >/dev/null || { echo "pencere: cargo is not installed: pencere is built from its source (https://rustup.rs)"; exit 1; }
	@ref="$(PENCERE_REF)"; test -n "$$ref" || { echo "pencere: Cargo.lock takes no pencere from $(PENCERE_GIT) at a commit"; exit 1; }; \
	if [ "$(PENCERE_SRC)" = "$(PENCERE_CACHE)" ]; then \
		if [ "$$(cat $(PENCERE_CACHE).ref 2>/dev/null)" != "$$ref" ]; then \
			echo "pencere: fetching $(PENCERE_GIT) at $$ref into $(PENCERE_CACHE)"; \
			{ test -d "$(PENCERE_CACHE)/.git" || git init -q "$(PENCERE_CACHE)"; } && \
			git -C "$(PENCERE_CACHE)" fetch -q --depth 1 "$(PENCERE_GIT)" "$$ref" && \
			git -C "$(PENCERE_CACHE)" checkout -q --detach --force FETCH_HEAD && \
			echo "$$ref" > "$(PENCERE_CACHE).ref" || exit 1; \
		fi; \
	else \
		head=$$(git -C "$(PENCERE_SRC)" rev-parse HEAD 2>/dev/null); \
		[ "$$head" = "$$ref" ] || \
			echo "pencere: $(PENCERE_SRC) is at $${head:-no commit}, not at $$ref (Cargo.lock) that toppen is built with"; \
	fi

# pencere builds the `pencere` command in PENCERE_SRC.
pencere: pencere-src
	cd "$(PENCERE_SRC)" && cargo build --release -p pencere-cli

# fake-host builds the stand-in terminal in PENCERE_SRC.
fake-host: pencere-src
	cd "$(PENCERE_SRC)" && cargo build -p pencere-fake-host

# pencere-ref says which pencere Cargo.lock takes.
pencere-ref:
	@echo "$(PENCERE_REF)"

# dev runs toppen from its files in this pane, in a terminal that hosts pencere (bateri): the page
# is shown again when a file of src/view/ or src/assets/ changes, and the program built and run again
# when one of it does.
dev: pencere
	"$(PENCERE)" dev
