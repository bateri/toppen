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
