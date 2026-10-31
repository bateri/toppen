//! The `overview` topic: the machine as the cards show it. The first value is the history the
//! sampler holds, as columns; each value after it is one sample.
//!
//! It starts a thread for each subscription, which watches the sampler and pushes; the sampler looks
//! at the machine only while a subscription is open and the updates are not paused.
//!
//! This is api/'s: the files of api/ know the page, and `sample/` does not.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use pencere::{Options, Subscription, ViewError};
use serde::Serialize;

use crate::sample::history::Point;
use crate::sample::{Sample, Volume, percent, primary_volume};
use crate::sampler::Sampler;
use crate::session::Session;

/// The most interfaces, devices and volumes a sample carries.
const LISTED: usize = 12;

/// The room a history may take of a message (a message is held to 1 MiB).
const HISTORY_BYTES: usize = 800 * 1024;

fn tenth(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[derive(Serialize)]
struct History {
    kind: &'static str,
    /// Seconds between two samples.
    interval: f64,
    at: Vec<f64>,
    cpu: Vec<f64>,
    /// For each sample, each logical processor, whole percent.
    cores: Vec<Vec<u8>>,
    mem: Vec<f64>,
    swap: Vec<f64>,
    rx: Vec<f64>,
    tx: Vec<f64>,
    read: Vec<f64>,
    write: Vec<f64>,
}

/// The history as columns, the newest points that fit a message.
fn history(points: &[Point], interval: Duration) -> History {
    let width = points.first().map_or(0, |point| point.cores.len());
    // What a point comes to, written: the numbers, and a processor's share is up to four characters.
    let room = HISTORY_BYTES / (120 + 4 * width);
    let points = &points[points.len().saturating_sub(room)..];
    History {
        kind: "history",
        interval: interval.as_secs_f64(),
        at: points.iter().map(|point| point.at.round()).collect(),
        cpu: points.iter().map(|point| tenth(point.cpu)).collect(),
        cores: points.iter().map(|point| point.cores.clone()).collect(),
        mem: points.iter().map(|point| tenth(point.memory)).collect(),
        swap: points.iter().map(|point| tenth(point.swap)).collect(),
        rx: points.iter().map(|point| point.rx.round()).collect(),
        tx: points.iter().map(|point| point.tx.round()).collect(),
        read: points.iter().map(|point| point.read.round()).collect(),
        write: points.iter().map(|point| point.write.round()).collect(),
    }
}

#[derive(Serialize)]
struct CpuMessage {
    total: f64,
    cores: Vec<f64>,
    freq: Vec<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryMessage {
    total: u64,
    used: u64,
    available: u64,
    swap_total: u64,
    swap_used: u64,
    pressure: Option<f64>,
}

#[derive(Serialize)]
struct InterfaceMessage<'a> {
    name: &'a str,
    rx: f64,
    tx: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NetworkMessage<'a> {
    rx: f64,
    tx: f64,
    total_rx: u64,
    total_tx: u64,
    interfaces: Vec<InterfaceMessage<'a>>,
}

#[derive(Serialize)]
struct DeviceMessage<'a> {
    name: &'a str,
    read: f64,
    write: f64,
}

#[derive(Serialize)]
struct VolumeMessage<'a> {
    name: &'a str,
    mount: &'a str,
    fs: &'a str,
    total: u64,
    used: u64,
    percent: f64,
}

#[derive(Serialize)]
struct DiskMessage<'a> {
    read: f64,
    write: f64,
    devices: Vec<DeviceMessage<'a>>,
    volumes: Vec<VolumeMessage<'a>>,
}

#[derive(Serialize)]
struct SampleMessage<'a> {
    kind: &'static str,
    at: f64,
    cpu: CpuMessage,
    mem: MemoryMessage,
    net: NetworkMessage<'a>,
    disk: DiskMessage<'a>,
    load: [f64; 3],
    uptime: u64,
}

/// The volumes with the one the person's files are on first.
fn primary_first(volumes: &[Volume]) -> Vec<&Volume> {
    let first = primary_volume(volumes);
    first
        .into_iter()
        .chain(volumes.iter().filter(|volume| Some(*volume) != first))
        .collect()
}

fn message(sample: &Sample) -> SampleMessage<'_> {
    SampleMessage {
        kind: "sample",
        at: sample.at,
        cpu: CpuMessage {
            total: tenth(sample.cpu.total),
            cores: sample.cpu.cores.iter().map(|core| tenth(*core)).collect(),
            freq: sample.cpu.freq.clone(),
        },
        mem: MemoryMessage {
            total: sample.memory.total,
            used: sample.memory.used,
            available: sample.memory.available,
            swap_total: sample.memory.swap_total,
            swap_used: sample.memory.swap_used,
            pressure: sample.memory.pressure.map(tenth),
        },
        net: NetworkMessage {
            rx: sample.network.rx.round(),
            tx: sample.network.tx.round(),
            total_rx: sample.network.total_rx,
            total_tx: sample.network.total_tx,
            interfaces: sample
                .network
                .interfaces
                .iter()
                .take(LISTED)
                .map(|interface| InterfaceMessage {
                    name: &interface.name,
                    rx: interface.rx.round(),
                    tx: interface.tx.round(),
                })
                .collect(),
        },
        disk: DiskMessage {
            read: sample.disk.read.round(),
            write: sample.disk.write.round(),
            devices: sample
                .disk
                .devices
                .iter()
                .take(LISTED)
                .map(|device| DeviceMessage {
                    name: &device.name,
                    read: device.read.round(),
                    write: device.write.round(),
                })
                .collect(),
            volumes: primary_first(&sample.disk.volumes)
                .into_iter()
                .take(LISTED)
                .map(|volume| VolumeMessage {
                    name: &volume.name,
                    mount: &volume.mount,
                    fs: &volume.fs,
                    total: volume.total,
                    used: volume.used,
                    percent: tenth(percent(volume.used, volume.total)),
                })
                .collect(),
        },
        load: sample.load.map(|load| (load * 100.0).round() / 100.0),
        uptime: sample.uptime,
    }
}

/// Tells `feed` the history and then each sample, until it ends.
fn watch(sampler: &Sampler, feed: &Subscription) {
    let mut watch = sampler.watch();
    let (points, interval) = watch.history();
    if let Ok(value) = serde_json::to_value(history(&points, interval)) {
        feed.push(value);
    }
    while let Some(samples) = watch.next(&|| feed.is_cancelled()) {
        for sample in samples {
            if let Ok(value) = serde_json::to_value(message(&sample)) {
                feed.push(value);
            }
        }
    }
}

pub fn topic(options: Options, session: &Arc<Session>) -> Options {
    let sampler = session.sampler.clone();
    options.topic("overview", move |_, subscription| {
        let sampler = sampler.clone();
        let feed = subscription.clone();
        let waker = sampler.clone();
        // The thread waits on the sampler, not on the feed: the end of the subscription wakes it.
        subscription.on_end(move || waker.wake());
        thread::Builder::new()
            .name("toppen-overview".to_owned())
            .spawn(move || watch(&sampler, &feed))
            .map(drop)
            .map_err(|why| {
                ViewError::with_message("internal", format!("the machine cannot be watched: {why}"))
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::history::History as Ring;
    use crate::sample::{Source, fixture::Fixture};

    fn points(mut fixture: Fixture, count: usize) -> Vec<Point> {
        let mut ring = Ring::new();
        for _ in 0..count {
            ring.push(Point::of(&fixture.sample()));
        }
        ring.points().cloned().collect()
    }

    #[test]
    fn the_history_is_columns_of_the_same_length() {
        let found = history(&points(Fixture::small(), 30), Duration::from_secs(1));
        let value = serde_json::to_value(&found).unwrap();
        assert_eq!(value["kind"], "history");
        for column in [
            "at", "cpu", "cores", "mem", "swap", "rx", "tx", "read", "write",
        ] {
            assert_eq!(value[column].as_array().unwrap().len(), 30, "{column}");
        }
        assert_eq!(value["cores"][0].as_array().unwrap().len(), 8);
    }

    #[test]
    fn a_full_history_of_the_biggest_machine_fits_a_message() {
        let found = history(&points(Fixture::large(), 900), Duration::from_secs(1));
        let text = serde_json::to_string(&found).unwrap();
        assert!(text.len() < 1024 * 1024, "{} bytes", text.len());
        // And a machine too big for all of it is told the newest.
        let wide = Point {
            cores: vec![100; 600],
            ..points(Fixture::small(), 1).remove(0)
        };
        let found = history(&vec![wide; 900], Duration::from_secs(1));
        let text = serde_json::to_string(&found).unwrap();
        assert!(text.len() < 1024 * 1024, "{} bytes", text.len());
        assert!(found.at.len() < 900);
    }

    #[test]
    fn a_sample_is_told_in_the_units_of_the_contract() {
        let mut fixture = Fixture::small();
        let sample = fixture.sample();
        let value = serde_json::to_value(message(&sample)).unwrap();
        assert_eq!(value["kind"], "sample");
        assert_eq!(value["cpu"]["cores"].as_array().unwrap().len(), 8);
        assert_eq!(value["mem"]["total"], sample.memory.total);
        assert!(value["net"]["totalRx"].is_u64());
        assert_eq!(value["disk"]["volumes"][0]["mount"], "/System/Volumes/Data");
        assert!(value["disk"]["volumes"][0]["percent"].is_number());
        assert_eq!(value["load"].as_array().unwrap().len(), 3);
        assert!(serde_json::to_string(&value).unwrap().len() < 4096);
    }
}
