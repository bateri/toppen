//! `boot`: what the page needs to know before it draws, once; and `sampling`: how often the machine
//! is looked at, and whether it is.

use std::sync::Arc;
use std::time::Duration;

use pencere::Options;
use serde::{Deserialize, Serialize};

use super::{Failure, call};
use crate::command_line::{MAX_INTERVAL, MIN_INTERVAL, interval_of};
use crate::sample::Machine;
use crate::session::Session;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MachineInfo {
    host: String,
    os: String,
    kernel: String,
    arch: String,
    cpu: String,
    threads: usize,
    cores: usize,
    memory: u64,
    booted: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Features {
    pressure: bool,
    disk_io: bool,
}

#[derive(Serialize)]
struct Sampling {
    /// Seconds between two looks.
    interval: f64,
    paused: bool,
    min: f64,
    max: f64,
}

#[derive(Serialize)]
struct Person {
    name: String,
    root: bool,
}

#[derive(Serialize)]
struct Boot {
    version: &'static str,
    /// Who is looking: the processes this person may signal are theirs, or all as root.
    person: Person,
    machine: MachineInfo,
    features: Features,
    sampling: Sampling,
}

fn sampling_of(session: &Session) -> Sampling {
    Sampling {
        interval: session.sampler.interval().as_secs_f64(),
        paused: session.sampler.paused(),
        min: MIN_INTERVAL,
        max: MAX_INTERVAL,
    }
}

fn info(machine: &Machine) -> MachineInfo {
    MachineInfo {
        host: machine.host.clone(),
        os: machine.os.clone(),
        kernel: machine.kernel.clone(),
        arch: machine.arch.clone(),
        cpu: machine.cpu.clone(),
        threads: machine.threads,
        cores: machine.cores,
        memory: machine.memory,
        booted: machine.booted,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoParams {}

fn boot(session: &Session, _: NoParams) -> Result<Boot, Failure> {
    let machine = session.sampler.machine();
    Ok(Boot {
        version: env!("CARGO_PKG_VERSION"),
        person: {
            let person = session.control.person();
            Person {
                name: person.name,
                root: person.root,
            }
        },
        machine: info(machine),
        features: Features {
            pressure: machine.features.pressure,
            disk_io: machine.features.disk_io,
        },
        sampling: sampling_of(session),
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SamplingParams {
    interval: Option<f64>,
    paused: Option<bool>,
}

fn sampling(session: &Session, params: SamplingParams) -> Result<Sampling, Failure> {
    if let Some(seconds) = params.interval {
        let interval: Duration = interval_of(seconds).ok_or_else(|| {
            Failure::BadParams(format!(
                "the interval is {MIN_INTERVAL} to {MAX_INTERVAL} seconds"
            ))
        })?;
        session.sampler.set_interval(interval);
    }
    if let Some(paused) = params.paused {
        session.sampler.set_paused(paused);
    }
    Ok(sampling_of(session))
}

pub fn calls(options: Options, session: &Arc<Session>) -> Options {
    let options = call(options, session, "boot", boot);
    call(options, session, "sampling", sampling)
}
