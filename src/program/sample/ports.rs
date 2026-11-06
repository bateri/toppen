//! The sockets that listen: which port, over what, and whose. Scanning them costs (every process's
//! files are looked at), so a scan is kept for a couple of seconds and is made only when someone
//! asks: the ports view, a `port:` filter, a process's details.

use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::{linux, macos};

/// How long a scan stands for the next one.
pub const FRESH: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Proto {
    Tcp,
    Udp,
}

impl Proto {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        }
    }
}

/// A socket that waits for others (TCP) or is bound to a port (UDP).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listener {
    pub proto: Proto,
    /// The address it is bound to; `*` for every address.
    pub addr: String,
    pub port: u16,
    /// The process that holds it, where the system lets this person see.
    pub pid: Option<u32>,
}

/// Where scans come from.
pub trait PortSource: Send + Sync {
    fn scan(&self) -> Vec<Listener>;
}

/// The system's own: `/proc` on Linux, `libproc` on macOS.
pub struct SystemPorts;

impl PortSource for SystemPorts {
    fn scan(&self) -> Vec<Listener> {
        let mut found = linux::scan_ports();
        found.extend(macos::listeners());
        found.sort_by(|a, b| {
            (a.port, a.proto, &a.addr, a.pid).cmp(&(b.port, b.proto, &b.addr, b.pid))
        });
        found.dedup();
        found
    }
}

/// What a scan said and when.
type Scan = Option<(Instant, Arc<Vec<Listener>>)>;

pub struct Ports {
    source: Box<dyn PortSource>,
    last: Mutex<Scan>,
}

impl Ports {
    #[must_use]
    pub fn new(source: Box<dyn PortSource>) -> Self {
        Self {
            source,
            last: Mutex::new(None),
        }
    }

    /// The listeners now, or as of the last scan if that was not long ago. A caller that comes
    /// while a scan is on waits for it rather than making another.
    #[must_use]
    pub fn listeners(&self) -> Arc<Vec<Listener>> {
        let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((at, found)) = last.as_ref()
            && at.elapsed() < FRESH
        {
            return Arc::clone(found);
        }
        let found = Arc::new(self.source.scan());
        *last = Some((Instant::now(), Arc::clone(&found)));
        found
    }

    /// The ports each process listens on.
    #[must_use]
    pub fn by_pid(&self) -> HashMap<u32, Vec<u16>> {
        let mut by_pid: HashMap<u32, Vec<u16>> = HashMap::new();
        for listener in self.listeners().iter() {
            if let Some(pid) = listener.pid {
                let ports = by_pid.entry(pid).or_default();
                if !ports.contains(&listener.port) {
                    ports.push(listener.port);
                }
            }
        }
        by_pid
    }
}

/// An address as a person reads it: `*` for every address.
#[must_use]
pub fn show_v4(address: Ipv4Addr) -> String {
    if address.is_unspecified() {
        "*".to_owned()
    } else {
        address.to_string()
    }
}

#[must_use]
pub fn show_v6(address: Ipv6Addr) -> String {
    if address.is_unspecified() {
        "*".to_owned()
    } else {
        address.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    struct Counting(Arc<AtomicUsize>);

    impl PortSource for Counting {
        fn scan(&self) -> Vec<Listener> {
            self.0.fetch_add(1, Ordering::SeqCst);
            vec![
                Listener {
                    proto: Proto::Tcp,
                    addr: "*".to_owned(),
                    port: 80,
                    pid: Some(7),
                },
                Listener {
                    proto: Proto::Udp,
                    addr: "*".to_owned(),
                    port: 80,
                    pid: Some(7),
                },
                Listener {
                    proto: Proto::Tcp,
                    addr: "*".to_owned(),
                    port: 22,
                    pid: None,
                },
            ]
        }
    }

    #[test]
    fn a_scan_stands_for_the_next_few_seconds_and_the_ports_are_told_by_process() {
        let scans = Arc::new(AtomicUsize::new(0));
        let ports = Ports::new(Box::new(Counting(Arc::clone(&scans))));
        assert_eq!(ports.listeners().len(), 3);
        let by_pid = ports.by_pid();
        assert_eq!(scans.load(Ordering::SeqCst), 1);
        // One port once, however many ways it is held; what nobody holds is nobody's.
        assert_eq!(by_pid.get(&7), Some(&vec![80]));
        assert_eq!(by_pid.len(), 1);
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn a_listener_this_test_opens_is_found_by_the_system_with_this_process() {
        let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = socket.local_addr().unwrap().port();
        let found = SystemPorts.scan();
        let mine = found
            .iter()
            .find(|listener| listener.proto == Proto::Tcp && listener.port == port)
            .expect("the listener is found");
        assert_eq!(mine.pid, Some(std::process::id()));
        assert_eq!(mine.addr, "127.0.0.1");
    }
}
