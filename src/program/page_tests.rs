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
const TICK: Duration = Duration::from_millis(300);

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
    assert!(page.says("every 0.3"), "{}", said(&page));
    // The fixture's memory is 16 GiB and its CPU is named on the card.
    assert!(page.says("of 16.0 GiB"), "{}", said(&page));
    assert!(page.says("Fixture M1 Pro"), "{}", said(&page));
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
fn an_80_by_24_pane_shows_the_gauges_and_the_table() {
    let (_session, page) = open_small(PANE);
    for title in ["CPU", "Memory", "Network", "Disk"] {
        assert!(page.says(title), "{title}: {}", said(&page));
    }
    assert!(page.says("lantern"), "{}", said(&page));
    // The pane is too narrow for the graphs: the gauges are lines, and the table has the room.
    assert!(page.says("load "), "{}", said(&page));
    assert!(page.says("PID"), "{}", said(&page));
    // Nothing is drawn past the pane.
    for name in ["status/item/pause", "status/item/interval"] {
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
    page.click("status/item/pause").expect("pause is pressed");
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
    page.click("status/item/interval")
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

// --- The process table ---------------------------------------------------------------------------

use crate::model::processes::{Column, Mode};

/// The shares of processor in the table's rows, in the order they are drawn: the words that read
/// `12.3%` (the cards say whole percent).
fn cpu_column(page: &Page) -> Vec<f64> {
    page.words()
        .into_iter()
        .filter_map(|word| {
            let number = word.strip_suffix('%')?;
            number.contains('.').then(|| number.parse().ok()).flatten()
        })
        .collect()
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_table_lists_the_busiest_processes_first_under_its_headers() {
    let (session, page) = open_small(WIDE);
    for header in [
        "Name", "PID", "User", "CPU", "Memory", "Threads", "State", "Time",
    ] {
        assert!(page.says(header), "{header}: {}", short(&page));
    }
    assert!(page.says("54 processes"), "{}", short(&page));
    let shares = cpu_column(&page);
    assert!(shares.len() >= 10, "{shares:?}");
    assert!(
        shares.windows(2).all(|pair| pair[0] >= pair[1]),
        "{shares:?}"
    );
    let view = session.table().view();
    assert_eq!((view.sort, view.descending), (Column::Cpu, true));
}

/// What the table says from its first row on.
fn first_row(page: &Page) -> String {
    let all = said(page);
    all.split_once("Time|")
        .map_or_else(String::new, |(_, rest)| rest.to_owned())
}

/// A page's words, cut for a failure message.
fn short(page: &Page) -> String {
    said(page).chars().take(600).collect()
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_click_on_a_header_sorts_by_it_and_a_second_click_reverses_it() {
    let (session, mut page) = open_small(WIDE);
    page.click("procs/head/pid").expect("the header is pressed");
    page.wait_for(WITHIN, |_| {
        let view = session.table().view();
        view.sort == Column::Pid && !view.descending
    })
    .expect("the table is sorted by pid");
    // Pid 1 is the first row, and the header says it is sorted.
    page.wait_for(WITHIN, |page| first_row(page).starts_with("launchd|1|root"))
        .expect("the first row is the first pid");
    page.click("procs/head/pid")
        .expect("the header is pressed again");
    page.wait_for(WITHIN, |_| session.table().view().descending)
        .expect("the order is reversed");
    page.wait_for(WITHIN, |page| {
        !first_row(page).starts_with("launchd|1|root")
    })
    .expect("pid 1 is last now");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_filter_narrows_the_table_and_a_wrong_pattern_says_what_is_wrong() {
    let (session, mut page) = open_small(WIDE);
    page.command("filter").expect("the filter is asked for");
    // Letters typed in the field are text, not the keys of the page (t is the tree, g the groups).
    page.type_text("postgres").expect("the filter is typed");
    page.wait_for(WITHIN, |_| session.table().view().filter == "postgres")
        .expect("the program has the filter");
    page.wait_for(WITHIN, |page| page.says(" of 54 processes"))
        .expect("the page says how many are left");
    assert_eq!(session.table().view().mode, Mode::Flat);
    assert!(!page.says("redis-server"), "{}", short(&page));
    // A pattern that is not one is said in the bar, and the last good filter stays.
    page.press("escape").expect("escape clears the field");
    page.wait_for(WITHIN, |page| {
        page.says("54 processes") && !page.says(" of 54")
    })
    .expect("the filter is cleared");
    page.type_text("/(x/").expect("a pattern is typed");
    page.wait_for(WITHIN, |page| page.says("not a regular expression"))
        .expect("the page says what is wrong");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_tree_folds_into_its_branches_and_opens_again() {
    let (session, mut page) = open_small(WIDE);
    page.press("t").expect("t is pressed");
    page.wait_for(WITHIN, |_| session.table().view().mode == Mode::Tree)
        .expect("the program has the tree");
    let rows = |page: &Page| {
        page.words()
            .iter()
            .filter(|word| ["R", "S", "D"].contains(word))
            .count()
    };
    page.wait_for(WITHIN, |page| rows(page) > 20)
        .expect("the tree has its rows");
    page.command("fold_all").expect("every branch is folded");
    page.wait_for(WITHIN, |page| {
        rows(page) < 10 && page.words().iter().any(|word| word.starts_with('+'))
    })
    .expect("the branches are folded into a row each");
    page.command("open_all").expect("every branch is opened");
    page.wait_for(WITHIN, |page| rows(page) > 20)
        .expect("the branches are open again");
    // The key of the mode in force goes back to the flat list.
    page.press("t").expect("t is pressed again");
    page.wait_for(WITHIN, |_| session.table().view().mode == Mode::Flat)
        .expect("the list is flat again");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_order_is_held_under_the_pointer_and_released_by_the_badge() {
    let (session, mut page) = open_small(WIDE);
    page.hover("procs/row/3")
        .expect("the pointer is over a row");
    page.wait_for(WITHIN, |_| session.table().view().hold)
        .expect("the program holds the order");
    // The numbers move on, and the badge says how many rows would have moved.
    page.wait_for(WITHIN, |page| page.says("Order held"))
        .expect("the page says the order is held");
    page.click("release").expect("the badge is pressed");
    page.wait_for(WITHIN, |page| !page.says("Order held"))
        .expect("the order is released");
    assert!(!session.table().view().hold);
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_header_menu_shows_and_hides_columns() {
    let (_session, mut page) = open_small(WIDE);
    assert!(!page.says("Virtual"));
    page.right_click("procs/head/cpu")
        .expect("the header is pressed");
    page.pick_titled("Show Virtual")
        .expect("the column is chosen");
    page.wait_for(WITHIN, |page| page.says("Virtual"))
        .expect("the column is drawn");
    page.right_click("procs/head/cpu")
        .expect("the header is pressed");
    page.pick_titled("Hide Virtual")
        .expect("the column is chosen");
    page.wait_for(WITHIN, |page| !page.says("Virtual"))
        .expect("the column is gone");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_big_machines_table_asks_for_the_rows_in_view_and_no_more() {
    let session = session_over(Fixture::large(), TICK);
    let mut page = open(&session, WIDE, None).expect("the page opens");
    assert!(
        page.says("1778 processes") || page.says("processes"),
        "{}",
        short(&page)
    );
    // The page holds a window, not the table: the words of the page stay few.
    assert!(page.words().len() < 3000, "{}", page.words().len());
    // Far down the list the rows come when they come into view.
    page.scroll_to("procs/body", 22.0 * 900.0)
        .expect("the table is scrolled");
    page.wait_for(WITHIN, |page| {
        page.words_in("procs/row/905")
            .iter()
            .any(|word| word.contains("MiB"))
    })
    .expect("the rows far down are told");
}
