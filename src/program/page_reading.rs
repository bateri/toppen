//! What an agent reads of the page, whole, in the states that matter, against goldens, and the reading audit
//! of each: how the page is read is held to what is written down.

use std::path::{Path, PathBuf};

use std::time::Duration;

use super::page_tests::{Page, WIDE, open, session_over};
use crate::sample::fixture::Fixture;

/// Sets what the page reads now against the golden `name` (`src/program/reading/<name>.txt`), and holds the
/// page to the reading audit: a control with no name, an item said twice, a modal's page read behind it.
/// `PENCERE_BLESS=1` writes the goldens (read the diff: it is how an agent reads the page); `READING_OUT`
/// names a folder each reading is copied to, for comparing two pencere.
fn check(name: &str, page: &mut Page) {
    let reading = page.reading().text();
    if let Some(dir) = std::env::var_os("READING_OUT") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("a folder for the readings");
        std::fs::write(dir.join(format!("{name}.txt")), &reading).expect("a reading");
    }
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/program/reading")
        .join(format!("{name}.txt"));
    if std::env::var_os("PENCERE_BLESS").is_some() {
        std::fs::create_dir_all(file.parent().expect("a folder"))
            .expect("a folder for the goldens");
        std::fs::write(&file, &reading).expect("a golden");
    } else {
        let golden = std::fs::read_to_string(&file)
            .unwrap_or_else(|_| panic!("{name}: no golden at {}: PENCERE_BLESS=1", file.display()));
        assert_eq!(
            golden, reading,
            "{name}: the page is read otherwise than the golden says: PENCERE_BLESS=1 writes it"
        );
    }
    let flaws = pencere_drive::audit::report(&page.audit());
    assert!(
        flaws.is_empty(),
        "{name}: the reading audit finds:\n{flaws}"
    );
}

#[test]
#[ignore = "page: compiles the kit's interpreter and drives the page; make test-page"]
fn the_page_is_read_in_its_states() {
    // The fixture machine is looked at once (backfill and the first sample, which do not depend on the time):
    // a page read again a moment later has other numbers and other rows in the lead, which is not a change of
    // how the page is read.
    let session = session_over(Fixture::small(), Duration::from_secs(3600));
    let mut page = open(&session, WIDE, None).expect("the page opens");
    check("main", &mut page);
    page.command("filter").expect("the filter is asked for");
    page.press("t").expect("t is pressed in the filter");
    page.wait_for(std::time::Duration::from_secs(20), |_| {
        session.table().view().filter == "t"
    })
    .expect("the filter has the letter");
    check("filter", &mut page);
    page.press("cmd+k").expect("command k is pressed");
    page.wait_for(std::time::Duration::from_secs(20), |page| {
        page.says("Find a command or a process")
    })
    .expect("the palette opens");
    check("palette", &mut page);
}
