//! What the machine measures: a [`Source`] answers with the machine's facts once and with a
//! [`Sample`] of its state each time it is asked. The real source ([`system::System`]) reads the
//! operating system; the fixture ([`fixture::Fixture`], debug builds) is a machine that changes with
//! time, the same way every run, which is what the tests and the pictures of `docs/` run on.
//!
//! This is `sample/`: it knows nothing of the page. It takes and gives plain Rust values.

pub mod control;
pub mod history;
pub mod linux;
pub mod macos;
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
