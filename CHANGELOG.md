# Changelog

## [Unreleased]

### Added

- **A terminal that speaks Kitty graphics is drawn on, as bateri does.** toppen shows the monitor
  as pixels there, with its own faces, icons, bars and graphs, the mouse and the keys, where it used
  to print a summary. A terminal with no Kitty graphics, and a multiplexer, still prints the summary;
  `PENCERE_TUI=0` keeps it, `PENCERE_TUI=1` draws on a terminal that does host views too.
- The palette and the settings are `mod+k` and `mod+,` (⌘ on a Mac, ⌃ elsewhere), and F2 opens the
  settings: a terminal that is drawn on does not send the ⌘ chords.
- Where a terminal draws no pictures, the icons of the page are characters (a magnifier, the arrows of
  a folded branch, the eyes of the secrets, an ellipsis for what is read), and the head of the table
  and its column lines are a row, a core's graph two rows, the rows of the tables are a row of cells
  high (a long list does not move as it scrolls) and the status line fits the width it is given. A
  card's parts are as many rows as the card, with no row left at its foot.

- **A feature `terminal`, on by default, for the screen of its own.** `cargo build
  --no-default-features` makes the file that does not draw a terminal that hosts nothing (it prints
  the summary there), without `pencere-tui` and so smaller. The screen has the interpreter compiled by
  the build and no PDF reader, so neither a code generator nor the reader is in the file. A profile `dist` (LTO, one code generation
  unit, stripped) is the file that is shipped.
- The keys of the page on a keyboard with no ⌘ are Ctrl's (`mod+k`), and none is one a terminal
  cannot tell from another (Ctrl+I, Ctrl+J, Ctrl+H, Ctrl+[) or sends as another (Ctrl+Shift or
  Ctrl+Alt with a letter): everything is in the palette as well.

### Changed

- On a terminal of few rows the table has the rows: no rule under its head or the bars, no row
  between the bar and the head, the band of the cards and the panel one row apart. The cards share the
  columns in whole ones (none is left over at the end of the row), and a card says two whole
  figures where it would have said three cut ones. The busy mark turns where the page is drawn
  with characters (braille, a tenth of a second a step).

- The mouse pointer is a hand over a button, a tab, a row, a head and a card, and the arrow elsewhere.

### Fixed

- The cursor of the table ends where a run of keys sent faster than the program answers put it: a
  page key or an arrow sent again before the rows were told, or before the program had heard the
  last choice, took the cursor back to a row of an earlier key.

## [0.2.0] - 2026-11-09

### Added

- **Releases carry a signed `app.json`.** A terminal that follows toppen (bateri, from its list of
  apps) reads it to find the newest file it can open and to know the file is the one the maintainer
  released. `make sums` writes it and its signature beside `SHA256SUMS`, and the release checks
  refuse a release without them.

### Changed

- Building from source now needs Rust 1.99 or later.

## [0.1.0] - 2026-11-03

### Added

- A system monitor for the terminal pane: cards for the CPU, memory, network and disks, each in a
  colour of its own, with the value of the moment large, three figures of what it is made of (the
  memory's app, wired and compressed and its pressure, the network's upload and totals, the disk's
  reads, writes and use) and the last minute as a matrix of dots; and a status line with the machine,
  its uptime and the update interval.
- Pause and resume the updates (Space, or the status line) and change how often the machine is looked
  at (0.5 to 10 seconds; `+` and `-`, or the status line).
- In a terminal that shows no views, or with `--once`, a summary of the machine is printed instead;
  `--json` prints the same as JSON, `--interval <seconds>` sets the update interval.
- Where the terminal takes the monitor down, what it said is printed under the prompt.
- A table of the processes under the cards: sort by any column, filter (words, `/regex/`, `user:`,
  `pid:`, `state:`, `cpu>`, `mem>`, `!` to negate), a tree, groups by name, folded branches that
  carry the totals of what they hide, the subtree of one process, marks, columns to show and hide,
  and an order that stays still for a moment while the pointer moves over it or after a key; a row is chosen, not
  a cell.
- The summary of `--once`, the pipe and `--json` lists the busiest processes.
- The details of a process in a panel beside the table (over it in a narrow pane): graphs of what it
  takes, the whole command line, working folder, parents and children (each one press away) and
  environment, with what looks like a secret hidden and an eye beside it to show it.
- Signals (TERM, INT, HUP, KILL, STOP, CONT, USR1, USR2, to one process or to it and what is below
  it), a new priority, a shell in the process's folder, and copies of its command line, id and folder,
  from keys, buttons and the row's menu. A signal is asked for first, and goes to the process that
  was named (id and start): one that ended and whose id was used again is not signalled.
- The details of each card in place of the table: every processor with its own graph, its frequency
  and the temperatures; what memory is made of and swap; each network interface with its graph; the
  volumes and the reads and writes of the disks. A graph says what it held under the pointer and how
  long ago, and spans one, five or fifteen minutes.
- The listening ports: every TCP socket that waits for connections and every bound UDP socket, with
  the address and the process that holds it, filtered by port, name or user; a signal, a new priority
  or a shell for the row's process; Enter shows it in the table. `port:3000` in the process filter
  leaves the processes that listen there, and a process's details list its ports.
- The GPU: a card (where the machine has a GPU that says how busy it is) and its details: the use
  of each GPU over time, its memory, the parts of it that are busy and its temperature. Apple's
  accelerators, the DRM drivers of Linux and NVIDIA's driver are read; the disk's reads and writes
  are read on a Mac as well (the storage drivers), and the summary lists the GPUs.
- A palette (Cmd+K) that finds any command that can be done now and filters the table by what is
  typed, a list of every command with its keys (`?`), and htop's function keys as second keys.
- Settings (`~/.config/toppen/settings.json`, and a window for them): the update interval, how the
  table starts, whether names are shown, and the keys of every command, changed by pressing them.
  A key that the terminal, the table or another command has is refused with the reason; a file that
  cannot be read is left as it is.
- An installer (`install.sh`) that fetches the newest release for the machine, checks it against the
  release's `SHA256SUMS` and puts it in `~/.local/bin`. The Linux files of a release are linked
  statically, so they show no NVIDIA card (a build made on the machine does).
