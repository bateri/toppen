//! toppen in a terminal that hosts nothing: it draws the monitor with the characters of the terminal it is
//! started in, takes the keys, and gives the terminal back as it was. The terminal is a pty
//! (`common::terminal`) that answers what a terminal answers and shows what is written to it.

// The screen is the `terminal` feature's: the build without it has none to start.
#![cfg(feature = "terminal")]
// A test that cannot go on fails where it stands, helpers included.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::terminal::{Start, Terminal, count};
use common::{Pane, program};

const LIMIT: Duration = Duration::from_secs(60);

/// toppen, looking at the fixture machine, in a terminal of `size` that hosts nothing.
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
        graphics: false,
    })
}

/// Waits for `needle` to be written since `from` (a length of `raw`), escape sequences and all.
fn wait_for_bytes(terminal: &mut Terminal, from: usize, needle: &[u8]) -> bool {
    let end = std::time::Instant::now() + LIMIT;
    while std::time::Instant::now() < end {
        terminal.until_since(from, "\u{ffff}", Duration::from_millis(100));
        if count(&terminal.raw[from..], needle) > 0 {
            return true;
        }
    }
    false
}

/// Whether the text holds a character of braille: the graphs, where the terminal draws no pictures.
fn braille(text: &str) -> bool {
    text.chars().any(|c| ('\u{2800}'..='\u{28ff}').contains(&c))
}

#[test]
fn a_terminal_that_hosts_nothing_gets_the_monitor_drawn_in_its_cells_and_is_the_persons_again() {
    let pane = Pane::new();
    // No host answers, and nothing asks for the screen: the program waits for a host a moment, and draws.
    let mut terminal = start(&pane, (160, 48), &[]);
    assert!(terminal.until("Processes", LIMIT), "{}", terminal.text());
    assert!(terminal.until("Threads", LIMIT), "{}", terminal.text());
    assert!(terminal.until("CPU", LIMIT), "{}", terminal.text());
    // The screen is the alternate one, and the graphs are the characters of braille.
    assert_eq!(count(&terminal.raw, b"\x1b[?1049h"), 1);
    assert!(braille(&terminal.text()), "{}", terminal.text());

    // The graphs go on: the samples come, and what changed is written again.
    let from = terminal.raw.len();
    assert!(
        terminal.until_with(from, LIMIT, braille),
        "the page draws what changes as the machine does"
    );

    // The table sorts by the next column, the other way round: the head says it with an arrow.
    let from = terminal.raw.len();
    terminal.type_bytes(b">");
    terminal.type_bytes(b"r");
    assert!(
        terminal.until_since(from, "\u{25b4}", LIMIT),
        "{}",
        terminal.text()
    );

    // The palette opens with Ctrl+K and lists the commands.
    let from = terminal.raw.len();
    terminal.type_bytes(b"\x0b");
    assert!(
        terminal.until_since(from, "Sort", LIMIT),
        "{}",
        common::terminal::plain(&terminal.raw[from..])
    );
    terminal.type_bytes(b"\x1b");
    // An Escape and what follows in one read would be one key (Alt with it).
    std::thread::sleep(Duration::from_millis(300));

    // The pointer moves down a column of the screen: it is a hand over what answers a press (a row, a
    // head), and the terminal is told so (OSC 22).
    let from = terminal.raw.len();
    let moves: String = (2..=46).map(|row| format!("\x1b[<35;12;{row}M")).collect();
    terminal.type_bytes(moves.as_bytes());
    assert!(
        wait_for_bytes(&mut terminal, from, b"\x1b]22;pointer"),
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
    let cursor = raw
        .windows(6)
        .rposition(|window| window == b"\x1b[?25h")
        .expect("the cursor was shown again");
    assert!(cursor > entered, "the cursor was given back");
    assert!(
        !common::terminal::plain(raw).contains("this is the summary"),
        "the summary was printed in place of the screen"
    );
}

#[test]
fn a_terminal_that_is_asked_to_draw_it_does_so_at_once_and_the_keys_reach_the_filter() {
    let pane = Pane::new();
    // `PENCERE_TUI=1` asks for the screen without the wait for a host.
    let started = std::time::Instant::now();
    let mut terminal = start(&pane, (100, 30), &[("PENCERE_TUI", "1")]);
    assert!(terminal.until("Processes", LIMIT), "{}", terminal.text());
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "the first frame took {:?}",
        started.elapsed()
    );
    // The keyboard goes to the page: `/` takes it for the filter, what is typed is the filter's.
    let from = terminal.raw.len();
    terminal.type_bytes(b"/");
    // The page gives the field the keyboard as it draws it, and the terminal's cursor is shown at the
    // caret: what is typed after is the field's.
    assert!(
        wait_for_bytes(&mut terminal, from, b"\x1b[?25h"),
        "the field did not take the keyboard: {}",
        common::terminal::plain(&terminal.raw[from..])
    );
    terminal.type_bytes(b"post");
    assert!(
        terminal.until_since(from, "postgres", LIMIT),
        "{}",
        common::terminal::plain(&terminal.raw[from..])
    );
    terminal.type_bytes(b"\x03");
    assert_eq!(terminal.ended(LIMIT), Some(130));
}
