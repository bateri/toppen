//! What a run of the program holds for the page: the sampler, the process table the page has set up
//! the way it wants it, what can be done to a process, and the hand on the view (to lend the pane to
//! a shell); the settings come to it as the page asks for them.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use pencere::ViewControl;

use crate::model::processes::Table;
use crate::sample::control::Control;
use crate::sample::ports::Ports;
use crate::sampler::Sampler;
use crate::state::settings::{Settings, Store};

pub struct Session {
    pub sampler: Sampler,
    /// What can be done to a process: signals, a nice value, a closer look.
    pub control: Arc<dyn Control>,
    /// The sockets that listen, scanned when someone asks.
    pub ports: Arc<Ports>,
    /// The program's hand on the view, to close it for a shell and show it again after.
    pub view: ViewControl,
    /// The person's settings file.
    pub settings: Store,
    table: Mutex<Table>,
    /// How many times the page asked to be told the rows whole again: a watcher that sees it change
    /// forgets what it told.
    retold: AtomicU64,
}

impl Session {
    #[must_use]
    pub fn new(
        sampler: Sampler,
        control: Arc<dyn Control>,
        ports: Arc<Ports>,
        view: ViewControl,
    ) -> Self {
        Self {
            sampler,
            control,
            ports,
            view,
            settings: Store::memory(Settings::default()),
            table: Mutex::new(Table::new()),
            retold: AtomicU64::new(0),
        }
    }

    /// The session with the settings of a file in place of those that are kept in memory.
    #[must_use]
    pub fn with_settings(mut self, settings: Store) -> Self {
        self.settings = settings;
        self
    }

    /// The page asks for the rows whole: what it was told of them is not what it holds.
    pub fn retell(&self) {
        self.retold.fetch_add(1, Ordering::Relaxed);
    }

    /// How many times the page asked for the rows whole.
    pub fn retold(&self) -> u64 {
        self.retold.load(Ordering::Relaxed)
    }

    /// The process table. It is held for the work of one change or one snapshot, not longer.
    pub fn table(&self) -> MutexGuard<'_, Table> {
        self.table.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
