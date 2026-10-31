//! The command line: `toppen`, `toppen --once`, `toppen --json`, `--interval <seconds>`.

use std::time::Duration;

/// The seconds between two looks at the machine: the least, the most, and the one a run starts with.
pub const MIN_INTERVAL: f64 = 0.5;
pub const MAX_INTERVAL: f64 = 10.0;
pub const DEFAULT_INTERVAL: f64 = 1.0;

const NEEDS_SECONDS: &str = "toppen: --interval takes a number of seconds";

const USAGE: &str = "\
toppen: a system monitor for the terminal pane

usage: toppen [--once | --json] [--interval <seconds>]

  --once               print a summary of the machine once and exit
  --json               print the same summary as JSON and exit
  --interval <s>       seconds between two looks at the machine, 0.5 to 10 (default 1)
  -V, --version        print the version
  -h, --help           print this

In a terminal that shows pencere views (bateri) toppen opens in the pane. In any other it prints
the summary and exits.
";

/// What a run is asked to do.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// Print the summary and end, as text.
    pub once: bool,
    /// Print the summary and end, as JSON.
    pub json: bool,
    pub interval: Duration,
    /// The interval was asked for here, which is stronger than the settings.
    pub interval_asked: bool,
}

/// What the command line comes to.
#[derive(Debug, PartialEq)]
pub enum Asked {
    Run(Line),
    /// Words for the person, on stdout; the program ends with 0.
    Say(String),
    /// Why the command line is not understood, on stderr; the program ends with 2.
    Refuse(String),
}

/// The interval of `seconds`, if it is one a run may have.
#[must_use]
pub fn interval_of(seconds: f64) -> Option<Duration> {
    (seconds.is_finite() && (MIN_INTERVAL..=MAX_INTERVAL).contains(&seconds))
        .then(|| Duration::from_secs_f64(seconds))
}

fn interval_words(word: &str) -> Result<Duration, String> {
    word.parse::<f64>()
        .ok()
        .and_then(interval_of)
        .ok_or_else(|| {
            format!("toppen: --interval takes seconds, {MIN_INTERVAL} to {MAX_INTERVAL}: {word:?}")
        })
}

#[must_use]
pub fn read(args: impl IntoIterator<Item = String>) -> Asked {
    let mut line = Line {
        once: false,
        json: false,
        interval: Duration::from_secs_f64(DEFAULT_INTERVAL),
        interval_asked: false,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Asked::Say(USAGE.to_owned()),
            "-V" | "--version" => {
                return Asked::Say(format!("toppen {}\n", env!("CARGO_PKG_VERSION")));
            }
            "--once" => line.once = true,
            "--json" => line.json = true,
            "--interval" => {
                let Some(word) = args.next() else {
                    return Asked::Refuse(NEEDS_SECONDS.to_owned());
                };
                match interval_words(&word) {
                    Ok(interval) => {
                        line.interval = interval;
                        line.interval_asked = true;
                    }
                    Err(why) => return Asked::Refuse(why),
                }
            }
            other => match other.strip_prefix("--interval=") {
                Some(word) => match interval_words(word) {
                    Ok(interval) => {
                        line.interval = interval;
                        line.interval_asked = true;
                    }
                    Err(why) => return Asked::Refuse(why),
                },
                None => {
                    return Asked::Refuse(format!(
                        "toppen: {other:?} is not understood (toppen --help says what is)"
                    ));
                }
            },
        }
    }
    Asked::Run(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_words(words: &[&str]) -> Asked {
        read(words.iter().map(|word| (*word).to_owned()))
    }

    #[test]
    fn no_arguments_run_the_view_every_second() {
        assert_eq!(
            read_words(&[]),
            Asked::Run(Line {
                once: false,
                json: false,
                interval: Duration::from_secs(1),
                interval_asked: false,
            })
        );
    }

    #[test]
    fn once_and_json_and_the_interval_are_read() {
        let Asked::Run(line) = read_words(&["--once", "--json", "--interval", "2.5"]) else {
            panic!("a run");
        };
        assert!(line.once && line.json);
        assert_eq!(line.interval, Duration::from_millis(2500));
        assert!(line.interval_asked);
        let Asked::Run(line) = read_words(&["--interval=0.5"]) else {
            panic!("a run");
        };
        assert_eq!(line.interval, Duration::from_millis(500));
    }

    #[test]
    fn an_interval_out_of_range_is_refused_with_the_range() {
        for words in [
            &["--interval", "0.1"][..],
            &["--interval", "11"],
            &["--interval", "soon"],
            &["--interval=nan"],
        ] {
            let Asked::Refuse(why) = read_words(words) else {
                panic!("a refusal: {words:?}");
            };
            assert!(why.contains("0.5 to 10"), "{why}");
        }
        assert!(matches!(read_words(&["--interval"]), Asked::Refuse(_)));
    }

    #[test]
    fn what_is_not_understood_is_refused_and_help_and_version_are_said() {
        assert!(matches!(read_words(&["--top"]), Asked::Refuse(why) if why.contains("--top")));
        assert!(matches!(read_words(&["--help"]), Asked::Say(text) if text.contains("--once")));
        assert!(matches!(read_words(&["-V"]), Asked::Say(text) if text.starts_with("toppen 0.")));
    }
}
