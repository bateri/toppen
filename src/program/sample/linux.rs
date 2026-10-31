//! What sysinfo does not say on Linux: the reads and writes of each block device
//! (`/proc/diskstats`) and how long tasks stall for memory (`/proc/pressure/memory`, PSI).
//!
//! The parsing is on text and runs everywhere, so that it is tested on every machine the program is
//! built on; only the reading of the files is Linux's.
// On a Mac the readers are not called.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use super::{Breakdown, Device};

/// A sector in `/proc/diskstats` is 512 bytes whatever the device's own.
const SECTOR: u64 = 512;

/// The names of devices that are stacked on others or are not storage: counting them would count
/// the same bytes twice (a logical volume and the disk under it) or count memory as a disk.
const NOT_DISKS: [&str; 9] = [
    "loop", "ram", "zram", "dm-", "md", "nbd", "sr", "fd", "mtdblock",
];

/// Bytes read and written since boot, by device.
pub type Counters = HashMap<String, (u64, u64)>;

/// Whether `name` is a whole disk as far as its name says (the partitions of a disk are in the
/// system's `/sys/block` under their disk, not beside it, which `exists` is asked of).
pub fn is_whole_disk(name: &str, exists: impl Fn(&str) -> bool) -> bool {
    !NOT_DISKS.iter().any(|prefix| name.starts_with(prefix)) && exists(name)
}

/// The bytes read and written, by whole disk, in the text of `/proc/diskstats`.
pub fn parse_diskstats(text: &str, whole: impl Fn(&str) -> bool) -> Counters {
    let mut found = Counters::new();
    for line in text.lines() {
        // major minor name reads merged sectors-read ms writes merged sectors-written ...
        let fields: Vec<&str> = line.split_whitespace().collect();
        let (Some(&name), Some(read), Some(written)) = (
            fields.get(2),
            fields.get(5).and_then(|field| field.parse::<u64>().ok()),
            fields.get(9).and_then(|field| field.parse::<u64>().ok()),
        ) else {
            continue;
        };
        if whole(name) {
            found.insert(
                name.to_owned(),
                (read.saturating_mul(SECTOR), written.saturating_mul(SECTOR)),
            );
        }
    }
    found
}

/// The disks as they are now, from the files of this machine.
pub fn read_diskstats() -> Counters {
    let text = std::fs::read_to_string("/proc/diskstats").unwrap_or_default();
    parse_diskstats(&text, |name| {
        is_whole_disk(name, |disk| Path::new("/sys/block").join(disk).exists())
    })
}

/// Bytes a second of each device that moved any, between two readings `elapsed` apart, busiest first.
pub fn rates(before: &Counters, now: &Counters, elapsed: Duration) -> Vec<Device> {
    let seconds = elapsed.as_secs_f64();
    if seconds <= 0.0 {
        return Vec::new();
    }
    let mut devices: Vec<Device> = now
        .iter()
        .filter_map(|(name, &(read, written))| {
            let &(read_before, written_before) = before.get(name)?;
            // A counter that went back (a device that was removed and came again) says nothing.
            #[allow(clippy::cast_precision_loss)]
            let per_second =
                |after: u64, before: u64| after.saturating_sub(before) as f64 / seconds;
            Some(Device {
                name: name.clone(),
                read: per_second(read, read_before),
                write: per_second(written, written_before),
            })
        })
        .collect();
    devices.sort_by(|a, b| {
        (b.read + b.write)
            .total_cmp(&(a.read + a.write))
            .then_with(|| a.name.cmp(&b.name))
    });
    devices
}

/// How much of the time some task stalled for memory, 0–100, in the text of
/// `/proc/pressure/memory` (the average of the last ten seconds).
pub fn parse_pressure(text: &str) -> Option<f64> {
    let some = text.lines().find(|line| line.starts_with("some "))?;
    let value = some
        .split_whitespace()
        .find_map(|field| field.strip_prefix("avg10="))?;
    value
        .parse::<f64>()
        .ok()
        .map(|share| share.clamp(0.0, 100.0))
}

/// The memory pressure of this machine, where its kernel has PSI.
pub fn read_pressure() -> Option<f64> {
    parse_pressure(&std::fs::read_to_string("/proc/pressure/memory").ok()?)
}

/// What `/proc/<pid>/stat` says that sysinfo does not: the state letter, the nice value and the
/// number of threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stat {
    pub state: char,
    pub nice: i32,
    pub threads: u32,
}

/// The `stat` text of a process. The name is in parentheses and may hold spaces and parentheses
/// itself, so the fields are counted from the last closing one.
pub fn parse_stat(text: &str) -> Option<Stat> {
    let after = &text[text.rfind(')')? + 1..];
    let fields: Vec<&str> = after.split_whitespace().collect();
    // state ppid pgrp session tty tpgid flags minflt cminflt majflt cmajflt utime stime cutime
    // cstime priority nice num_threads
    Some(Stat {
        state: fields.first()?.chars().next()?,
        nice: fields.get(16)?.parse().ok()?,
        threads: fields.get(17)?.parse().ok()?,
    })
}

/// The `stat` of the process `pid`, where it is still there.
pub fn read_stat(pid: u32) -> Option<Stat> {
    parse_stat(&std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
}

/// What memory is made of, from the text of `/proc/meminfo`: the files kept in memory (the page
/// cache and the buffers, and what the kernel could give back of its own, less what is shared and
/// cannot be dropped), what is free, and what programs hold, which is the rest.
#[must_use]
pub fn parse_meminfo(text: &str) -> Option<Breakdown> {
    let kib = |name: &str| -> Option<u64> {
        text.lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix(':'))
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|number| number.parse::<u64>().ok())
            .map(|kib| kib * 1024)
    };
    let total = kib("MemTotal")?;
    let free = kib("MemFree")?.min(total);
    let shared = kib("Shmem").unwrap_or(0);
    let cached = (kib("Cached").unwrap_or(0)
        + kib("Buffers").unwrap_or(0)
        + kib("SReclaimable").unwrap_or(0))
    .saturating_sub(shared)
    .min(total - free);
    Some(Breakdown {
        app: total - free - cached,
        wired: 0,
        compressed: 0,
        cached,
        free,
    })
}

#[cfg(target_os = "linux")]
pub fn read_meminfo() -> Option<Breakdown> {
    parse_meminfo(&std::fs::read_to_string("/proc/meminfo").ok()?)
}

#[cfg(not(target_os = "linux"))]
pub fn read_meminfo() -> Option<Breakdown> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISKSTATS: &str = "\
   7       0 loop0 10 0 80 5 0 0 0 0 0 4 5
 259       0 nvme0n1 5000 0 2000000 900 3000 0 1000000 700 0 800 1600
 259       1 nvme0n1p1 4000 0 1500000 800 2000 0 500000 600 0 700 1400
 253       0 dm-0 4000 0 1500000 800 2000 0 500000 600 0 700 1400
   8       0 sda 100 0 4096 10 50 0 2048 5 0 10 15
";

    fn whole(name: &str) -> bool {
        // /sys/block has the disks, not their partitions.
        is_whole_disk(name, |disk| {
            matches!(disk, "nvme0n1" | "sda" | "loop0" | "dm-0")
        })
    }

    #[test]
    fn only_whole_disks_are_counted_and_not_the_ones_stacked_on_them() {
        let counters = parse_diskstats(DISKSTATS, whole);
        let mut names: Vec<&String> = counters.keys().collect();
        names.sort();
        assert_eq!(names, ["nvme0n1", "sda"]);
        assert_eq!(counters["nvme0n1"], (2_000_000 * 512, 1_000_000 * 512));
        assert_eq!(counters["sda"], (4096 * 512, 2048 * 512));
    }

    #[test]
    fn a_line_that_is_cut_short_is_passed_over() {
        let counters = parse_diskstats("259 0 nvme0n1 5000 0\n\n", whole);
        assert!(counters.is_empty());
    }

    #[test]
    fn rates_are_of_the_time_between_two_readings_and_busiest_first() {
        let before = parse_diskstats(DISKSTATS, whole);
        let mut now = before.clone();
        now.insert("sda".to_owned(), (4096 * 512 + 2_000_000, 2048 * 512));
        now.insert(
            "nvme0n1".to_owned(),
            (2_000_000 * 512, 1_000_000 * 512 + 1_000_000),
        );
        now.insert("sdb".to_owned(), (1, 1));
        let devices = rates(&before, &now, Duration::from_secs(2));
        // sdb has no reading before it: it is not guessed at.
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].name, "sda");
        assert!((devices[0].read - 1_000_000.0).abs() < 1e-6);
        assert!((devices[1].write - 500_000.0).abs() < 1e-6);
    }

    #[test]
    fn a_counter_that_went_back_is_a_rate_of_nothing() {
        let before = Counters::from([("sda".to_owned(), (100, 100))]);
        let now = Counters::from([("sda".to_owned(), (10, 10))]);
        let devices = rates(&before, &now, Duration::from_secs(1));
        assert!(devices[0].read.abs() < 1e-9 && devices[0].write.abs() < 1e-9);
        assert!(rates(&before, &now, Duration::ZERO).is_empty());
    }

    #[test]
    fn a_process_stat_is_read_from_the_last_parenthesis() {
        let text = "4242 (tmux: server (1)) S 1 4242 4242 0 -1 4194560 3000 0 0 0 31 12 0 0 20 -5 3 0 9100 1 2 3";
        assert_eq!(
            parse_stat(text),
            Some(Stat {
                state: 'S',
                nice: -5,
                threads: 3
            })
        );
        assert_eq!(parse_stat("4242 (short) S 1"), None);
        assert_eq!(parse_stat(""), None);
    }

    #[test]
    fn pressure_is_the_ten_second_average_of_some() {
        let text = "some avg10=12.50 avg60=3.00 avg300=1.00 total=123\nfull avg10=4.00 avg60=1.00 avg300=0.00 total=9\n";
        assert_eq!(parse_pressure(text), Some(12.5));
        assert_eq!(parse_pressure(""), None);
        assert_eq!(parse_pressure("some avg60=1.0\n"), None);
    }
}

#[cfg(test)]
mod meminfo_tests {
    use super::*;

    #[test]
    fn memory_is_what_programs_hold_what_is_cached_and_what_is_free() {
        let text = "MemTotal:       16000000 kB\nMemFree:         2000000 kB\nBuffers:          500000 kB\nCached:          5000000 kB\nShmem:            500000 kB\nSReclaimable:     500000 kB\n";
        let breakdown = parse_meminfo(text).unwrap();
        assert_eq!(breakdown.free, 2_000_000 * 1024);
        assert_eq!(breakdown.cached, 5_500_000 * 1024);
        assert_eq!(breakdown.app, 8_500_000 * 1024);
        assert_eq!(
            breakdown.app + breakdown.cached + breakdown.free,
            16_000_000 * 1024
        );
        assert!(parse_meminfo("nothing here").is_none());
    }
}
