//! The GPUs the kernel's DRM drivers keep files for: AMD's say how busy the card is and how much of
//! its video memory is used, in `/sys/class/drm/card*/device`. (A card whose driver keeps no
//! `gpu_busy_percent` is not listed: a zero would look like a measurement.)

use super::Gpu;
use std::fs;
use std::path::Path;

const ROOT: &str = "/sys/class/drm";

fn number(path: &Path) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn text(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// `DRIVER=` out of a device's `uevent`.
fn driver(device: &Path) -> Option<String> {
    fs::read_to_string(device.join("uevent"))
        .ok()?
        .lines()
        .find_map(|line| line.strip_prefix("DRIVER=").map(str::to_owned))
}

/// The temperature the card's hwmon says, degrees (the file is in thousandths).
fn celsius(device: &Path) -> Option<f64> {
    let hwmon = fs::read_dir(device.join("hwmon")).ok()?;
    let mut names: Vec<_> = hwmon
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    names.sort();
    names.iter().find_map(|path| {
        #[allow(clippy::cast_precision_loss)]
        number(&path.join("temp1_input")).map(|milli| milli as f64 / 1000.0)
    })
}

/// The GPU of one `card*` directory, if its driver says how busy it is.
#[must_use]
pub fn read_card(card: &Path) -> Option<Gpu> {
    let device = card.join("device");
    #[allow(clippy::cast_precision_loss)]
    let load = (number(&device.join("gpu_busy_percent"))? as f64).clamp(0.0, 100.0);
    let id = card.file_name()?.to_string_lossy().into_owned();
    let name = text(&device.join("product_name")).unwrap_or_else(|| match driver(&device) {
        Some(driver) => format!("{driver} {id}"),
        None => id.clone(),
    });
    Some(Gpu {
        name,
        load,
        memory_used: number(&device.join("mem_info_vram_used")),
        memory_total: number(&device.join("mem_info_vram_total")),
        celsius: celsius(&device),
        engines: Vec::new(),
    })
}

/// The GPUs under `root`, in the order of their cards.
#[must_use]
pub fn read_in(root: &Path) -> Vec<Gpu> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut cards: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            // `card0`, not `card0-HDMI-A-1` (a connector) or `renderD128`.
            path.file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_prefix("card"))
                .is_some_and(|rest| {
                    !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit())
                })
        })
        .collect();
    cards.sort_by_key(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix("card"))
            .and_then(|rest| rest.parse::<u32>().ok())
    });
    cards.iter().filter_map(|card| read_card(card)).collect()
}

/// The GPUs of this machine the kernel's drivers speak for.
pub fn read() -> Vec<Gpu> {
    if cfg!(target_os = "linux") {
        read_in(Path::new(ROOT))
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(root: &Path, id: &str, files: &[(&str, &str)]) {
        let device = root.join(id).join("device");
        fs::create_dir_all(device.join("hwmon").join("hwmon3")).unwrap();
        for (name, content) in files {
            fs::write(device.join(name), content).unwrap();
        }
    }

    #[test]
    fn an_amd_card_has_its_load_its_video_memory_and_its_temperature() {
        let root = tempfile::tempdir().unwrap();
        card(
            root.path(),
            "card1",
            &[
                ("gpu_busy_percent", "37\n"),
                ("mem_info_vram_used", "1073741824\n"),
                ("mem_info_vram_total", "8589934592\n"),
                ("uevent", "DRIVER=amdgpu\nPCI_ID=1002:73BF\n"),
            ],
        );
        fs::write(
            root.path().join("card1/device/hwmon/hwmon3/temp1_input"),
            "52000\n",
        )
        .unwrap();
        // Connectors, render nodes and cards with nothing to say are not GPUs.
        fs::create_dir_all(root.path().join("card1-HDMI-A-1")).unwrap();
        fs::create_dir_all(root.path().join("renderD128")).unwrap();
        card(root.path(), "card0", &[("uevent", "DRIVER=i915\n")]);

        let gpus = read_in(root.path());
        assert_eq!(gpus.len(), 1);
        let gpu = &gpus[0];
        assert_eq!(gpu.name, "amdgpu card1");
        assert!((gpu.load - 37.0).abs() < 1e-9);
        assert_eq!(gpu.memory_used, Some(1_073_741_824));
        assert_eq!(gpu.memory_total, Some(8_589_934_592));
        assert_eq!(gpu.celsius, Some(52.0));
    }

    #[test]
    fn the_product_name_names_the_card_where_the_driver_has_one() {
        let root = tempfile::tempdir().unwrap();
        card(
            root.path(),
            "card0",
            &[
                ("gpu_busy_percent", "0"),
                ("product_name", "Radeon RX 6700 XT\n"),
            ],
        );
        let gpus = read_in(root.path());
        assert_eq!(gpus[0].name, "Radeon RX 6700 XT");
        assert_eq!(gpus[0].memory_used, None);
        assert_eq!(gpus[0].celsius, None);
    }

    #[test]
    fn a_directory_that_is_not_there_has_no_gpus() {
        assert!(read_in(Path::new("/nonexistent/toppen/drm")).is_empty());
    }
}
