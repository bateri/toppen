//! toppen in a terminal pane, with pencere's stand-in terminal (`pencere-fake-host`) playing the
//! terminal: the page's calls travel to the program through the tty, as they do in bateri, and the
//! stand-in writes what the page heard to its record (stderr), which the tests read.
//!
//! The program is the one Cargo built for the tests, looking at the fixture machine
//! (`TOPPEN_FIXTURE=small`), never at the one the tests run on. The stand-in is the one `FAKE_HOST`
//! names, or the one `make fake-host` builds in `.cache/pencere`.
//! Nothing is skipped: a stand-in that is not there, or one too old to show a whole result, fails
//! the run and says which, since a gate that passes because it did not look proves nothing.

// Each file of tests uses a part of what is here.
#![allow(dead_code)]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use tempfile::TempDir;

/// The stand-in terminal, checked once.
pub fn host() -> &'static Path {
    static HOST: OnceLock<PathBuf> = OnceLock::new();
    HOST.get_or_init(|| {
        // A path from here (the package, where Cargo runs the tests), since the stand-in is started
        // in the directory of each test.
        let host = match std::env::var_os("FAKE_HOST") {
            Some(path) if !path.is_empty() => {
                std::path::absolute(&path).unwrap_or_else(|_| PathBuf::from(path))
            }
            _ => Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(".cache/pencere/target/debug/pencere-fake-host"),
        };
        assert!(
            host.is_file(),
            "the tests cannot run: pencere-fake-host is not built ({}): make fake-host, or FAKE_HOST=<a stand-in built in a pencere checkout>",
            host.display()
        );
        let help = Command::new(&host)
            .arg("--help")
            .output()
            .unwrap_or_else(|why| panic!("{} cannot be run: {why}", host.display()));
        let said = format!(
            "{}{}",
            String::from_utf8_lossy(&help.stdout),
            String::from_utf8_lossy(&help.stderr)
        );
        assert!(
            said.contains("--subscribe"),
            "the tests cannot run: pencere-fake-host speaks an older protocol ({})",
            host.display()
        );
        assert!(
            said.contains("PENCERE_FAKE_GIST"),
            "the tests cannot run: pencere-fake-host cuts a result at 100 characters ({}): it needs PENCERE_FAKE_GIST, which the host of a recent pencere checkout has",
            host.display()
        );
        host
    })
}

/// The program the pane runs.
pub fn program() -> &'static str {
    env!("CARGO_BIN_EXE_toppen")
}

/// What a process said, and how it ended.
pub struct Ran {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Runs `command` to its end, or for `limit` and then kills it.
pub fn run(mut command: Command, input: Option<&str>, limit: Duration) -> Ran {
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("the process starts");
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        use std::io::Write;
        let _ = stdin.write_all(input.as_bytes());
    }
    let read = |pipe: Option<Box<dyn Read + Send>>| {
        thread::spawn(move || {
            let mut out = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut out);
            }
            String::from_utf8_lossy(&out).into_owned()
        })
    };
    let stdout = read(
        child
            .stdout
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    );
    let stderr = read(
        child
            .stderr
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    );
    let deadline = Instant::now() + limit;
    let status = loop {
        if let Some(status) = child.try_wait().expect("the process can be waited for") {
            break status.code();
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        thread::sleep(Duration::from_millis(20));
    };
    Ran {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    }
}

/// What the page asks, and how the pane ends.
pub struct Script {
    calls: Vec<(String, Value)>,
    then: Vec<String>,
    limit: u64,
    env: Vec<(String, String)>,
    args: Vec<String>,
    host_args: Vec<String>,
}

impl Script {
    /// The page makes no calls; then ⌃C is typed; toppen is run with no arguments.
    pub fn new() -> Self {
        Self {
            calls: Vec::new(),
            then: vec!["--then".to_owned(), "interrupt".to_owned()],
            limit: 25,
            env: Vec::new(),
            args: Vec::new(),
            host_args: Vec::new(),
        }
    }

    /// The page calls `method` with `params`, after the answer to the call before.
    pub fn call(mut self, method: &str, params: Value) -> Self {
        self.calls.push((method.to_owned(), params));
        self
    }

    /// What happens when the page has its answers (the stand-in's `--then` and the like).
    pub fn then(mut self, then: &[&str]) -> Self {
        self.then = then.iter().map(|word| (*word).to_owned()).collect();
        self
    }

    /// How long the stand-in lets the program run, in seconds.
    pub fn limit(mut self, seconds: u64) -> Self {
        self.limit = seconds;
        self
    }

    pub fn env(mut self, name: &str, value: &str) -> Self {
        self.env.push((name.to_owned(), value.to_owned()));
        self
    }

    /// toppen's command line.
    pub fn args(mut self, args: &[&str]) -> Self {
        self.args = args.iter().map(|arg| (*arg).to_owned()).collect();
        self
    }

    /// More of the stand-in's options (`--subscribe overview`).
    pub fn host_args(mut self, args: &[&str]) -> Self {
        self.host_args = args.iter().map(|arg| (*arg).to_owned()).collect();
        self
    }
}

/// What the page heard in answer to a call.
#[derive(Debug, Clone)]
pub enum Answer {
    Result(Value),
    Error {
        code: String,
        message: Option<String>,
    },
}

impl Answer {
    /// The result of a call that was answered with one.
    pub fn result(&self) -> &Value {
        match self {
            Self::Result(value) => value,
            Self::Error { code, message } => panic!("an error, {code}: {message:?}"),
        }
    }

    /// Whether the call failed with `code`.
    pub fn failed(&self, code: &str) -> bool {
        matches!(self, Self::Error { code: said, .. } if said == code)
    }

    /// The words of an error.
    pub fn message(&self) -> &str {
        match self {
            Self::Error {
                message: Some(message),
                ..
            } => message,
            other => panic!("no words: {other:?}"),
        }
    }
}

/// The answers the page heard, in order, in the stand-in's record.
pub fn answers(log: &str) -> Vec<Answer> {
    let mut found = Vec::new();
    for line in log.lines() {
        let Some(rest) = line.strip_prefix("view: <- response id=") else {
            continue;
        };
        let Some((_, said)) = rest.split_once(' ') else {
            continue;
        };
        if let Some(result) = said.strip_prefix("result=") {
            found.push(Answer::Result(
                serde_json::from_str(result).expect("a whole result"),
            ));
        } else if let Some(error) = said.strip_prefix("error code=") {
            let (code, message) = match error.split_once(" message=") {
                Some((code, message)) => (
                    code.to_owned(),
                    Some(serde_json::from_str(message).expect("words that are a JSON string")),
                ),
                None => (error.to_owned(), None),
            };
            found.push(Answer::Error { code, message });
        }
    }
    found
}

/// The values the page heard pushed, in order.
pub fn pushes(log: &str) -> Vec<Value> {
    log.lines()
        .filter_map(|line| line.strip_prefix("view: <- push id="))
        .filter_map(|rest| rest.split_once(" value="))
        .map(|(_, value)| serde_json::from_str(value).expect("a whole value"))
        .collect()
}

/// How many times `needle` is in `log`.
pub fn count(log: &str, needle: &str) -> usize {
    log.matches(needle).count()
}

/// A home of its own: nothing the program keeps is written under the real one.
pub struct Pane {
    pub dir: TempDir,
}

impl Pane {
    pub fn new() -> Self {
        Self {
            dir: tempfile::Builder::new()
                .prefix("toppen-test-")
                .tempdir()
                .expect("a directory for the test"),
        }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// A command run with its home here, over the fixture machine.
    pub fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut command = Command::new(program);
        command
            .current_dir(self.dir.path())
            .env("HOME", self.dir.path())
            .env("XDG_STATE_HOME", self.path("state"))
            .env("XDG_CONFIG_HOME", self.path("config"))
            .env("TOPPEN_FIXTURE", "small")
            .env("PENCERE_FAKE_GIST", "100000000")
            .env_remove("SSH_CONNECTION")
            .env_remove("SSH_TTY");
        command
    }

    /// The stand-in's record of a pane in which the page makes the calls of `script`.
    pub fn record(&self, script: &Script) -> String {
        self.run(script).stderr
    }

    /// The stand-in, ready to run the pane of `script`.
    pub fn host_command(&self, script: &Script) -> Command {
        let mut command = self.command(host());
        command.args(["--size", "100x30", "--limit", &script.limit.to_string()]);
        command.args(&script.host_args);
        for (method, params) in &script.calls {
            command.args(["--call", &format!("{method}={params}")]);
        }
        command
            .args(&script.then)
            .arg("--")
            .arg(program())
            .args(&script.args);
        for (name, value) in &script.env {
            command.env(name, value);
        }
        command
    }

    /// The pane of `script` run to its end: the record, and what the program wrote on the terminal
    /// when the stand-in passes it through (`--passthrough`).
    pub fn run(&self, script: &Script) -> Ran {
        run(
            self.host_command(script),
            None,
            Duration::from_secs(script.limit + 15),
        )
    }

    /// The answers to the calls of `script`, one for each: the record's tail says why when not.
    pub fn ask(&self, script: &Script) -> Vec<Answer> {
        let log = self.record(script);
        let heard = answers(&log);
        let tail: String = {
            let chars: Vec<char> = log.chars().collect();
            chars[chars.len().saturating_sub(1500)..].iter().collect()
        };
        assert_eq!(
            heard.len(),
            script.calls.len(),
            "every call was answered:\n{tail}"
        );
        heard
    }
}
