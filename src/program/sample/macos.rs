//! What sysinfo does not say on macOS: how pressed memory is.

/// How pressed memory is, 0–100: the complement of `kern.memorystatus_level`, the share of memory
/// the system counts as free (the figure behind Activity Monitor's memory pressure graph).
#[cfg(target_os = "macos")]
pub fn memory_pressure() -> Option<f64> {
    let mut level: libc::c_int = 0;
    let mut size = std::mem::size_of::<libc::c_int>();
    // SAFETY: the name is a NUL-terminated string, `level` is a c_int and `size` is its size; no
    // new value is set.
    let status = unsafe {
        libc::sysctlbyname(
            c"kern.memorystatus_level".as_ptr(),
            std::ptr::addr_of_mut!(level).cast(),
            &raw mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    (status == 0 && (0..=100).contains(&level)).then(|| f64::from(100 - level))
}

#[cfg(not(target_os = "macos"))]
pub fn memory_pressure() -> Option<f64> {
    None
}

/// What memory is made of, as Activity Monitor adds it up: app memory (what programs hold, less what
/// can be purged), wired, compressed, cached files (and purgeable), and free.
#[cfg(target_os = "macos")]
pub fn memory_breakdown(total: u64) -> Option<super::Breakdown> {
    static HOST: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    // SAFETY: `mach_host_self` takes nothing; its port is kept for the run, not asked for again.
    // (libc points to the `mach2` crate for it; one call is not worth a dependency, and the lock
    // file holds the libc that has it.)
    #[allow(deprecated)]
    let host = *HOST.get_or_init(|| unsafe { libc::mach_host_self() });
    // SAFETY: a zeroed plain-data struct of the size the call is told it has.
    let mut stats: libc::vm_statistics64 = unsafe { std::mem::zeroed() };
    let mut count = libc::HOST_VM_INFO64_COUNT;
    // SAFETY: `stats` is as big as `count` says, and the call writes no more.
    let status = unsafe {
        libc::host_statistics64(
            host,
            libc::HOST_VM_INFO64,
            std::ptr::addr_of_mut!(stats).cast(),
            &raw mut count,
        )
    };
    if status != libc::KERN_SUCCESS {
        return None;
    }
    // SAFETY: `sysconf` takes a name and changes nothing.
    let page = u64::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }).ok()?;
    let pages = |count: u32| u64::from(count) * page;
    let purgeable = pages(stats.purgeable_count);
    let wired = pages(stats.wire_count);
    let compressed = pages(stats.compressor_page_count);
    let cached = pages(stats.external_page_count) + purgeable;
    let app = pages(stats.internal_page_count).saturating_sub(purgeable);
    let free = total.saturating_sub(app + wired + compressed + cached);
    Some(super::Breakdown {
        app,
        wired,
        compressed,
        cached,
        free,
    })
}

#[cfg(not(target_os = "macos"))]
pub fn memory_breakdown(_total: u64) -> Option<super::Breakdown> {
    None
}

/// What sysinfo does not say of a process on macOS: its nice value and the number of its threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Task {
    pub nice: i32,
    pub threads: u32,
}

#[cfg(target_os = "macos")]
pub fn task(pid: u32) -> Option<Task> {
    let pid = libc::c_int::try_from(pid).ok()?;
    // SAFETY: `info` is a zeroed plain-data struct of the size handed to the call, which fills at
    // most that many bytes and says how many it did.
    let bsd = unsafe {
        let mut info: libc::proc_bsdinfo = std::mem::zeroed();
        let size = libc::c_int::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).ok()?;
        let got = libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            std::ptr::addr_of_mut!(info).cast(),
            size,
        );
        (got == size).then_some(info)?
    };
    // SAFETY: as above.
    let threads = unsafe {
        let mut info: libc::proc_taskinfo = std::mem::zeroed();
        let size = libc::c_int::try_from(std::mem::size_of::<libc::proc_taskinfo>()).ok()?;
        let got = libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTASKINFO,
            0,
            std::ptr::addr_of_mut!(info).cast(),
            size,
        );
        if got == size {
            u32::try_from(info.pti_threadnum).unwrap_or(1)
        } else {
            1
        }
    };
    Some(Task {
        nice: bsd.pbi_nice,
        threads,
    })
}

#[cfg(not(target_os = "macos"))]
pub fn task(_pid: u32) -> Option<Task> {
    None
}

/// The offsets of what is read of a socket's description (`socket_fdinfo` of `<sys/proc_info.h>`):
/// libc has the call and not the structures, so the bytes are read where the header puts them (the
/// test that opens a listener of its own and finds it says when they are not there).
#[cfg(target_os = "macos")]
mod socket {
    /// `proc_fileinfo` comes first.
    const FILE_INFO: usize = 24;
    /// `socket_info`: the `vinfo_stat` (136), then the pointers, type, protocol and family.
    pub const PROTOCOL: usize = FILE_INFO + 156;
    pub const FAMILY: usize = FILE_INFO + 160;
    /// `soi_kind`, and the union that follows it.
    pub const KIND: usize = FILE_INFO + 232;
    const PROTO: usize = FILE_INFO + 240;
    pub const LOCAL_PORT: usize = PROTO + 4;
    pub const IP_VERSION: usize = PROTO + 24;
    pub const LOCAL_ADDRESS: usize = PROTO + 48;
    /// In a `tcp_sockinfo`, the state follows the `in_sockinfo` (80 bytes).
    pub const TCP_STATE: usize = PROTO + 80;
    pub const KIND_IN: i32 = 1;
    pub const KIND_TCP: i32 = 2;
    pub const TCP_LISTEN: i32 = 1;
    pub const SIZE: usize = 1024;
}

#[cfg(target_os = "macos")]
fn int_at(buffer: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_ne_bytes(buffer.get(at..at + 4)?.try_into().ok()?))
}

/// The sockets that listen, in the processes this person may look into.
#[cfg(target_os = "macos")]
pub fn listeners() -> Vec<super::ports::Listener> {
    use super::ports::{Listener, Proto, show_v4, show_v6};
    // SAFETY: with no buffer the call only says how many bytes the list of pids would take.
    let bytes = unsafe { libc::proc_listpids(1, 0, std::ptr::null_mut(), 0) };
    let Ok(bytes) = usize::try_from(bytes) else {
        return Vec::new();
    };
    // Room for the processes that start meanwhile.
    let mut pids = vec![0_i32; bytes / std::mem::size_of::<i32>() + 64];
    let Ok(size) = libc::c_int::try_from(pids.len() * std::mem::size_of::<i32>()) else {
        return Vec::new();
    };
    // SAFETY: `pids` is as big as the size handed over, which the call fills at most.
    let got = unsafe { libc::proc_listpids(1, 0, pids.as_mut_ptr().cast(), size) };
    let count = usize::try_from(got).unwrap_or(0) / std::mem::size_of::<i32>();
    pids.truncate(count);

    let each = std::mem::size_of::<libc::proc_fdinfo>();
    let mut found = Vec::new();
    let mut buffer = vec![0_u8; socket::SIZE];
    for pid in pids.into_iter().filter(|pid| *pid > 0) {
        // SAFETY: as above, to learn how big the list of files is.
        let need =
            unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0) };
        let Ok(need) = usize::try_from(need) else {
            continue;
        };
        if need == 0 {
            continue;
        }
        let mut fds: Vec<libc::proc_fdinfo> = Vec::new();
        fds.resize_with(need / each + 8, || libc::proc_fdinfo {
            proc_fd: 0,
            proc_fdtype: 0,
        });
        let Ok(room) = libc::c_int::try_from(fds.len() * each) else {
            continue;
        };
        // SAFETY: `fds` is as big as `room` says.
        let got = unsafe {
            libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, fds.as_mut_ptr().cast(), room)
        };
        let listed = usize::try_from(got).unwrap_or(0) / each;
        for fd in fds.iter().take(listed) {
            if fd.proc_fdtype != libc::PROX_FDTYPE_SOCKET as u32 {
                continue;
            }
            buffer.fill(0);
            // SAFETY: `buffer` is as big as the size handed over; 3 is `PROC_PIDFDSOCKETINFO`.
            let got = unsafe {
                libc::proc_pidfdinfo(
                    pid,
                    fd.proc_fd,
                    3,
                    buffer.as_mut_ptr().cast(),
                    libc::c_int::try_from(socket::SIZE).unwrap_or(0),
                )
            };
            if usize::try_from(got).unwrap_or(0) < socket::TCP_STATE + 4 {
                continue;
            }
            let (Some(kind), Some(protocol), Some(family)) = (
                int_at(&buffer, socket::KIND),
                int_at(&buffer, socket::PROTOCOL),
                int_at(&buffer, socket::FAMILY),
            ) else {
                continue;
            };
            let proto = match (kind, protocol) {
                (socket::KIND_TCP, _)
                    if int_at(&buffer, socket::TCP_STATE) == Some(socket::TCP_LISTEN) =>
                {
                    Proto::Tcp
                }
                (socket::KIND_IN, 17) => Proto::Udp,
                _ => continue,
            };
            let Some(port) = int_at(&buffer, socket::LOCAL_PORT) else {
                continue;
            };
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let port = u16::from_be(port as u16);
            if port == 0 {
                continue;
            }
            let version = buffer.get(socket::IP_VERSION).copied().unwrap_or(0);
            let addr = if version & 2 != 0 && family == libc::AF_INET6 {
                let mut bytes = [0_u8; 16];
                bytes.copy_from_slice(&buffer[socket::LOCAL_ADDRESS..socket::LOCAL_ADDRESS + 16]);
                show_v6(std::net::Ipv6Addr::from(bytes))
            } else {
                // An IPv4 address sits in the last four bytes of the union.
                let at = socket::LOCAL_ADDRESS + 12;
                let mut bytes = [0_u8; 4];
                bytes.copy_from_slice(&buffer[at..at + 4]);
                show_v4(std::net::Ipv4Addr::from(bytes))
            };
            found.push(Listener {
                proto,
                addr,
                port,
                pid: u32::try_from(pid).ok(),
            });
        }
    }
    found
}

#[cfg(not(target_os = "macos"))]
pub fn listeners() -> Vec<super::ports::Listener> {
    Vec::new()
}

/// How many files, sockets and pipes a process has open: the size the system says a list of them
/// would take, divided by what one takes.
#[cfg(target_os = "macos")]
pub fn open_files(pid: u32) -> Option<usize> {
    let pid = libc::c_int::try_from(pid).ok()?;
    // SAFETY: with no buffer the call only says how many bytes the list would take.
    let bytes =
        unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0) };
    let each = libc::c_int::try_from(std::mem::size_of::<libc::proc_fdinfo>()).ok()?;
    (bytes >= 0 && each > 0).then(|| usize::try_from(bytes / each).unwrap_or(0))
}

#[cfg(not(target_os = "macos"))]
pub fn open_files(_pid: u32) -> Option<usize> {
    None
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn the_parts_of_memory_add_up_to_the_whole_and_the_system_keeps_some_wired() {
        let mut system = sysinfo::System::new();
        system.refresh_memory();
        let total = system.total_memory();
        let parts = memory_breakdown(total).expect("this Mac says what memory is made of");
        assert_eq!(
            parts.app + parts.wired + parts.compressed + parts.cached + parts.free,
            total
        );
        assert!(parts.wired > 0 && parts.app > 0, "{parts:?}");
    }
}

#[cfg(all(test, target_os = "macos"))]
mod socket_tests {
    use super::*;
    use crate::sample::ports::Proto;

    #[test]
    fn a_listener_this_test_opens_is_found_with_its_port_and_this_process() {
        let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let udp = std::net::UdpSocket::bind("0.0.0.0:0").unwrap();
        let (tcp_port, udp_port) = (
            tcp.local_addr().unwrap().port(),
            udp.local_addr().unwrap().port(),
        );
        let found = listeners();
        let me = std::process::id();
        let mine = |proto, port| {
            found
                .iter()
                .find(|listener| listener.proto == proto && listener.port == port)
        };
        let tcp_found = mine(Proto::Tcp, tcp_port).expect("the TCP listener is found");
        assert_eq!(
            (tcp_found.pid, tcp_found.addr.as_str()),
            (Some(me), "127.0.0.1")
        );
        let udp_found = mine(Proto::Udp, udp_port).expect("the UDP socket is found");
        assert_eq!((udp_found.pid, udp_found.addr.as_str()), (Some(me), "*"));
    }
}
