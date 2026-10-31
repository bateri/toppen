//! The `ports` topic: the sockets that listen, with the process that holds each, filtered and ordered
//! the way the page asked when it subscribed (to change either, it subscribes again). The program
//! scans only while a subscription is open, and not more often than every couple of seconds.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use pencere::{Options, Subscription, ViewError};
use serde::{Deserialize, Serialize};

use super::params;
use crate::sample::ports::{FRESH, Listener, Ports};
use crate::sample::{Process, Sample};
use crate::session::Session;

/// How many rows a message carries; `matched` says how many there were.
const MAX_ROWS: usize = 500;

/// How long a filter may be.
const MAX_FILTER: usize = 200;

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Asked {
    #[serde(default)]
    filter: String,
    #[serde(default)]
    sort: Sort,
    #[serde(default)]
    descending: bool,
}

#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Sort {
    #[default]
    Port,
    Address,
    Pid,
    Process,
    User,
}

#[derive(Debug, Serialize, PartialEq)]
struct Row {
    port: u16,
    proto: &'static str,
    addr: String,
    /// `null` where the system does not let this person see who holds the socket.
    pid: Option<u32>,
    start: Option<u64>,
    name: Option<String>,
    user: Option<String>,
    command: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
struct Message {
    kind: &'static str,
    total: usize,
    matched: usize,
    rows: Vec<Row>,
}

/// The listener joined with the process that holds it, if the sample has it.
fn row_of(listener: &Listener, sample: &Sample) -> Row {
    let process: Option<&Process> = listener
        .pid
        .and_then(|pid| sample.processes.iter().find(|process| process.pid == pid));
    Row {
        port: listener.port,
        proto: listener.proto.word(),
        addr: listener.addr.clone(),
        pid: listener.pid,
        start: process.map(|process| process.start),
        name: process.map(|process| process.name.clone()),
        user: process.map(|process| process.user.clone()),
        command: process.map(|process| process.command.chars().take(300).collect()),
    }
}

/// Whether `row` holds every word of `filter`: `:80` is the port 80, any other word is part of the
/// port, the protocol, the address, the process, the user or the pid, whatever the case.
fn passes(row: &Row, filter: &str) -> bool {
    filter.split_whitespace().all(|word| {
        if let Some(port) = word.strip_prefix(':') {
            return port.parse::<u16>().is_ok_and(|port| port == row.port);
        }
        let word = word.to_lowercase();
        let pid = row.pid.map(|pid| pid.to_string());
        [
            Some(row.port.to_string()),
            Some(row.proto.to_owned()),
            Some(row.addr.to_lowercase()),
            row.name.as_ref().map(|name| name.to_lowercase()),
            row.user.as_ref().map(|user| user.to_lowercase()),
            pid,
        ]
        .into_iter()
        .flatten()
        .any(|field| field.contains(&word))
    })
}

fn message(listeners: &[Listener], sample: &Sample, asked: &Asked) -> Message {
    let mut rows: Vec<Row> = listeners
        .iter()
        .map(|listener| row_of(listener, sample))
        .collect();
    let total = rows.len();
    rows.retain(|row| passes(row, &asked.filter));
    let matched = rows.len();
    rows.sort_by(|a, b| {
        let order = match asked.sort {
            Sort::Port => (a.port, a.proto, &a.addr).cmp(&(b.port, b.proto, &b.addr)),
            Sort::Address => a.addr.cmp(&b.addr).then(a.port.cmp(&b.port)),
            Sort::Pid => a.pid.cmp(&b.pid).then(a.port.cmp(&b.port)),
            Sort::Process => a
                .name
                .as_deref()
                .map(str::to_lowercase)
                .cmp(&b.name.as_deref().map(str::to_lowercase))
                .then(a.port.cmp(&b.port)),
            Sort::User => a.user.cmp(&b.user).then(a.port.cmp(&b.port)),
        };
        if asked.descending {
            order.reverse()
        } else {
            order
        }
    });
    rows.truncate(MAX_ROWS);
    Message {
        kind: "ports",
        total,
        matched,
        rows,
    }
}

/// Tells `feed` the ports when they change, until it ends.
fn watch(session: &Session, asked: &Asked, feed: &Subscription) {
    let mut last: Option<Message> = None;
    while !feed.is_cancelled() {
        let began = Instant::now();
        let ports: &Ports = &session.ports;
        let listeners = ports.listeners();
        if let Some(sample) = session.sampler.latest() {
            let told = message(&listeners, &sample, asked);
            if last.as_ref() != Some(&told) {
                if let Ok(value) = serde_json::to_value(&told) {
                    feed.push(value);
                }
                last = Some(told);
            }
        }
        // Until the scan is stale again, looking at whether the page has gone.
        while began.elapsed() < FRESH + Duration::from_millis(100) && !feed.is_cancelled() {
            thread::sleep(Duration::from_millis(100));
        }
    }
}

pub fn topic(options: Options, session: &Arc<Session>) -> Options {
    let session = Arc::clone(session);
    options.topic("ports", move |params, subscription| {
        let asked: Asked =
            params::parse(params).map_err(|why| ViewError::with_message("bad-params", why))?;
        if asked.filter.chars().count() > MAX_FILTER {
            return Err(ViewError::with_message(
                "bad-params",
                format!("a filter of the ports is at most {MAX_FILTER} characters"),
            ));
        }
        let session = Arc::clone(&session);
        let feed = subscription.clone();
        thread::Builder::new()
            .name("toppen-ports".to_owned())
            .spawn(move || watch(&session, &asked, &feed))
            .map(drop)
            .map_err(|why| {
                ViewError::with_message("internal", format!("the ports cannot be watched: {why}"))
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::Source;
    use crate::sample::fixture::Fixture;
    use crate::sample::ports::Proto;

    fn told(filter: &str, sort: Sort, descending: bool) -> Message {
        let mut fixture = Fixture::small();
        let ports = fixture.ports();
        let sample = fixture.sample();
        message(
            &ports.listeners(),
            &sample,
            &Asked {
                filter: filter.to_owned(),
                sort,
                descending,
            },
        )
    }

    #[test]
    fn the_ports_come_with_their_processes_in_the_order_asked() {
        let all = told("", Sort::Port, false);
        assert!(all.total > 20 && all.matched == all.total, "{}", all.total);
        assert!(all.rows.windows(2).all(|pair| pair[0].port <= pair[1].port));
        // A socket nobody this person can see holds is told with no process.
        let orphan = all.rows.iter().find(|row| row.port == 631).unwrap();
        assert_eq!((orphan.pid, orphan.name.as_deref()), (None, None));
        // A process's own port names it.
        let owned = all.rows.iter().find(|row| row.pid.is_some()).unwrap();
        assert!(owned.name.is_some() && owned.user.is_some() && owned.start.is_some());
        let backwards = told("", Sort::Port, true);
        assert_eq!(
            backwards.rows.first().map(|row| row.port),
            all.rows.last().map(|row| row.port)
        );
    }

    #[test]
    fn a_filter_is_a_port_after_a_colon_or_words_of_the_row() {
        let all = told("", Sort::Port, false);
        let wanted = all.rows.iter().find(|row| row.pid.is_some()).unwrap();
        let by_port = told(&format!(":{}", wanted.port), Sort::Port, false);
        assert!(by_port.rows.iter().all(|row| row.port == wanted.port));
        assert!(by_port.matched >= 1);
        let by_name = told(wanted.name.as_deref().unwrap(), Sort::Port, false);
        assert!(by_name.rows.iter().any(|row| row.port == wanted.port));
        let none = told("zzz-nobody", Sort::Port, false);
        assert_eq!((none.matched, none.total), (0, all.total));
        // Words must all hold, whatever the case.
        let udp = told("UDP *", Sort::Port, false);
        assert!(
            udp.rows
                .iter()
                .all(|row| row.proto == "udp" && row.addr == "*")
        );
        let _ = Proto::Tcp;
    }
}
