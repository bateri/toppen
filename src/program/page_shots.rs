//! The pictures of docs/, made from the page as it is: the page and the program's handlers in one
//! process (as `page_tests` runs them), over a fixture machine, laid out and drawn in the fonts of
//! this machine (macOS: SF Mono for the terminal's face, as bateri's default is; SF for the
//! system's) by pencere's host with no screen. Not a terminal's drawing — no shaping, no shadows —
//! but the same boxes, words and colours where the kit puts them. Nothing in them is from the machine
//! they are made on: not a process, a host name or a command line.
//!
//! They draw in the fonts of the machine they are made on, so they are not part of the gate: `make
//! shots` makes them.

use std::path::{Path, PathBuf};
use std::time::Duration;

use pencere_drive::headless::Error;
use pencere_drive::shot::{Fonts, Palette};

use crate::page_tests::{PANE, Page, on_my_processes, open, session_over};
use crate::sample::fixture::Fixture;

/// A page as large as a terminal pane on a laptop.
const BIG: (f32, f32) = (1280.0, 800.0);

/// The fonts of this machine: the pictures' own layer (`Fonts::installed`), which draws as the live
/// terminal does: SF Mono and SF on a Mac (as bateri's), with the bold and italic of their variable
/// faces, and for a character they lack the first face of the machine that has it. Where the
/// machine has none, the faces the program carries.
fn fonts() -> Fonts {
    Fonts::installed()
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

    // The details of a process of the person, and the question before a signal is sent.
    let session = session_over(Fixture::small(), Duration::from_secs(1));
    let mut page = open(&session, BIG, Some(&fonts))?;
    on_my_processes(&session, &mut page);
    page.command("detail")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Working folder"))?;
    // The graphs are what the topic told since the panel opened: let it tell a while.
    std::thread::sleep(Duration::from_secs(9));
    page.advance(1.0)?;
    save(&page, &fonts, Palette::Dark, "details.png");
    page.command("signal_kill")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Cancel"))?;
    // The dialog fades in.
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "signal.png");

    // The cards at length: the processor with the pointer on its graph, memory, the network.
    let session = session_over(Fixture::small(), Duration::from_secs(1));
    let mut page = open(&session, BIG, Some(&fonts))?;
    page.press("1")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Processor use"))?;
    std::thread::sleep(Duration::from_secs(3));
    page.advance(1.0)?;
    page.hover("graph/cpu")?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "card-cpu.png");
    page.leave()?;
    page.press("2")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("What it is made of")
    })?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "card-memory.png");
    page.press("3")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Interfaces"))?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Light, "card-network-light.png");
    page.press("4")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("Reads and writes")
    })?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "card-disk.png");
    page.press("5")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("GPU use"))?;
    std::thread::sleep(Duration::from_secs(2));
    page.advance(1.0)?;
    page.hover("graph/gpu/1")?;
    page.advance(0.4)?;
    save(&page, &fonts, Palette::Dark, "card-gpu.png");

    // A machine with two cards of its own, and one with no GPU at all (the overview keeps its four).
    let session = session_over(Fixture::duo(), Duration::from_secs(1));
    let mut page = open(&session, BIG, Some(&fonts))?;
    page.press("5")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("Fixture RTX 4070")
    })?;
    std::thread::sleep(Duration::from_secs(2));
    page.advance(1.0)?;
    save(&page, &fonts, Palette::Light, "card-gpu-two.png");

    // The palette, the keys and the settings.
    let session = session_over(Fixture::small(), Duration::from_secs(1));
    let mut page = open(&session, BIG, Some(&fonts))?;
    page.press("cmd+k")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("Show the process tree")
    })?;
    page.type_text("sort")?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "palette.png");
    page.press("escape")?;
    page.advance(0.3)?;
    page.press("?")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.says("Keys and commands")
    })?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Light, "help.png");
    page.press("escape")?;
    page.advance(0.3)?;
    page.press("f2")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Settings"))?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Dark, "settings.png");

    // The palette and the settings in an 80x24 pane, in the light theme.
    let session = session_over(Fixture::small(), Duration::from_secs(1));
    let mut page = open(&session, PANE, Some(&fonts))?;
    page.press("f2")?;
    page.wait_for(Duration::from_secs(20), |page| page.says("Settings"))?;
    page.advance(0.6)?;
    save(&page, &fonts, Palette::Light, "settings-80x24.png");

    // The ports.
    let session = session_over(Fixture::small(), Duration::from_secs(1));
    let mut page = open(&session, BIG, Some(&fonts))?;
    page.press("p")?;
    page.wait_for(Duration::from_secs(20), |page| {
        page.words()
            .iter()
            .any(|word| word.ends_with(" listening sockets"))
    })?;
    page.advance(0.5)?;
    save(&page, &fonts, Palette::Dark, "ports.png");
    Ok(())
}

#[path = "page_terminal.rs"]
mod terminal;
