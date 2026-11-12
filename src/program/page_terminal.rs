//! The page as `pencere-tui` runs it where no terminal hosts it: in the cells of a plain terminal, on a
//! keyboard that is not a Mac's (Ctrl is the main modifier, there is no ⌘). What the keys of a Mac do,
//! the keys of Linux do, and what a terminal cannot send is not a key of the page.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pencere_drive::headless::{Error, Headless, InProcess};
use pencere_tui::Cells;
use pencere_view::{Drawing, Key, KeyConvention, Shortcut};

use crate::model::processes::Column;
use crate::page_tests::{Page, limits, on_my_processes, said, session_over};
use crate::sample::fixture::Fixture;
use crate::session::Session;

const WITHIN: Duration = Duration::from_secs(20);

/// A column of the cells the page is laid out on, points, and a row.
const COLUMN: f32 = 8.0;
const ROW: f32 = 18.0;

const SIZES: [(u16, u16); 2] = [(80, 24), (160, 48)];

/// The page of a terminal of `cols` by `rows` that hosts nothing, on a Linux keyboard.
fn open_on(session: &Arc<Session>, (cols, rows): (u16, u16)) -> Result<Page, Error> {
    let program = InProcess::new(crate::options(session, &Arc::new(Mutex::new(None))));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |since| since.as_secs_f64());
    let mut page = Headless::app(env!("CARGO_MANIFEST_DIR"))
        .size(f32::from(cols) * COLUMN, f32::from(rows) * ROW)
        .measure(Cells::new(COLUMN, ROW))
        .keyboard(KeyConvention::Linux)
        .draws(vec![Drawing::Cells])
        .wall_clock(now, 0)
        .limits(limits(None))
        .peer(program)
        .open()?;
    page.wait_for(WITHIN, |page| page.says("load "))?;
    page.advance(0.5)?;
    Ok(page)
}

/// The graphs move, the table sorts and filters, the palette and the help open, a card is looked into:
/// with the keys of a Linux keyboard, at 80×24 and wide.
#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn a_linux_keyboard_sorts_filters_opens_the_palette_the_help_and_a_card_with_its_keys()
-> Result<(), Error> {
    for size in SIZES {
        let session = session_over(Fixture::small(), Duration::from_secs(1));
        let mut page = open_on(&session, size)?;
        assert!(page.says("54 processes"), "{size:?}: {}", said(&page));
        // The sort: `>` is the next column, `r` reverses it.
        let sorted_by = session.table().view().sort;
        assert_eq!(sorted_by, Column::Cpu, "{size:?}");
        page.press(">")?;
        page.wait_for(WITHIN, |_| session.table().view().sort != sorted_by)?;
        page.press("r")?;
        page.wait_for(WITHIN, |_| !session.table().view().descending)?;
        // The palette: Ctrl+K, with what it lists, filtered by what is typed; Esc leaves it.
        page.press("ctrl+k")?;
        page.advance(0.5)?;
        assert!(
            page.says("Pause or resume updates"),
            "{size:?}: {}",
            said(&page)
        );
        page.type_text("sort")?;
        page.advance(0.5)?;
        assert!(page.says("Sort by the next column"), "{size:?}");
        page.press("escape")?;
        page.advance(0.4)?;
        assert!(!page.says("Sort by the next column"), "{size:?}");
        // The filter: `/` takes the keyboard, Enter gives it back.
        page.press("/")?;
        page.type_text("post")?;
        page.wait_for(WITHIN, |_| session.table().view().filter == "post")?;
        page.press("enter")?;
        page.advance(0.4)?;
        // The help names the keys as a Linux keyboard writes them: Ctrl, not ⌘.
        page.press("?")?;
        page.advance(0.6)?;
        let words = page.words();
        assert!(
            words.iter().any(|word| word.contains("Ctrl+K")),
            "{size:?}: {words:?}"
        );
        assert!(
            words
                .iter()
                .all(|word| !word.contains('⌘') && !word.contains('⌥') && !word.contains('⇧')),
            "{size:?}: {words:?}"
        );
        page.press("escape")?;
        page.advance(0.4)?;
        // A card at length.
        page.press("1")?;
        page.wait_for(WITHIN, |page| page.says("Processor use"))?;
    }
    Ok(())
}

/// The keys the page gives its commands on a Linux keyboard: in each state it can be in, what is done
/// with one is one command, and not one is a key a terminal cannot tell from another (Ctrl+C is the
/// interrupt, Ctrl+I is Tab, Ctrl+J and Ctrl+M are Enter, Ctrl+H is Backspace, Ctrl+[ is Esc), and no
/// command is given Ctrl with Shift or Alt and a letter, which a terminal with no Kitty keyboard
/// protocol sends without them.
#[test]
#[ignore = "opens the kit's interpreter: make test-page"]
fn the_keys_of_the_commands_are_one_for_each_and_none_a_terminal_cannot_send_on_a_linux_keyboard()
-> Result<(), Error> {
    let mut states: Vec<(String, Page)> = Vec::new();

    let session = session_over(Fixture::small(), Duration::from_secs(1));
    let page = open_on(&session, (160, 48))?;
    states.push(("the overview".to_owned(), page));

    let session = session_over(Fixture::small(), Duration::from_secs(1));
    let mut page = open_on(&session, (160, 48))?;
    on_my_processes(&session, &mut page);
    states.push(("a process of the person".to_owned(), page));

    let session = session_over(Fixture::small(), Duration::from_secs(1));
    let mut page = open_on(&session, (160, 48))?;
    page.press("p")?;
    page.wait_for(WITHIN, |page| {
        page.words()
            .iter()
            .any(|word| word.ends_with(" listening sockets"))
    })?;
    page.advance(0.5)?;
    states.push(("the listening ports".to_owned(), page));

    let blind = |shortcut: &Shortcut| -> Option<&'static str> {
        let (mods, key) = (shortcut.mods, shortcut.key);
        if !mods.control || mods.command {
            return None;
        }
        let letter = match key {
            Key::Character(c) => Some(c.to_ascii_lowercase()),
            _ => None,
        };
        match (letter, mods.shift, mods.alt) {
            (Some('c'), false, false) => Some("the interrupt"),
            (Some('i'), false, false) => Some("Tab"),
            (Some('j' | 'm'), false, false) => Some("Enter"),
            (Some('h'), false, false) => Some("Backspace"),
            (Some('['), false, false) => Some("Esc"),
            (Some('z' | 'c'), _, true) => {
                Some("Ctrl+Z or Ctrl+C, the keys a terminal sends for Ctrl with Alt")
            }
            (Some('z'), true, _) => Some("Ctrl+Z, the key a terminal sends for Ctrl with Shift"),
            _ => None,
        }
    };

    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut commands_seen = 0;
    for (state, page) in &states {
        let laid = page.laid().expect("a page");
        let commands = laid.commands();
        commands_seen += commands.len();
        let mut by_key: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for command in &commands {
            for shortcut in &command.keys {
                if let Some(why) = blind(shortcut) {
                    found
                        .entry(format!("{shortcut:?} is {why}"))
                        .or_default()
                        .push(command.title.clone());
                }
                if command.enabled {
                    by_key
                        .entry(format!("{shortcut:?}"))
                        .or_default()
                        .insert(command.title.clone());
                }
            }
        }
        for (key, owners) in by_key {
            if owners.len() > 1 {
                found
                    .entry(format!(
                        "{state}: {key} does {}",
                        owners.into_iter().collect::<Vec<_>>().join(" and ")
                    ))
                    .or_default();
            }
        }
    }
    assert!(
        commands_seen > 40,
        "only {commands_seen} commands were looked at"
    );
    assert!(found.is_empty(), "{found:#?}");
    Ok(())
}
