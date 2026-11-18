//! What the machine measures: a [`Source`] answers with the machine's facts once and with a
//! [`Sample`] of its state each time it is asked. The real source ([`system::System`]) reads the
//! operating system; the fixture ([`fixture::Fixture`], debug builds) is a machine that changes with
//! time, the same way every run, which is what the tests and the pictures of `docs/` run on.
//!
//! This is `sample/`: it knows nothing of the page. It takes and gives plain Rust values.

pub mod control;
pub mod gpu;
pub mod history;
// What a Mac's registry says is read by code every system builds, so that its parsing is tested
// everywhere; only a Mac calls it, and on another system the clippy gate sees no caller.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub mod iokit;
pub mod linux;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub mod macos;
pub mod ports;
pub mod system;

#[cfg(debug_assertions)]
pub mod fixture;

/// What a machine is, which does not change while the program runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Machine {
    pub host: String,
    /// "macOS 26.0", "Ubuntu 24.04.1 LTS".
    pub os: String,
    pub kernel: String,
    pub arch: String,
    /// The processor's brand string.
    pub cpu: String,
    /// Logical processors, the ones the per-core bars are drawn for.
    pub threads: usize,
    pub cores: usize,
    /// Physical memory, bytes.
    pub memory: u64,
    /// Seconds since the epoch the machine started at.
    pub booted: u64,
    pub features: Features,
}

/// What this machine can say. A card whose reading is not here is not drawn at all, rather than
/// drawn as a zero that looks like a measurement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Features {
    /// The system's own measure of how hard memory is pressed (macOS's memory pressure, Linux's PSI).
    pub pressure: bool,
    /// Disk reads and writes in bytes a second.
    pub disk_io: bool,
    /// At least one graphics processor that says how busy it is: no GPU card is drawn without.
    pub gpu: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cpu {
    /// All processors together, 0–100.
    pub total: f64,
    /// Each logical processor, 0–100.
    pub cores: Vec<f64>,
    /// The frequency of each logical processor, MHz (0 where the system does not say).
    pub freq: Vec<u64>,
}

/// What memory is made of, bytes, where the system says; the parts add up to the total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breakdown {
    /// What programs hold.
    pub app: u64,
    /// What the system keeps in place and cannot give back (macOS).
    pub wired: u64,
    /// What the system squeezed to hold more (macOS).
    pub compressed: u64,
    /// Files kept in memory, which can be dropped.
    pub cached: u64,
    pub free: u64,
}

/// A temperature sensor, degrees Celsius: or the sensors of a family told as one ([`told_sensors`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Sensor {
    pub name: String,
    pub celsius: f64,
    /// How many sensors it is: 1, or more where a family is told as one, at the hottest of them.
    pub count: usize,
}

/// The most sensors a sample tells.
pub const MOST_SENSORS: usize = 24;

/// The sensors as they are told: those whose names differ only by an index at the end (`PMU tdie0`
/// to `PMU tdie10`, `PMU TP0s` to `PMU TP3g`, `coretemp Core 0` to `Core 11`) as one, under the name
/// they share, at the hottest of them and with how many they are; then by name, at most
/// [`MOST_SENSORS`]. A Mac tells two dozen and more, most of them of a few such families: one by one
/// they filled the page, and the cap cut away the last of them (the battery's).
pub fn told_sensors(sensors: Vec<Sensor>) -> Vec<Sensor> {
    let mut families: std::collections::BTreeMap<String, Sensor> =
        std::collections::BTreeMap::new();
    for sensor in sensors {
        match families.get_mut(family_of(&sensor.name)) {
            Some(told) => {
                told.count += sensor.count;
                told.celsius = told.celsius.max(sensor.celsius);
            }
            None => {
                families.insert(family_of(&sensor.name).to_owned(), sensor);
            }
        }
    }
    let mut told: Vec<Sensor> = families
        .into_iter()
        .map(|(family, mut sensor)| {
            if sensor.count > 1 {
                sensor.name = family;
            }
            sensor
        })
        .collect();
    told.sort_by(|a, b| a.name.cmp(&b.name));
    told.truncate(MOST_SENSORS);
    told
}

/// The name a sensor shares with the others of its family: its own without the index at its end, a
/// number with at most one letter after it (`PMU tdie10`, `PMU TP1g`); the whole name where it ends
/// in no number, or is nothing but one.
fn family_of(name: &str) -> &str {
    let bytes = name.as_bytes();
    let mut end = bytes.len();
    if end >= 2 && bytes[end - 1].is_ascii_alphabetic() && bytes[end - 2].is_ascii_digit() {
        end -= 1;
    }
    let letters_end = end;
    while end > 0 && bytes[end - 1].is_ascii_digit() {
        end -= 1;
    }
    // Only ASCII was taken off the end, so `end` is at a character's boundary.
    let family = name[..end].trim_end();
    if end == letters_end || family.is_empty() {
        name
    } else {
        family
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Memory {
    pub total: u64,
    /// What programs hold: total minus what could be given to a program at once.
    pub used: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    /// How pressed memory is, 0–100, where the system says.
    pub pressure: Option<f64>,
    pub breakdown: Option<Breakdown>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Interface {
    pub name: String,
    /// Bytes a second since the sample before.
    pub rx: f64,
    pub tx: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Network {
    pub rx: f64,
    pub tx: f64,
    /// Bytes since the interfaces came up.
    pub total_rx: u64,
    pub total_tx: u64,
    /// The interfaces that carried something, busiest first.
    pub interfaces: Vec<Interface>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub name: String,
    /// Bytes a second since the sample before.
    pub read: f64,
    pub write: f64,
}

/// A place files live, counted once however many mount points it has.
#[derive(Debug, Clone, PartialEq)]
pub struct Volume {
    pub name: String,
    pub mount: String,
    pub fs: String,
    pub total: u64,
    pub used: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Disk {
    pub read: f64,
    pub write: f64,
    pub devices: Vec<Device>,
    pub volumes: Vec<Volume>,
}

/// A process as one look at the machine found it. A process is named by its pid and the second it
/// started in: a pid is used again after its process ends.
#[derive(Debug, Clone, PartialEq)]
pub struct Process {
    pub pid: u32,
    pub ppid: u32,
    /// Seconds since the epoch the process started at.
    pub start: u64,
    pub name: String,
    pub user: String,
    /// The command line, words joined by a space; the name where the system says none.
    pub command: String,
    /// Percent of one processor: 100 is one processor busy, and a process of many threads passes it.
    pub cpu: f64,
    /// Resident memory, bytes.
    pub memory: u64,
    /// Virtual memory, bytes.
    pub virt: u64,
    pub threads: u32,
    /// One letter: R running, S sleeping, D waiting on a disk, T stopped, Z zombie, I idle.
    pub state: char,
    pub nice: i32,
    /// Processor time the process has used, milliseconds.
    pub cpu_time: u64,
    /// Bytes a second since the sample before.
    pub read: f64,
    pub write: f64,
}

impl Process {
    /// What a person names the process by: its pid and when it started.
    #[must_use]
    pub fn key(&self) -> (u32, u64) {
        (self.pid, self.start)
    }
}

/// One look at the machine.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// Seconds since the epoch.
    pub at: f64,
    pub cpu: Cpu,
    pub memory: Memory,
    pub network: Network,
    pub disk: Disk,
    /// The load average of the last 1, 5 and 15 minutes.
    pub load: [f64; 3],
    /// Seconds since the machine started.
    pub uptime: u64,
    pub processes: Vec<Process>,
    /// The temperatures, while a card's details are open (`Source::detail`); none otherwise.
    pub sensors: Vec<Sensor>,
    /// The graphics processors, as many as the machine has that say how busy they are.
    pub gpus: Vec<gpu::Gpu>,
}

/// Where measurements come from.
pub trait Source: Send {
    fn machine(&self) -> Machine;

    /// Looks at the machine. Rates are of the time since the call before, or since the source was
    /// made.
    fn sample(&mut self) -> Sample;

    /// What the source saw before it was asked, oldest first: a fixture machine has been running for
    /// a while, which is what a picture or a test of the page wants to start from. A real source has
    /// seen nothing the program did not.
    fn backfill(&mut self) -> Vec<Sample> {
        Vec::new()
    }

    /// How long after a look the next one has a meaning: the system's CPU usage is of the time
    /// between two looks, and two looks at once say nothing.
    fn warmup(&self) -> std::time::Duration {
        std::time::Duration::ZERO
    }

    /// Whether a card's details are open: what costs more to read (the sensors) is read only then.
    fn detail(&mut self, _open: bool) {}
}

/// The volume the disk card speaks of: the one the person's files are on.
#[must_use]
pub fn primary_volume(volumes: &[Volume]) -> Option<&Volume> {
    // On a Mac "/" is the sealed system; what the person fills is the data volume.
    ["/System/Volumes/Data", "/"]
        .iter()
        .find_map(|mount| volumes.iter().find(|volume| volume.mount == *mount))
        .or_else(|| volumes.iter().max_by_key(|volume| volume.total))
}

/// The share of `part` in `whole`, 0–100.
#[must_use]
pub fn percent(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        #[allow(clippy::cast_precision_loss)]
        let share = part as f64 * 100.0 / whole as f64;
        share.clamp(0.0, 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sensor(name: &str, celsius: f64) -> Sensor {
        Sensor {
            name: name.to_owned(),
            celsius,
            count: 1,
        }
    }

    /// What an M1 Pro told on 2026-11-18, by sysinfo.
    const A_MAC: [(&str, f64); 28] = [
        ("NAND CH0 temp", 39.0),
        ("PMU TP0s", 49.4),
        ("PMU TP1g", 50.4),
        ("PMU TP1s", 49.7),
        ("PMU TP2g", 50.4),
        ("PMU TP2s", 49.4),
        ("PMU TP3g", 50.6),
        ("PMU tcal", 51.9),
        ("PMU tdev1", 44.7),
        ("PMU tdev2", 48.1),
        ("PMU tdev3", 49.6),
        ("PMU tdev4", 37.1),
        ("PMU tdev5", 35.8),
        ("PMU tdev6", 45.0),
        ("PMU tdev7", 49.5),
        ("PMU tdev8", 50.5),
        ("PMU tdie0", 50.1),
        ("PMU tdie1", 50.5),
        ("PMU tdie10", 50.2),
        ("PMU tdie2", 50.2),
        ("PMU tdie3", 50.4),
        ("PMU tdie4", 50.6),
        ("PMU tdie5", 49.7),
        ("PMU tdie6", 49.8),
        ("PMU tdie7", 50.3),
        ("PMU tdie8", 49.9),
        ("PMU tdie9", 49.8),
        ("gas gauge battery", 35.6),
    ];

    #[test]
    fn a_family_of_sensors_is_told_as_one_at_the_hottest_and_the_battery_is_not_cut_away() {
        let told = told_sensors(
            A_MAC
                .iter()
                .map(|(name, celsius)| sensor(name, *celsius))
                .collect(),
        );
        let seen: Vec<(&str, f64, usize)> = told
            .iter()
            .map(|sensor| (sensor.name.as_str(), sensor.celsius, sensor.count))
            .collect();
        assert_eq!(
            seen,
            [
                ("NAND CH0 temp", 39.0, 1),
                ("PMU TP", 50.6, 6),
                ("PMU tcal", 51.9, 1),
                ("PMU tdev", 50.5, 8),
                ("PMU tdie", 50.6, 11),
                ("gas gauge battery", 35.6, 1),
            ]
        );
    }

    #[test]
    fn a_sensor_with_no_family_keeps_its_name_and_the_most_told_is_the_cap() {
        let names = ["CPU performance cores", "GPU", "SSD", "Core 7", "123"];
        let told = told_sensors(names.iter().map(|name| sensor(name, 40.0)).collect());
        let mut kept: Vec<&str> = told.iter().map(|sensor| sensor.name.as_str()).collect();
        kept.sort_unstable();
        assert_eq!(
            kept,
            ["123", "CPU performance cores", "Core 7", "GPU", "SSD"]
        );
        assert!(told.iter().all(|sensor| sensor.count == 1));
        // Thirty sensors of thirty families: the first 24 by name.
        let many: Vec<Sensor> = (0..30)
            .map(|at| sensor(&format!("{}x", char::from(b'A' + at)), 40.0))
            .collect();
        assert_eq!(told_sensors(many).len(), MOST_SENSORS);
    }

    fn volume(mount: &str, total: u64) -> Volume {
        Volume {
            name: mount.to_owned(),
            mount: mount.to_owned(),
            fs: "apfs".to_owned(),
            total,
            used: total / 2,
        }
    }

    #[test]
    fn the_data_volume_is_the_disk_card_of_a_mac() {
        let volumes = [
            volume("/", 10),
            volume("/System/Volumes/Data", 20),
            volume("/mnt", 90),
        ];
        assert_eq!(
            primary_volume(&volumes).unwrap().mount,
            "/System/Volumes/Data"
        );
    }

    #[test]
    fn root_is_the_disk_card_where_there_is_no_data_volume() {
        let volumes = [volume("/mnt", 90), volume("/", 10)];
        assert_eq!(primary_volume(&volumes).unwrap().mount, "/");
    }

    #[test]
    fn the_largest_volume_stands_in_when_none_is_root() {
        let volumes = [volume("/a", 10), volume("/b", 30)];
        assert_eq!(primary_volume(&volumes).unwrap().mount, "/b");
        assert!(primary_volume(&[]).is_none());
    }

    #[test]
    fn a_share_is_kept_between_nothing_and_everything() {
        assert!((percent(1, 4) - 25.0).abs() < 1e-9);
        assert!(percent(5, 0).abs() < 1e-9);
        assert!((percent(9, 3) - 100.0).abs() < 1e-9);
    }
}
