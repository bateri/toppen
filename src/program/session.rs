//! What a run of the program holds for the page: the sampler, the process table the page has set up
//! the way it wants it, what can be done to a process, and the hand on the view (to lend the pane to
//! a shell); the settings come to it as the page asks for them.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use pencere::ViewControl;

use crate::model::processes::Table;
use crate::sample::control::Control;
use crate::sampler::Sampler;

pub struct Session {
    pub sampler: Sampler,
    /// What can be done to a process: signals, a nice value, a closer look.
    pub control: Arc<dyn Control>,
    /// The program's hand on the view, to close it for a shell and show it again after.
    pub view: ViewControl,
    table: Mutex<Table>,
}

impl Session {
    #[must_use]
    pub fn new(sampler: Sampler, control: Arc<dyn Control>, view: ViewControl) -> Self {
        Self {
            sampler,
            control,
            view,
            table: Mutex::new(Table::new()),
        }
    }

    /// The process table. It is held for the work of one change or one snapshot, not longer.
    pub fn table(&self) -> MutexGuard<'_, Table> {
        self.table.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
