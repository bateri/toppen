//! A machine that changes with time and is the same every run: what the tests, the pictures of
//! `docs/` and `TOPPEN_FIXTURE=<name> make dev` look at instead of the machine the program runs on,
//! so that no test depends on what is running there and no picture shows a person's own processes,
//! host name or command lines.
//!
//! Debug builds only: a release has no fixture, and `TOPPEN_FIXTURE` means nothing to it.

use super::control::{Control, Detail, Key, Person, Refused, Signal};
use super::gpu::{Engine, Gpu, celsius_of};
use super::ports::{Listener, PortSource, Ports, Proto};
use super::{
    Breakdown, Cpu, Device, Disk, Features, Interface, Machine, Memory, Network, Process, Sample,
    Sensor, Source, Volume,
};

const GIB: u64 = 1 << 30;

/// Seconds since the epoch the fixture's clock starts at (2025-10-31).
const START: f64 = 1_761_868_800.0;

/// What a fixture is made of.
#[derive(Debug, Clone, Copy)]
struct Shape {
    name: &'static str,
    host: &'static str,
    cpu: &'static str,
    threads: usize,
    memory: u64,
    swap: u64,
    /// The share of the processors the machine keeps busy, on average.
    busy: f64,
    pressure: bool,
    disk_io: bool,
    /// The graphics processors: none, one that shares the machine's memory (Apple Silicon), or
    /// cards with memory of their own.
    gpus: Gpus,
    processes: usize,
}

#[derive(Debug, Clone, Copy)]
enum Gpus {
    None,
    Unified(&'static str),
    Cards(&'static [&'static str]),
}

const SMALL: Shape = Shape {
    name: "small",
    host: "lantern",
    cpu: "Fixture M1 Pro",
    threads: 8,
    memory: 16 * GIB,
    swap: 4 * GIB,
    busy: 28.0,
    pressure: true,
    disk_io: true,
    gpus: Gpus::Unified("Fixture M1 Pro GPU"),
    processes: 60,
};

/// A machine with two cards of their own, for the pages that list more than one GPU.
const DUO: Shape = Shape {
    name: "duo",
    host: "anvil",
    cpu: "Fixture Ryzen 9",
    threads: 16,
    memory: 64 * GIB,
    swap: 8 * GIB,
    busy: 35.0,
    pressure: false,
    disk_io: true,
    gpus: Gpus::Cards(&["Fixture RTX 4090", "Fixture RTX 4070"]),
    processes: 120,
};

const LARGE: Shape = Shape {
    name: "large",
    host: "forge",
    cpu: "Fixture EPYC 7B13",
    threads: 64,
    memory: 256 * GIB,
    swap: 0,
    busy: 55.0,
    pressure: false,
    disk_io: true,
    gpus: Gpus::None,
    processes: 2000,
};

/// The names the fixture's processes have, and the users they run as: a few names many times over,
/// so that a group has more than one in it.
const NAMES: [&str; 16] = [
    "zsh",
    "node",
    "Code Helper",
    "chrome",
    "postgres",
    "redis-server",
    "python3",
    "cargo",
    "rustc",
    "ssh",
    "tmux",
    "nginx",
    "kworker",
    "java",
    "sleep",
    "bash",
];
const USERS: [&str; 4] = ["lantern", "root", "postgres", "www"];

/// A process of the fixture, as it is when it is not changing.
struct Spec {
    pid: u32,
    ppid: u32,
    start: u64,
    name: String,
    user: &'static str,
    command: String,
    cpu: f64,
    memory: u64,
    threads: u32,
    nice: i32,
    /// A process that comes and goes: it is there two ticks of three, a few ticks at a time.
    churn: bool,
}

/// The processes of a machine of `count`, the same every time: one init, then each with a parent
/// among the ones before it.
fn specs(count: usize) -> Vec<Spec> {
    let mut specs = Vec::with_capacity(count);
    for index in 0..count {
        let seed = index as u64;
        let name = if index == 0 {
            "launchd"
        } else {
            NAMES[(noise(seed * 31 + 7) * NAMES.len() as f64) as usize % NAMES.len()]
        };
        let ppid = if index == 0 {
            0
        } else if index < 6 {
            1
        } else {
            // Mostly the processes just before it: a tree that is deep in places and wide in others.
            let back = 1 + (noise(seed * 17 + 3) * noise(seed * 5 + 1) * 40.0) as usize;
            specs[index.saturating_sub(back).max(1)..index.max(2)]
                .first()
                .map_or(1, |parent: &Spec| parent.pid)
        };
        let user = if index == 0 {
            "root"
        } else {
            USERS[(noise(seed * 13 + 9) * 4.0) as usize % 4]
        };
        specs.push(Spec {
            pid: if index == 0 {
                1
            } else {
                100 + index as u32 * 7
            },
            ppid,
            start: START as u64 - 200_000 + seed * 90,
            command: if index == 0 {
                "/sbin/launchd".to_owned()
            } else {
                format!(
                    "/usr/local/bin/{name} --serve --port {} --config /etc/{name}/{index}.toml",
                    3000 + index
                )
            },
            name: name.to_owned(),
            user,
            cpu: (noise(seed * 7 + 2).powi(3) * 90.0).max(0.0),
            memory: ((4.0 + noise(seed * 11 + 5).powi(2) * 900.0) * 1_048_576.0) as u64,
            threads: 1 + (noise(seed * 3 + 8) * 24.0) as u32,
            nice: [0, 0, 0, 0, 5, 10, -5][(noise(seed * 19 + 4) * 7.0) as usize % 7],
            churn: index > 5 && index % 9 == 0,
        });
    }
    specs
}

pub struct Fixture {
    shape: Shape,
    specs: Vec<Spec>,
    tick: u64,
    total_rx: u64,
    total_tx: u64,
    /// A card's details are open: the sensors are told.
    detail: bool,
}

/// The listening sockets of the fixture, which do not change.
struct FixturePorts(Vec<Listener>);

impl PortSource for FixturePorts {
    fn scan(&self) -> Vec<Listener> {
        self.0.clone()
    }
}

/// What the fixture's control was asked to do, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Done {
    Signalled(Key, Signal),
    Reniced(Key, i32),
    /// A shell was asked for, in this directory.
    Shell(Option<String>),
}

/// The control of the fixture machine: it acts on nothing, keeps what it was asked, and refuses as
/// a machine would. The person is `lantern`; the processes of the other users are not theirs
/// (`Denied`), and a name that is none of the fixture's is `Gone`.
#[derive(Default)]
pub struct FixtureControl {
    known: std::collections::HashMap<Key, (String, String, String)>,
    done: std::sync::Mutex<Vec<Done>>,
}

impl FixtureControl {
    /// What it was asked to do so far.
    #[cfg(test)]
    #[must_use]
    pub fn done(&self) -> Vec<Done> {
        self.done
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn own(&self, key: Key) -> Result<(), Refused> {
        match self.known.get(&key) {
            None => Err(Refused::Gone),
            Some((_, user, _)) if user != "lantern" => Err(Refused::Denied),
            Some(_) => Ok(()),
        }
    }

    fn record(&self, done: Done) {
        self.done
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(done);
    }
}

impl Control for FixtureControl {
    fn person(&self) -> Person {
        Person {
            name: "lantern".to_owned(),
            root: false,
        }
    }

    fn detail(&self, key: Key) -> Result<Detail, Refused> {
        let (name, user, command) = self.known.get(&key).ok_or(Refused::Gone)?;
        Ok(Detail {
            argv: command.split_whitespace().map(str::to_owned).collect(),
            exe: command.split_whitespace().next().map(str::to_owned),
            cwd: Some(format!("/home/{user}/{name}")),
            env: if user == "lantern" {
                vec![
                    ("HOME".to_owned(), "/home/lantern".to_owned()),
                    ("LANG".to_owned(), "en_US.UTF-8".to_owned()),
                    ("API_TOKEN".to_owned(), "tok-8f2a91c4d7".to_owned()),
                    ("PATH".to_owned(), "/usr/local/bin:/usr/bin:/bin".to_owned()),
                ]
            } else {
                Vec::new()
            },
            fds: Some(8 + usize::try_from(key.0 % 23).unwrap_or(0)),
        })
    }

    fn signal(&self, key: Key, signal: Signal) -> Result<(), Refused> {
        self.own(key)?;
        self.record(Done::Signalled(key, signal));
        Ok(())
    }

    fn shell(&self, cwd: Option<&str>) -> Option<std::process::Command> {
        // No shell is started: the pane a test runs in is not lent to anything.
        self.record(Done::Shell(cwd.map(str::to_owned)));
        None
    }

    fn renice(&self, key: Key, nice: i32) -> Result<(), Refused> {
        self.own(key)?;
        // A person may only make a process of their own nicer.
        if nice < 0 {
            return Err(Refused::Denied);
        }
        self.record(Done::Reniced(key, nice));
        Ok(())
    }
}

impl Fixture {
    /// The listening sockets of the fixture machine: each server process on the port its command
    /// line says, a few over UDP, and one nobody this person can see owns.
    #[must_use]
    pub fn ports(&self) -> Ports {
        let mut listeners = Vec::new();
        for (index, spec) in self.specs.iter().enumerate().skip(1) {
            listeners.push(Listener {
                proto: Proto::Tcp,
                addr: if index % 3 == 0 { "*" } else { "127.0.0.1" }.to_owned(),
                port: 3000 + u16::try_from(index).unwrap_or(0),
                pid: Some(spec.pid),
            });
            if index % 7 == 0 {
                listeners.push(Listener {
                    proto: Proto::Udp,
                    addr: "*".to_owned(),
                    port: 5000 + u16::try_from(index).unwrap_or(0),
                    pid: Some(spec.pid),
                });
            }
        }
        listeners.push(Listener {
            proto: Proto::Tcp,
            addr: "127.0.0.1".to_owned(),
            port: 631,
            pid: None,
        });
        Ports::new(Box::new(FixturePorts(listeners)))
    }

    /// The control of this fixture's processes.
    #[must_use]
    pub fn control(&self) -> FixtureControl {
        FixtureControl {
            known: self
                .specs
                .iter()
                .map(|spec| {
                    (
                        (spec.pid, spec.start),
                        (
                            spec.name.clone(),
                            spec.user.to_owned(),
                            spec.command.clone(),
                        ),
                    )
                })
                .collect(),
            done: std::sync::Mutex::default(),
        }
    }

    /// The fixture called `name` ("small", "large", "duo"), if there is one.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        [SMALL, LARGE, DUO]
            .into_iter()
            .find(|shape| shape.name == name)
            .map(Self::of)
    }

    #[cfg(test)]
    pub fn small() -> Self {
        Self::of(SMALL)
    }

    #[cfg(test)]
    pub fn large() -> Self {
        Self::of(LARGE)
    }

    #[cfg(test)]
    pub fn duo() -> Self {
        Self::of(DUO)
    }

    fn of(shape: Shape) -> Self {
        Self {
            shape,
            specs: specs(shape.processes),
            tick: 0,
            total_rx: 40 * GIB,
            total_tx: 6 * GIB,
            detail: false,
        }
    }
}

/// A number in 0–1 that is the same for the same `seed`, and unlike the one beside it.
fn noise(seed: u64) -> f64 {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 32;
    #[allow(clippy::cast_precision_loss)]
    let unit = (x >> 11) as f64 / (1_u64 << 53) as f64;
    unit
}

/// A triangle wave in 0–1 with the period `period` ticks.
fn wave(tick: u64, period: u64, phase: u64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let at = ((tick + phase) % period) as f64 / period as f64;
    1.0 - (2.0 * at - 1.0).abs()
}

/// How many samples a fixture has seen before the program asks for one.
const BACKFILL: u64 = 120;

impl Source for Fixture {
    fn detail(&mut self, open: bool) {
        self.detail = open;
    }

    fn backfill(&mut self) -> Vec<Sample> {
        (self.tick..BACKFILL).map(|_| self.sample()).collect()
    }

    fn machine(&self) -> Machine {
        Machine {
            host: self.shape.host.to_owned(),
            os: "Fixture OS 1.0".to_owned(),
            kernel: "1.0.0-fixture".to_owned(),
            arch: "arm64".to_owned(),
            cpu: self.shape.cpu.to_owned(),
            threads: self.shape.threads,
            cores: self.shape.threads / 2,
            memory: self.shape.memory,
            booted: 1_761_600_000,
            features: Features {
                pressure: self.shape.pressure,
                disk_io: self.shape.disk_io,
                gpu: !matches!(self.shape.gpus, Gpus::None),
            },
        }
    }

    #[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
    fn sample(&mut self) -> Sample {
        let tick = self.tick;
        self.tick += 1;
        let shape = self.shape;

        let swell = wave(tick, 90, 0);
        let cores: Vec<f64> = (0..shape.threads)
            .map(|core| {
                let seed = tick * 1009 + core as u64;
                let share = (core as f64 + 1.0) / shape.threads as f64;
                (shape.busy * (0.4 + 1.2 * swell) * (1.3 - share) + 40.0 * noise(seed) - 12.0)
                    .clamp(1.0, 100.0)
            })
            .collect();
        let total = cores.iter().sum::<f64>() / shape.threads as f64;
        let freq = (0..shape.threads)
            .map(|core| 1200 + (core as u64 % 4) * 600 + (total as u64) * 10)
            .collect();

        let used_share = 0.52 + 0.14 * wave(tick, 600, 100) + 0.02 * noise(tick);
        let used = (shape.memory as f64 * used_share) as u64;
        let swap_used = if shape.swap == 0 {
            0
        } else {
            (shape.swap as f64 * (0.2 + 0.1 * wave(tick, 1200, 0))) as u64
        };
        let memory = Memory {
            total: shape.memory,
            used,
            available: shape.memory - used,
            swap_total: shape.swap,
            swap_used,
            pressure: shape
                .pressure
                .then(|| (used_share * 100.0 - 30.0).clamp(0.0, 100.0)),
            breakdown: Some({
                let wired = shape.memory / 12;
                let compressed = shape.memory / 25 + (tick % 7) * 1_048_576;
                let cached = shape.memory / 6;
                let app = used.saturating_sub(wired + compressed);
                Breakdown {
                    app,
                    wired,
                    compressed,
                    cached,
                    free: shape
                        .memory
                        .saturating_sub(app + wired + compressed + cached),
                }
            }),
        };

        let burst = if noise(tick / 7) > 0.8 { 6.0 } else { 1.0 };
        let rx = (180_000.0 + 140_000.0 * wave(tick, 40, 5)) * burst;
        let tx = 30_000.0 + 25_000.0 * wave(tick, 55, 11) + 40_000.0 * noise(tick + 77);
        self.total_rx += rx as u64;
        self.total_tx += tx as u64;
        let network = Network {
            rx,
            tx,
            total_rx: self.total_rx,
            total_tx: self.total_tx,
            interfaces: vec![
                Interface {
                    name: "en0".to_owned(),
                    rx: rx * 0.9,
                    tx: tx * 0.8,
                },
                Interface {
                    name: "en5".to_owned(),
                    rx: rx * 0.07,
                    tx: tx * 0.15,
                },
                Interface {
                    name: "bridge100".to_owned(),
                    rx: rx * 0.03,
                    tx: tx * 0.05,
                },
            ],
        };

        let writing = if noise(tick / 11 + 3) > 0.7 { 9.0 } else { 1.0 };
        let read = 2_000_000.0 * wave(tick, 70, 30) * noise(tick + 5) * 3.0;
        let write = 600_000.0 * writing * (0.5 + noise(tick + 9));
        let volumes = vec![
            Volume {
                name: "/dev/disk3s5".to_owned(),
                mount: "/System/Volumes/Data".to_owned(),
                fs: "apfs".to_owned(),
                total: 494 * GIB,
                used: 301 * GIB + tick * 40_000,
            },
            Volume {
                name: "/dev/disk4s1".to_owned(),
                mount: "/Volumes/Archive".to_owned(),
                fs: "apfs".to_owned(),
                total: 1862 * GIB,
                used: 1204 * GIB,
            },
        ];
        let disk = Disk {
            read,
            write,
            devices: vec![Device {
                name: "nvme0n1".to_owned(),
                read,
                write,
            }],
            volumes,
        };

        let sensors = if self.detail {
            let heat = 54.0 + 14.0 * swell + 2.0 * noise(tick + 3);
            vec![
                Sensor {
                    name: "CPU performance cores".to_owned(),
                    celsius: heat,
                },
                Sensor {
                    name: "CPU efficiency cores".to_owned(),
                    celsius: heat - 9.0,
                },
                Sensor {
                    name: "GPU".to_owned(),
                    celsius: 41.0 + 3.0 * noise(tick + 8),
                },
                Sensor {
                    name: "SSD".to_owned(),
                    celsius: 36.0 + 2.0 * wave(tick, 300, 4),
                },
            ]
        } else {
            Vec::new()
        };
        let mut gpus = Self::gpus(shape.gpus, tick);
        // As the real source does: one GPU is the machine's, and the sensors may name it.
        if let [only] = gpus.as_mut_slice() {
            only.celsius = only.celsius.or_else(|| celsius_of(&sensors));
        }
        let processes = self.processes(tick);
        Sample {
            at: START + tick as f64,
            cpu: Cpu { total, cores, freq },
            memory,
            network,
            disk,
            load: [
                shape.threads as f64 * (0.08 + 0.1 * swell),
                shape.threads as f64 * 0.12,
                shape.threads as f64 * 0.1,
            ],
            uptime: 3 * 86_400 + tick,
            processes,
            sensors,
            gpus,
        }
    }
}

impl Fixture {
    /// The graphics processors at `tick`.
    #[allow(clippy::cast_precision_loss)]
    fn gpus(kind: Gpus, tick: u64) -> Vec<Gpu> {
        match kind {
            Gpus::None => Vec::new(),
            Gpus::Unified(name) => {
                let load = 8.0 + 70.0 * wave(tick, 75, 10) + 6.0 * noise(tick + 11);
                vec![Gpu {
                    name: name.to_owned(),
                    load,
                    memory_used: Some(380_000_000 + (1.5 * GIB as f64 * wave(tick, 110, 3)) as u64),
                    memory_total: None,
                    celsius: None,
                    engines: vec![
                        Engine {
                            name: "Renderer".to_owned(),
                            load: (load - 1.0).max(0.0),
                        },
                        Engine {
                            name: "Tiler".to_owned(),
                            load: (load * 0.8 - 2.0).max(0.0),
                        },
                    ],
                }]
            }
            Gpus::Cards(names) => names
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    let phase = 17 * index as u64;
                    let load = (12.0
                        + 80.0 * wave(tick, 60 + 20 * index as u64, phase)
                        + 8.0 * noise(tick + phase))
                    .clamp(0.0, 100.0);
                    let total = if index == 0 { 24 } else { 12 } * GIB;
                    Gpu {
                        name: (*name).to_owned(),
                        load,
                        memory_used: Some(
                            (total as f64 * (0.2 + 0.5 * wave(tick, 130, phase))) as u64,
                        ),
                        memory_total: Some(total),
                        celsius: Some(45.0 + 0.3 * load + 2.0 * noise(tick + 5 + phase)),
                        engines: vec![Engine {
                            name: "Memory bus".to_owned(),
                            load: (load * 0.6).clamp(0.0, 100.0),
                        }],
                    }
                })
                .collect(),
        }
    }

    /// The processes at `tick`: what each does changes with time, and the churning ones come and go.
    #[allow(clippy::cast_precision_loss)]
    fn processes(&self, tick: u64) -> Vec<Process> {
        self.specs
            .iter()
            .enumerate()
            .filter(|(index, spec)| !spec.churn || !(tick / 4 + *index as u64).is_multiple_of(3))
            .map(|(index, spec)| {
                let seed = tick * 7919 + index as u64;
                let busy = spec.cpu * (0.5 + wave(tick, 30 + index as u64 % 40, index as u64))
                    + 3.0 * noise(seed);
                Process {
                    pid: spec.pid,
                    ppid: spec.ppid,
                    start: spec.start,
                    name: spec.name.clone(),
                    user: spec.user.to_owned(),
                    command: spec.command.clone(),
                    cpu: busy.clamp(0.0, 800.0),
                    memory: spec.memory + (noise(seed + 1) * 4_000_000.0) as u64,
                    virt: spec.memory * 6 + 400_000_000,
                    threads: spec.threads,
                    state: if busy > 40.0 {
                        'R'
                    } else if index % 11 == 0 {
                        'D'
                    } else {
                        'S'
                    },
                    nice: spec.nice,
                    cpu_time: spec.start % 1000 * 1000 + tick * (busy as u64 * 10),
                    read: if index % 5 == 0 {
                        40_000.0 * noise(seed + 2)
                    } else {
                        0.0
                    },
                    write: if index % 7 == 0 {
                        90_000.0 * noise(seed + 3)
                    } else {
                        0.0
                    },
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixture_is_the_same_every_time() {
        let (mut one, mut other) = (Fixture::small(), Fixture::small());
        for _ in 0..20 {
            assert_eq!(one.sample(), other.sample());
        }
    }

    #[test]
    fn it_changes_from_one_sample_to_the_next() {
        let mut fixture = Fixture::small();
        let (first, second) = (fixture.sample(), fixture.sample());
        assert_ne!(first.cpu.cores, second.cpu.cores);
        assert!(second.network.total_rx > first.network.total_rx);
        assert!((second.at - first.at - 1.0).abs() < 1e-9);
    }

    #[test]
    fn the_numbers_are_in_the_ranges_the_page_is_told() {
        for mut fixture in [Fixture::small(), Fixture::large()] {
            let machine = fixture.machine();
            for _ in 0..400 {
                let sample = fixture.sample();
                assert_eq!(sample.cpu.cores.len(), machine.threads);
                assert!(
                    sample
                        .cpu
                        .cores
                        .iter()
                        .all(|core| (0.0..=100.0).contains(core))
                );
                assert!((0.0..=100.0).contains(&sample.cpu.total));
                assert!(sample.memory.used <= sample.memory.total);
                assert!(sample.memory.swap_used <= sample.memory.swap_total);
                assert!(
                    sample
                        .disk
                        .volumes
                        .iter()
                        .all(|volume| volume.used <= volume.total)
                );
            }
        }
    }

    #[test]
    fn a_fixture_has_seen_two_minutes_before_it_is_asked() {
        let mut fixture = Fixture::small();
        let seen = fixture.backfill();
        assert_eq!(seen.len(), 120);
        assert!(seen.windows(2).all(|pair| pair[0].at < pair[1].at));
        // What it is asked next goes on from there, and it has nothing more to say of the past.
        assert!(fixture.sample().at > seen[119].at);
        assert!(fixture.backfill().is_empty());
    }

    #[test]
    fn the_fixtures_have_one_gpu_two_or_none_and_say_so_in_their_features() {
        for (mut fixture, count) in [
            (Fixture::small(), 1),
            (Fixture::duo(), 2),
            (Fixture::large(), 0),
        ] {
            let sample = fixture.sample();
            assert_eq!(sample.gpus.len(), count);
            assert_eq!(fixture.machine().features.gpu, count > 0);
            for gpu in &sample.gpus {
                assert!((0.0..=100.0).contains(&gpu.load), "{gpu:?}");
                assert!(gpu.memory_used.is_some());
            }
        }
        // The Apple-style one shares the machine's memory; the cards have their own.
        assert_eq!(Fixture::small().sample().gpus[0].memory_total, None);
        assert!(Fixture::duo().sample().gpus[0].memory_total.is_some());
    }

    #[test]
    fn a_fixture_is_found_by_its_name() {
        assert!(Fixture::named("small").is_some());
        assert!(Fixture::named("large").is_some());
        assert!(Fixture::named("duo").is_some());
        assert!(Fixture::named("nothing").is_none());
    }
}
