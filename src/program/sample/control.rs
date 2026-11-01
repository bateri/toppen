//! What the program does to a process, and what it reads of one that a measurement does not say:
//! the command line, the working directory, the environment, the open files; signals and a new
//! nice value.
//!
//! A process is named by its pid and the second it started in ([`Key`]), and the name is checked
//! again right before anything is sent: a pid is used again after its process ends, and a signal
//! that went to the process which has the number now would be sent to a stranger. A name that does
//! not hold is [`Refused::Gone`], and nothing is sent.
//!
//! This is `sample/`: it knows nothing of the page. [`SystemControl`] acts on the machine the program
//! runs on; the fixture's control (debug builds) only keeps a record of what it was asked.

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// A process by its pid and the second it started in.
pub type Key = (u32, u64);

/// The signals the page offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Term,
    Int,
    Hup,
    Kill,
    Stop,
    Cont,
    Usr1,
    Usr2,
}

impl Signal {
    pub const ALL: [Signal; 8] = [
        Signal::Term,
        Signal::Int,
        Signal::Hup,
        Signal::Kill,
        Signal::Stop,
        Signal::Cont,
        Signal::Usr1,
        Signal::Usr2,
    ];

    /// The name a person says it by, without `SIG`.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Signal::Term => "TERM",
            Signal::Int => "INT",
            Signal::Hup => "HUP",
            Signal::Kill => "KILL",
            Signal::Stop => "STOP",
            Signal::Cont => "CONT",
            Signal::Usr1 => "USR1",
            Signal::Usr2 => "USR2",
        }
    }

    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|signal| signal.word() == word)
    }

    fn number(self) -> i32 {
        match self {
            Signal::Term => libc::SIGTERM,
            Signal::Int => libc::SIGINT,
            Signal::Hup => libc::SIGHUP,
            Signal::Kill => libc::SIGKILL,
            Signal::Stop => libc::SIGSTOP,
            Signal::Cont => libc::SIGCONT,
            Signal::Usr1 => libc::SIGUSR1,
            Signal::Usr2 => libc::SIGUSR2,
        }
    }
}

/// Who is looking: the name the samples give their processes, and whether they are root.
#[derive(Debug, Clone)]
pub struct Person {
    pub name: String,
    pub root: bool,
}

/// Why nothing was done to a process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// There is no such process: it ended, or its pid belongs to another now.
    Gone,
    /// The system does not let this person do it (a process of another user, a lower nice value).
    Denied,
    /// Another reason, in the system's own words.
    Failed(String),
}

/// What a measurement does not say of a process.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Detail {
    /// The command line, a word each.
    pub argv: Vec<String>,
    pub exe: Option<String>,
    pub cwd: Option<String>,
    /// The environment, in the order the system gives it; empty where it is not this person's to read.
    pub env: Vec<(String, String)>,
    /// How many files, sockets and pipes are open.
    pub fds: Option<usize>,
}

/// What can be done to a process, by the name that was checked.
pub trait Control: Send + Sync {
    /// # Errors
    ///
    /// [`Refused::Gone`] where the process is not the one `key` names.
    fn detail(&self, key: Key) -> Result<Detail, Refused>;

    /// # Errors
    ///
    /// [`Refused`], and nothing was sent.
    fn signal(&self, key: Key, signal: Signal) -> Result<(), Refused>;

    /// # Errors
    ///
    /// [`Refused`], and nothing was changed.
    fn renice(&self, key: Key, nice: i32) -> Result<(), Refused>;

    /// Who is looking: the name the samples give their processes, and whether they are root.
    fn person(&self) -> Person;

    /// Whether this person may signal and renice a process of `user`: their own, or any as root.
    fn may(&self, user: &str) -> bool {
        let person = self.person();
        person.root || person.name == user
    }

    /// The shell to lend the pane to, started in `cwd` where that is a directory, else in the
    /// person's home; `None` where this control starts none (the fixture's).
    fn shell(&self, cwd: Option<&str>) -> Option<std::process::Command>;
}

/// The control of the machine the program runs on.
pub struct SystemControl;

/// A look at one process: what is read of it and how.
fn look(pid: u32, kind: ProcessRefreshKind) -> System {
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true, kind);
    system
}

/// The second the process `pid` started in, if there is one. The same reading as a sample's.
#[must_use]
pub fn start_of(pid: u32) -> Option<u64> {
    let system = look(pid, ProcessRefreshKind::nothing());
    system
        .process(Pid::from_u32(pid))
        .map(sysinfo::Process::start_time)
}

/// The pid as the system's calls take it, for a process that may be signalled: not 0 (the caller's
/// own group), and one that is a pid.
fn target(pid: u32) -> Result<libc::pid_t, Refused> {
    match libc::pid_t::try_from(pid) {
        Ok(number) if number > 0 => Ok(number),
        _ => Err(Refused::Failed(format!("{pid} is not a process"))),
    }
}

/// Whether `key` still names the process it did, read right now.
fn verify(key: Key) -> Result<libc::pid_t, Refused> {
    let number = target(key.0)?;
    match start_of(key.0) {
        Some(start) if start == key.1 => Ok(number),
        _ => Err(Refused::Gone),
    }
}

/// The thread ids in `dir` (Linux's `/proc/<pid>/task`), the process's own `pid` first; `pid` alone
/// where the directory cannot be read.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn threads_in(dir: &std::path::Path, pid: libc::id_t) -> Vec<libc::id_t> {
    let mut ids = vec![pid];
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut others: Vec<libc::id_t> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
            .filter(|id| *id != pid)
            .collect();
        others.sort_unstable();
        ids.extend(others);
    }
    ids
}

/// What the system said last, as a refusal.
fn refusal() -> Refused {
    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(code) if code == libc::ESRCH => Refused::Gone,
        Some(code) if code == libc::EPERM || code == libc::EACCES => Refused::Denied,
        _ => Refused::Failed(error.to_string()),
    }
}

#[cfg(target_os = "linux")]
fn open_files(pid: u32) -> Option<usize> {
    std::fs::read_dir(format!("/proc/{pid}/fd"))
        .ok()
        .map(Iterator::count)
}

#[cfg(not(target_os = "linux"))]
fn open_files(pid: u32) -> Option<usize> {
    super::macos::open_files(pid)
}

/// Secrets are not told apart here; the page hides what looks like one.
fn pair(word: &std::ffi::OsStr) -> Option<(String, String)> {
    let word = word.to_string_lossy();
    word.split_once('=')
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
}

/// The shell the person uses, as the environment says, else the system's.
fn shell_path() -> String {
    std::env::var("SHELL")
        .ok()
        .filter(|shell| !shell.is_empty())
        .unwrap_or_else(|| "/bin/sh".to_owned())
}

/// The name of the user the program runs as, as a sample names the owner of a process.
fn own_name() -> Option<String> {
    // SAFETY: `geteuid` takes nothing and cannot fail.
    let id = unsafe { libc::geteuid() };
    let users = sysinfo::Users::new_with_refreshed_list();
    users
        .iter()
        .find(|user| **user.id() == id)
        .map(|user| user.name().to_owned())
}

impl Control for SystemControl {
    fn person(&self) -> Person {
        static PERSON: std::sync::OnceLock<Person> = std::sync::OnceLock::new();
        PERSON
            .get_or_init(|| Person {
                // SAFETY: `geteuid` takes nothing and cannot fail.
                root: unsafe { libc::geteuid() } == 0,
                name: own_name().unwrap_or_default(),
            })
            .clone()
    }

    fn detail(&self, key: Key) -> Result<Detail, Refused> {
        let kind = ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_exe(UpdateKind::Always)
            .with_cwd(UpdateKind::Always)
            .with_environ(UpdateKind::Always);
        let system = look(key.0, kind);
        let process = system
            .process(Pid::from_u32(key.0))
            .filter(|process| process.start_time() == key.1)
            .ok_or(Refused::Gone)?;
        Ok(Detail {
            argv: process
                .cmd()
                .iter()
                .map(|word| word.to_string_lossy().into_owned())
                .collect(),
            exe: process.exe().map(|path| path.display().to_string()),
            cwd: process.cwd().map(|path| path.display().to_string()),
            env: process
                .environ()
                .iter()
                .filter_map(|word| pair(word))
                .collect(),
            fds: open_files(key.0),
        })
    }

    fn signal(&self, key: Key, signal: Signal) -> Result<(), Refused> {
        let number = verify(key)?;
        // SAFETY: `kill` is given a pid that is positive (so never a group) and a signal number.
        let sent = unsafe { libc::kill(number, signal.number()) };
        if sent == 0 { Ok(()) } else { Err(refusal()) }
    }

    fn shell(&self, cwd: Option<&str>) -> Option<std::process::Command> {
        let mut command = std::process::Command::new(shell_path());
        let dir = cwd
            .map(std::path::PathBuf::from)
            .filter(|dir| dir.is_dir())
            .or_else(|| std::env::var_os("HOME").map(std::path::PathBuf::from));
        if let Some(dir) = dir {
            command.current_dir(dir);
        }
        Some(command)
    }

    fn renice(&self, key: Key, nice: i32) -> Result<(), Refused> {
        let number = verify(key)?;
        let who = libc::id_t::try_from(number).map_err(|_| Refused::Gone)?;
        // On Linux `setpriority` changes the one thread the id names: the priority of a process is
        // that of each of its threads.
        let ids = if cfg!(target_os = "linux") {
            threads_in(std::path::Path::new(&format!("/proc/{who}/task")), who)
        } else {
            vec![who]
        };
        let mut changed = false;
        for id in ids {
            // SAFETY: `setpriority` is given a positive process or thread id and a nice value.
            let set = unsafe { libc::setpriority(libc::PRIO_PROCESS, id, nice) };
            if set == 0 {
                changed = true;
            } else {
                match refusal() {
                    // A thread that ended on the way is not a failure of the rest.
                    Refused::Gone if changed => {}
                    other => return Err(other),
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    use super::*;

    /// A process the test starts and ends itself: the only one these tests ever signal.
    struct Sleeper(Child);

    impl Sleeper {
        fn start() -> Self {
            Self(
                Command::new("sleep")
                    .arg("300")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .expect("sleep starts"),
            )
        }

        fn key(&self) -> Key {
            let pid = self.0.id();
            let start = start_of(pid).expect("the child is a process");
            (pid, start)
        }
    }

    impl Drop for Sleeper {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn the_threads_of_a_process_are_its_task_directory_with_its_own_id_first() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["42", "40", "41", "not-a-thread"] {
            std::fs::create_dir(dir.path().join(name)).unwrap();
        }
        assert_eq!(threads_in(dir.path(), 41), [41, 40, 42]);
        // A directory that cannot be read leaves the process itself.
        assert_eq!(threads_in(&dir.path().join("none"), 7), [7]);
    }

    /// What `waitpid` says of the child at once, with the flags: a stop, a continue, an end.
    fn waited(child: &Sleeper, flags: i32) -> Option<i32> {
        let pid = libc::pid_t::try_from(child.0.id()).expect("a pid");
        let began = Instant::now();
        while began.elapsed() < Duration::from_secs(5) {
            let mut status = 0;
            // SAFETY: `status` is a plain integer the call writes; the pid is the test's own child.
            let got = unsafe { libc::waitpid(pid, &mut status, flags | libc::WNOHANG) };
            if got == pid {
                return Some(status);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        None
    }

    #[test]
    fn the_shell_starts_where_the_process_works_or_at_home() {
        let command = SystemControl.shell(Some("/tmp")).expect("a shell");
        assert_eq!(
            command.get_current_dir(),
            Some(std::path::Path::new("/tmp"))
        );
        let command = SystemControl
            .shell(Some("/no/such/directory"))
            .expect("a shell");
        assert_eq!(
            command.get_current_dir().map(std::path::Path::to_path_buf),
            std::env::var_os("HOME").map(std::path::PathBuf::from)
        );
    }

    #[test]
    fn a_signal_is_a_word_and_a_number() {
        for signal in Signal::ALL {
            assert_eq!(Signal::parse(signal.word()), Some(signal));
        }
        assert_eq!(Signal::parse("SEGV"), None);
        assert_eq!(Signal::parse("term"), None);
    }

    #[test]
    fn the_detail_of_a_process_is_what_it_was_started_with() {
        let child = Sleeper::start();
        let detail = SystemControl.detail(child.key()).expect("a detail");
        assert_eq!(detail.argv.last().map(String::as_str), Some("300"));
        assert!(
            detail
                .argv
                .first()
                .is_some_and(|first| first.ends_with("sleep")),
            "{:?}",
            detail.argv
        );
        assert!(detail.exe.is_some_and(|exe| exe.ends_with("sleep")));
        assert!(detail.cwd.is_some());
        assert!(detail.fds.is_some_and(|fds| fds >= 1));
    }

    #[test]
    fn a_stop_a_continue_a_new_nice_value_and_an_end_reach_the_process_that_was_named() {
        let child = Sleeper::start();
        let key = child.key();
        SystemControl
            .renice(key, 7)
            .expect("a higher nice is allowed");
        // SAFETY: reads the priority of the test's own child.
        let nice = unsafe { libc::getpriority(libc::PRIO_PROCESS, key.0) };
        assert_eq!(nice, 7);
        SystemControl.signal(key, Signal::Stop).expect("stopped");
        let status = waited(&child, libc::WUNTRACED).expect("the child stopped");
        assert!(libc::WIFSTOPPED(status), "{status}");
        SystemControl.signal(key, Signal::Cont).expect("continued");
        let status = waited(&child, libc::WCONTINUED).expect("the child went on");
        assert!(libc::WIFCONTINUED(status), "{status}");
        SystemControl.signal(key, Signal::Term).expect("ended");
        let status = waited(&child, 0).expect("the child ended");
        assert!(libc::WIFSIGNALED(status));
        assert_eq!(libc::WTERMSIG(status), libc::SIGTERM);
        // Once it has ended, the name no longer names anything.
        assert_eq!(SystemControl.signal(key, Signal::Term), Err(Refused::Gone));
    }

    #[test]
    fn a_start_that_does_not_hold_is_gone_and_nothing_is_sent() {
        let mut child = Sleeper::start();
        let (pid, start) = child.key();
        // The child has the nice value of the test, whatever that is where it runs.
        // SAFETY: reads the priority of the test's own child.
        let nice = unsafe { libc::getpriority(libc::PRIO_PROCESS, pid) };
        // The pid is the child's; the second it started in is not: the name is a stranger's.
        for wrong in [start + 1000, start.saturating_sub(1000)] {
            assert_eq!(
                SystemControl.signal((pid, wrong), Signal::Kill),
                Err(Refused::Gone)
            );
            assert_eq!(SystemControl.renice((pid, wrong), 9), Err(Refused::Gone));
            assert_eq!(SystemControl.detail((pid, wrong)), Err(Refused::Gone));
        }
        assert!(
            child
                .0
                .try_wait()
                .expect("the child can be asked")
                .is_none(),
            "the child was touched"
        );
        // And the nice value did not change.
        // SAFETY: reads the priority of the test's own child.
        assert_eq!(unsafe { libc::getpriority(libc::PRIO_PROCESS, pid) }, nice);
    }

    #[test]
    fn a_pid_that_is_no_process_is_gone_and_pid_zero_is_never_signalled() {
        assert_eq!(
            SystemControl.signal((4_000_000_000, 1), Signal::Term),
            Err(Refused::Failed("4000000000 is not a process".to_owned()))
        );
        assert!(matches!(
            SystemControl.signal((0, 1), Signal::Kill),
            Err(Refused::Failed(_))
        ));
        assert_eq!(
            SystemControl.signal((2_000_000_000, 1), Signal::Term),
            Err(Refused::Gone)
        );
    }

    #[test]
    fn another_users_process_is_denied_where_the_system_says_so() {
        // Process 1 is not this person's (the test never runs as root where it matters): it is
        // either denied, or, as root, gone from `verify`'s view of a start that does not hold. Either
        // way nothing is sent: the start given is not the real one.
        let result = SystemControl.signal((1, 1), Signal::Term);
        assert!(
            matches!(result, Err(Refused::Gone | Refused::Denied)),
            "{result:?}"
        );
    }
}
