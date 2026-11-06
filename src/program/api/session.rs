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
use crate::state::settings::Settings;

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
    gpu: bool,
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
    settings: SettingsMessage,
}

/// The settings as the page is told them: their values, where the file is, and why it is not used.
#[derive(Serialize)]
struct SettingsMessage {
    values: Settings,
    path: Option<String>,
    error: Option<String>,
}

fn settings_of(session: &Session) -> SettingsMessage {
    SettingsMessage {
        values: session.settings.get(),
        path: session.settings.path(),
        error: session.settings.broken(),
    }
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
            gpu: machine.features.gpu,
        },
        sampling: sampling_of(session),
        settings: settings_of(session),
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

/// The cards of the overview, which have details.
const CARDS: [&str; 5] = ["cpu", "memory", "network", "disk", "gpu"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CardParams {
    card: Option<String>,
}

#[derive(Serialize)]
struct Card {
    card: Option<String>,
}

/// Which card's details the person has open. The program reads what costs more (the sensors) only
/// while one is.
fn card(session: &Session, params: CardParams) -> Result<Card, Failure> {
    if let Some(name) = &params.card
        && !CARDS.contains(&name.as_str())
    {
        return Err(Failure::BadParams(format!(
            "card is one of {}, not {name:?}",
            CARDS.join(", ")
        )));
    }
    session.sampler.set_detail(params.card.is_some());
    Ok(Card { card: params.card })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsParams {
    /// The settings to change: a member replaces that setting, `null` takes it back to its default.
    set: serde_json::Map<String, serde_json::Value>,
}

/// Changes the settings and writes them. The answer is what they are now.
fn settings(session: &Session, params: SettingsParams) -> Result<SettingsMessage, Failure> {
    session
        .settings
        .update(&params.set)
        .map_err(Failure::BadParams)?;
    Ok(settings_of(session))
}

pub fn calls(options: Options, session: &Arc<Session>) -> Options {
    let options = call(options, session, "boot", boot);
    let options = call(options, session, "sampling", sampling);
    let options = call(options, session, "settings", settings);
    call(options, session, "card", card)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page_tests::session_over;
    use crate::sample::fixture::Fixture;
    use crate::state::settings::Store;
    use std::time::Duration;

    fn set(value: &serde_json::Value) -> SettingsParams {
        SettingsParams {
            set: value.as_object().cloned().unwrap_or_default(),
        }
    }

    #[test]
    fn the_boot_tells_the_settings_and_a_change_is_told_back_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let session = Arc::new(
            Arc::into_inner(session_over(Fixture::small(), Duration::from_millis(300)))
                .unwrap()
                .with_settings(Store::open(dir.path())),
        );
        let booted = serde_json::to_value(boot(&session, NoParams {}).unwrap()).unwrap();
        assert_eq!(booted["settings"]["values"]["privacy"], false);
        assert_eq!(booted["features"]["gpu"], true);
        let told = settings(
            &session,
            set(&serde_json::json!({ "privacy": true, "keys": { "pause": ["p"] } })),
        )
        .unwrap();
        assert!(told.values.privacy);
        assert!(told.error.is_none());
        assert!(Store::open(dir.path()).get().privacy);
        // A value the program cannot use is `bad-params`, and changes nothing.
        let refused = settings(&session, set(&serde_json::json!({ "interval": 99 })));
        assert!(matches!(refused, Err(Failure::BadParams(_))));
        assert!(session.settings.get().privacy);
    }
}
