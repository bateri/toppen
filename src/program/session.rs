//! What a run of the program holds for the page: the sampler, and the process table the page has
//! set up the way it wants it; the settings come to it as the page asks for them.

use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::model::processes::Table;
use crate::sampler::Sampler;

pub struct Session {
    pub sampler: Sampler,
    table: Mutex<Table>,
}

impl Session {
    #[must_use]
    pub fn new(sampler: Sampler) -> Self {
        Self {
            sampler,
            table: Mutex::new(Table::new()),
        }
    }

    /// The process table. It is held for the work of one change or one snapshot, not longer.
    pub fn table(&self) -> MutexGuard<'_, Table> {
        self.table.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
