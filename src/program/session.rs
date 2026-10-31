//! What a run of the program holds for the page: for now the sampler; the process model and the
//! settings come to it as the page asks for them.

use crate::sampler::Sampler;

pub struct Session {
    pub sampler: Sampler,
}

impl Session {
    #[must_use]
    pub fn new(sampler: Sampler) -> Self {
        Self { sampler }
    }
}
