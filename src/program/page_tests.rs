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
use crate::sample::control::Control;
use crate::sample::fixture::{Fixture, FixtureControl};
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
    session_with_control(fixture, interval).0
}

/// A session over `fixture`, and the control that records what the page asked it to do.
pub(crate) fn session_with_control(
    fixture: Fixture,
    interval: Duration,
) -> (Arc<Session>, Arc<FixtureControl>) {
    let control = Arc::new(fixture.control());
    let ports = Arc::new(fixture.ports());
    let session = Arc::new(Session::new(
        Sampler::start(Box::new(fixture), interval),
        Arc::clone(&control) as Arc<dyn Control>,
        ports,
        pencere::ViewControl::new(),
    ));
    (session, control)
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
    assert!(page.says("pause"));
    page.click("status/item/pause").expect("pause is pressed");
    page.wait_for(WITHIN, |page| page.says("Paused") && page.says("resume"))
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
    page.words_in("procs/row/1").join("|")
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
            .filter(|word| ["running", "sleeping", "disk"].contains(word))
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
    page.scroll_to("procs/body", 26.0 * 900.0)
        .expect("the table is scrolled");
    page.wait_for(WITHIN, |page| {
        page.words_in("procs/row/905")
            .iter()
            .any(|word| word.contains("MiB"))
    })
    .expect("the rows far down are told");
}

// --- One process: its details and what is done to it ----------------------------------------------

use crate::sample::control::Signal;
use crate::sample::fixture::Done;

/// Narrows the table to the processes of the person and puts the cursor on the first of them, with
/// the keyboard in the table.
pub(crate) fn on_my_processes(session: &Arc<Session>, page: &mut Page) {
    page.command("filter").expect("the filter is asked for");
    page.type_text("user:lantern").expect("the filter is typed");
    page.wait_for(WITHIN, |_| session.table().view().filter == "user:lantern")
        .expect("the program has the filter");
    page.press("enter").expect("the field is left");
    page.advance(0.4).expect("the table settles");
}

/// Waits until `words` is said in one piece among the words of the page.
fn wait_for_said(page: &mut Page, words: &str) {
    page.wait_for(WITHIN, |page| said(page).contains(words))
        .unwrap_or_else(|_| panic!("the page says {words:?}: {}", short(page)));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_process_shows_its_details_beside_the_table_and_its_secrets_stay_hidden_until_asked() {
    let (session, mut page) = open_small(WIDE);
    on_my_processes(&session, &mut page);
    page.command("detail").expect("the details are asked for");
    page.wait_for(WITHIN, |page| page.says("Working folder"))
        .expect("the panel is drawn");
    for part in ["Environment", "Started by", "API_TOKEN", "HOME"] {
        assert!(page.says(part), "{part}: {}", short(&page));
    }
    // A variable that looks like a secret has its value hidden.
    assert!(!said(&page).contains("tok-8f2a91c4d7"));
    assert!(page.says("••••••••"));
    // The environment is at the foot of the panel, which scrolls.
    page.scroll_to("detail/body", 4000.0)
        .expect("the panel is scrolled");
    page.click("env-reveal").expect("the values are asked for");
    wait_for_said(&mut page, "tok-8f2a91c4d7");
    page.click("env-reveal").expect("the values are hidden");
    page.wait_for(WITHIN, |page| !said(page).contains("tok-8f2a91c4d7"))
        .expect("the secret is hidden again");
    page.click("detail-close").expect("the panel is closed");
    page.wait_for(WITHIN, |page| !page.says("Working folder"))
        .expect("the panel is gone");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_narrow_pane_has_the_details_over_the_table_and_escape_puts_them_away() {
    let (session, mut page) = open_small(PANE);
    on_my_processes(&session, &mut page);
    page.command("detail").expect("the details are asked for");
    page.wait_for(WITHIN, |page| page.says("Working folder"))
        .expect("the drawer is drawn");
    page.press("escape").expect("escape is pressed");
    page.wait_for(WITHIN, |page| !page.says("Working folder"))
        .expect("the drawer is gone");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_signal_is_asked_for_with_the_command_and_sent_only_when_answered() {
    let (session, control) = session_with_control(Fixture::small(), TICK);
    let mut page = open(&session, WIDE, None).expect("the page opens");
    on_my_processes(&session, &mut page);
    // The menu of signals is a key away; the signal itself is a command.
    page.command("signal_kill").expect("a kill is asked for");
    page.wait_for(WITHIN, |page| page.says("Cancel"))
        .expect("the person is asked");
    let said_now = said(&page);
    assert!(said_now.contains("/usr/local/bin/"), "{said_now}");
    assert!(said_now.contains("Kill"), "{said_now}");
    assert!(control.done().is_empty());
    page.click("confirm/action/cancel")
        .expect("the person gives up");
    page.wait_for(WITHIN, |page| !page.says("Cancel"))
        .expect("the question is gone");
    page.advance(0.2).expect("the page settles");
    assert!(control.done().is_empty(), "{:?}", control.done());
    // Answered, it goes to the process, and the toast says so.
    page.command("signal_term").expect("an end is asked for");
    page.wait_for(WITHIN, |page| page.says("Cancel"))
        .expect("the person is asked");
    page.click("confirm/action/send")
        .expect("the person agrees");
    page.wait_for(WITHIN, |_| !control.done().is_empty())
        .expect("the signal is sent");
    assert!(matches!(
        control.done().as_slice(),
        [Done::Signalled(_, Signal::Term)]
    ));
    wait_for_said(&mut page, "Sent TERM to");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn what_belongs_to_another_user_is_not_signalled_and_the_page_says_why() {
    let (session, control) = session_with_control(Fixture::small(), TICK);
    let mut page = open(&session, WIDE, None).expect("the page opens");
    page.command("filter").expect("the filter is asked for");
    page.type_text("user:root").expect("the filter is typed");
    page.wait_for(WITHIN, |_| session.table().view().filter == "user:root")
        .expect("the program has the filter");
    page.press("enter").expect("the field is left");
    page.advance(0.4).expect("the table settles");
    page.command("detail").expect("the details are asked for");
    page.wait_for(WITHIN, |page| page.says("Working folder"))
        .expect("the panel is drawn");
    page.command("signal_kill").expect("a kill is asked for");
    wait_for_said(&mut page, "takes root");
    assert!(!page.says("Cancel"));
    assert!(control.done().is_empty());
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_priority_is_typed_checked_and_set_and_a_shell_is_asked_for_in_the_working_folder() {
    let (session, control) = session_with_control(Fixture::small(), TICK);
    let mut page = open(&session, WIDE, None).expect("the page opens");
    on_my_processes(&session, &mut page);
    page.command("renice").expect("a priority is asked for");
    wait_for_said(&mut page, "Priority of");
    page.type_text("99").expect("a number is typed");
    page.press("enter").expect("the person agrees");
    wait_for_said(&mut page, "A priority is a whole number from -20 to 19.");
    assert!(control.done().is_empty());
    // The dialog stays open on a number that is none; a good one is set.
    page.fill("renice/input/field", "7")
        .expect("a good number is typed");
    page.press("enter").expect("the person agrees");
    page.wait_for(WITHIN, |_| {
        control
            .done()
            .iter()
            .any(|done| matches!(done, Done::Reniced(_, 7)))
    })
    .expect("the priority is set");
    page.advance(0.2).expect("the page settles");
    page.command("shell").expect("a shell is asked for");
    page.wait_for(WITHIN, |_| {
        control
            .done()
            .iter()
            .any(|done| matches!(done, Done::Shell(Some(cwd)) if cwd.starts_with("/home/lantern/")))
    })
    .expect("the shell is asked for where the process works");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_click_on_a_parent_in_the_panel_shows_that_process() {
    let (session, mut page) = open_small(WIDE);
    // A process of the person whose parent is neither init nor the same name.
    let sample = session.sampler.latest().expect("a sample");
    let child = sample
        .processes
        .iter()
        .find(|process| {
            process.user == "lantern"
                && process.ppid != 1
                && sample
                    .processes
                    .iter()
                    .any(|up| up.pid == process.ppid && up.name != process.name)
        })
        .expect("a process with a parent of another name");
    let parent = sample
        .processes
        .iter()
        .find(|up| up.pid == child.ppid)
        .expect("its parent");
    page.command("filter").expect("the filter is asked for");
    page.type_text(&format!("pid:{}", child.pid))
        .expect("the filter is typed");
    page.wait_for(WITHIN, |_| {
        session.table().view().filter.starts_with("pid:")
    })
    .expect("the program has the filter");
    page.press("enter").expect("the field is left");
    page.advance(0.4).expect("the table settles");
    page.command("detail").expect("the details are asked for");
    wait_for_said(&mut page, &format!("pid {} · lantern", child.pid));
    // The parents are below the graphs: the panel is scrolled to them.
    let link = format!("detail-go/{}/{}/up", parent.pid, parent.start);
    let body = page.bounds("detail/body").expect("the panel's body");
    let at = page.bounds(&link).expect("the parent is listed");
    page.scroll_to("detail/body", at.y - body.y - 40.0)
        .expect("the panel is scrolled");
    page.click(&link).expect("the parent is pressed");
    wait_for_said(&mut page, &format!("pid {} · {}", parent.pid, parent.user));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_menu_of_a_row_does_what_the_keys_do() {
    let (session, mut page) = open_small(WIDE);
    on_my_processes(&session, &mut page);
    page.right_click("procs/row/1")
        .expect("the row is pressed with the right button");
    page.pick_titled("Copy the process id")
        .expect("the entry is chosen");
    wait_for_said(&mut page, "Copied the process id.");
    page.right_click("procs/row/1")
        .expect("the row is pressed with the right button");
    page.pick_titled("Kill it (KILL)")
        .expect("the entry is chosen");
    wait_for_said(&mut page, "Kill ");
    assert!(page.says("Cancel"));
    page.press("escape").expect("the person gives up");
    page.wait_for(WITHIN, |page| !page.says("Cancel"))
        .expect("the question is gone");
}

// --- The cards at length ----------------------------------------------------------------------------

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn each_card_opens_to_its_details_by_key_and_by_press_and_escape_goes_back() {
    let (session, mut page) = open_small(WIDE);
    // The processor: its processors one by one, and the temperatures the program reads for it.
    page.press("1").expect("the CPU is asked for");
    page.wait_for(WITHIN, |page| page.says("Processor use"))
        .expect("the CPU is drawn at length");
    for part in ["Processors · 8", "Temperatures"] {
        assert!(page.says(part), "{part}: {}", short(&page));
    }
    wait_for_said(&mut page, "CPU performance cores");
    assert!(!page.says("Name"), "the table stays away: {}", short(&page));
    page.press("escape").expect("escape goes back");
    page.wait_for(WITHIN, |page| {
        page.says("Name") && !page.says("Processor use")
    })
    .expect("the table is back");
    // Memory: what it is made of.
    page.click("card/memory").expect("the card is pressed");
    page.wait_for(WITHIN, |page| page.says("What it is made of"))
        .expect("the memory is drawn at length");
    for part in ["App", "Wired", "Compressed", "Cached files", "Free"] {
        assert!(page.says(part), "{part}: {}", short(&page));
    }
    // The network: the interfaces, and a choice of one that the arrows move.
    page.press("3").expect("the network is asked for");
    wait_for_said(&mut page, "bridge100");
    assert!(page.says("All interfaces"));
    page.press("down").expect("the first interface is chosen");
    page.click("iface/en5").expect("an interface is pressed");
    page.advance(0.3).expect("the page settles");
    // The chosen interface is the one the graph is titled with.
    assert!(said(&page).contains("en5"), "{}", short(&page));
    // The disk: its volumes.
    page.press("4").expect("the disk is asked for");
    wait_for_said(&mut page, "/System/Volumes/Data");
    page.click("machine-close")
        .expect("the way back is pressed");
    page.wait_for(WITHIN, |page| page.says("Name"))
        .expect("the table is back");
    drop(session);
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_graph_says_what_it_held_where_the_pointer_rests_and_how_long_ago_that_was() {
    let (_session, mut page) = open_small(WIDE);
    page.press("1").expect("the CPU is asked for");
    page.wait_for(WITHIN, |page| page.says("Processor use"))
        .expect("the CPU is drawn at length");
    assert!(!said(&page).contains(" ago"));
    page.hover("graph/cpu")
        .expect("the pointer rests on the graph");
    wait_for_said(&mut page, " ago");
    // A longer range reaches further back.
    page.press("w").expect("the range is stepped");
    page.hover("graph/cpu").expect("the pointer rests again");
    page.advance(0.3).expect("the page settles");
    page.leave().expect("the pointer leaves");
    page.wait_for(WITHIN, |page| !said(page).contains(" ago"))
        .expect("the readout goes with the pointer");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_narrow_pane_draws_each_card_at_length_too() {
    let (_session, mut page) = open_small(PANE);
    page.click("card/memory").expect("the gauge is pressed");
    page.wait_for(WITHIN, |page| page.says("What it is made of"))
        .expect("the memory is drawn at length");
    page.press("escape").expect("escape goes back");
    page.wait_for(WITHIN, |page| page.says("Name"))
        .expect("the table is back");
}

// --- The GPU ------------------------------------------------------------------------------------------

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_machine_with_a_gpu_has_its_card_and_the_card_opens_to_its_details() {
    let (_session, mut page) = open_small(WIDE);
    // The card is among the others, with the memory the GPU uses.
    assert!(page.says("GPU"), "{}", short(&page));
    assert!(said(&page).contains(" in use"), "{}", short(&page));
    page.press("5").expect("the GPU is asked for");
    page.wait_for(WITHIN, |page| page.says("GPU use"))
        .expect("the GPU is drawn at length");
    assert!(
        said(&page).contains("Fixture M1 Pro GPU"),
        "{}",
        short(&page)
    );
    for part in ["Engines", "Renderer", "Tiler"] {
        assert!(page.says(part), "{part}: {}", short(&page));
    }
    assert!(
        said(&page).contains("shared with the machine"),
        "{}",
        short(&page)
    );
    // The temperature is told while the details are open: the program reads the sensors then.
    wait_for_said(&mut page, " °C");
    assert!(!page.says("Name"), "the table stays away: {}", short(&page));
    page.hover("graph/gpu/1")
        .expect("the pointer rests on the graph");
    wait_for_said(&mut page, " ago");
    page.press("escape").expect("escape goes back");
    page.wait_for(WITHIN, |page| page.says("Name") && !page.says("GPU use"))
        .expect("the table is back");
    // The card is pressed with the mouse as well.
    page.click("card/gpu").expect("the card is pressed");
    page.wait_for(WITHIN, |page| page.says("GPU use"))
        .expect("the GPU is drawn at length again");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_machine_with_two_gpus_lists_each_with_the_memory_of_its_own() {
    let session = session_over(Fixture::duo(), TICK);
    let mut page = open(&session, WIDE, None).expect("the page opens");
    assert!(said(&page).contains("2 GPUs"), "{}", short(&page));
    page.press("5").expect("the GPU is asked for");
    page.wait_for(WITHIN, |page| page.says("Fixture RTX 4090"))
        .expect("the GPUs are drawn at length");
    for part in ["Fixture RTX 4070", "Memory bus", "Temperature"] {
        assert!(page.says(part), "{part}: {}", short(&page));
    }
    assert!(said(&page).contains(" of 24.0 GiB"), "{}", short(&page));
    assert!(said(&page).contains(" of 12.0 GiB"), "{}", short(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_machine_without_a_gpu_has_no_card_for_it_and_the_layout_is_whole() {
    let session = session_over(Fixture::large(), TICK);
    let mut page = open(&session, WIDE, None).expect("the page opens");
    for card in ["CPU", "Memory", "Network", "Disk"] {
        assert!(page.says(card), "{card}: {}", short(&page));
    }
    assert!(!page.says("GPU"), "{}", short(&page));
    // The key of the GPU's card is not a key of this machine's.
    page.press("5").expect("the key is pressed");
    page.advance(0.4).expect("the page settles");
    assert!(
        page.says("Name") && !page.says("GPU use"),
        "{}",
        short(&page)
    );
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_narrow_pane_has_a_gauge_for_the_gpu_that_opens_it() {
    let (_session, mut page) = open_small(PANE);
    assert!(said(&page).contains("GPU"), "{}", short(&page));
    page.click("card/gpu").expect("the gauge is pressed");
    page.wait_for(WITHIN, |page| page.says("GPU use"))
        .expect("the GPU is drawn at length");
}

// --- The keys, the palette, the help and the settings -------------------------------------------------

/// A session over the small fixture whose settings are those of the folder `dir`.
fn session_with_settings(dir: &std::path::Path) -> Arc<Session> {
    let fixture = Fixture::small();
    let control = Arc::new(fixture.control());
    let ports = Arc::new(fixture.ports());
    Arc::new(
        Session::new(
            Sampler::start(Box::new(fixture), TICK),
            control as Arc<dyn Control>,
            ports,
            pencere::ViewControl::new(),
        )
        .with_settings(crate::state::settings::Store::open(dir)),
    )
}

/// The settings file written for a test, and the folder it is in.
fn settings_dir(text: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    std::fs::write(dir.path().join("settings.json"), text).expect("the file");
    dir
}

fn settings_text(dir: &tempfile::TempDir) -> String {
    std::fs::read_to_string(dir.path().join("settings.json")).unwrap_or_default()
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_help_lists_the_commands_with_their_keys_and_escape_puts_it_away() {
    let (_session, mut page) = open_small(WIDE);
    page.press("?").expect("the help is asked for");
    page.wait_for(WITHIN, |page| page.says("Keys and commands"))
        .expect("the help is drawn");
    for part in [
        "Movement",
        "Pause or resume updates",
        "Show the process tree",
        "Filter processes",
        "Find a command or a process",
        "Open the settings",
    ] {
        assert!(page.says(part), "{part}: {}", short(&page));
    }
    page.press("escape").expect("escape closes it");
    page.wait_for(WITHIN, |page| !page.says("Keys and commands"))
        .expect("the help is gone");
    // The status line has the way to it, and the function key does the same.
    page.click("status/item/help").expect("the item is pressed");
    page.wait_for(WITHIN, |page| page.says("Keys and commands"))
        .expect("the help is drawn again");
    page.click("help-close").expect("the way out is pressed");
    page.wait_for(WITHIN, |page| !page.says("Keys and commands"))
        .expect("the help is gone");
    page.advance(0.2).expect("the keyboard is given back");
    page.press("f1").expect("the function key is pressed");
    page.wait_for(WITHIN, |page| page.says("Keys and commands"))
        .expect("the help is drawn by the function key");
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_palette_does_a_command_and_filters_the_table_by_what_is_typed() {
    use crate::model::processes::Mode;
    let (session, mut page) = open_small(WIDE);
    page.press("cmd+k").expect("the palette is asked for");
    page.wait_for(WITHIN, |page| page.says("Show the process tree"))
        .expect("the commands are listed");
    // The keys of a command are in its row.
    assert!(page.says("Pause or resume updates"), "{}", short(&page));
    page.type_text("process tree").expect("the words are typed");
    page.advance(0.3).expect("the page settles");
    assert!(!page.says("Pause or resume updates"), "{}", short(&page));
    page.press("enter").expect("the command is done");
    page.wait_for(WITHIN, |_| session.table().view().mode == Mode::Tree)
        .expect("the table is a tree");
    page.advance(0.2).expect("the keyboard is given back");
    // A name that is no command is the table's filter.
    page.press("ctrl+k")
        .expect("the palette is asked for again");
    page.wait_for(WITHIN, |page| page.says("Show the process tree"))
        .expect("the commands are listed again");
    page.type_text("nginx").expect("a name is typed");
    wait_for_said(&mut page, "Filter the processes by");
    page.press("enter").expect("the filter is made");
    page.wait_for(WITHIN, |_| session.table().view().filter == "nginx")
        .expect("the table is filtered");
    wait_for_said(&mut page, "nginx");
    assert!(!page.says("Show the process tree"), "{}", short(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_key_of_the_settings_does_what_the_default_did_and_is_shown_where_keys_are() {
    // Pause is Z; the mark's P is the ports' too, so it is not used and the page says why.
    let dir = settings_dir(r#"{ "keys": { "pause": ["z"], "mark": ["p"] } }"#);
    let session = session_with_settings(dir.path());
    let mut page = open(&session, WIDE, None).expect("the page opens");
    page.press("z").expect("the new key is pressed");
    page.wait_for(WITHIN, |_| session.sampler.paused())
        .expect("it paused the updates");
    // The old key is no more the command's.
    page.press("space").expect("the old key is pressed");
    page.advance(0.4).expect("the page settles");
    assert!(session.sampler.paused());
    page.press("z").expect("the new key is pressed again");
    page.wait_for(WITHIN, |_| !session.sampler.paused())
        .expect("it resumed them");
    // The help and the palette say the new key.
    page.press("?").expect("the help is asked for");
    page.wait_for(WITHIN, |page| page.says("Keys and commands"))
        .expect("the help is drawn");
    assert!(page.says("Z"), "{}", short(&page));
    page.press("escape").expect("the help is closed");
    page.advance(0.2).expect("the keyboard is given back");
    // The key that is not used is told, with the reason, in the settings.
    page.press("f2").expect("the settings are asked for");
    page.wait_for(WITHIN, |page| page.says("Settings"))
        .expect("the settings are drawn");
    let said_now = said(&page);
    assert!(
        said_now.contains("Not used: “p” for Mark or unmark the process is used by Show or hide the listening ports."),
        "{}",
        short(&page)
    );
    // Neither the settings nor the file was changed by asking.
    assert!(settings_text(&dir).contains(r#""pause""#));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_key_is_changed_by_pressing_it_a_taken_key_is_refused_and_the_file_is_written() {
    let dir = tempfile::tempdir().expect("a folder");
    let session = session_with_settings(dir.path());
    let mut page = open(&session, WIDE, None).expect("the page opens");
    page.press("f2").expect("the settings are asked for");
    page.wait_for(WITHIN, |page| page.says("Settings"))
        .expect("the settings are drawn");
    page.scroll_to("settings/scroll", 560.0)
        .expect("the keys are scrolled to");
    page.click("key-change/reverse")
        .expect("the change of a key is pressed");
    wait_for_said(&mut page, "Press the new key");
    // A key the ports have is refused, with who has it, and the command goes on waiting.
    page.press("p").expect("a taken key is pressed");
    wait_for_said(&mut page, "is used by Show or hide the listening ports");
    assert!(!dir.path().join("settings.json").exists());
    page.press("x").expect("a free key is pressed");
    page.wait_for(WITHIN, |_| settings_text(&dir).contains(r#""reverse""#))
        .expect("the file was written");
    let text = settings_text(&dir);
    assert!(text.contains('x'), "{text}");
    assert!(session.settings.get().keys.contains_key("reverse"));
    // The key does the command, once the settings are closed.
    page.click("settings-close")
        .expect("the settings are closed");
    page.wait_for(WITHIN, |page| !page.says("Settings"))
        .expect("the settings are gone");
    let before = session.table().view().descending;
    page.press("x").expect("the new key is pressed");
    page.wait_for(WITHIN, |_| session.table().view().descending != before)
        .expect("the order was reversed");
    // And the old one is the command's no longer.
    let after = session.table().view().descending;
    page.press("r").expect("the old key is pressed");
    page.advance(0.4).expect("the page settles");
    assert_eq!(session.table().view().descending, after);
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_broken_file_is_told_once_left_where_it_is_and_nothing_is_written_over_it() {
    let dir = settings_dir("{ \"keys\": ");
    let session = session_with_settings(dir.path());
    let mut page = open(&session, WIDE, None).expect("the page opens");
    wait_for_said(&mut page, "The settings file could not be used");
    page.press("f2").expect("the settings are asked for");
    page.wait_for(WITHIN, |page| page.says("Settings"))
        .expect("the settings are drawn");
    assert!(
        said(&page).contains("Nothing is written until it is fixed."),
        "{}",
        short(&page)
    );
    page.click("set-privacy").ok();
    page.press("escape").expect("the settings are closed");
    assert_eq!(settings_text(&dir), "{ \"keys\": ");
    assert!(session.settings.get().keys.is_empty());
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_settings_start_the_table_and_hide_the_names_the_person_chose_to_hide() {
    use crate::model::processes::{Column, Mode};
    let dir = settings_dir(
        r#"{ "privacy": true, "sort": "memory", "descending": false, "mode": "tree",
             "hidden": ["threads"], "details": true }"#,
    );
    let session = session_with_settings(dir.path());
    let mut page = open(&session, WIDE, None).expect("the page opens");
    page.wait_for(WITHIN, |_| {
        let view = session.table().view();
        view.sort == Column::Memory && !view.descending && view.mode == Mode::Tree
    })
    .expect("the table starts as it was left");
    // The panel of a process is open from the start.
    page.wait_for(WITHIN, |page| page.says("Working folder"))
        .expect("the panel is open");
    // The columns the person hid are not drawn.
    assert!(!page.says("Threads"), "{}", short(&page));
    // The machine's name and the person's own are not.
    let leaks: Vec<&str> = page
        .words()
        .into_iter()
        // A path is a path, whatever the name in it: what is hidden is the name a line says.
        .filter(|word| word.contains("lantern") && !word.contains('/'))
        .collect();
    assert!(leaks.is_empty(), "{leaks:?}");
    assert!(page.says("this machine"), "{}", short(&page));
    assert!(said(&page).contains("you"), "{}", short(&page));
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_help_the_palette_and_the_settings_fit_a_narrow_pane() {
    let (_session, mut page) = open_small(PANE);
    page.press("?").expect("the help is asked for");
    page.wait_for(WITHIN, |page| page.says("Keys and commands"))
        .expect("the help is drawn");
    page.press("escape").expect("the help is closed");
    page.advance(0.2).expect("the keyboard is given back");
    page.press("f2").expect("the settings are asked for");
    page.wait_for(WITHIN, |page| page.says("Settings"))
        .expect("the settings are drawn");
    // No key of the table of commands is taken by another of it.
    assert!(!said(&page).contains("Not used"), "{}", short(&page));
    page.press("escape").expect("the settings are closed");
    page.advance(0.2).expect("the keyboard is given back");
    page.press("cmd+k").expect("the palette is asked for");
    page.wait_for(WITHIN, |page| page.says("Show the process tree"))
        .expect("the palette is drawn");
    page.press("escape").expect("the palette is closed");
    page.wait_for(WITHIN, |page| page.says("Name"))
        .expect("the table is back");
}

// --- The listening ports ------------------------------------------------------------------------------

/// A port a process of the person listens on, with that process, in the fixture machine.
fn my_listener(session: &Arc<Session>) -> (u16, crate::sample::Process) {
    let sample = session.sampler.latest().expect("a sample");
    session
        .ports
        .listeners()
        .iter()
        .find_map(|listener| {
            let process = sample
                .processes
                .iter()
                .find(|process| Some(process.pid) == listener.pid && process.user == "lantern")?;
            Some((listener.port, process.clone()))
        })
        .expect("a port of the person's")
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_ports_are_listed_filtered_and_lead_to_their_process_in_the_table() {
    let (session, mut page) = open_small(WIDE);
    let (port, process) = my_listener(&session);
    page.press("p").expect("the ports are asked for");
    page.wait_for(WITHIN, |page| said(page).contains(" listening sockets"))
        .expect("the ports are listed");
    assert!(page.says("Port") && page.says("Proto"), "{}", short(&page));
    assert!(said(&page).contains(&port.to_string()), "{}", short(&page));
    // A filter is the program's: a colon and the port leave that one.
    page.command("filter").expect("the filter is asked for");
    page.type_text(&format!(":{port}"))
        .expect("the port is typed");
    wait_for_said(&mut page, " of ");
    wait_for_said(&mut page, &process.name);
    // Enter shows its process in the table, with its details.
    page.press("enter").expect("the field is left");
    page.advance(0.3).expect("the page settles");
    page.press("enter").expect("the row is opened");
    page.wait_for(WITHIN, |page| page.says("Working folder"))
        .expect("the process is shown with its details");
    wait_for_said(&mut page, "Listening on");
    assert_eq!(
        session.table().view().filter,
        format!("pid:{}", process.pid)
    );
    assert!(
        !said(&page).contains(" listening sockets"),
        "{}",
        short(&page)
    );
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_port_filter_of_the_table_leaves_the_process_that_listens_there() {
    let (session, mut page) = open_small(WIDE);
    let (port, process) = my_listener(&session);
    page.command("filter").expect("the filter is asked for");
    page.type_text(&format!("port:{port}"))
        .expect("the filter is typed");
    page.wait_for(WITHIN, |page| page.says("1 of 54 processes"))
        .expect("one process is left");
    wait_for_said(&mut page, &process.pid.to_string());
}

#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_signal_is_asked_for_of_the_process_of_the_row_of_the_ports() {
    let (session, control) = session_with_control(Fixture::small(), TICK);
    let mut page = open(&session, WIDE, None).expect("the page opens");
    let (port, process) = my_listener(&session);
    page.press("p").expect("the ports are asked for");
    page.command("filter").expect("the filter is asked for");
    page.type_text(&format!(":{port}"))
        .expect("the port is typed");
    page.wait_for(WITHIN, |page| page.says("1 of ")).ok();
    page.press("enter").expect("the field is left");
    page.advance(0.4).expect("the page settles");
    page.command("signal_kill").expect("a kill is asked for");
    wait_for_said(&mut page, &format!("Kill {}?", process.name));
    assert!(control.done().is_empty());
    page.press("escape").expect("the person gives up");
    page.wait_for(WITHIN, |page| !page.says("Cancel"))
        .expect("the question is gone");
}
