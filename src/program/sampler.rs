//! The one thread that looks at the machine. It looks while someone is watching and the updates are
//! not paused, every `interval`, keeps what it saw as the history, and wakes the watchers (one for
//! each subscription of the page) with each new sample. With no one watching it does nothing: a
//! monitor that sits in a pane all day costs nothing while its pane is not on a screen.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use crate::sample::history::{History, Point};
use crate::sample::{Machine, Sample, Source};

/// How many of the latest samples a late watcher can still be given one by one.
const RECENT: usize = 16;

struct State {
    history: History,
    recent: VecDeque<(u64, Arc<Sample>)>,
    /// How many samples have been taken: the number of the latest.
    seq: u64,
    /// How many times a watcher asked to be told again (a view of the table changed).
    touches: u64,
    watchers: usize,
    interval: Duration,
    paused: bool,
    closed: bool,
}

struct Shared {
    state: Mutex<State>,
    changed: Condvar,
    machine: Machine,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn wait<'a>(&self, state: MutexGuard<'a, State>) -> MutexGuard<'a, State> {
        self.changed
            .wait(state)
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn wait_for<'a>(&self, state: MutexGuard<'a, State>, time: Duration) -> MutexGuard<'a, State> {
        self.changed
            .wait_timeout(state, time)
            .map_or_else(|failed| failed.into_inner().0, |waited| waited.0)
    }

    /// Wakes whoever waits on the sampler. The lock is taken first so that a waiter that has just
    /// looked at what changed is already waiting.
    fn notify(&self) {
        drop(self.lock());
        self.changed.notify_all();
    }
}

/// Ends the thread when the last handle of the sampler is gone.
struct Closer(Arc<Shared>);

impl Drop for Closer {
    fn drop(&mut self) {
        self.0.lock().closed = true;
        self.0.changed.notify_all();
    }
}

#[derive(Clone)]
pub struct Sampler {
    shared: Arc<Shared>,
    _closer: Arc<Closer>,
}

impl Sampler {
    /// Starts the thread that looks at `source` every `interval`.
    #[must_use]
    pub fn start(mut source: Box<dyn Source>, interval: Duration) -> Self {
        let mut history = History::new();
        for sample in source.backfill() {
            history.push(Point::of(&sample));
        }
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                history,
                recent: VecDeque::new(),
                seq: 0,
                touches: 0,
                watchers: 0,
                interval,
                paused: false,
                closed: false,
            }),
            changed: Condvar::new(),
            machine: source.machine(),
        });
        let thread_shared = Arc::clone(&shared);
        // A thread that cannot be made leaves a sampler that never samples: the page shows its
        // cards empty, which says so.
        let _ = thread::Builder::new()
            .name("toppen-sampler".to_owned())
            .spawn(move || run(&thread_shared, source));
        Self {
            _closer: Arc::new(Closer(Arc::clone(&shared))),
            shared,
        }
    }

    #[must_use]
    pub fn machine(&self) -> &Machine {
        &self.shared.machine
    }

    #[must_use]
    pub fn interval(&self) -> Duration {
        self.shared.lock().interval
    }

    #[must_use]
    pub fn paused(&self) -> bool {
        self.shared.lock().paused
    }

    pub fn set_interval(&self, interval: Duration) {
        self.shared.lock().interval = interval;
        self.shared.notify();
    }

    pub fn set_paused(&self, paused: bool) {
        self.shared.lock().paused = paused;
        self.shared.notify();
    }

    /// The sample taken last, if one was.
    #[must_use]
    pub fn latest(&self) -> Option<Arc<Sample>> {
        self.shared
            .lock()
            .recent
            .back()
            .map(|(_, sample)| Arc::clone(sample))
    }

    /// Wakes the watchers so that they look whether theirs ended.
    pub fn wake(&self) {
        self.shared.notify();
    }

    /// Asks the watchers of the latest sample to be told it again, as something other than the
    /// machine changed what they say about it.
    pub fn touch(&self) {
        self.shared.lock().touches += 1;
        self.shared.notify();
    }

    /// Starts to watch: the thread looks at the machine while a watch is held.
    #[must_use]
    pub fn watch(&self) -> Watch {
        self.shared.lock().watchers += 1;
        self.shared.notify();
        Watch {
            shared: Arc::clone(&self.shared),
            seen: 0,
            touched: 0,
        }
    }
}

/// Someone who watches: the history so far, then the samples as they come.
pub struct Watch {
    shared: Arc<Shared>,
    seen: u64,
    touched: u64,
}

impl Watch {
    /// What was seen so far, oldest first; the samples that come after are the next ones.
    pub fn history(&mut self) -> (Vec<Point>, Duration) {
        let state = self.shared.lock();
        self.seen = state.seq;
        (state.history.points().cloned().collect(), state.interval)
    }

    /// Waits for samples newer than the ones seen, and gives them oldest first; `None` once `ended`
    /// says so (the caller wakes the sampler with [`Sampler::wake`] when it does).
    pub fn next(&mut self, ended: &dyn Fn() -> bool) -> Option<Vec<Arc<Sample>>> {
        let mut state = self.shared.lock();
        loop {
            if ended() || state.closed {
                return None;
            }
            if state.seq > self.seen {
                let news: Vec<Arc<Sample>> = state
                    .recent
                    .iter()
                    .filter(|(number, _)| *number > self.seen)
                    .map(|(_, sample)| Arc::clone(sample))
                    .collect();
                self.seen = state.seq;
                return Some(news);
            }
            state = self.shared.wait(state);
        }
    }

    /// Waits for a sample newer than the one seen, or for a touch, and gives the latest sample; the
    /// ones between are skipped. `None` once `ended` says so. Where nothing was sampled yet it waits
    /// on, touched or not.
    pub fn latest(&mut self, ended: &dyn Fn() -> bool) -> Option<Arc<Sample>> {
        let mut state = self.shared.lock();
        loop {
            if ended() || state.closed {
                return None;
            }
            if state.seq > self.seen || state.touches > self.touched {
                if let Some((number, sample)) = state.recent.back() {
                    let found = Arc::clone(sample);
                    self.seen = *number;
                    self.touched = state.touches;
                    return Some(found);
                }
            }
            state = self.shared.wait(state);
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        let mut state = self.shared.lock();
        state.watchers = state.watchers.saturating_sub(1);
        drop(state);
        self.shared.changed.notify_all();
    }
}

/// The thread: waits until someone watches and updates are not paused, warms the source up when it
/// has been idle, then looks once an interval.
fn run(shared: &Shared, mut source: Box<dyn Source>) {
    let mut next_at = Instant::now();
    let mut warm = false;
    loop {
        {
            let mut state = shared.lock();
            loop {
                if state.closed {
                    return;
                }
                if state.watchers == 0 || state.paused {
                    warm = false;
                    state = shared.wait(state);
                    continue;
                }
                let now = Instant::now();
                if warm && now < next_at {
                    state = shared.wait_for(state, next_at - now);
                    continue;
                }
                break;
            }
        }
        if !warm {
            // Idle for a while: the rates of the first look would be of the whole idle time.
            let _ = source.sample();
            thread::sleep(source.warmup());
            warm = true;
        }
        let sample = Arc::new(source.sample());
        let mut state = shared.lock();
        state.seq += 1;
        let number = state.seq;
        state.history.push(Point::of(&sample));
        if state.recent.len() == RECENT {
            state.recent.pop_front();
        }
        state.recent.push_back((number, sample));
        next_at = Instant::now() + state.interval;
        drop(state);
        shared.changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::fixture::Fixture;

    const TICK: Duration = Duration::from_millis(15);

    fn started() -> Sampler {
        Sampler::start(Box::new(Fixture::small()), TICK)
    }

    fn never() -> bool {
        false
    }

    #[test]
    fn nothing_is_looked_at_while_no_one_watches() {
        let sampler = started();
        thread::sleep(TICK * 6);
        let mut watch = sampler.watch();
        let (history, _) = watch.history();
        // Only what the fixture had seen before the program asked.
        assert_eq!(history.len(), 120);
    }

    #[test]
    fn a_watcher_is_given_the_samples_as_they_come_in_order() {
        let sampler = started();
        let mut watch = sampler.watch();
        let mut times = Vec::new();
        while times.len() < 4 {
            for sample in watch.next(&never).unwrap() {
                times.push(sample.at);
            }
        }
        assert!(times.windows(2).all(|pair| pair[0] < pair[1]));
        let (history, interval) = watch.history();
        assert!(history.len() >= 4);
        assert_eq!(interval, TICK);
    }

    #[test]
    fn a_pause_stops_the_looking_and_a_resume_starts_it() {
        let sampler = started();
        let mut watch = sampler.watch();
        watch.next(&never).unwrap();
        sampler.set_paused(true);
        thread::sleep(TICK * 4);
        let (before, _) = watch.history();
        thread::sleep(TICK * 8);
        let (after, _) = watch.history();
        assert_eq!(before.len(), after.len());
        assert!(sampler.paused());
        sampler.set_paused(false);
        assert!(watch.next(&never).is_some());
    }

    #[test]
    fn the_watcher_that_is_ended_stops_waiting() {
        let sampler = started();
        sampler.set_paused(true);
        let mut watch = sampler.watch();
        let ended = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = Arc::clone(&ended);
        let waiting =
            thread::spawn(move || watch.next(&|| flag.load(std::sync::atomic::Ordering::SeqCst)));
        thread::sleep(TICK * 3);
        ended.store(true, std::sync::atomic::Ordering::SeqCst);
        sampler.wake();
        assert!(waiting.join().unwrap().is_none());
    }

    #[test]
    fn the_latest_sample_is_given_again_when_touched_even_while_paused() {
        let sampler = started();
        let mut watch = sampler.watch();
        let first = watch.latest(&never).unwrap();
        sampler.set_paused(true);
        thread::sleep(TICK * 4);
        sampler.touch();
        let again = watch.latest(&never).unwrap();
        assert!(again.at >= first.at);
        assert!(!again.processes.is_empty());
        // Paused, nothing new comes: a touch gives the same sample.
        sampler.touch();
        assert_eq!(watch.latest(&never).unwrap().at, again.at);
    }

    #[test]
    fn the_interval_can_be_changed() {
        let sampler = started();
        sampler.set_interval(Duration::from_secs(2));
        assert_eq!(sampler.interval(), Duration::from_secs(2));
    }
}
