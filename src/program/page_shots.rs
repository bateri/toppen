//! The pictures of docs/, made from the page as it is: the page and the program's handlers in one
//! process (as `page_tests` runs them), over a fixture machine, laid out and drawn in the fonts of
//! this machine (macOS: SF Mono, or Menlo, for the terminal's face, as bateri's default is; SF for the
//! system's) by pencere's host with no screen. Not a terminal's drawing — no shaping, no shadows —
//! but the same boxes, words and colours where the kit puts them. Nothing in them is from the machine
//! they are made on: not a process, a host name or a command line.
//!
//! They need the system's fonts, so they are not part of the gate: `make shots` makes them.

use std::path::{Path, PathBuf};
use std::time::Duration;

use pencere_drive::headless::Error;
use pencere_drive::shot::{Fonts, Palette};

use crate::page_tests::{PANE, Page, open, session_over};
use crate::sample::fixture::Fixture;

/// A page as large as a terminal pane on a laptop.
const BIG: (f32, f32) = (1280.0, 800.0);

/// The fonts a Mac draws a terminal and its system in: bateri's terminal face is SF Mono where the
/// system has it, else Menlo.
fn fonts() -> Fonts {
    let read = |path: &str| {
        std::fs::read(path)
            .unwrap_or_else(|why| panic!("{path}: {why} (make shots needs the system's fonts)"))
    };
    let terminal = std::fs::read("/System/Library/Fonts/SFNSMono.ttf")
        .unwrap_or_else(|_| read("/System/Library/Fonts/Menlo.ttc"));
    Fonts::none()
        .terminal(&terminal)
        .and_then(|fonts| fonts.system(&read("/System/Library/Fonts/SFNS.ttf")))
        .expect("the fonts")
}

fn docs(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join(name)
}

fn save(page: &Page, fonts: &Fonts, palette: Palette, name: &str) {
    let png = page.shot_in(fonts, palette).expect("a picture");
    std::fs::write(docs(name), png).expect("the picture written");
}

/// The page over a fixture that has run a minute, with a few samples more drawn.
fn page_of(fixture: Fixture, size: (f32, f32), fonts: &Fonts) -> Result<Page, Error> {
    // The fixture is looked at every second, as a run's default: the minute behind it is its own.
    let session = session_over(fixture, Duration::from_secs(1));
    let mut page = open(&session, size, Some(fonts))?;
    page.advance(0.5)?;
    Ok(page)
}

#[test]
#[ignore = "shots: draws the page in the fonts of this machine into docs/; make shots"]
fn the_pictures_of_the_docs() -> Result<(), Error> {
    let fonts = fonts();

    let page = page_of(Fixture::small(), BIG, &fonts)?;
    save(&page, &fonts, Palette::Dark, "overview.png");
    save(&page, &fonts, Palette::Light, "overview-light.png");

    let page = page_of(Fixture::small(), PANE, &fonts)?;
    save(&page, &fonts, Palette::Dark, "overview-80x24.png");
    Ok(())
}
