//! toppen: a system monitor in the terminal pane, what `top` and `btop` do. In a terminal that shows
//! pencere views the monitor opens in the pane; in one that does not, the program prints a summary
//! of the machine and ends, as `top -l 1` does.
//!
//! The program looks at the machine and keeps what it saw; the page (`src/view/`, in pencere's kit)
//! is the monitor, drawn by the terminal, and asks the program for what it needs with the calls and
//! the topics of `docs/calls.md`.
//!
//! This file is the entry and nothing more: it reads the command line, makes the session, gives the
//! calls and the topics of `api/` to `serve`, and prints the summary where there is no view.
//!
//!   api/       everything the page talks to the program about; it knows the page
//!   sample/    what the machine measures: the real source and the fixture; knows nothing of the page
//!   model/     the process table: sorted, filtered, folded and held; knows nothing of the page
//!   text/      the summary for a person at a terminal that shows no views, and as JSON; same
//!   sampler    the thread that looks at the machine while someone watches

mod api;
mod command_line;
mod icons;
mod model;
#[cfg(test)]
mod page_shots;
#[cfg(test)]
mod page_tests;
mod sample;
mod sampler;
mod session;
mod text;

use std::process::ExitCode;
use std::sync::{Arc, Mutex, PoisonError};

use pencere::{EndedBy, NoViewReason, Options, Outcome, ViewControl};

use command_line::Asked;
use sample::Source;
use sample::control::{Control, SystemControl};
use sample::ports::{Ports, SystemPorts};
use sample::system::System;
use sampler::Sampler;
use session::Session;
use text::summary::Summary;

/// The words under the summary of a terminal that shows no views.
const NO_VIEWS: &str = "This terminal shows no views. toppen opens as a monitor in a terminal that hosts\npencere (bateri); run it there, or over ssh from one.";

/// What looks at the machine, what acts on its processes, and what lists its listening sockets.
type Parts = (Box<dyn Source>, Arc<dyn Control>, Arc<Ports>);

/// Where the measurements come from: the machine the program runs on, or, in a debug build, the
/// fixture machine that `TOPPEN_FIXTURE=<name>` names (`small`, `large`, `duo`), which is what the tests and
/// the pictures of `docs/` look at.
fn source() -> Result<Parts, String> {
    #[cfg(debug_assertions)]
    if let Some(name) = std::env::var_os("TOPPEN_FIXTURE") {
        let name = name.to_string_lossy();
        return sample::fixture::Fixture::named(&name)
            .map(|fixture| {
                let control = Arc::new(fixture.control()) as Arc<dyn Control>;
                let ports = Arc::new(fixture.ports());
                (Box::new(fixture) as Box<dyn Source>, control, ports)
            })
            .ok_or_else(|| format!("toppen: there is no fixture called {name:?} (small, large)"));
    }
    Ok((
        Box::new(System::new()),
        Arc::new(SystemControl),
        Arc::new(Ports::new(Box::new(SystemPorts))),
    ))
}

/// The summary of the machine now: text, or JSON, with `note` under the text.
fn summary(json: bool, note: Option<&str>) -> Result<String, String> {
    let (mut source, _, _) = source()?;
    // The first look primes the rates; the second is the one that says something.
    let machine = source.machine();
    let _ = source.sample();
    std::thread::sleep(source.warmup());
    let summary = Summary::of(&machine, &source.sample());
    Ok(if json {
        summary.json() + "\n"
    } else {
        summary.text(note)
    })
}

/// What `serve` is given: the app, its name and icons, the calls and the topics of `api/`, and the
/// summary for a terminal that shows no views, whose status goes to `fallen_back`.
fn options(session: &Arc<Session>, fallen_back: &Arc<Mutex<Option<u8>>>) -> Options {
    let mut options = Options::new()
        .app(pencere::app!())
        .program("toppen")
        // What a terminal writes above the view.
        .title("Monitor");
    for icon in icons::all() {
        options = options.icon(icon);
    }
    let options = api::serve(options, session);
    let status = Arc::clone(fallen_back);
    options.fallback(move |why| {
        // A pipe is a script's: it is given the summary alone.
        let note = (why.reason != NoViewReason::NoTty).then_some(NO_VIEWS);
        let ended = match summary(false, note) {
            Ok(text) => {
                print!("{text}");
                0
            }
            Err(words) => {
                eprintln!("{words}");
                2
            }
        };
        *status.lock().unwrap_or_else(PoisonError::into_inner) = Some(ended);
    })
}

fn main() -> ExitCode {
    let line = match command_line::read(std::env::args().skip(1)) {
        Asked::Run(line) => line,
        Asked::Say(words) => {
            print!("{words}");
            return ExitCode::SUCCESS;
        }
        Asked::Refuse(words) => {
            eprintln!("{words}");
            return ExitCode::from(2);
        }
    };
    if line.once || line.json {
        return match summary(line.json, None) {
            Ok(text) => {
                print!("{text}");
                ExitCode::SUCCESS
            }
            Err(words) => {
                eprintln!("{words}");
                ExitCode::from(2)
            }
        };
    }
    let (source, control, ports) = match source() {
        Ok(found) => found,
        Err(words) => {
            eprintln!("{words}");
            return ExitCode::from(2);
        }
    };
    let view = ViewControl::new();
    let session = Arc::new(Session::new(
        Sampler::start(source, line.interval),
        control,
        ports,
        view.clone(),
    ));
    let fallen_back = Arc::new(Mutex::new(None));
    let outcome = pencere::serve(options(&session, &fallen_back).control(view));
    let status = match outcome {
        Ok(Outcome::Closed(closed)) if closed.by == EndedBy::Error => 1,
        Ok(outcome) => fallen_back
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .unwrap_or_else(|| outcome.shell_status()),
        Err(why) => {
            eprintln!("toppen: {why}");
            1
        }
    };
    ExitCode::from(status)
}
