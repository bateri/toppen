//! The real source: sysinfo for what it measures on both platforms, `linux.rs` and `macos.rs` for
//! what it does not.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sysinfo::{
    Components, CpuRefreshKind, DiskRefreshKind, Disks, MemoryRefreshKind, Networks,
    ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, RefreshKind, System as Sysinfo,
    UpdateKind, Users,
};

use super::{
    Cpu, Disk, Features, Interface, Machine, Memory, Network, Process, Sample, Sensor, Source,
    Volume, linux, macos,
};

/// The volumes are listed again each so many samples: they change when a disk is mounted, and their
/// sizes move slowly.
const VOLUMES_EVERY: u64 = 5;

/// The users are listed again each so many samples: a person is added now and then.
const USERS_EVERY: u64 = 30;

/// File systems that are not storage a person has files on.
const VIRTUAL_FILESYSTEMS: [&str; 24] = [
    "tmpfs",
    "devtmpfs",
    "devfs",
    "overlay",
    "squashfs",
    "proc",
    "sysfs",
    "cgroup",
    "cgroup2",
    "autofs",
    "ramfs",
    "nsfs",
    "efivarfs",
    "bpf",
    "tracefs",
    "debugfs",
    "securityfs",
    "pstore",
    "configfs",
    "fusectl",
    "mqueue",
    "hugetlbfs",
    "binfmt_misc",
    "rpc_pipefs",
];

/// A mount point as sysinfo says it, before it is judged.
#[derive(Debug, Clone)]
pub struct Mount {
    pub device: String,
    pub mount: String,
    pub fs: String,
    pub total: u64,
    pub available: u64,
}

/// Whether the interface `name` carries traffic of its own: not the loopback, and not the virtual
/// ones that carry what another interface carried already (a container's pair, a VPN's tunnel).
#[must_use]
pub fn counts_interface(name: &str) -> bool {
    const VIRTUAL: [&str; 14] = [
        "lo", "veth", "docker", "br-", "virbr", "utun", "awdl", "llw", "bridge", "ap", "anpi",
        "gif", "stf", "vnet",
    ];
    !VIRTUAL.iter().any(|prefix| {
        // "lo" is the loopback and "lo0" too, but "lowpan0" is not.
        if *prefix == "lo" {
            name == "lo" || name == "lo0"
        } else {
            name.starts_with(prefix)
        }
    })
}

/// The places files live, from the mount points: virtual file systems and the volumes a Mac hides
/// are left out, and the several mount points of one device (BTRFS subvolumes, bind mounts) are one,
/// named by the shortest path.
#[must_use]
pub fn volumes_of(mounts: &[Mount]) -> Vec<Volume> {
    let mut volumes: Vec<Volume> = Vec::new();
    for mount in mounts {
        let hidden = mount.mount.starts_with("/System/Volumes/")
            && mount.mount != "/System/Volumes/Data"
            || mount.mount.starts_with("/snap/")
            || mount.mount.starts_with("/var/lib/docker/")
            || mount.mount.starts_with("/run/")
            || mount.mount.starts_with("/sys/")
            || mount.mount.starts_with("/proc/");
        if mount.total == 0 || hidden || VIRTUAL_FILESYSTEMS.contains(&mount.fs.as_str()) {
            continue;
        }
        let volume = Volume {
            name: mount.device.clone(),
            mount: mount.mount.clone(),
            fs: mount.fs.clone(),
            total: mount.total,
            used: mount.total.saturating_sub(mount.available),
        };
        match volumes.iter_mut().find(|seen| seen.name == volume.name) {
            Some(seen) if volume.mount.len() < seen.mount.len() => *seen = volume,
            Some(_) => {}
            None => volumes.push(volume),
        }
    }
    volumes.sort_by(|a, b| a.mount.cmp(&b.mount));
    volumes
}

pub struct System {
    sys: Sysinfo,
    networks: Networks,
    disks: Disks,
    last: Instant,
    /// The disks' counters at the last sample (Linux).
    counters: linux::Counters,
    volumes: Vec<Volume>,
    users: Users,
    ticks: u64,
    features: Features,
    /// The temperature sensors, listed the first time a card's details are open.
    components: Option<Components>,
    detail: bool,
}

/// What a process is asked of the system at each look; its command line and its user are read once.
fn process_kind() -> ProcessRefreshKind {
    ProcessRefreshKind::nothing()
        .with_cpu()
        .with_memory()
        .with_disk_usage()
        .with_user(UpdateKind::OnlyIfNotSet)
        .with_cmd(UpdateKind::OnlyIfNotSet)
}

/// The state letter of a process where `/proc` does not say it more exactly.
fn state_letter(status: ProcessStatus) -> char {
    match status {
        ProcessStatus::Run => 'R',
        ProcessStatus::Sleep => 'S',
        ProcessStatus::Idle => 'I',
        ProcessStatus::Stop | ProcessStatus::Tracing => 'T',
        ProcessStatus::Zombie => 'Z',
        ProcessStatus::Dead => 'X',
        ProcessStatus::UninterruptibleDiskSleep => 'D',
        _ => '?',
    }
}

impl System {
    #[must_use]
    pub fn new() -> Self {
        let sys = Sysinfo::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        let networks = Networks::new_with_refreshed_list();
        let disks =
            Disks::new_with_refreshed_list_specifics(DiskRefreshKind::nothing().with_storage());
        let counters = if cfg!(target_os = "linux") {
            linux::read_diskstats()
        } else {
            linux::Counters::new()
        };
        let features = Features {
            pressure: pressure().is_some(),
            disk_io: !counters.is_empty(),
        };
        let mut system = Self {
            sys,
            networks,
            disks,
            last: Instant::now(),
            counters,
            volumes: Vec::new(),
            users: Users::new_with_refreshed_list(),
            ticks: 0,
            features,
            components: None,
            detail: false,
        };
        system.volumes = volumes_of(&mounts(&system.disks));
        // The first look at the processes has no usage to say: the next has.
        system
            .sys
            .refresh_processes_specifics(ProcessesToUpdate::All, true, process_kind());
        system
    }
}

impl System {
    /// The temperatures now, while a card's details are open.
    fn sensors(&mut self) -> Vec<Sensor> {
        if !self.detail {
            return Vec::new();
        }
        match self.components.as_mut() {
            Some(components) => components.refresh(false),
            None => self.components = Some(Components::new_with_refreshed_list()),
        }
        let mut sensors: Vec<Sensor> = self
            .components
            .iter()
            .flatten()
            .filter_map(|component| {
                let celsius = f64::from(component.temperature()?);
                (celsius.is_finite() && celsius > 0.0 && celsius < 150.0).then(|| Sensor {
                    name: component.label().to_owned(),
                    celsius,
                })
            })
            .collect();
        sensors.sort_by(|a, b| a.name.cmp(&b.name));
        sensors.truncate(24);
        sensors
    }

    /// The processes now, with what each did since the look before (`seconds` ago).
    fn processes(&mut self, seconds: f64) -> Vec<Process> {
        self.sys
            .refresh_processes_specifics(ProcessesToUpdate::All, true, process_kind());
        if self.ticks % USERS_EVERY == 0 {
            self.users.refresh();
        }
        #[allow(clippy::cast_precision_loss)]
        let per_second = |bytes: u64| {
            if seconds > 0.0 {
                bytes as f64 / seconds
            } else {
                0.0
            }
        };
        self.sys
            .processes()
            .iter()
            .map(|(pid, process)| {
                let pid = pid.as_u32();
                let name = process.name().to_string_lossy().into_owned();
                let command = process
                    .cmd()
                    .iter()
                    .map(|word| word.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ");
                let user = process.user_id().map_or_else(String::new, |uid| {
                    self.users
                        .get_user_by_id(uid)
                        .map_or_else(|| uid.to_string(), |user| user.name().to_owned())
                });
                let io = process.disk_usage();
                let (mut state, mut nice, mut threads) = (state_letter(process.status()), 0, 1);
                if let Some(stat) = linux::read_stat(pid).filter(|_| cfg!(target_os = "linux")) {
                    (state, nice, threads) = (stat.state, stat.nice, stat.threads);
                } else if let Some(task) = macos::task(pid) {
                    (nice, threads) = (task.nice, task.threads);
                }
                Process {
                    pid,
                    ppid: process.parent().map_or(0, sysinfo::Pid::as_u32),
                    start: process.start_time(),
                    command: if command.is_empty() {
                        name.clone()
                    } else {
                        command
                    },
                    name,
                    user,
                    cpu: f64::from(process.cpu_usage()).max(0.0),
                    memory: process.memory(),
                    virt: process.virtual_memory(),
                    threads,
                    state,
                    nice,
                    cpu_time: process.accumulated_cpu_time(),
                    read: per_second(io.read_bytes),
                    write: per_second(io.written_bytes),
                }
            })
            .collect()
    }
}

impl Default for System {
    fn default() -> Self {
        Self::new()
    }
}

/// How pressed memory is, where this system says.
fn pressure() -> Option<f64> {
    if cfg!(target_os = "macos") {
        macos::memory_pressure()
    } else {
        linux::read_pressure()
    }
}

fn mounts(disks: &Disks) -> Vec<Mount> {
    disks
        .list()
        .iter()
        .map(|disk| Mount {
            device: disk.name().to_string_lossy().into_owned(),
            mount: disk.mount_point().to_string_lossy().into_owned(),
            fs: disk.file_system().to_string_lossy().into_owned(),
            total: disk.total_space(),
            available: disk.available_space(),
        })
        .collect()
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |since| since.as_secs_f64())
}

impl Source for System {
    fn machine(&self) -> Machine {
        let cpus = self.sys.cpus();
        let brand = cpus
            .first()
            .map(|cpu| cpu.brand().trim().to_owned())
            .unwrap_or_default();
        Machine {
            host: Sysinfo::host_name().unwrap_or_else(|| "localhost".to_owned()),
            os: Sysinfo::long_os_version()
                .or_else(Sysinfo::name)
                .unwrap_or_else(|| std::env::consts::OS.to_owned()),
            kernel: Sysinfo::kernel_version().unwrap_or_default(),
            arch: Sysinfo::cpu_arch(),
            cpu: brand,
            threads: cpus.len(),
            cores: Sysinfo::physical_core_count().unwrap_or(cpus.len()),
            memory: self.sys.total_memory(),
            booted: Sysinfo::boot_time(),
            features: self.features,
        }
    }

    fn detail(&mut self, open: bool) {
        self.detail = open;
    }

    fn warmup(&self) -> Duration {
        // sysinfo's own floor for a CPU usage that means anything.
        sysinfo::MINIMUM_CPU_UPDATE_INTERVAL.max(Duration::from_millis(250))
    }

    fn sample(&mut self) -> Sample {
        let at = Instant::now();
        let elapsed = at.saturating_duration_since(self.last);
        self.last = at;
        self.ticks += 1;

        self.sys.refresh_cpu_specifics(CpuRefreshKind::everything());
        self.sys
            .refresh_memory_specifics(MemoryRefreshKind::everything());
        self.networks.refresh(true);
        let seconds = elapsed.as_secs_f64();

        let cpus = self.sys.cpus();
        let cpu = Cpu {
            total: f64::from(self.sys.global_cpu_usage()).clamp(0.0, 100.0),
            cores: cpus
                .iter()
                .map(|cpu| f64::from(cpu.cpu_usage()).clamp(0.0, 100.0))
                .collect(),
            freq: cpus.iter().map(sysinfo::Cpu::frequency).collect(),
        };

        let memory = Memory {
            total: self.sys.total_memory(),
            used: self.sys.used_memory(),
            available: self.sys.available_memory(),
            swap_total: self.sys.total_swap(),
            swap_used: self.sys.used_swap(),
            pressure: if self.features.pressure {
                pressure()
            } else {
                None
            },
            breakdown: macos::memory_breakdown(self.sys.total_memory())
                .or_else(linux::read_meminfo),
        };

        let mut interfaces: Vec<Interface> = Vec::new();
        let (mut total_rx, mut total_tx) = (0_u64, 0_u64);
        let (mut rx, mut tx) = (0.0, 0.0);
        for (name, data) in &self.networks {
            if !counts_interface(name) {
                continue;
            }
            total_rx = total_rx.saturating_add(data.total_received());
            total_tx = total_tx.saturating_add(data.total_transmitted());
            #[allow(clippy::cast_precision_loss)]
            let per_second = |bytes: u64| {
                if seconds > 0.0 {
                    bytes as f64 / seconds
                } else {
                    0.0
                }
            };
            let (received, sent) = (per_second(data.received()), per_second(data.transmitted()));
            rx += received;
            tx += sent;
            if received + sent > 0.0 {
                interfaces.push(Interface {
                    name: name.clone(),
                    rx: received,
                    tx: sent,
                });
            }
        }
        interfaces.sort_by(|a, b| {
            (b.rx + b.tx)
                .total_cmp(&(a.rx + a.tx))
                .then_with(|| a.name.cmp(&b.name))
        });
        let network = Network {
            rx,
            tx,
            total_rx,
            total_tx,
            interfaces,
        };

        let devices = if self.features.disk_io {
            let counters = linux::read_diskstats();
            let devices = linux::rates(&self.counters, &counters, elapsed);
            self.counters = counters;
            devices
        } else {
            Vec::new()
        };
        if self.ticks % VOLUMES_EVERY == 0 {
            self.disks
                .refresh_specifics(true, DiskRefreshKind::nothing().with_storage());
            self.volumes = volumes_of(&mounts(&self.disks));
        }
        let disk = Disk {
            read: devices.iter().map(|device| device.read).sum(),
            write: devices.iter().map(|device| device.write).sum(),
            devices,
            volumes: self.volumes.clone(),
        };

        let processes = self.processes(seconds);
        let load = Sysinfo::load_average();
        Sample {
            at: now(),
            cpu,
            memory,
            network,
            disk,
            load: [load.one, load.five, load.fifteen],
            uptime: Sysinfo::uptime(),
            processes,
            sensors: self.sensors(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mount(device: &str, mount: &str, fs: &str) -> Mount {
        Mount {
            device: device.to_owned(),
            mount: mount.to_owned(),
            fs: fs.to_owned(),
            total: 1000,
            available: 400,
        }
    }

    #[test]
    fn the_loopback_and_the_virtual_interfaces_are_not_counted_twice() {
        for name in ["en0", "eth0", "wlan0", "enp3s0", "pdp_ip0", "lowpan0"] {
            assert!(counts_interface(name), "{name}");
        }
        for name in [
            "lo",
            "lo0",
            "veth12ab",
            "docker0",
            "br-1f2e",
            "utun3",
            "awdl0",
            "bridge100",
        ] {
            assert!(!counts_interface(name), "{name}");
        }
    }

    #[test]
    fn mount_points_of_one_device_are_one_volume_named_by_the_shortest() {
        let mounts = [
            mount("/dev/nvme0n1p2", "/home", "btrfs"),
            mount("/dev/nvme0n1p2", "/", "btrfs"),
            mount("/dev/nvme0n1p2", "/var/log", "btrfs"),
            mount("tmpfs", "/tmp", "tmpfs"),
            mount("/dev/nvme0n1p1", "/boot/efi", "vfat"),
        ];
        let volumes = volumes_of(&mounts);
        let found: Vec<(&str, u64)> = volumes.iter().map(|v| (v.mount.as_str(), v.used)).collect();
        assert_eq!(found, [("/", 600), ("/boot/efi", 600)]);
    }

    #[test]
    fn the_volumes_a_mac_hides_are_left_out_and_the_data_volume_stays() {
        let mounts = [
            mount("/dev/disk3s1s1", "/", "apfs"),
            mount("/dev/disk3s6", "/System/Volumes/VM", "apfs"),
            mount("/dev/disk3s2", "/System/Volumes/Preboot", "apfs"),
            mount("/dev/disk3s5", "/System/Volumes/Data", "apfs"),
            Mount {
                total: 0,
                ..mount("map auto_home", "/System/Volumes/Data/home", "autofs")
            },
        ];
        let volumes = volumes_of(&mounts);
        let found: Vec<&str> = volumes.iter().map(|v| v.mount.as_str()).collect();
        assert_eq!(found, ["/", "/System/Volumes/Data"]);
    }

    #[test]
    fn this_machine_can_be_looked_at() {
        let mut system = System::new();
        std::thread::sleep(std::time::Duration::from_millis(250));
        let machine = system.machine();
        let sample = system.sample();
        assert!(machine.threads >= 1 && machine.memory > 0);
        assert_eq!(sample.cpu.cores.len(), machine.threads);
        assert!((0.0..=100.0).contains(&sample.cpu.total));
        assert!(sample.memory.used <= sample.memory.total);
        assert!(sample.uptime > 0);
        // This process is one of them, found by the pid it has and as the user it is.
        let own = sample
            .processes
            .iter()
            .find(|process| process.pid == std::process::id())
            .expect("this process is listed");
        assert!(own.memory > 0 && own.threads >= 1 && !own.name.is_empty());
        assert!(!own.user.is_empty() && own.start > 0, "{own:?}");
    }
}
