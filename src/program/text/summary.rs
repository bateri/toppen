//! The summary of the machine: what `top` prints once, with the one thing a monitor can say in
//! words: CPU, memory, network and disk. `--once` and the fallback print it as text, `--json` as
//! JSON; both are made from the same [`Summary`].

use std::fmt::Write as _;

use serde::Serialize;

use crate::sample::{Machine, Sample, percent, primary_volume};

/// What the summary says, in the units the JSON states: bytes, percent 0–100, seconds.
#[derive(Debug, Serialize)]
pub struct Summary {
    pub host: String,
    pub os: String,
    pub kernel: String,
    pub arch: String,
    #[serde(rename = "uptimeSeconds")]
    pub uptime: u64,
    pub cpu: CpuSummary,
    pub memory: MemorySummary,
    pub network: NetworkSummary,
    pub disk: DiskSummary,
}

#[derive(Debug, Serialize)]
pub struct CpuSummary {
    pub model: String,
    pub threads: usize,
    pub cores: usize,
    pub percent: f64,
    pub load: [f64; 3],
}

#[derive(Debug, Serialize)]
pub struct MemorySummary {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub percent: f64,
    #[serde(rename = "swapTotal")]
    pub swap_total: u64,
    #[serde(rename = "swapUsed")]
    pub swap_used: u64,
    /// Where the system says how pressed memory is; null where it does not.
    pub pressure: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct NetworkSummary {
    #[serde(rename = "receiveBytesPerSecond")]
    pub rx: f64,
    #[serde(rename = "transmitBytesPerSecond")]
    pub tx: f64,
    #[serde(rename = "receivedBytes")]
    pub total_rx: u64,
    #[serde(rename = "transmittedBytes")]
    pub total_tx: u64,
}

#[derive(Debug, Serialize)]
pub struct DiskSummary {
    /// Null where the system does not say.
    #[serde(rename = "readBytesPerSecond")]
    pub read: Option<f64>,
    #[serde(rename = "writeBytesPerSecond")]
    pub write: Option<f64>,
    pub volume: Option<VolumeSummary>,
}

#[derive(Debug, Serialize)]
pub struct VolumeSummary {
    pub mount: String,
    pub total: u64,
    pub used: u64,
    pub percent: f64,
}

impl Summary {
    #[must_use]
    pub fn of(machine: &Machine, sample: &Sample) -> Self {
        let io = machine.features.disk_io;
        Self {
            host: machine.host.clone(),
            os: machine.os.clone(),
            kernel: machine.kernel.clone(),
            arch: machine.arch.clone(),
            uptime: sample.uptime,
            cpu: CpuSummary {
                model: machine.cpu.clone(),
                threads: machine.threads,
                cores: machine.cores,
                percent: tenth(sample.cpu.total),
                load: sample.load.map(hundredth),
            },
            memory: MemorySummary {
                total: sample.memory.total,
                used: sample.memory.used,
                available: sample.memory.available,
                percent: tenth(percent(sample.memory.used, sample.memory.total)),
                swap_total: sample.memory.swap_total,
                swap_used: sample.memory.swap_used,
                pressure: sample.memory.pressure.map(tenth),
            },
            network: NetworkSummary {
                rx: sample.network.rx.round(),
                tx: sample.network.tx.round(),
                total_rx: sample.network.total_rx,
                total_tx: sample.network.total_tx,
            },
            disk: DiskSummary {
                read: io.then(|| sample.disk.read.round()),
                write: io.then(|| sample.disk.write.round()),
                volume: primary_volume(&sample.disk.volumes).map(|volume| VolumeSummary {
                    mount: volume.mount.clone(),
                    total: volume.total,
                    used: volume.used,
                    percent: tenth(percent(volume.used, volume.total)),
                }),
            },
        }
    }

    /// The JSON of `--json`: one object, one line.
    #[must_use]
    pub fn json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_owned())
    }

    /// The text of `--once`. `note` ends it, apart: why it is text and not a view.
    #[must_use]
    pub fn text(&self, note: Option<&str>) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "toppen on {} · {} · up {}",
            self.host,
            self.os,
            duration(self.uptime)
        );
        out.push('\n');
        let mut line = |label: &str, words: String| {
            let _ = writeln!(out, "{label:<8} {words}");
        };
        line(
            "CPU",
            format!(
                "{:>5.1}%   {} threads · load {:.2} {:.2} {:.2}",
                self.cpu.percent,
                self.cpu.threads,
                self.cpu.load[0],
                self.cpu.load[1],
                self.cpu.load[2]
            ),
        );
        let mut memory = format!(
            "{:>5.1}%   {} of {}",
            self.memory.percent,
            bytes(self.memory.used),
            bytes(self.memory.total)
        );
        if let Some(pressure) = self.memory.pressure {
            let _ = write!(memory, " · pressure {pressure:.0}%");
        }
        line("Memory", memory);
        if self.memory.swap_total > 0 {
            line(
                "",
                format!(
                    "swap {} of {}",
                    bytes(self.memory.swap_used),
                    bytes(self.memory.swap_total)
                ),
            );
        }
        line(
            "Network",
            format!(
                "↓ {}  ↑ {} · {} down, {} up since boot",
                rate(self.network.rx),
                rate(self.network.tx),
                bytes(self.network.total_rx),
                bytes(self.network.total_tx)
            ),
        );
        let mut label = "Disk";
        if let (Some(read), Some(write)) = (self.disk.read, self.disk.write) {
            line(label, format!("read {}  write {}", rate(read), rate(write)));
            label = "";
        }
        if let Some(volume) = &self.disk.volume {
            line(
                label,
                format!(
                    "{} {} of {} ({:.0}%)",
                    volume.mount,
                    bytes(volume.used),
                    bytes(volume.total),
                    volume.percent
                ),
            );
        }
        if let Some(note) = note {
            out.push('\n');
            out.push_str(note);
            out.push('\n');
        }
        out
    }
}

fn tenth(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn hundredth(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// Bytes as a person reads them: 1024 to the step, one decimal.
#[must_use]
pub fn bytes(count: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    #[allow(clippy::cast_precision_loss)]
    let mut value = count as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{count} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Bytes a second.
#[must_use]
pub fn rate(per_second: f64) -> String {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let whole = per_second.max(0.0).round() as u64;
    format!("{}/s", bytes(whole))
}

/// "3d 4h", "2h 5m", "7m", "40s": the two biggest steps.
#[must_use]
pub fn duration(seconds: u64) -> String {
    let (days, hours, minutes) = (
        seconds / 86_400,
        seconds % 86_400 / 3600,
        seconds % 3600 / 60,
    );
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        format!("{seconds}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::Source;
    use crate::sample::fixture::Fixture;

    fn summary() -> Summary {
        let mut fixture = Fixture::small();
        let machine = fixture.machine();
        let _ = fixture.sample();
        Summary::of(&machine, &fixture.sample())
    }

    #[test]
    fn bytes_are_said_in_the_biggest_unit_that_keeps_them_above_one() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(1023), "1023 B");
        assert_eq!(bytes(1536), "1.5 KiB");
        assert_eq!(bytes(16 * 1024 * 1024 * 1024), "16.0 GiB");
        assert_eq!(rate(2_621_440.0), "2.5 MiB/s");
    }

    #[test]
    fn a_duration_is_its_two_biggest_steps() {
        assert_eq!(duration(40), "40s");
        assert_eq!(duration(7 * 60 + 5), "7m");
        assert_eq!(duration(2 * 3600 + 5 * 60), "2h 5m");
        assert_eq!(duration(3 * 86_400 + 4 * 3600 + 200), "3d 4h");
    }

    #[test]
    fn the_text_says_each_card_and_ends_with_the_note() {
        let text = summary().text(Some("This terminal shows no views."));
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines[0].starts_with("toppen on lantern · Fixture OS 1.0 · up 3d"),
            "{text}"
        );
        for card in ["CPU ", "Memory ", "Network ", "Disk "] {
            assert!(
                lines.iter().any(|line| line.starts_with(card)),
                "{card}: {text}"
            );
        }
        assert!(text.contains("swap "), "{text}");
        assert_eq!(lines.last(), Some(&"This terminal shows no views."));
        assert!(
            lines.iter().all(|line| line.chars().count() <= 80),
            "{text}"
        );
    }

    #[test]
    fn the_json_is_one_line_in_the_units_it_states() {
        let summary = summary();
        let json = summary.json();
        assert!(!json.contains('\n'));
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["host"], "lantern");
        assert!((0.0..=100.0).contains(&value["cpu"]["percent"].as_f64().unwrap()));
        assert!(value["memory"]["total"].as_u64().unwrap() > 0);
        assert!(value["disk"]["readBytesPerSecond"].is_number());
        assert_eq!(value["disk"]["volume"]["mount"], "/System/Volumes/Data");
    }

    #[test]
    fn a_disk_the_system_does_not_measure_says_null_and_not_zero() {
        let mut fixture = Fixture::small();
        let mut machine = fixture.machine();
        machine.features.disk_io = false;
        let summary = Summary::of(&machine, &fixture.sample());
        assert!(summary.disk.read.is_none());
        let value: serde_json::Value = serde_json::from_str(&summary.json()).unwrap();
        assert!(value["disk"]["readBytesPerSecond"].is_null());
        assert!(!summary.text(None).contains("read "));
    }
}
