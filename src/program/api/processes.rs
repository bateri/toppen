//! The process table, for the page: the `view` call sets how the table is ordered, narrowed and
//! folded, and which rows are in view; the `processes` topic tells the rows in view (and some
//! around them) with each sample, and again with each change of the view.
//!
//! The page never holds the table. This is api/'s: `model/` and `sample/` do not know the page.
//!
//! The page holds the rows of the last message, and no more: a row it holds is told again by its
//! place in that message and what changed of it, so that a sample in which little changed, or a move
//! of the cursor, costs the wire a few bytes a row and not the row's words again.

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

use pencere::{Options, Subscription, ViewError};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::{Failure, call};
use crate::model::filter::Hits;
use crate::model::processes::{Change, Column, Fold, Id, Key, Mode, Row, Snapshot, View};
use crate::session::Session;

/// The most rows a window may ask for; a message is held to 1 MiB and a row may carry a long
/// command line.
const MAX_WINDOW: usize = 200;

/// The room the rows of a message may take.
const MESSAGE_BYTES: usize = 700 * 1024;

/// The longest filter a person may type.
const MAX_FILTER: usize = 400;

/// A member that may be present and `null`: `Some(None)` is the `null`.
fn present<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessRef {
    pid: u32,
    start: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupRef {
    group: String,
}

/// A row, as the page names it.
#[derive(Deserialize)]
#[serde(untagged)]
enum Ref {
    Process(ProcessRef),
    Group(GroupRef),
}

impl Ref {
    fn id(self) -> Id {
        match self {
            Ref::Process(process) => Id::Process((process.pid, process.start)),
            Ref::Group(group) => Id::Group(group.group),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Window {
    first: usize,
    count: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ViewParams {
    sort: Option<String>,
    descending: Option<bool>,
    mode: Option<String>,
    filter: Option<String>,
    hold: Option<bool>,
    #[serde(default, deserialize_with = "present")]
    root: Option<Option<ProcessRef>>,
    window: Option<Window>,
    #[serde(default, deserialize_with = "present")]
    selected: Option<Option<Ref>>,
    toggle: Option<Ref>,
    fold: Option<String>,
    /// The page does not hold what it was told: the next message tells the rows whole.
    again: Option<bool>,
}

#[derive(Serialize)]
struct RootMessage {
    pid: u32,
    start: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewMessage {
    sort: &'static str,
    descending: bool,
    mode: &'static str,
    filter: String,
    hold: bool,
    root: Option<RootMessage>,
    error: Option<String>,
}

fn view_message(view: &View) -> ViewMessage {
    ViewMessage {
        sort: view.sort.word(),
        descending: view.descending,
        mode: view.mode.word(),
        filter: view.filter.clone(),
        hold: view.hold,
        root: view.root.map(|(pid, start)| RootMessage { pid, start }),
        error: view.error.clone(),
    }
}

fn column(word: &str) -> Result<Column, Failure> {
    Column::parse(word).ok_or_else(|| {
        let words: Vec<&str> = Column::ALL.iter().map(|column| column.word()).collect();
        Failure::BadParams(format!("sort is one of {}, not {word:?}", words.join(", ")))
    })
}

fn change_of(params: ViewParams) -> Result<Change, Failure> {
    if params
        .filter
        .as_ref()
        .is_some_and(|text| text.chars().count() > MAX_FILTER)
    {
        return Err(Failure::BadParams(format!(
            "the filter is at most {MAX_FILTER} characters"
        )));
    }
    Ok(Change {
        sort: params.sort.as_deref().map(column).transpose()?,
        descending: params.descending,
        mode: params
            .mode
            .as_deref()
            .map(|word| {
                Mode::parse(word).ok_or_else(|| {
                    Failure::BadParams(format!("mode is flat, tree or group, not {word:?}"))
                })
            })
            .transpose()?,
        filter: params.filter,
        hold: params.hold,
        root: params
            .root
            .map(|root| root.map(|root| -> Key { (root.pid, root.start) })),
        window: params
            .window
            .map(|window| (window.first, window.count.min(MAX_WINDOW))),
        selected: params.selected.map(|selected| selected.map(Ref::id)),
        toggle: params.toggle.map(Ref::id),
        fold: params
            .fold
            .as_deref()
            .map(|word| match word {
                "all" => Ok(Fold::All),
                "none" => Ok(Fold::None),
                _ => Err(Failure::BadParams(format!(
                    "fold is all or none, not {word:?}"
                ))),
            })
            .transpose()?,
    })
}

fn view(session: &Session, params: ViewParams) -> Result<ViewMessage, Failure> {
    if params.again == Some(true) {
        session.retell();
    }
    let change = change_of(params)?;
    let echo = {
        let mut table = session.table();
        table.apply(change);
        view_message(&table.view())
    };
    // The watchers of the table say it again, in the new shape.
    session.sampler.touch();
    Ok(echo)
}

#[derive(Serialize)]
struct HitsMessage<'a> {
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    name: &'a [(usize, usize)],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    command: &'a [(usize, usize)],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RowMessage<'a> {
    pid: u32,
    start: u64,
    ppid: u32,
    /// A group of the processes of one name: `pid` and `start` are zero.
    group: bool,
    name: &'a str,
    user: &'a str,
    command: &'a str,
    cpu: f64,
    mem: u64,
    virt: u64,
    threads: u32,
    state: char,
    nice: i32,
    /// Seconds of processor time.
    time: f64,
    read: f64,
    write: f64,
    depth: u16,
    children: usize,
    folded: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    exited: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    hits: Option<HitsMessage<'a>>,
}

fn hundredth(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn row_message(row: &Row) -> RowMessage<'_> {
    let Row {
        id,
        process,
        depth,
        children,
        folded,
        exited,
        hits,
    } = row;
    let Hits { name, command } = hits;
    RowMessage {
        pid: process.pid,
        start: process.start,
        ppid: process.ppid,
        group: matches!(id, Id::Group(_)),
        name: &process.name,
        user: &process.user,
        command: &process.command,
        cpu: (process.cpu * 10.0).round() / 10.0,
        mem: process.memory,
        virt: process.virt,
        threads: process.threads,
        state: process.state,
        nice: process.nice,
        #[allow(clippy::cast_precision_loss)]
        time: hundredth(process.cpu_time as f64 / 1000.0),
        read: process.read.round(),
        write: process.write.round(),
        depth: *depth,
        children: *children,
        folded: *folded,
        exited: *exited,
        hits: (!name.is_empty() || !command.is_empty()).then_some(HitsMessage { name, command }),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Message {
    kind: &'static str,
    at: f64,
    total: usize,
    matched: usize,
    visible: usize,
    moved: usize,
    held: bool,
    first: usize,
    /// This message's number, counted from 1 along the subscription.
    seq: u64,
    /// The message whose rows the rows told by their place are; none where every row is whole.
    #[serde(skip_serializing_if = "Option::is_none")]
    base: Option<u64>,
    rows: Vec<Value>,
    selected_index: Option<usize>,
    view: ViewMessage,
}

/// The members a row has where its message leaves them out, so that a row told again says when one
/// goes: it did not end, it holds no hits.
fn whole(row: &RowMessage<'_>) -> Option<serde_json::Map<String, Value>> {
    let Value::Object(mut members) = serde_json::to_value(row).ok()? else {
        return None;
    };
    members.entry("exited").or_insert(Value::Bool(false));
    members
        .entry("hits")
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    Some(members)
}

/// The rows of the last message the page was sent, by the row: where each stood in it, and what it
/// was, whole. A row told again is compared with it.
#[derive(Default)]
pub(crate) struct Told {
    /// The number of that message; 0 before the first.
    seq: u64,
    rows: HashMap<Id, (usize, serde_json::Map<String, Value>)>,
}

/// `now`, which the page holds as the row at `place` of the last message, as it is told again: the
/// bare place where nothing changed, else the place (`at`) and the members that changed.
fn told_again(
    place: usize,
    now: &serde_json::Map<String, Value>,
    before: &serde_json::Map<String, Value>,
) -> Value {
    let mut members = serde_json::Map::new();
    for (key, value) in now {
        if before.get(key) != Some(value) {
            members.insert(key.clone(), value.clone());
        }
    }
    if members.is_empty() {
        return Value::from(place);
    }
    members.insert("at".to_owned(), Value::from(place));
    Value::Object(members)
}

/// A row as a full message has it: the members left out where they say nothing.
fn told_whole(row: &RowMessage<'_>) -> Option<Value> {
    serde_json::to_value(row).ok()
}

/// The message for `snapshot` of the sample at `at`, with as many of its rows as fit a message
/// (the newest rows the window asked for come first; a page that misses some asks again), and what
/// the page holds once it has it. A row the page holds from `told` is told by its place there.
fn message(snapshot: &Snapshot, view: &View, at: f64, told: &Told) -> Option<(Value, Told)> {
    let messages: Vec<RowMessage<'_>> = snapshot.rows.iter().map(row_message).collect();
    let wholes: Vec<serde_json::Map<String, Value>> =
        messages.iter().map(whole).collect::<Option<_>>()?;
    let rows: Vec<Value> = messages
        .iter()
        .zip(&wholes)
        .zip(&snapshot.rows)
        .map(|((message, now), row)| match told.rows.get(&row.id) {
            Some((place, before)) => Some(told_again(*place, now, before)),
            None => told_whole(message),
        })
        .collect::<Option<_>>()?;
    let based = snapshot
        .rows
        .iter()
        .any(|row| told.rows.contains_key(&row.id));
    let seq = told.seq + 1;
    let mut count = rows.len();
    loop {
        let found = Message {
            kind: "processes",
            at,
            total: snapshot.total,
            matched: snapshot.matched,
            visible: snapshot.visible,
            moved: snapshot.moved,
            held: snapshot.held,
            first: snapshot.first,
            seq,
            base: based.then_some(told.seq),
            rows: rows[..count].to_vec(),
            selected_index: snapshot.selected_index,
            view: view_message(view),
        };
        let value = serde_json::to_value(&found).ok()?;
        if count <= 1 || serde_json::to_vec(&value).ok()?.len() <= MESSAGE_BYTES {
            let now = Told {
                seq,
                rows: snapshot.rows[..count]
                    .iter()
                    .zip(&wholes[..count])
                    .enumerate()
                    .map(|(place, (row, whole))| (row.id.clone(), (place, whole.clone())))
                    .collect(),
            };
            return Some((value, now));
        }
        count /= 2;
    }
}

/// Tells `feed` the table with each new sample and each change of its view, until it ends.
fn watch(session: &Session, feed: &Subscription) {
    let mut watch = session.sampler.watch();
    let mut told = Told::default();
    let mut retold = session.retold();
    while let Some(sample) = watch.latest(&|| feed.is_cancelled()) {
        if session.retold() != retold {
            retold = session.retold();
            told.rows.clear();
        }
        // A scan costs: it is made only for a filter that asks about ports, and not under the lock.
        let wants_ports = session.table().wants_ports();
        let ports = wants_ports.then(|| session.ports.by_pid());
        let (snapshot, view) = {
            let mut table = session.table();
            if let Some(ports) = ports {
                table.set_ports(ports);
            }
            let snapshot = table.snapshot(&sample.processes, sample.at);
            (snapshot, table.view())
        };
        if let Some((value, now)) = message(&snapshot, &view, sample.at, &told) {
            if feed.push(value) {
                told = now;
            } else {
                // The page did not get it: what it holds is the message before, which the next one is
                // not told against. The numbers go on, so that the page sees the gap.
                told = Told {
                    seq: now.seq,
                    rows: HashMap::new(),
                };
            }
        }
    }
}

pub fn topic(options: Options, session: &Arc<Session>) -> Options {
    let session = Arc::clone(session);
    options.topic("processes", move |_, subscription| {
        let session = Arc::clone(&session);
        let feed = subscription.clone();
        let waker = session.sampler.clone();
        subscription.on_end(move || waker.wake());
        thread::Builder::new()
            .name("toppen-processes".to_owned())
            .spawn(move || watch(&session, &feed))
            .map(drop)
            .map_err(|why| {
                ViewError::with_message(
                    "internal",
                    format!("the processes cannot be watched: {why}"),
                )
            })
    })
}

pub fn calls(options: Options, session: &Arc<Session>) -> Options {
    call(options, session, "view", view)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::model::processes::Table;
    use crate::sample::fixture::Fixture;
    use crate::sample::{Process, Source};

    fn params(value: Value) -> Result<Change, Failure> {
        super::super::params::parse::<ViewParams>(value)
            .map_err(Failure::BadParams)
            .and_then(change_of)
    }

    #[test]
    fn the_view_params_become_a_change() {
        let change = params(json!({
            "sort": "memory", "descending": false, "mode": "tree", "filter": "chrome",
            "hold": true, "root": { "pid": 5, "start": 9 },
            "window": { "first": 10, "count": 5000 },
            "selected": { "group": "Safari" }, "toggle": { "pid": 1, "start": 2 }, "fold": "all",
        }))
        .unwrap();
        assert_eq!(change.sort, Some(Column::Memory));
        assert_eq!(change.mode, Some(Mode::Tree));
        assert_eq!(change.root, Some(Some((5, 9))));
        assert_eq!(change.window, Some((10, MAX_WINDOW)));
        assert_eq!(change.selected, Some(Some(Id::Group("Safari".to_owned()))));
        assert_eq!(change.toggle, Some(Id::Process((1, 2))));
        assert_eq!(change.fold, Some(Fold::All));
        // null leaves; absent keeps.
        let change = params(json!({ "root": null, "selected": null })).unwrap();
        assert_eq!((change.root, change.selected), (Some(None), Some(None)));
        let change = params(json!({})).unwrap();
        assert_eq!(
            (change.root, change.selected, change.sort),
            (None, None, None)
        );
    }

    #[test]
    fn what_does_not_fit_is_refused_by_name() {
        for (value, word) in [
            (json!({ "sort": "speed" }), "speed"),
            (json!({ "mode": "ring" }), "ring"),
            (json!({ "fold": "some" }), "some"),
            (json!({ "filter": "x".repeat(MAX_FILTER + 1) }), "at most"),
        ] {
            let Err(Failure::BadParams(why)) = params(value) else {
                panic!("{word} was accepted");
            };
            assert!(why.contains(word), "{why}");
        }
        assert!(params(json!({ "speed": 1 })).is_err());
        assert!(params(json!({ "window": { "first": 1 } })).is_err());
        assert!(params(json!({ "selected": { "pid": 1 } })).is_err());
    }

    fn table_message(rows: usize) -> Value {
        let mut fixture = Fixture::large();
        let sample = fixture.sample();
        let mut table = Table::new();
        table.apply(Change {
            window: Some((0, rows)),
            filter: Some("a".to_owned()),
            ..Change::default()
        });
        let snapshot = table.snapshot(&sample.processes, sample.at);
        message(&snapshot, &table.view(), sample.at, &Told::default())
            .unwrap()
            .0
    }

    #[test]
    fn a_message_is_the_window_the_counts_and_the_view() {
        let value = table_message(30);
        assert_eq!(value["kind"], "processes");
        assert_eq!(value["rows"].as_array().unwrap().len(), 50);
        assert!(value["total"].as_u64().unwrap() > 1500);
        assert!(value["matched"].as_u64().unwrap() <= 2000);
        assert_eq!(value["view"]["sort"], "cpu");
        assert_eq!(value["view"]["filter"], "a");
        assert_eq!(value["view"]["root"], Value::Null);
        let row = &value["rows"][0];
        for member in [
            "pid", "start", "ppid", "group", "name", "user", "command", "cpu", "mem", "virt",
            "threads", "state", "nice", "time", "read", "write", "depth", "children", "folded",
        ] {
            assert!(!row[member].is_null(), "{member}: {row}");
        }
        assert!(row["state"].is_string());
        assert!(row.get("exited").is_none());
        assert!(
            row["hits"]["name"].is_array() || row["hits"]["command"].is_array(),
            "{row}"
        );
        // A row stays small, and the full window of the page fits a message by far.
        let big = table_message(MAX_WINDOW);
        assert!(serde_json::to_vec(&big).unwrap().len() < MESSAGE_BYTES);
    }

    #[test]
    fn rows_of_a_message_too_big_are_cut_from_the_end() {
        let mut fixture = Fixture::small();
        let sample = fixture.sample();
        let mut table = Table::new();
        table.apply(Change {
            window: Some((0, 40)),
            ..Change::default()
        });
        let mut snapshot = table.snapshot(&sample.processes, sample.at);
        let model = snapshot.rows[0].clone();
        snapshot.rows = (0..200)
            .map(|at| {
                let mut row = model.clone();
                row.process = Process {
                    pid: at,
                    command: "x".repeat(6000),
                    ..row.process
                };
                row.id = Id::Process((at, row.process.start));
                row
            })
            .collect();
        let (value, told) = message(&snapshot, &table.view(), sample.at, &Told::default()).unwrap();
        let sent = value["rows"].as_array().unwrap().len();
        // The page holds the rows it was sent, not the ones cut.
        assert_eq!(told.rows.len(), sent);
        assert!(sent > 0 && sent < 200, "{sent}");
        assert!(serde_json::to_vec(&value).unwrap().len() <= MESSAGE_BYTES);
        assert_eq!(value["rows"][0]["pid"], 0);
    }

    /// The rows of `value` as the page reads them: a number is the row at that place of `before`; an
    /// object with `at` is that row with the members told over it.
    fn read(value: &Value, before: &[Value]) -> Vec<Value> {
        value["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                if let Some(place) = row.as_u64() {
                    return before[usize::try_from(place).unwrap()].clone();
                }
                let Some(place) = row["at"].as_u64() else {
                    return row.clone();
                };
                let mut now = before[usize::try_from(place).unwrap()]
                    .as_object()
                    .unwrap()
                    .clone();
                for (key, member) in row.as_object().unwrap() {
                    if key != "at" {
                        now.insert(key.clone(), member.clone());
                    }
                }
                Value::Object(now)
            })
            .collect()
    }

    /// A row as the page uses it: a member that says nothing is as one that is not there.
    fn plain(row: &Value) -> Value {
        let mut row = row.as_object().unwrap().clone();
        if row.get("exited") == Some(&Value::Bool(false)) {
            row.remove("exited");
        }
        if row
            .get("hits")
            .is_some_and(|hits| hits.as_object().is_some_and(serde_json::Map::is_empty))
        {
            row.remove("hits");
        }
        Value::Object(row)
    }

    #[test]
    fn a_row_the_page_holds_is_told_by_its_place_and_what_changed() {
        let mut fixture = Fixture::large();
        let mut table = Table::new();
        table.apply(Change {
            window: Some((0, 40)),
            filter: Some("a".to_owned()),
            ..Change::default()
        });
        let sample = fixture.sample();
        let snapshot = table.snapshot(&sample.processes, sample.at);
        let (first, told) = message(&snapshot, &table.view(), sample.at, &Told::default()).unwrap();
        assert_eq!((first["seq"].as_u64(), first.get("base")), (Some(1), None));
        assert!(
            first["rows"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row.is_object() && row.get("at").is_none())
        );

        // The same rows again (a move of the cursor): their places, and nothing else.
        let (again, told_again) = message(&snapshot, &table.view(), sample.at, &told).unwrap();
        assert_eq!(
            (again["seq"].as_u64(), again["base"].as_u64()),
            (Some(2), Some(1))
        );
        let places: Vec<u64> = again["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row.as_u64().unwrap())
            .collect();
        assert_eq!(places, (0..places.len() as u64).collect::<Vec<_>>());
        let whole = serde_json::to_vec(&first).unwrap().len();
        let placed = serde_json::to_vec(&again).unwrap().len();
        assert!(placed * 20 < whole, "{placed} of {whole}");

        // The next sample: what the page reads is the message it would have had whole.
        let before = first["rows"].as_array().unwrap().clone();
        let sample = fixture.sample();
        let snapshot = table.snapshot(&sample.processes, sample.at);
        let (next, _) = message(&snapshot, &table.view(), sample.at, &told_again).unwrap();
        let (fresh, _) = message(&snapshot, &table.view(), sample.at, &Told::default()).unwrap();
        let read_rows: Vec<Value> = read(&next, &before).iter().map(plain).collect();
        let fresh_rows: Vec<Value> = fresh["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(plain)
            .collect();
        assert_eq!(read_rows, fresh_rows);
        assert!(
            serde_json::to_vec(&next).unwrap().len() < serde_json::to_vec(&fresh).unwrap().len()
        );
    }

    #[test]
    fn a_member_that_went_is_told_as_saying_nothing() {
        let mut fixture = Fixture::large();
        let sample = fixture.sample();
        let mut table = Table::new();
        table.apply(Change {
            window: Some((0, 20)),
            filter: Some("a".to_owned()),
            ..Change::default()
        });
        let snapshot = table.snapshot(&sample.processes, sample.at);
        let (_, told) = message(&snapshot, &table.view(), sample.at, &Told::default()).unwrap();
        let mut snapshot = table.snapshot(&sample.processes, sample.at);
        for row in &mut snapshot.rows {
            row.hits = Hits::default();
        }
        let (value, _) = message(&snapshot, &table.view(), sample.at, &told).unwrap();
        let row = &value["rows"][0];
        assert_eq!(row["at"], 0);
        assert_eq!(row["hits"], serde_json::json!({}), "{row}");
    }
}
