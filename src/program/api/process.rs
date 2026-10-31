//! The process the person looks at closely: `detail` (what a measurement does not say of it),
//! `signal`, `renice` and `shell` (what can be done to it), and the `process` topic that tells its
//! live values with each sample.
//!
//! A process is named by its pid and the second it started in; what is sent to it is checked again
//! by the control right before (`sample/control.rs`). This is api/'s: it knows the page.

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

use pencere::{HandOffResult, Options, Subscription, ViewError};
use serde::{Deserialize, Serialize};

use super::{Failure, call, params};
use crate::sample::control::{Key, Refused, Signal};
use crate::sample::{Process, Sample};
use crate::session::Session;

/// How many children a detail lists; the rest are counted.
const CHILDREN: usize = 40;

/// How far up the parents are followed.
const ANCESTORS: usize = 24;

/// A name of an environment variable that looks like it holds a secret.
const SECRET_WORDS: [&str; 8] = [
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "KEY",
    "CREDENTIAL",
    "AUTH",
    "COOKIE",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    pid: u32,
    start: u64,
}

impl Target {
    fn key(&self) -> Key {
        (self.pid, self.start)
    }
}

fn tenth(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn is_secret(name: &str) -> bool {
    let upper = name.to_uppercase();
    SECRET_WORDS.iter().any(|word| upper.contains(word))
}

/// The process `key` in `sample`, if it is there and still the one that was named.
fn find(sample: &Sample, key: Key) -> Option<&Process> {
    sample.processes.iter().find(|process| process.key() == key)
}

/// The processes below `key`, the deepest first, then `key`: the order a tree is ended in.
fn tree_of(sample: &Sample, key: Key) -> Vec<Key> {
    let mut kids: HashMap<u32, Vec<&Process>> = HashMap::new();
    for process in &sample.processes {
        kids.entry(process.ppid).or_default().push(process);
    }
    let mut order = vec![key];
    let mut at = 0;
    // A parent started no later than its child: a pid used again is not a parent.
    while at < order.len() && order.len() < 5000 {
        let (pid, start) = order[at];
        if let Some(list) = kids.get(&pid) {
            for child in list {
                if child.start >= start && !order.contains(&child.key()) {
                    order.push(child.key());
                }
            }
        }
        at += 1;
    }
    order.reverse();
    order
}

#[derive(Serialize)]
struct Link<'a> {
    pid: u32,
    start: u64,
    name: &'a str,
    cpu: f64,
}

#[derive(Serialize)]
struct EnvEntry {
    name: String,
    value: String,
    secret: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DetailMessage<'a> {
    pid: u32,
    start: u64,
    ppid: u32,
    name: &'a str,
    user: &'a str,
    state: char,
    nice: i32,
    threads: u32,
    cpu: f64,
    mem: u64,
    virt: u64,
    time: f64,
    read: f64,
    write: f64,
    /// The command line, a word each; the words of the sample's command where the system gave none.
    argv: Vec<String>,
    exe: Option<String>,
    cwd: Option<String>,
    env: Vec<EnvEntry>,
    fds: Option<usize>,
    /// From the nearest parent to the oldest.
    parents: Vec<Link<'a>>,
    children: Vec<Link<'a>>,
    children_total: usize,
    /// The person may signal and renice it.
    may: bool,
}

fn link(process: &Process) -> Link<'_> {
    Link {
        pid: process.pid,
        start: process.start,
        name: &process.name,
        cpu: tenth(process.cpu),
    }
}

fn the_sample(session: &Session) -> Result<Arc<Sample>, Failure> {
    session
        .sampler
        .latest()
        .ok_or_else(|| Failure::Gone("Nothing has been measured yet.".to_owned()))
}

fn gone_words(name: Option<&str>) -> String {
    match name {
        Some(name) => format!("{name} has ended."),
        None => "That process has ended.".to_owned(),
    }
}

fn detail(session: &Session, target: Target) -> Result<serde_json::Value, Failure> {
    let sample = the_sample(session)?;
    let key = target.key();
    let process = find(&sample, key).ok_or_else(|| Failure::Gone(gone_words(None)))?;
    let looked = session
        .control
        .detail(key)
        .map_err(|_| Failure::Gone(gone_words(Some(&process.name))))?;
    let mut parents = Vec::new();
    let mut at = process;
    while parents.len() < ANCESTORS {
        let Some(up) = sample
            .processes
            .iter()
            .find(|other| other.pid == at.ppid && other.pid != at.pid && other.start <= at.start)
        else {
            break;
        };
        parents.push(link(up));
        at = up;
    }
    let mut below: Vec<&Process> = sample
        .processes
        .iter()
        .filter(|other| other.ppid == process.pid && other.start >= process.start)
        .collect();
    below.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(a.pid.cmp(&b.pid)));
    let children_total = below.len();
    let children = below.into_iter().take(CHILDREN).map(link).collect();
    #[allow(clippy::cast_precision_loss)]
    let message = DetailMessage {
        pid: process.pid,
        start: process.start,
        ppid: process.ppid,
        name: &process.name,
        user: &process.user,
        state: process.state,
        nice: process.nice,
        threads: process.threads,
        cpu: tenth(process.cpu),
        mem: process.memory,
        virt: process.virt,
        time: process.cpu_time as f64 / 1000.0,
        read: process.read.round(),
        write: process.write.round(),
        argv: if looked.argv.is_empty() {
            process.command.split(' ').map(str::to_owned).collect()
        } else {
            looked.argv
        },
        exe: looked.exe,
        cwd: looked.cwd,
        env: looked
            .env
            .into_iter()
            .map(|(name, value)| EnvEntry {
                secret: is_secret(&name),
                name,
                value,
            })
            .collect(),
        fds: looked.fds,
        parents,
        children,
        children_total,
        may: session.control.may(&process.user),
    };
    serde_json::to_value(&message).map_err(|why| Failure::BadParams(why.to_string()))
}

#[derive(Serialize)]
struct Outcome {
    /// `done`, `gone`, `denied` or `failed`: what came of the process asked for.
    outcome: &'static str,
    message: Option<String>,
    /// For a signal to a tree: how each of the processes in it came out.
    done: usize,
    gone: usize,
    denied: usize,
    failed: usize,
}

impl Outcome {
    fn of(result: &Result<(), Refused>) -> (&'static str, Option<String>) {
        match result {
            Ok(()) => ("done", None),
            Err(Refused::Gone) => ("gone", None),
            Err(Refused::Denied) => ("denied", None),
            Err(Refused::Failed(why)) => ("failed", Some(why.clone())),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SignalParams {
    pid: u32,
    start: u64,
    signal: String,
    #[serde(default)]
    tree: bool,
}

fn signal(session: &Session, params: SignalParams) -> Result<Outcome, Failure> {
    let signal = Signal::parse(&params.signal).ok_or_else(|| {
        let words: Vec<&str> = Signal::ALL.iter().map(|signal| signal.word()).collect();
        Failure::BadParams(format!(
            "signal is one of {}, not {:?}",
            words.join(", "),
            params.signal
        ))
    })?;
    let key = (params.pid, params.start);
    let targets = if params.tree {
        the_sample(session).map_or_else(|_| vec![key], |sample| tree_of(&sample, key))
    } else {
        vec![key]
    };
    let mut out = Outcome {
        outcome: "done",
        message: None,
        done: 0,
        gone: 0,
        denied: 0,
        failed: 0,
    };
    for target in targets {
        let result = session.control.signal(target, signal);
        let (word, message) = Outcome::of(&result);
        match word {
            "done" => out.done += 1,
            "gone" => out.gone += 1,
            "denied" => out.denied += 1,
            _ => out.failed += 1,
        }
        if target == key {
            out.outcome = word;
            out.message = message;
        }
    }
    Ok(out)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReniceParams {
    pid: u32,
    start: u64,
    nice: i32,
}

fn renice(session: &Session, params: ReniceParams) -> Result<Outcome, Failure> {
    if !(-20..=19).contains(&params.nice) {
        return Err(Failure::BadParams(
            "nice is a whole number from -20 to 19".to_owned(),
        ));
    }
    let result = session
        .control
        .renice((params.pid, params.start), params.nice);
    let (word, message) = Outcome::of(&result);
    Ok(Outcome {
        outcome: word,
        message,
        done: usize::from(result.is_ok()),
        gone: 0,
        denied: 0,
        failed: 0,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShellParams {
    cwd: Option<String>,
}

#[derive(Serialize)]
struct Handed {
    ok: bool,
}

/// Lends the pane to the person's shell, which has the terminal until it ends; the view is shown
/// again after, afresh. The answer goes out first, and is lost with the view that asked.
fn shell(session: &Session, params: ShellParams) -> Result<Handed, Failure> {
    let Some(command) = session.control.shell(params.cwd.as_deref()) else {
        return Ok(Handed { ok: true });
    };
    match session.view.hand_off(command).wait() {
        HandOffResult::Unsupported => Err(Failure::Unavailable(
            "This terminal cannot lend its pane to a shell.".to_owned(),
        )),
        HandOffResult::Unavailable | HandOffResult::Busy => Err(Failure::Unavailable(
            "The pane cannot be lent to a shell just now.".to_owned(),
        )),
        _ => Ok(Handed { ok: true }),
    }
}

pub fn calls(options: Options, session: &Arc<Session>) -> Options {
    let options = call(options, session, "detail", detail);
    let options = call(options, session, "signal", signal);
    let options = call(options, session, "renice", renice);
    call(options, session, "shell", shell)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Live {
    kind: &'static str,
    at: f64,
    /// The process is not in this sample: it ended.
    gone: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    cpu: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mem: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    threads: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<char>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nice: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    time: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    read: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    write: Option<f64>,
}

#[allow(clippy::cast_precision_loss)]
fn live(sample: &Sample, key: Key) -> Live {
    let process = find(sample, key);
    Live {
        kind: "process",
        at: sample.at,
        gone: process.is_none(),
        cpu: process.map(|process| tenth(process.cpu)),
        mem: process.map(|process| process.memory),
        threads: process.map(|process| process.threads),
        state: process.map(|process| process.state),
        nice: process.map(|process| process.nice),
        time: process.map(|process| process.cpu_time as f64 / 1000.0),
        read: process.map(|process| process.read.round()),
        write: process.map(|process| process.write.round()),
    }
}

/// Tells `feed` the process's live values with each sample, until it ends.
fn watch(session: &Session, key: Key, feed: &Subscription) {
    let mut watch = session.sampler.watch();
    while let Some(sample) = watch.latest(&|| feed.is_cancelled()) {
        let told = live(&sample, key);
        let gone = told.gone;
        if let Ok(value) = serde_json::to_value(told) {
            feed.push(value);
        }
        // A process that ended does not come back: it is told once.
        if gone {
            break;
        }
    }
}

pub fn topic(options: Options, session: &Arc<Session>) -> Options {
    let session = Arc::clone(session);
    options.topic("process", move |params, subscription| {
        let target: Target =
            params::parse(params).map_err(|why| ViewError::with_message("bad-params", why))?;
        let session = Arc::clone(&session);
        let feed = subscription.clone();
        let waker = session.sampler.clone();
        subscription.on_end(move || waker.wake());
        thread::Builder::new()
            .name("toppen-process".to_owned())
            .spawn(move || watch(&session, target.key(), &feed))
            .map(drop)
            .map_err(|why| {
                ViewError::with_message("internal", format!("the process cannot be watched: {why}"))
            })
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::page_tests::session_with_control;
    use crate::sample::Source;
    use crate::sample::fixture::{Done, Fixture};
    use std::time::Duration;

    fn tick() -> Duration {
        Duration::from_millis(30)
    }

    /// A session whose sampler has taken a sample, with the fixture's control.
    fn started() -> (
        Arc<Session>,
        Arc<crate::sample::fixture::FixtureControl>,
        Arc<Sample>,
    ) {
        let (session, control) = session_with_control(Fixture::small(), tick());
        let mut watch = session.sampler.watch();
        let sample = watch.latest(&|| false).expect("a sample");
        (session, control, sample)
    }

    fn mine(sample: &Sample) -> &Process {
        sample
            .processes
            .iter()
            .find(|process| process.user == "lantern" && process.pid != 1)
            .expect("a process of the person")
    }

    #[test]
    fn a_detail_has_the_command_the_chain_of_parents_and_whether_it_may_be_signalled() {
        let (session, _, sample) = started();
        let process = mine(&sample);
        let value = detail(
            &session,
            Target {
                pid: process.pid,
                start: process.start,
            },
        )
        .unwrap();
        assert_eq!(value["pid"], process.pid);
        assert_eq!(value["may"], true);
        assert!(value["argv"].as_array().unwrap().len() >= 2);
        assert!(value["cwd"].as_str().unwrap().starts_with("/home/lantern"));
        let secret = value["env"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["name"] == "API_TOKEN")
            .unwrap();
        assert_eq!(secret["secret"], true);
        let home = value["env"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["name"] == "HOME")
            .unwrap();
        assert_eq!(home["secret"], false);
        // The chain goes up to init.
        let parents = value["parents"].as_array().unwrap();
        assert!(!parents.is_empty());
        assert_eq!(parents.last().unwrap()["pid"], 1);
        // Another user's process may not be signalled by this person.
        let other = sample
            .processes
            .iter()
            .find(|process| process.user == "root")
            .unwrap();
        let value = detail(
            &session,
            Target {
                pid: other.pid,
                start: other.start,
            },
        )
        .unwrap();
        assert_eq!(value["may"], false);
        // A name that holds no process is gone.
        let Err(Failure::Gone(_)) = detail(
            &session,
            Target {
                pid: process.pid,
                start: process.start + 5,
            },
        ) else {
            panic!("a start that does not hold was taken");
        };
    }

    #[test]
    fn a_signal_goes_to_the_named_process_or_says_why_not() {
        let (session, control, sample) = started();
        let process = mine(&sample);
        let sent = signal(
            &session,
            SignalParams {
                pid: process.pid,
                start: process.start,
                signal: "TERM".to_owned(),
                tree: false,
            },
        )
        .unwrap();
        assert_eq!((sent.outcome, sent.done), ("done", 1));
        assert_eq!(
            control.done(),
            [Done::Signalled(process.key(), Signal::Term)]
        );
        // A start that does not hold is gone, and nothing is sent.
        let gone = signal(
            &session,
            SignalParams {
                pid: process.pid,
                start: process.start + 9,
                signal: "KILL".to_owned(),
                tree: false,
            },
        )
        .unwrap();
        assert_eq!((gone.outcome, gone.gone), ("gone", 1));
        // The processes of others are denied.
        let other = sample
            .processes
            .iter()
            .find(|process| process.user == "root")
            .unwrap();
        let denied = signal(
            &session,
            SignalParams {
                pid: other.pid,
                start: other.start,
                signal: "KILL".to_owned(),
                tree: false,
            },
        )
        .unwrap();
        assert_eq!(denied.outcome, "denied");
        assert_eq!(control.done().len(), 1);
        // A signal that is none is refused before anything is done.
        let Err(Failure::BadParams(why)) = signal(
            &session,
            SignalParams {
                pid: 1,
                start: 1,
                signal: "SEGV".to_owned(),
                tree: false,
            },
        ) else {
            panic!("a signal that is none was taken");
        };
        assert!(why.contains("TERM"), "{why}");
    }

    #[test]
    fn a_tree_is_ended_from_its_leaves_to_its_root() {
        let (session, control, sample) = started();
        // The first process of the person that has children of its own.
        let root = sample
            .processes
            .iter()
            .filter(|process| process.user == "lantern")
            .find(|process| {
                sample.processes.iter().any(|other| {
                    other.ppid == process.pid
                        && other.user == "lantern"
                        && other.start >= process.start
                })
            })
            .expect("a branch");
        let order = tree_of(&sample, root.key());
        assert!(order.len() >= 2);
        assert_eq!(order.last(), Some(&root.key()));
        let out = signal(
            &session,
            SignalParams {
                pid: root.pid,
                start: root.start,
                signal: "TERM".to_owned(),
                tree: true,
            },
        )
        .unwrap();
        assert_eq!(out.done + out.denied + out.gone, order.len());
        assert_eq!(out.outcome, "done");
        // What was sent went in the order of the tree, and only to what the person owns.
        let sent: Vec<Key> = control
            .done()
            .into_iter()
            .filter_map(|done| match done {
                Done::Signalled(key, _) => Some(key),
                _ => None,
            })
            .collect();
        assert_eq!(sent.last(), Some(&root.key()));
        assert!(sent.iter().all(|key| order.contains(key)));
    }

    #[test]
    fn a_nice_value_is_in_range_and_a_lower_one_is_the_systems_to_refuse() {
        let (session, control, sample) = started();
        let process = mine(&sample);
        let ok = renice(
            &session,
            ReniceParams {
                pid: process.pid,
                start: process.start,
                nice: 10,
            },
        )
        .unwrap();
        assert_eq!(ok.outcome, "done");
        assert_eq!(control.done(), [Done::Reniced(process.key(), 10)]);
        let denied = renice(
            &session,
            ReniceParams {
                pid: process.pid,
                start: process.start,
                nice: -5,
            },
        )
        .unwrap();
        assert_eq!(denied.outcome, "denied");
        assert!(matches!(
            renice(
                &session,
                ReniceParams {
                    pid: process.pid,
                    start: process.start,
                    nice: 40
                }
            ),
            Err(Failure::BadParams(_))
        ));
    }

    #[test]
    fn a_shell_is_asked_for_where_the_process_works_and_the_fixture_starts_none() {
        let (session, control, _) = started();
        let handed = shell(
            &session,
            ShellParams {
                cwd: Some("/home/lantern/x".to_owned()),
            },
        )
        .unwrap();
        assert!(handed.ok);
        assert_eq!(
            control.done(),
            [Done::Shell(Some("/home/lantern/x".to_owned()))]
        );
    }

    #[test]
    fn the_live_values_say_when_the_process_has_ended() {
        let mut fixture = Fixture::small();
        let sample = fixture.sample();
        let process = mine(&sample);
        let value = serde_json::to_value(live(&sample, process.key())).unwrap();
        assert_eq!(value["gone"], false);
        assert!(value["cpu"].is_number());
        let value = serde_json::to_value(live(&sample, (process.pid, process.start + 1))).unwrap();
        assert_eq!(
            value,
            json!({ "kind": "process", "at": sample.at, "gone": true })
        );
    }
}
