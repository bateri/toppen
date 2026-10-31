//! NVIDIA's GPUs, through the driver's own library (`libnvidia-ml.so.1`), opened at run time when
//! it is there: a machine without the driver loses nothing and the program does not link to a
//! library it may not find. The few calls it needs are declared here.

use super::{Engine, Gpu};
use std::ffi::{CStr, c_char, c_int, c_uint, c_void};

type Device = usize;

#[repr(C)]
struct Utilization {
    gpu: c_uint,
    memory: c_uint,
}

#[repr(C)]
struct MemoryInfo {
    total: u64,
    free: u64,
    used: u64,
}

type Count = unsafe extern "C" fn(*mut c_uint) -> c_int;
type Handle = unsafe extern "C" fn(c_uint, *mut *mut c_void) -> c_int;
type Name = unsafe extern "C" fn(*mut c_void, *mut c_char, c_uint) -> c_int;
type Rates = unsafe extern "C" fn(*mut c_void, *mut Utilization) -> c_int;
type Memory = unsafe extern "C" fn(*mut c_void, *mut MemoryInfo) -> c_int;
type Temperature = unsafe extern "C" fn(*mut c_void, c_uint, *mut c_uint) -> c_int;

/// The library, its calls, and the devices it counted when it was opened.
pub struct Nvml {
    devices: Vec<Device>,
    name: Name,
    rates: Rates,
    memory: Memory,
    temperature: Temperature,
}

/// The name of a call in the opened library as the type of that call.
macro_rules! symbol {
    ($library:expr, $name:expr, $kind:ty) => {{
        // SAFETY: `$library` is a handle `dlopen` returned and the name is NUL-terminated.
        let address = unsafe { libc::dlsym($library, $name.as_ptr()) };
        if address.is_null() {
            return None;
        }
        // SAFETY: the library documents the call under this name with this signature.
        unsafe { std::mem::transmute::<*mut c_void, $kind>(address) }
    }};
}

impl Nvml {
    /// Opens the library and counts the devices; None where there is no NVIDIA driver (or no GPU
    /// it speaks for). Only Linux has one.
    #[must_use]
    pub fn open() -> Option<Self> {
        if !cfg!(target_os = "linux") {
            return None;
        }
        // SAFETY: a NUL-terminated name and flags `dlopen` takes. The library stays open for the run.
        let library = unsafe { libc::dlopen(c"libnvidia-ml.so.1".as_ptr(), libc::RTLD_NOW) };
        if library.is_null() {
            return None;
        }
        let init = symbol!(library, c"nvmlInit_v2", unsafe extern "C" fn() -> c_int);
        let count = symbol!(library, c"nvmlDeviceGetCount_v2", Count);
        let handle = symbol!(library, c"nvmlDeviceGetHandleByIndex_v2", Handle);
        let name = symbol!(library, c"nvmlDeviceGetName", Name);
        let rates = symbol!(library, c"nvmlDeviceGetUtilizationRates", Rates);
        let memory = symbol!(library, c"nvmlDeviceGetMemoryInfo", Memory);
        let temperature = symbol!(library, c"nvmlDeviceGetTemperature", Temperature);
        // SAFETY: the library was opened above and `init` is its initialiser.
        if unsafe { init() } != 0 {
            return None;
        }
        let mut total: c_uint = 0;
        // SAFETY: `total` is where the call says how many there are.
        if unsafe { count(&raw mut total) } != 0 {
            return None;
        }
        let devices: Vec<Device> = (0..total)
            .filter_map(|index| {
                let mut device: *mut c_void = std::ptr::null_mut();
                // SAFETY: `device` is where the call says which it is.
                (unsafe { handle(index, &raw mut device) } == 0).then_some(device as Device)
            })
            .collect();
        (!devices.is_empty()).then_some(Self {
            devices,
            name,
            rates,
            memory,
            temperature,
        })
    }

    /// The GPUs now.
    #[must_use]
    pub fn read(&self) -> Vec<Gpu> {
        self.devices
            .iter()
            .filter_map(|&device| {
                let device = device as *mut c_void;
                let mut rates = Utilization { gpu: 0, memory: 0 };
                // SAFETY: `device` is a handle of this library and `rates` is the struct it fills.
                if unsafe { (self.rates)(device, &raw mut rates) } != 0 {
                    return None;
                }
                let mut buffer = [0 as c_char; 96];
                // SAFETY: the buffer is as big as the size the call is told.
                let named = unsafe { (self.name)(device, buffer.as_mut_ptr(), 96) } == 0;
                let name = if named {
                    // SAFETY: the call wrote a NUL-terminated string.
                    unsafe { CStr::from_ptr(buffer.as_ptr()) }
                        .to_string_lossy()
                        .into_owned()
                } else {
                    "NVIDIA GPU".to_owned()
                };
                let mut memory = MemoryInfo {
                    total: 0,
                    free: 0,
                    used: 0,
                };
                // SAFETY: as above, for the memory.
                let sized = unsafe { (self.memory)(device, &raw mut memory) } == 0;
                let mut celsius: c_uint = 0;
                // SAFETY: sensor 0 is the GPU's own; `celsius` is where the call says it.
                let warm = unsafe { (self.temperature)(device, 0, &raw mut celsius) } == 0;
                Some(Gpu {
                    name,
                    load: f64::from(rates.gpu.min(100)),
                    memory_used: sized.then_some(memory.used),
                    memory_total: sized.then_some(memory.total),
                    celsius: warm.then(|| f64::from(celsius)),
                    engines: vec![Engine {
                        name: "Memory bus".to_owned(),
                        load: f64::from(rates.memory.min(100)),
                    }],
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_machine_without_the_driver_has_no_library_and_where_it_is_the_gpus_are_read() {
        // Where the driver is (a Linux box with an NVIDIA card) its GPUs are read; elsewhere there is
        // nothing to open, and that is not an error.
        if let Some(nvml) = Nvml::open() {
            for gpu in nvml.read() {
                assert!((0.0..=100.0).contains(&gpu.load));
            }
        } else {
            assert!(Nvml::open().is_none());
        }
    }
}
