//! The page drawn into the cells of a terminal (`pencere-tui`), for the eye: the scenes of the
//! documentation's pictures at 80×24 and 160×48, written to `target/shots/<scene>-<cols>x<rows>.ans`
//! (what a terminal is sent) and `.txt` (the characters). The graphs are shown in both forms, as
//! braille characters (`-chars`) and as pictures of the terminal's own (`-pixels`). Not committed.
//!
//! The page is laid out on the cells (the measure says a column is TUI_N points, default 8, and a row
//! TUI_ROW, default 18), and the frames are written with `-r<TUI_ROW>` after their size. `TUI_THEME=light`
//! draws them in the light theme (`-light` after that). `TUI_PICTURES=none` draws them as a terminal
//! with no graphics protocol does (pictures in half blocks, and the page told so: `-nopics` last).

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use pencere_drive::headless::Error;
use pencere_tui::{CanvasMode, Cells, Frame, ImageMode, Options, Theme, render, show_texts};
use pencere_view::Id;

use crate::model::processes::Column;
use crate::page_tests::{Page, limits, on_my_processes, session_over};
use crate::sample::fixture::Fixture;

const WITHIN: Duration = Duration::from_secs(20);

fn cell_pt() -> f32 {
    std::env::var("TUI_N")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(8.0)
}

/// Whether the terminal the scenes are drawn for has Kitty graphics (the default) or none.
fn has_pictures() -> bool {
    std::env::var("TUI_PICTURES").map_or(true, |pictures| pictures != "none")
}

fn row_pt() -> f32 {
    std::env::var("TUI_ROW")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(18.0)
}

/// What the page's fields hold, by the names the library gives them (a field of an input is
/// `<input>/field`; the palette's input is a part of its own, and a new palette each time): the screen
/// draws it over the frame.
fn typed(page: &Page) -> HashMap<Id, String> {
    let palettes = (1..=9).map(|serial| format!("palette{serial}/input/field"));
    ["filter/field", "ports-filter/field", "help-filter/field"]
        .into_iter()
        .map(str::to_owned)
        .chain(palettes)
        .filter_map(|name| {
            let text = page.field(&name).filter(|text| !text.is_empty())?;
            Some((Id::named(&name), text.to_string()))
        })
        .collect()
}

fn open_cells(
    session: &std::sync::Arc<crate::session::Session>,
    (cols, rows): (u16, u16),
) -> Result<Page, Error> {
    let (n, row) = (cell_pt(), row_pt());
    let program = pencere_drive::headless::InProcess::new(crate::options(
        session,
        &std::sync::Arc::new(std::sync::Mutex::new(None)),
    ));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |since| since.as_secs_f64());
    let mut page = pencere_drive::headless::Headless::app(env!("CARGO_MANIFEST_DIR"))
        .size(f32::from(cols) * n, f32::from(rows) * row)
        .measure(Cells::new(n, row))
        .keyboard(pencere_view::KeyConvention::Linux)
        .draws(if has_pictures() {
            vec![pencere_view::Drawing::Cells]
        } else {
            vec![
                pencere_view::Drawing::Cells,
                pencere_view::Drawing::CoarseImages,
            ]
        })
        .wall_clock(now, 0)
        .limits(limits(None))
        .peer(program)
        .open()?;
    // The first sample is in once the cards speak (the load has its own word at some widths and
    // another at others).
    page.wait_for(WITHIN, |page| page.says("Memory"))?;
    page.advance(0.5)?;
    Ok(page)
}

fn shoot(
    page: &mut Page,
    name: &str,
    size: (u16, u16),
    canvases: CanvasMode,
) -> Result<Frame, Error> {
    let options = Options {
        col: cell_pt(),
        row: row_pt(),
        images: if has_pictures() {
            ImageMode::Kitty
        } else {
            ImageMode::Blocks
        },
        canvases,
        pixels: (12, 24),
    };
    let light = std::env::var("TUI_THEME").is_ok_and(|theme| theme == "light");
    let theme = if light { Theme::light() } else { Theme::dark() };
    let mut tries = 0;
    let mut frame = loop {
        let frame = render(
            page.list(),
            size.0,
            size.1,
            &theme,
            &page.page().images(),
            &options,
        );
        if !frame.waiting || tries >= 100 {
            break frame;
        }
        tries += 1;
        page.advance(0.1)?;
        std::thread::sleep(Duration::from_millis(60));
    };
    // What a field says (its hint) and holds, which the screen puts over the frame as it draws it.
    show_texts(
        &mut frame,
        page.page().fields(),
        &typed(page),
        (cell_pt(), row_pt()),
        &theme,
    );
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/shots");
    std::fs::create_dir_all(&out).expect("the folder");
    let stem = format!(
        "{name}-{}x{}-r{}{}{}",
        size.0,
        size.1,
        row_pt(),
        if light { "-light" } else { "" },
        if has_pictures() { "" } else { "-nopics" }
    );
    std::fs::write(out.join(format!("{stem}.ans")), frame.ansi()).expect("the frame written");
    std::fs::write(out.join(format!("{stem}.txt")), frame.text()).expect("the text written");
    eprintln!(
        "SHOT {stem}: text_cells={} split={} overlaps={} placed={} waiting={}",
        frame.stats.text_cells,
        frame.stats.split,
        frame.stats.overlaps,
        frame.placements.len(),
        frame.waiting,
    );
    Ok(frame)
}

/// The same page with its graphs as characters and as pictures.
fn both(page: &mut Page, name: &str, size: (u16, u16)) -> Result<(), Error> {
    shoot(page, &format!("{name}-chars"), size, CanvasMode::Characters)?;
    shoot(page, &format!("{name}-pixels"), size, CanvasMode::Picture)?;
    Ok(())
}

fn scenes(size: (u16, u16)) -> Result<(), Error> {
    // The overview: the cards of the machine and the table of its processes.
    let session = session_over(Fixture::small(), Duration::from_secs(1));
    let mut page = open_cells(&session, size)?;
    page.advance(1.0)?;
    both(&mut page, "overview", size)?;

    // The pointer over a card: its border, faint. In the next scene it stays there, over the
    // card the person has not chosen, and the chosen one is firm.
    page.hover("card/memory")?;
    page.advance(0.4)?;
    shoot(&mut page, "overview-hover", size, CanvasMode::Characters)?;

    // A card at length: the processor, with a minute of what it did.
    page.press("1")?;
    page.wait_for(WITHIN, |page| page.says("Processor use"))?;
    std::thread::sleep(Duration::from_secs(3));
    page.advance(1.0)?;
    both(&mut page, "machine", size)?;

    // The details of a process of the person.
    let session = session_over(Fixture::small(), Duration::from_secs(1));
    let mut page = open_cells(&session, size)?;
    on_my_processes(&session, &mut page);
    page.command("detail")?;
    page.wait_for(WITHIN, |page| page.says("Working folder"))?;
    std::thread::sleep(Duration::from_secs(3));
    page.advance(1.0)?;
    both(&mut page, "process", size)?;

    // The table sorted by another column, and the other way round: `>` and `r`.
    let session = session_over(Fixture::small(), Duration::from_secs(1));
    let mut page = open_cells(&session, size)?;
    page.advance(1.0)?;
    page.press(">")?;
    page.wait_for(WITHIN, |_| session.table().view().sort != Column::Cpu)?;
    page.press("r")?;
    page.wait_for(WITHIN, |_| !session.table().view().descending)?;
    page.advance(0.6)?;
    shoot(&mut page, "sorted", size, CanvasMode::Characters)?;

    // The field of the filter: at rest with its hint, taken by the keys, and with what is typed.
    let session = session_over(Fixture::small(), Duration::from_secs(1));
    let mut page = open_cells(&session, size)?;
    page.advance(1.0)?;
    shoot(&mut page, "filter-idle", size, CanvasMode::Characters)?;
    page.press("/")?;
    page.advance(0.4)?;
    shoot(&mut page, "filter-focus", size, CanvasMode::Characters)?;
    page.type_text("post")?;
    page.advance(0.4)?;
    shoot(&mut page, "filter-typed", size, CanvasMode::Characters)?;

    // The palette, with what is typed.
    let session = session_over(Fixture::small(), Duration::from_secs(1));
    let mut page = open_cells(&session, size)?;
    page.press("mod+k")?;
    page.wait_for(WITHIN, |page| page.says("Show the process tree"))?;
    page.advance(0.6)?;
    shoot(&mut page, "palette-all", size, CanvasMode::Characters)?;
    page.type_text("sort")?;
    page.advance(0.6)?;
    shoot(&mut page, "palette", size, CanvasMode::Characters)?;
    Ok(())
}

#[test]
#[ignore = "shots: draws the page into the cells of a terminal, target/shots/"]
fn the_scenes_of_the_cells() -> Result<(), Error> {
    // `TUI_SIZE=139x48` draws the scenes at that size alone.
    let asked = std::env::var("TUI_SIZE").ok().and_then(|size| {
        let (cols, rows) = size.split_once('x')?;
        Some((cols.parse().ok()?, rows.parse().ok()?))
    });
    for size in asked.map_or_else(|| vec![(80, 24), (160, 48)], |size| vec![size]) {
        scenes(size)?;
    }
    Ok(())
}
