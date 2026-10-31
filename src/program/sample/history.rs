//! What the machine did lately: the series of the cards, 900 samples of them (a quarter of an hour
//! at the default interval), the oldest forgotten as new ones come.

use std::collections::VecDeque;

use super::Sample;

/// How many samples are kept.
pub const CAPACITY: usize = 900;

/// The numbers of one sample that a graph draws.
#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub at: f64,
    pub cpu: f64,
    /// Each logical processor, whole percent.
    pub cores: Vec<u8>,
    pub memory: f64,
    pub swap: f64,
    pub rx: f64,
    pub tx: f64,
    pub read: f64,
    pub write: f64,
    /// The busiest GPU's load, 0–100 (0 on a machine with none).
    pub gpu: f64,
    /// Each GPU, whole percent.
    pub gpus: Vec<u8>,
}

/// A load as a whole percent.
fn whole(load: f64) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let whole = load.round().clamp(0.0, 100.0) as u8;
    whole
}

impl Point {
    #[must_use]
    pub fn of(sample: &Sample) -> Self {
        Self {
            at: sample.at,
            cpu: sample.cpu.total,
            cores: sample.cpu.cores.iter().map(|core| whole(*core)).collect(),
            memory: super::percent(sample.memory.used, sample.memory.total),
            swap: super::percent(sample.memory.swap_used, sample.memory.swap_total),
            rx: sample.network.rx,
            tx: sample.network.tx,
            read: sample.disk.read,
            write: sample.disk.write,
            gpu: sample.gpus.iter().map(|gpu| gpu.load).fold(0.0, f64::max),
            gpus: sample.gpus.iter().map(|gpu| whole(gpu.load)).collect(),
        }
    }
}

#[derive(Debug, Default)]
pub struct History {
    points: VecDeque<Point>,
}

impl History {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, point: Point) {
        if self.points.len() == CAPACITY {
            self.points.pop_front();
        }
        self.points.push_back(point);
    }

    /// The points, oldest first.
    pub fn points(&self) -> impl ExactSizeIterator<Item = &Point> {
        self.points.iter()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.points.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(at: f64) -> Point {
        Point {
            at,
            cpu: 0.0,
            cores: Vec::new(),
            memory: 0.0,
            swap: 0.0,
            rx: 0.0,
            tx: 0.0,
            read: 0.0,
            write: 0.0,
            gpu: 0.0,
            gpus: Vec::new(),
        }
    }

    #[test]
    fn the_oldest_is_forgotten_past_the_capacity() {
        let mut history = History::new();
        for n in 0..CAPACITY + 5 {
            history.push(point(f64::from(u32::try_from(n).unwrap())));
        }
        assert_eq!(history.len(), CAPACITY);
        assert!((history.points().next().unwrap().at - 5.0).abs() < 1e-9);
    }
}
