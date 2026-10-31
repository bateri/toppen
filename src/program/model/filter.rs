//! The filter of the process table: what a person types after `/`.
//!
//! Terms are separated by spaces and all of them must hold:
//!
//! - `word` — in the name or the command, whatever the case; `"two words"` for a phrase;
//! - `/regex/` — a regular expression, whatever the case (`(?-i)` says otherwise);
//! - `!term` — what does not hold;
//! - `user:root,www`, `pid:1,42`, `port:8080`, `state:RD` — one of the values;
//! - `cpu>50`, `cpu<5` — percent of one processor; `mem>500M`, `mem<2G` — resident memory (a number
//!   with no letter is megabytes; `K`, `M`, `G`, `T` are 1024 to the step).
//!
//! What matched in the name and in the command is told as ranges of characters, for the page to
//! mark.

use regex::Regex;

use crate::sample::Process;

/// How many marks a field of a row carries.
const MARKS: usize = 16;

/// Ranges of characters `[from, to)` that a term matched, in the name and in the command.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Hits {
    pub name: Vec<(usize, usize)>,
    pub command: Vec<(usize, usize)>,
}

#[derive(Debug)]
enum Test {
    /// Lower-cased characters.
    Word(Vec<char>),
    Pattern(Regex),
    User(Vec<String>),
    Pid(Vec<u32>),
    Port(Vec<u16>),
    State(Vec<char>),
    CpuOver(f64),
    CpuUnder(f64),
    MemOver(u64),
    MemUnder(u64),
}

#[derive(Debug)]
struct Term {
    negated: bool,
    test: Test,
}

#[derive(Debug, Default)]
pub struct Filter {
    terms: Vec<Term>,
}

/// The characters of `text`, each as its lower case where that is one character (the ranges of a
/// match are counted in characters, so the lengths must stay).
fn lowered(text: &str) -> Vec<char> {
    text.chars()
        .map(|c| {
            let mut lower = c.to_lowercase();
            match (lower.next(), lower.next()) {
                (Some(one), None) => one,
                _ => c,
            }
        })
        .collect()
}

/// Where `needle` is in `haystack`, as ranges of characters, not overlapping.
fn find_all(haystack: &[char], needle: &[char]) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    if needle.is_empty() || needle.len() > haystack.len() {
        return found;
    }
    let mut at = 0;
    while at + needle.len() <= haystack.len() && found.len() < MARKS {
        if haystack[at..at + needle.len()] == *needle {
            found.push((at, at + needle.len()));
            at += needle.len();
        } else {
            at += 1;
        }
    }
    found
}

/// The ranges of characters of the matches of `pattern` in `text`.
fn pattern_hits(pattern: &Regex, text: &str) -> Vec<(usize, usize)> {
    let chars: Vec<usize> = text.char_indices().map(|(at, _)| at).collect();
    let character_of = |byte: usize| chars.partition_point(|&at| at < byte);
    pattern
        .find_iter(text)
        .filter(|found| !found.is_empty())
        .take(MARKS)
        .map(|found| (character_of(found.start()), character_of(found.end())))
        .collect()
}

fn number(words: &str) -> Option<f64> {
    words
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0)
}

/// A size as a number of bytes: `500M`, `2G`, `1.5g`; a bare number is megabytes.
fn size(words: &str) -> Option<u64> {
    let (digits, unit) = match words.chars().last()? {
        'k' | 'K' => (&words[..words.len() - 1], 1024.0),
        'm' | 'M' => (&words[..words.len() - 1], 1024.0 * 1024.0),
        'g' | 'G' => (&words[..words.len() - 1], 1024.0 * 1024.0 * 1024.0),
        't' | 'T' => (&words[..words.len() - 1], 1024.0 * 1024.0 * 1024.0 * 1024.0),
        _ => (words, 1024.0 * 1024.0),
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    number(digits).map(|value| (value * unit) as u64)
}

/// The first line of what the regex crate says is wrong, without its decoration.
fn short(error: &regex::Error) -> String {
    let text = error.to_string();
    let line = text
        .lines()
        .rev()
        .find(|line| line.trim_start().starts_with("error:"))
        .or_else(|| text.lines().next())
        .unwrap_or("");
    line.trim().trim_start_matches("error:").trim().to_owned()
}

/// A term other than a plain word: `key:values`, `cpu>5`; `None` where it is a word.
fn keyed(word: &str) -> Option<Result<Test, String>> {
    for (key, over) in [
        ("cpu>", true),
        ("cpu<", false),
        ("mem>", true),
        ("mem<", false),
    ] {
        if let Some(value) = word.to_lowercase().strip_prefix(key).map(str::to_owned) {
            let test = if key.starts_with("cpu") {
                number(&value).map(|percent| {
                    if over {
                        Test::CpuOver(percent)
                    } else {
                        Test::CpuUnder(percent)
                    }
                })
            } else {
                size(&value).map(|bytes| {
                    if over {
                        Test::MemOver(bytes)
                    } else {
                        Test::MemUnder(bytes)
                    }
                })
            };
            return Some(test.ok_or_else(|| {
                if key.starts_with("cpu") {
                    format!("{} takes a number: cpu>50", &key[..4])
                } else {
                    format!("{} takes a size: mem>500M, mem<2G", &key[..4])
                }
            }));
        }
    }
    let (key, values) = word.split_once(':')?;
    let list = || values.split(',').filter(|value| !value.is_empty());
    Some(match key.to_lowercase().as_str() {
        "user" => Ok(Test::User(list().map(str::to_lowercase).collect())),
        "pid" => list()
            .map(|value| {
                value
                    .parse::<u32>()
                    .map_err(|_| format!("pid: takes numbers: pid:1,42 (not {value:?})"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Test::Pid),
        "port" => list()
            .map(|value| {
                value
                    .parse::<u16>()
                    .map_err(|_| format!("port: takes port numbers: port:8080 (not {value:?})"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Test::Port),
        "state" => Ok(Test::State(
            values.chars().map(|c| c.to_ascii_uppercase()).collect(),
        )),
        _ => return None,
    })
}

/// The terms of `text`: each is a word, a phrase in quotes, a pattern between slashes, or a keyed
/// test, each with its `!`.
fn terms_of(text: &str) -> Result<Vec<Term>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut terms = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        if chars[at].is_whitespace() {
            at += 1;
            continue;
        }
        let negated = chars[at] == '!' && at + 1 < chars.len() && !chars[at + 1].is_whitespace();
        if negated {
            at += 1;
        }
        let test = match chars[at] {
            '/' => {
                // A pattern ends at the next slash that is not escaped.
                let mut end = at + 1;
                while end < chars.len() && chars[end] != '/' {
                    end += if chars[end] == '\\' { 2 } else { 1 };
                }
                if end >= chars.len() {
                    return Err("a regular expression ends with a slash: /name/".to_owned());
                }
                let body: String = chars[at + 1..end].iter().collect();
                at = end + 1;
                if body.is_empty() {
                    return Err("a regular expression has something in it: /name/".to_owned());
                }
                let pattern = Regex::new(&format!("(?i){body}"))
                    .map_err(|why| format!("not a regular expression: {}", short(&why)))?;
                Test::Pattern(pattern)
            }
            '"' => {
                let mut end = at + 1;
                while end < chars.len() && chars[end] != '"' {
                    end += 1;
                }
                let phrase: String = chars[at + 1..end.min(chars.len())].iter().collect();
                at = (end + 1).min(chars.len());
                if phrase.is_empty() {
                    continue;
                }
                Test::Word(lowered(&phrase))
            }
            _ => {
                let mut end = at;
                while end < chars.len() && !chars[end].is_whitespace() {
                    end += 1;
                }
                let word: String = chars[at..end].iter().collect();
                at = end;
                match keyed(&word) {
                    Some(test) => test?,
                    None => Test::Word(lowered(&word)),
                }
            }
        };
        terms.push(Term { negated, test });
    }
    Ok(terms)
}

impl Filter {
    /// Whether the filter asks about ports, which cost a scan to know.
    #[must_use]
    pub fn uses_ports(&self) -> bool {
        self.terms
            .iter()
            .any(|term| matches!(term.test, Test::Port(_)))
    }

    /// The filter `text` says, or why it is not one.
    pub fn parse(text: &str) -> Result<Self, String> {
        Ok(Self {
            terms: terms_of(text)?,
        })
    }

    /// Whether `process` is let through, and what matched in it. `ports` are the ports it listens on.
    #[must_use]
    pub fn matches(&self, process: &Process, ports: &[u16]) -> Option<Hits> {
        let mut hits = Hits::default();
        let mut text: Option<(Vec<char>, Vec<char>)> = None;
        for term in &self.terms {
            let found = match &term.test {
                Test::Word(word) => {
                    let (name, command) = text
                        .get_or_insert_with(|| (lowered(&process.name), lowered(&process.command)));
                    let (in_name, in_command) = (find_all(name, word), find_all(command, word));
                    let any = !in_name.is_empty() || !in_command.is_empty();
                    if any && !term.negated {
                        hits.name.extend(in_name);
                        hits.command.extend(in_command);
                    }
                    any
                }
                Test::Pattern(pattern) => {
                    let in_name = pattern_hits(pattern, &process.name);
                    let in_command = pattern_hits(pattern, &process.command);
                    let any = pattern.is_match(&process.name) || pattern.is_match(&process.command);
                    if any && !term.negated {
                        hits.name.extend(in_name);
                        hits.command.extend(in_command);
                    }
                    any
                }
                Test::User(users) => users
                    .iter()
                    .any(|user| *user == process.user.to_lowercase()),
                Test::Pid(pids) => pids.contains(&process.pid),
                Test::Port(wanted) => wanted.iter().any(|port| ports.contains(port)),
                Test::State(states) => states.contains(&process.state.to_ascii_uppercase()),
                Test::CpuOver(percent) => process.cpu > *percent,
                Test::CpuUnder(percent) => process.cpu < *percent,
                Test::MemOver(bytes) => process.memory > *bytes,
                Test::MemUnder(bytes) => process.memory < *bytes,
            };
            if found == term.negated {
                return None;
            }
        }
        hits.name.sort_unstable();
        hits.name.dedup();
        hits.command.sort_unstable();
        hits.command.dedup();
        hits.name.truncate(MARKS);
        hits.command.truncate(MARKS);
        Some(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, name: &str, command: &str, user: &str, cpu: f64, memory: u64) -> Process {
        Process {
            pid,
            ppid: 1,
            start: 1000,
            name: name.to_owned(),
            user: user.to_owned(),
            command: command.to_owned(),
            cpu,
            memory,
            virt: memory * 2,
            threads: 1,
            state: 'S',
            nice: 0,
            cpu_time: 0,
            read: 0.0,
            write: 0.0,
        }
    }

    fn chrome() -> Process {
        process(
            42,
            "Google Chrome",
            "/Applications/Google Chrome.app --type=renderer",
            "lantern",
            12.5,
            800 << 20,
        )
    }

    fn passes(filter: &str, process: &Process) -> bool {
        Filter::parse(filter)
            .unwrap()
            .matches(process, &[])
            .is_some()
    }

    #[test]
    fn words_are_found_in_the_name_and_the_command_whatever_the_case() {
        let process = chrome();
        assert!(passes("", &process));
        assert!(passes("chrome", &process));
        assert!(passes("CHROME renderer", &process));
        assert!(passes("--type", &process));
        assert!(!passes("firefox", &process));
        assert!(!passes("chrome firefox", &process));
        assert!(passes("\"google chrome\"", &process));
        assert!(!passes("\"chrome google\"", &process));
    }

    #[test]
    fn the_hits_are_ranges_of_characters_in_the_name_and_the_command() {
        let process = chrome();
        let hits = Filter::parse("chrome")
            .unwrap()
            .matches(&process, &[])
            .unwrap();
        assert_eq!(hits.name, [(7, 13)]);
        assert_eq!(hits.command, [(21, 27)]);
        let hits = Filter::parse("o").unwrap().matches(&process, &[]).unwrap();
        assert_eq!(hits.name, [(1, 2), (2, 3), (10, 11)]);
        // Marks are in characters, not bytes.
        let process = process_with_accents();
        let hits = Filter::parse("ş").unwrap().matches(&process, &[]).unwrap();
        assert_eq!(hits.name, [(1, 2)]);
    }

    fn process_with_accents() -> Process {
        process(7, "aşçı", "aşçı --x", "u", 0.0, 0)
    }

    #[test]
    fn a_term_with_a_bang_is_what_does_not_hold() {
        let process = chrome();
        assert!(!passes("!chrome", &process));
        assert!(passes("!firefox", &process));
        assert!(passes("google !firefox", &process));
        assert!(!passes("!user:lantern", &process));
        // A lone bang is a word.
        assert!(!passes("!", &process));
        // What does not hold marks nothing.
        let hits = Filter::parse("!firefox google")
            .unwrap()
            .matches(&process, &[])
            .unwrap();
        assert_eq!(hits.name, [(0, 6)]);
    }

    #[test]
    fn a_regular_expression_is_between_slashes_and_ignores_case() {
        let process = chrome();
        assert!(passes("/chr.me/", &process));
        assert!(passes("/^google/", &process));
        assert!(!passes("/^chrome/", &process));
        assert!(passes("/(renderer|gpu)$/", &process));
        assert!(!passes("!/renderer/", &process));
        assert!(!passes("/(?-i)CHROME/", &process));
        assert!(passes("/a b|google c/", &process));
        let hits = Filter::parse("/c.r/")
            .unwrap()
            .matches(&process, &[])
            .unwrap();
        assert_eq!(hits.name, [(7, 10)]);
    }

    #[test]
    fn a_regular_expression_that_is_wrong_says_so_in_one_line() {
        for text in ["/(unclosed/", "/[a-/", "/*x/"] {
            let why = Filter::parse(text).unwrap_err();
            assert!(
                why.starts_with("not a regular expression: "),
                "{text}: {why}"
            );
            assert!(!why.contains('\n'), "{why}");
        }
        assert!(
            Filter::parse("/name")
                .unwrap_err()
                .contains("ends with a slash")
        );
        assert!(Filter::parse("//").unwrap_err().contains("something in it"));
    }

    #[test]
    fn keyed_terms_test_one_thing_each() {
        let process = chrome();
        assert!(passes("user:lantern", &process));
        assert!(passes("user:Root,LANTERN", &process));
        assert!(!passes("user:root", &process));
        assert!(passes("pid:42", &process));
        assert!(passes("pid:1,42", &process));
        assert!(!passes("pid:43", &process));
        assert!(passes("state:S", &process));
        assert!(passes("state:rs", &process));
        assert!(!passes("state:Z", &process));
        assert!(passes("cpu>10", &process) && !passes("cpu>12.5", &process));
        assert!(passes("cpu<20", &process) && !passes("cpu<12", &process));
        assert!(passes("mem>500M", &process) && !passes("mem>1G", &process));
        assert!(passes("mem<1G", &process) && !passes("mem<700", &process));
        assert!(passes("mem>800000K", &process));
    }

    #[test]
    fn a_port_matches_nothing_until_the_ports_are_known() {
        let process = chrome();
        assert!(!passes("port:8080", &process));
        let filter = Filter::parse("port:8080").unwrap();
        assert!(filter.matches(&process, &[22, 8080]).is_some());
        assert!(filter.matches(&process, &[22]).is_none());
    }

    #[test]
    fn a_keyed_term_with_what_it_cannot_take_says_what_it_takes() {
        assert!(
            Filter::parse("cpu>fast")
                .unwrap_err()
                .contains("cpu> takes a number")
        );
        assert!(
            Filter::parse("mem>lots")
                .unwrap_err()
                .contains("mem> takes a size")
        );
        assert!(
            Filter::parse("pid:abc")
                .unwrap_err()
                .contains("pid: takes numbers")
        );
        assert!(
            Filter::parse("port:99999")
                .unwrap_err()
                .contains("port: takes port numbers")
        );
        // Something with a colon that is no key is a word.
        assert!(passes(
            "https://x",
            &process(1, "x", "curl https://x", "u", 0.0, 0)
        ));
    }

    #[test]
    fn terms_are_all_needed() {
        let process = chrome();
        assert!(passes("chrome user:lantern cpu>10", &process));
        assert!(!passes("chrome user:lantern cpu>50", &process));
        assert!(Filter::parse("   ").unwrap().terms.is_empty());
    }
}
