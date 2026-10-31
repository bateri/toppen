//! A machine that changes with time and is the same every run: what the tests, the pictures of
//! `docs/` and `TOPPEN_FIXTURE=<name> make dev` look at instead of the machine the program runs on,
//! so that no test depends on what is running there and no picture shows a person's own processes,
//! host name or command lines.
//!
//! Debug builds only: a release has no fixture, and `TOPPEN_FIXTURE` means nothing to it.

use super::{
    Cpu, Device, Disk, Features, Interface, Machine, Memory, Network, Sample, Source, Volume,
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
};

pub struct Fixture {
    shape: Shape,
    tick: u64,
    total_rx: u64,
    total_tx: u64,
}

impl Fixture {
    /// The fixture called `name` ("small", "large"), if there is one.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        [SMALL, LARGE]
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

    fn of(shape: Shape) -> Self {
        Self {
            shape,
            tick: 0,
            total_rx: 40 * GIB,
            total_tx: 6 * GIB,
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
                    rx: rx * 0.97,
                    tx: tx * 0.9,
                },
                Interface {
                    name: "en5".to_owned(),
                    rx: rx * 0.03,
                    tx: tx * 0.1,
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
        }
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
    fn a_fixture_is_found_by_its_name() {
        assert!(Fixture::named("small").is_some());
        assert!(Fixture::named("large").is_some());
        assert!(Fixture::named("nothing").is_none());
    }
}
