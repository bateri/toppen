//! toppen in a terminal pane, end to end: the view is opened, the page's calls and topic reach the
//! program through the tty, the ways out (⌃C, the view closed) end it, and a terminal that shows no
//! views, a script and a pipe are given the summary. See `common` for how the stand-in terminal is
//! found.

// A test that cannot go on fails where it stands, helpers included.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use common::{Pane, Script, answers, host, program, pushes, run};
use serde_json::json;

#[test]
fn the_view_is_opened_and_the_page_boots() {
    let pane = Pane::new();
    let log = pane.record(&Script::new().call("boot", json!({})));
    assert!(
        log.lines().any(
            |line| line.starts_with("program: open id=") && line.contains(" title=\"Monitor\"")
        ),
        "{log}"
    );
    assert!(
        log.contains("view: title=\"Monitor\" program=\"toppen\""),
        "{log}"
    );
    assert!(log.contains("view: icons png 32x32 dark"), "{log}");
    let heard = answers(&log);
    assert_eq!(heard.len(), 1, "{log}");
    let boot = heard[0].result();
    assert_eq!(boot["machine"]["host"], "lantern");
    assert_eq!(boot["machine"]["threads"], 8);
    assert_eq!(boot["machine"]["memory"], 16u64 << 30);
    assert_eq!(boot["features"]["pressure"], true);
    assert_eq!(boot["sampling"]["interval"], 1.0);
    assert_eq!(boot["sampling"]["paused"], false);
}

#[test]
fn the_overview_pushes_the_history_and_then_each_sample() {
    let pane = Pane::new();
    let log = pane.record(
        &Script::new()
            .host_args(&["--subscribe", "overview", "--cancel-after", "3"])
            .limit(40),
    );
    let told = pushes(&log);
    assert!(told.len() >= 3, "{log}");
    assert_eq!(told[0]["kind"], "history");
    // The fixture had run two minutes before it was asked.
    assert_eq!(told[0]["cpu"].as_array().unwrap().len(), 120);
    assert_eq!(told[0]["cores"][0].as_array().unwrap().len(), 8);
    assert_eq!(told[1]["kind"], "sample");
    assert_eq!(told[2]["kind"], "sample");
    assert!(told[2]["at"].as_f64().unwrap() > told[1]["at"].as_f64().unwrap());
    assert_eq!(told[1]["mem"]["total"], 16u64 << 30);
    assert_eq!(
        told[1]["disk"]["volumes"][0]["mount"],
        "/System/Volumes/Data"
    );
}

#[test]
fn the_sampling_call_changes_how_often_and_refuses_what_the_program_cannot() {
    let pane = Pane::new();
    let heard = pane.ask(
        &Script::new()
            .call("sampling", json!({ "interval": 2.5, "paused": true }))
            .call("sampling", json!({ "interval": 99 }))
            .call("sampling", json!({ "speed": 1 }))
            .call("boot", json!({})),
    );
    assert_eq!(heard[0].result()["interval"], 2.5);
    assert_eq!(heard[0].result()["paused"], true);
    assert!(heard[1].failed("bad-params"), "{:?}", heard[1]);
    assert!(heard[1].message().contains("0.5 to 10"));
    assert!(heard[2].failed("bad-params"), "{:?}", heard[2]);
    // What was refused changed nothing.
    assert_eq!(heard[3].result()["sampling"]["interval"], 2.5);
    assert_eq!(heard[3].result()["sampling"]["paused"], true);
}

#[test]
fn a_call_the_program_does_not_know_is_not_found() {
    let pane = Pane::new();
    let heard = pane.ask(&Script::new().call("kill-everything", json!({})));
    assert!(heard[0].failed("not-found"), "{:?}", heard[0]);
}

#[test]
fn interrupt_ends_toppen_as_a_shell_says() {
    let pane = Pane::new();
    let log = pane.record(&Script::new().call("boot", json!({})));
    assert!(log.contains("program: exited with status 130"), "{log}");
}

#[test]
fn closing_the_view_ends_toppen_with_success() {
    let pane = Pane::new();
    let log = pane.record(&Script::new().then(&["--then", "close"]));
    assert!(log.contains("reason=user"), "{log}");
    assert!(log.contains("program: exited with status 0"), "{log}");
}

#[test]
fn a_terminal_without_pencere_is_given_the_summary_and_the_words_that_say_why() {
    let pane = Pane::new();
    let mut command = pane.command(host());
    command
        .args(["--no-host", "--passthrough", "--limit", "15", "--"])
        .arg(program());
    let ran = run(command, None, Duration::from_secs(30));
    for card in ["CPU ", "Memory ", "Network ", "Disk "] {
        assert!(ran.stdout.contains(card), "{card}: {}", ran.stdout);
    }
    assert!(ran.stdout.contains("toppen on lantern"), "{}", ran.stdout);
    assert!(
        ran.stdout.contains("This terminal shows no views"),
        "{}",
        ran.stdout
    );
    assert!(
        ran.stderr.contains("exited with status 0"),
        "{}",
        ran.stderr
    );
}

#[test]
fn a_pipe_is_given_the_summary_alone() {
    let pane = Pane::new();
    let ran = run(pane.command(program()), None, Duration::from_secs(20));
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    assert!(ran.stdout.contains("CPU "), "{}", ran.stdout);
    assert!(!ran.stdout.contains("shows no views"), "{}", ran.stdout);
}

#[test]
fn once_prints_the_summary_of_this_machine_and_ends() {
    let pane = Pane::new();
    let mut command = pane.command(program());
    command.arg("--once").env_remove("TOPPEN_FIXTURE");
    let ran = run(command, None, Duration::from_secs(20));
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    for card in ["CPU ", "Memory ", "Network ", "Disk "] {
        assert!(ran.stdout.contains(card), "{card}: {}", ran.stdout);
    }
    assert!(
        ran.stdout.lines().all(|line| line.chars().count() <= 80),
        "{}",
        ran.stdout
    );
}

#[test]
fn json_prints_the_same_summary_as_one_line_of_json() {
    let pane = Pane::new();
    let mut command = pane.command(program());
    command.arg("--json").env_remove("TOPPEN_FIXTURE");
    let ran = run(command, None, Duration::from_secs(20));
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    assert_eq!(ran.stdout.lines().count(), 1, "{}", ran.stdout);
    let summary: serde_json::Value = serde_json::from_str(&ran.stdout).expect("JSON");
    assert!(summary["cpu"]["threads"].as_u64().unwrap() >= 1);
    assert!(summary["memory"]["total"].as_u64().unwrap() > 0);
    assert!((0.0..=100.0).contains(&summary["cpu"]["percent"].as_f64().unwrap()));
    assert!(summary["uptimeSeconds"].as_u64().unwrap() > 0);
}

#[test]
fn a_command_line_that_is_not_understood_ends_with_2_and_says_what_is() {
    let pane = Pane::new();
    let mut command = pane.command(program());
    command.args(["--interval", "99"]);
    let ran = run(command, None, Duration::from_secs(20));
    assert_eq!(ran.status, Some(2));
    assert!(ran.stderr.contains("0.5 to 10"), "{}", ran.stderr);
    assert!(ran.stdout.is_empty());
    let mut command = pane.command(program());
    command.arg("--version");
    let ran = run(command, None, Duration::from_secs(20));
    assert_eq!(ran.status, Some(0));
    assert!(ran.stdout.starts_with("toppen 0."), "{}", ran.stdout);
}
