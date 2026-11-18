//! toppen in a terminal that hosts nothing and draws pictures: it draws the monitor as one picture of the
//! window, takes the keys and the pointer, and gives the terminal back as it was. The terminal is a pty
//! (`common::terminal`) that answers what a terminal that draws pictures answers and keeps what is written
//! to it; what the program sent is read as the pictures and the escape sequences it wrote. (A terminal that
//! draws no pictures is given the summary: `e2e.rs`.)

// The screen is the `terminal` feature's: the build without it has none to start.
#![cfg(feature = "terminal")]
// A test that cannot go on fails where it stands, helpers included.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::{Duration, Instant};

use common::terminal::{Start, Terminal, count, plain, sends, sent, shows_page};
use common::{Pane, program};

const LIMIT: Duration = Duration::from_secs(60);

/// toppen, looking at the fixture machine, in a terminal of `size` that hosts nothing and draws pictures.
fn start(pane: &Pane, size: (u16, u16), extra: &[(&str, &str)]) -> Terminal {
    let home = pane.dir.path().to_string_lossy().into_owned();
    let state = pane.path("state").to_string_lossy().into_owned();
    let config = pane.path("config").to_string_lossy().into_owned();
    let mut env = vec![
        ("HOME", home.as_str()),
        ("XDG_STATE_HOME", state.as_str()),
        ("XDG_CONFIG_HOME", config.as_str()),
        ("TOPPEN_FIXTURE", "small"),
        ("TERM", "xterm-256color"),
        ("COLORTERM", "truecolor"),
    ];
    env.extend_from_slice(extra);
    Terminal::start(&Start {
        program: program(),
        args: &[],
        dir: pane.dir.path(),
        env: &env,
        env_remove: &["SSH_CONNECTION", "SSH_TTY", "PENCERE_TUI"],
        size,
        graphics: true,
    })
}

/// Waits until what the program has written is as `shown` says: it is asked again each time the program
/// has written more, with the same stream grown.
fn until_shown(terminal: &mut Terminal, mut shown: impl FnMut(&[u8]) -> bool) -> bool {
    let end = Instant::now() + LIMIT;
    while Instant::now() < end {
        terminal.settle(Duration::from_millis(100));
        if shown(&terminal.raw) {
            return true;
        }
    }
    false
}

/// Whether the program has sent the window as one picture: the `cols` by `rows` cells of 10 by 20 pixels,
/// a cell more each way.
fn the_window((cols, rows): (u16, u16)) -> impl FnMut(&[u8]) -> bool {
    sends(move |picture| {
        (picture.width, picture.height) == (u32::from(cols) * 10 + 10, u32::from(rows) * 20 + 20)
    })
}

/// How many parts of the picture the program has changed in place.
fn edits(raw: &[u8]) -> usize {
    count(raw, b"\x1b_Ga=f,i=")
}

#[test]
fn a_terminal_that_draws_pictures_gets_the_monitor_drawn_as_a_picture_and_is_the_persons_again() {
    let pane = Pane::new();
    // No host answers, and nothing asks for the screen: the program waits for a host a moment, asks the
    // terminal what it draws, and draws the page as one picture of the window.
    let mut terminal = start(&pane, (160, 48), &[]);
    assert!(
        until_shown(&mut terminal, the_window((160, 48))),
        "{:?}",
        sent(&terminal.raw)
    );
    // The screen is the alternate one, and no text is written to it: the picture is the screen.
    assert_eq!(count(&terminal.raw, b"\x1b[?1049h"), 1);
    assert!(
        !plain(&terminal.raw).contains("Processes"),
        "{}",
        plain(&terminal.raw)
    );
    // The first picture may be of the line that stands in for the page while its module starts, which
    // a busy machine takes seconds over: what is pressed on it reaches no page.
    assert!(
        until_shown(&mut terminal, shows_page()),
        "the page never came: the window has the line that stands in for it"
    );

    // The graphs go on: the samples come, and what changed is written again, in place.
    let had = terminal.raw.len();
    assert!(
        until_shown(&mut terminal, |raw| edits(&raw[had..]) >= 2),
        "the page draws what changes as the machine does"
    );

    // The pointer goes to a place that answers a press (the fixture's screen is the same at each start) in
    // two steps, as a hand does, and the terminal is told it is a hand (OSC 22). The program answers as
    // fast as it draws, which is slowly in a build for tests: the answer is waited for, not the steps.
    let from = terminal.raw.len();
    for y in [100, 120] {
        terminal.type_bytes(format!("\x1b[<35;125;{y}M").as_bytes());
        terminal.settle(Duration::from_millis(100));
    }
    assert!(
        until_shown(&mut terminal, |raw| count(&raw[from..], b"\x1b]22;pointer")
            > 0),
        "no hand was asked for"
    );

    // Ctrl+C ends it as a signal does, and the terminal is given back: the screen the person had, the
    // cursor, the keyboard and the pointer as they were.
    terminal.type_bytes(b"\x03");
    assert_eq!(terminal.ended(LIMIT), Some(130), "{}", terminal.text());
    let raw = &terminal.raw;
    let entered = raw
        .windows(8)
        .position(|window| window == b"\x1b[?1049h")
        .expect("the alternate screen was entered");
    let left = raw
        .windows(8)
        .rposition(|window| window == b"\x1b[?1049l")
        .expect("the alternate screen was left");
    assert!(entered < left);
    let freed = raw
        .windows(b"\x1b_Ga=d,d=A".len())
        .rposition(|window| window == b"\x1b_Ga=d,d=A")
        .expect("the pictures were given up");
    let reset = raw
        .windows(b"\x1b]22;\x1b".len())
        .rposition(|window| window == b"\x1b]22;\x1b")
        .expect("the pointer was given back");
    assert!(
        freed < left && reset < left,
        "given back before the screen is left"
    );
    let cursor = raw
        .windows(6)
        .rposition(|window| window == b"\x1b[?25h")
        .expect("the cursor was shown again");
    assert!(cursor > entered, "the cursor was given back");
    assert!(
        !plain(raw).contains("this is the summary"),
        "the summary was printed in place of the screen"
    );
}

#[test]
fn a_terminal_that_is_asked_to_draw_it_does_so_at_once_and_the_keys_do_not_end_it() {
    let pane = Pane::new();
    // `PENCERE_TUI=1` asks for the screen without the wait for a host.
    let started = Instant::now();
    let mut terminal = start(&pane, (100, 30), &[("PENCERE_TUI", "1")]);
    assert!(
        until_shown(&mut terminal, the_window((100, 30))),
        "{:?}",
        sent(&terminal.raw)
    );
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "the first frame took {:?}",
        started.elapsed()
    );
    assert!(
        until_shown(&mut terminal, shows_page()),
        "the page never came: the window has the line that stands in for it"
    );
    // The keyboard goes to the page: `/` takes it for the filter, what is typed is the filter's, and none
    // of it is the program's own.
    terminal.type_bytes(b"/post");
    let had = terminal.raw.len();
    assert!(
        until_shown(&mut terminal, |raw| edits(&raw[had..]) > 0),
        "the page drew nothing for the keys"
    );
    terminal.type_bytes(b"\x03");
    assert_eq!(terminal.ended(LIMIT), Some(130));
}
