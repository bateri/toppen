//! The accelerators of a Mac: each keeps its statistics in the I/O registry, in the dictionary
//! `PerformanceStatistics`. Apple Silicon's GPU shares the machine's memory (what it has in use is
//! one figure); a discrete AMD card of an Intel Mac says how much video memory is used and free.

use super::{Engine, Gpu};
use crate::sample::iokit::{Entry, Value};
use std::collections::HashMap;

/// The accelerators of this machine and how busy they are.
#[cfg(target_os = "macos")]
pub fn read() -> Vec<Gpu> {
    from_entries(&crate::sample::iokit::services(c"IOAccelerator"))
}

#[cfg(not(target_os = "macos"))]
pub fn read() -> Vec<Gpu> {
    Vec::new()
}

fn number(stats: &HashMap<String, Value>, key: &str) -> Option<f64> {
    stats.get(key).and_then(Value::number)
}

fn whole(stats: &HashMap<String, Value>, key: &str) -> Option<u64> {
    stats.get(key).and_then(Value::whole)
}

/// The GPUs the accelerators of the registry stand for. An entry that says nothing of how busy it
/// is (a driver that does not keep the statistics) is left out rather than shown as idle.
#[must_use]
pub fn from_entries(entries: &[Entry]) -> Vec<Gpu> {
    entries
        .iter()
        .filter_map(|entry| {
            let stats = entry.get("PerformanceStatistics")?.dict()?;
            let load = number(stats, "Device Utilization %")?.clamp(0.0, 100.0);
            let name = entry
                .get("model")
                .and_then(Value::text)
                .or_else(|| entry.get("IOClass").and_then(Value::text))
                .unwrap_or(&entry.name)
                .to_owned();
            let engines = [
                ("Renderer", "Renderer Utilization %"),
                ("Tiler", "Tiler Utilization %"),
            ]
            .into_iter()
            .filter_map(|(label, key)| {
                Some(Engine {
                    name: label.to_owned(),
                    load: number(stats, key)?.clamp(0.0, 100.0),
                })
            })
            .collect();
            let (memory_used, memory_total) = match whole(stats, "vramUsedBytes") {
                Some(used) => (
                    Some(used),
                    whole(stats, "vramFreeBytes").map(|free| used.saturating_add(free)),
                ),
                None => (whole(stats, "In use system memory"), None),
            };
            Some(Gpu {
                name,
                load,
                memory_used,
                memory_total,
                celsius: None,
                engines,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(model: &str, stats: &[(&str, i64)]) -> Entry {
        Entry {
            name: "accelerator".to_owned(),
            properties: HashMap::from([
                ("model".to_owned(), Value::Text(model.to_owned())),
                (
                    "PerformanceStatistics".to_owned(),
                    Value::Dict(
                        stats
                            .iter()
                            .map(|(key, value)| ((*key).to_owned(), Value::Int(*value)))
                            .collect(),
                    ),
                ),
            ]),
            parent: None,
        }
    }

    #[test]
    fn an_apple_silicon_gpu_has_its_load_its_engines_and_the_memory_it_uses() {
        let gpus = from_entries(&[entry(
            "Apple M1 Pro",
            &[
                ("Device Utilization %", 44),
                ("Renderer Utilization %", 43),
                ("Tiler Utilization %", 40),
                ("In use system memory", 380_436_480),
            ],
        )]);
        assert_eq!(gpus.len(), 1);
        let gpu = &gpus[0];
        assert_eq!(gpu.name, "Apple M1 Pro");
        assert!((gpu.load - 44.0).abs() < 1e-9);
        assert_eq!(gpu.memory_used, Some(380_436_480));
        assert_eq!(gpu.memory_total, None);
        assert_eq!(
            gpu.engines
                .iter()
                .map(|engine| engine.name.as_str())
                .collect::<Vec<_>>(),
            ["Renderer", "Tiler"]
        );
    }

    #[test]
    fn a_discrete_card_says_how_much_of_its_own_memory_is_used() {
        let gpus = from_entries(&[entry(
            "AMD Radeon Pro 5500M",
            &[
                ("Device Utilization %", 7),
                ("vramUsedBytes", 1_000),
                ("vramFreeBytes", 3_000),
            ],
        )]);
        assert_eq!(gpus[0].memory_used, Some(1_000));
        assert_eq!(gpus[0].memory_total, Some(4_000));
        assert!(gpus[0].engines.is_empty());
    }

    #[test]
    fn an_accelerator_that_does_not_say_how_busy_it_is_is_not_a_gpu_here() {
        let mut quiet = entry("Quiet", &[("In use system memory", 1)]);
        assert!(from_entries(&[quiet.clone()]).is_empty());
        quiet.properties.remove("PerformanceStatistics");
        assert!(from_entries(&[quiet]).is_empty());
    }
}
