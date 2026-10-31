//! The page (src/view/) and the program's own handlers in one process, as a terminal runs them: the
//! kit's interpreter under pencere's host with no screen (`pencere-drive`), a clock the test moves,
//! the calls and the topics of `api/` served by `pencere::Conversation` as `serve` serves them — the
//! handlers `main` gives a terminal (`options`), over a fixture machine, never the one the tests run
//! on. What the page shows is read as words; what the person does is done to it as a terminal does it.
//! A call of the page is held to the kit's 64 MiB, and to its 250 ms a call when asked
//! (`make check-timing`; `limits`).
//!
//! Each opens the interpreter (compiled once a run): `make test-page` runs them, in `make check`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use pencere_drive::headless::{Error, Headless, InProcess};
use pencere_view::Paint;

use crate::options;
use crate::sample::fixture::Fixture;
use crate::sampler::Sampler;
use crate::session::Session;

/// A page as large as the docs' pictures, and an 80×24 pane.
pub(crate) const WIDE: (f32, f32) = (1100.0, 720.0);
pub(crate) const PANE: (f32, f32) = (600.0, 330.0);

/// How long a test waits for what the program says on its own (a sample, a call).
const WITHIN: Duration = Duration::from_secs(20);

/// How often the fixture machine is looked at in a test.
const TICK: Duration = Duration::from_millis(40);

pub(crate) type Page = Headless<InProcess>;

/// What the page's module is held to. The tests say what the page shows and does, not how fast a
/// machine is: a runner under load takes ten times as long as one at rest, so the kit's 250 ms a
/// call, 2 s to start and 20 s to compile, which are the wall clock's, are far wider here by
/// default (`GENEROUS_CALL`). With `TOPPEN_CALL_MS` set (`make check-timing`) a call is held to that
/// many milliseconds, the rest to the kit's own: the page in the time a terminal gives it, or in
/// half of it (`TOPPEN_CALL_MS=125`), to see how near it comes. Meant for a machine at rest.
pub(crate) fn limits(call_ms: Option<u64>) -> pencere_sandbox::ModuleLimits {
    let kit = pencere_sandbox::ModuleLimits::default();
    match call_ms {
        Some(ms) => pencere_sandbox::ModuleLimits {
            call: Duration::from_millis(ms),
            ..kit
        },
        None => pencere_sandbox::ModuleLimits {
            call: GENEROUS_CALL,
            start: GENEROUS_CALL * 6,
            compile: GENEROUS_CALL * 12,
            ..kit
        },
    }
}

/// A call of the page, in a test that is not about time: forty times the kit's.
const GENEROUS_CALL: Duration = Duration::from_secs(10);

/// A session over `fixture`, looked at every `interval`.
pub(crate) fn session_over(fixture: Fixture, interval: Duration) -> Arc<Session> {
    Arc::new(Session::new(Sampler::start(Box::new(fixture), interval)))
}

/// The page of this checkout's src/view/ (and its pictures, src/assets/), on the handlers of
/// `session`, at `size`, with the first sample drawn.
pub(crate) fn open(
    session: &Arc<Session>,
    size: (f32, f32),
    fonts: Option<&pencere_drive::shot::Fonts>,
) -> Result<Page, Error> {
    let program = InProcess::new(options(session, &Arc::new(Mutex::new(None))));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |since| since.as_secs_f64());
    let mut opening = Headless::app(env!("CARGO_MANIFEST_DIR"))
        .size(size.0, size.1)
        .wall_clock(now, 0)
        .limits(limits(
            std::env::var("TOPPEN_CALL_MS")
                .ok()
                .and_then(|ms| ms.parse().ok()),
        ));
    if let Some(fonts) = fonts {
        opening = opening.measure(fonts.clone()).scale(2.0);
    }
    let mut page = opening.peer(program).open()?;
    // What the page reads first comes from the program on its own threads.
    page.wait_for(WITHIN, |page| page.says("load "))?;
    page.advance(0.2)?;
    Ok(page)
}

/// The page over the small fixture, at `size`.
pub(crate) fn open_small(size: (f32, f32)) -> (Arc<Session>, Page) {
    let session = session_over(Fixture::small(), TICK);
    let page = open(&session, size, None).expect("the page opens");
    (session, page)
}

/// The words of the page, one string.
pub(crate) fn said(page: &Page) -> String {
    page.words().join("|")
}

/// Waits until what the page says is not what `before` is.
fn wait_for_change(page: &mut Page, before: &str) {
    page.wait_for(WITHIN, |page| said(page) != before)
        .expect("the page changes");
}

/// How many graphs are drawn.
fn figures(page: &Page) -> usize {
    page.list()
        .items
        .iter()
        .filter(|item| matches!(item, Paint::Figure(_)))
        .count()
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_four_cards_are_drawn_with_the_machines_numbers() {
    let (_session, page) = open_small(WIDE);
    for title in ["CPU", "Memory", "Network", "Disk"] {
        assert!(page.says(title), "{title}: {}", said(&page));
    }
    // The status line: the machine, how long it has been up, how often it is looked at.
    assert!(page.says("lantern"), "{}", said(&page));
    assert!(page.says("up 3d 0h"), "{}", said(&page));
    assert!(page.says("every 0.0"), "{}", said(&page));
    // The fixture's memory is 16 GiB and its CPU is named on the card.
    assert!(page.says("of 16.0 GiB"), "{}", said(&page));
    assert!(page.says("Fixture M1 Pro"), "{}", said(&page));
    assert!(page.says("8 threads"), "{}", said(&page));
    assert!(page.says("swap "), "{}", said(&page));
    assert!(page.says("pressure "), "{}", said(&page));
    assert!(page.says("since boot"), "{}", said(&page));
    assert!(page.says("/System/Volumes/Data"), "{}", said(&page));
    assert!(
        figures(&page) >= 4,
        "a graph in each card: {}",
        figures(&page)
    );
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_values_change_as_the_program_tells_the_page_new_samples() {
    let (_session, mut page) = open_small(WIDE);
    let before = said(&page);
    wait_for_change(&mut page, &before);
    let middle = said(&page);
    wait_for_change(&mut page, &middle);
    assert_ne!(said(&page), before);
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn an_80_by_24_pane_shows_every_card_with_its_graph() {
    let (_session, page) = open_small(PANE);
    for title in ["CPU", "Memory", "Network", "Disk"] {
        assert!(page.says(title), "{title}: {}", said(&page));
    }
    assert!(page.says("lantern"), "{}", said(&page));
    assert!(
        figures(&page) >= 4,
        "a graph in each card: {}",
        figures(&page)
    );
    // Nothing is drawn past the pane.
    for name in ["status-pause", "status-interval"] {
        let at = page.bounds(name).expect("the item is laid out");
        assert!(
            at.x + at.w <= PANE.0 + 0.5 && at.y + at.h <= PANE.1 + 0.5,
            "{name}: {at:?}"
        );
    }
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_big_machine_with_sixty_four_threads_fits_the_cards() {
    let session = session_over(Fixture::large(), TICK);
    let page = open(&session, WIDE, None).expect("the page opens");
    assert!(page.says("64 threads"), "{}", said(&page));
    assert!(page.says("of 256.0 GiB"), "{}", said(&page));
    assert!(page.says("forge"), "{}", said(&page));
    // No swap, no pressure on this one: its card says nothing of them.
    assert!(!page.says("swap "), "{}", said(&page));
    assert!(!page.says("pressure "), "{}", said(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn pausing_stops_the_program_looking_and_resuming_starts_it_again() {
    let (session, mut page) = open_small(WIDE);
    assert!(page.says("Pause"));
    page.click("status-pause").expect("pause is pressed");
    page.wait_for(WITHIN, |page| page.says("Paused") && page.says("Resume"))
        .expect("the page says it is paused");
    assert!(session.sampler.paused());
    // A paused page does not change.
    let frozen = said(&page);
    std::thread::sleep(TICK * 6);
    page.advance(0.3).expect("a moment passes");
    assert_eq!(said(&page), frozen);
    // The space bar does the same as the button, from the keyboard.
    page.press("space").expect("space is pressed");
    page.wait_for(WITHIN, |page| !page.says("Paused"))
        .expect("the page says it is going again");
    assert!(!session.sampler.paused());
    wait_for_change(&mut page, &frozen);
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_interval_is_stepped_from_the_status_line_and_the_keyboard() {
    let (session, mut page) = open_small(WIDE);
    page.press("-").expect("minus is pressed");
    page.wait_for(WITHIN, |_| session.sampler.interval() > TICK)
        .expect("the program slowed down");
    page.advance(0.2).expect("a moment passes");
    let slower = session.sampler.interval();
    page.click("status-interval")
        .expect("the interval is pressed");
    page.wait_for(WITHIN, |_| session.sampler.interval() > slower)
        .expect("the program slowed down once more");
    // A step is one of the five the program accepts: 0.5, 1, 2, 5, 10 seconds.
    let seconds = session.sampler.interval().as_secs_f64();
    assert!([0.5, 1.0, 2.0, 5.0, 10.0].contains(&seconds), "{seconds}");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_page_says_when_the_program_does_not_answer() {
    // A program that answers no `boot` leaves the page at its spinner, which says nothing; one that
    // fails it is told so.
    let session = session_over(Fixture::small(), TICK);
    let program = InProcess::new(
        pencere::Options::new()
            .app(pencere::app!())
            .program("toppen"),
    );
    let opening = Headless::app(env!("CARGO_MANIFEST_DIR"))
        .size(WIDE.0, WIDE.1)
        .limits(limits(None));
    let mut page = opening.peer(program).open().expect("the page opens");
    page.wait_for(WITHIN, |page| page.says("The program did not answer"))
        .expect("the page says so");
    drop(session);
}
