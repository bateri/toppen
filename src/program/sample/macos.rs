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
