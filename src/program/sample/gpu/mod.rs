//! Graphics processors: where each kind of machine says how busy they are. Apple's accelerators say
//! it in the I/O registry ([`apple`]), the kernel's DRM drivers in `/sys/class/drm`
//! ([`linux_drm`]), and NVIDIA's driver through its own library, opened when it is there
//! ([`nvml`]). A machine none of these speak for has no GPU as far as this program can tell, and
//! then no card is drawn.

pub mod apple;
pub mod linux_drm;
pub mod nvml;

use super::Sensor;

/// A part of a GPU that is busy on its own (the work of a GPU is not one thing).
#[derive(Debug, Clone, PartialEq)]
pub struct Engine {
    pub name: String,
    /// 0–100.
    pub load: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Gpu {
    pub name: String,
    /// How busy the processor is, 0–100.
    pub load: f64,
    /// Memory in use, bytes, where the system says.
    pub memory_used: Option<u64>,
    /// Memory the GPU has of its own, bytes. None where it uses the machine's (Apple Silicon).
    pub memory_total: Option<u64>,
    /// Degrees Celsius, where the system says.
    pub celsius: Option<f64>,
    pub engines: Vec<Engine>,
}

/// The readers of this machine's GPUs.
pub struct Gpus {
    nvml: Option<nvml::Nvml>,
}

impl Gpus {
    #[must_use]
    pub fn new() -> Self {
        Self {
            nvml: nvml::Nvml::open(),
        }
    }

    /// The GPUs now. Empty on a machine none of the readers speak for.
    pub fn read(&mut self) -> Vec<Gpu> {
        let mut found = apple::read();
        found.extend(linux_drm::read());
        if let Some(nvml) = &self.nvml {
            found.extend(nvml.read());
        }
        found
    }
}

impl Default for Gpus {
    fn default() -> Self {
        Self::new()
    }
}

/// The GPU's temperature out of the sensors a card's details read: the first that names a GPU.
#[must_use]
pub fn celsius_of(sensors: &[Sensor]) -> Option<f64> {
    sensors
        .iter()
        .find(|sensor| sensor.name.to_ascii_lowercase().contains("gpu"))
        .map(|sensor| sensor.celsius)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sensor_that_names_a_gpu_gives_its_temperature() {
        let sensors = [
            Sensor {
                name: "CPU die".to_owned(),
                celsius: 50.0,
            },
            Sensor {
                name: "GPU MTR Temp".to_owned(),
                celsius: 44.5,
            },
        ];
        assert_eq!(celsius_of(&sensors), Some(44.5));
        assert_eq!(celsius_of(&sensors[..1]), None);
    }

    #[test]
    fn this_machine_is_read_without_harm_and_says_what_it_has_or_nothing() {
        let mut gpus = Gpus::new();
        for gpu in gpus.read() {
            assert!(!gpu.name.is_empty());
            assert!((0.0..=100.0).contains(&gpu.load), "{gpu:?}");
        }
    }
}
