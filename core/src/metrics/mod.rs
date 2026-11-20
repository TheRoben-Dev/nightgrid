//! Agentless host metrics: one small `sh` script per sample reads `/proc`,
//! `/sys` and `df` over the host's pooled SSH connection, the parsers in
//! [`parse`] turn its sectioned output into a [`Raw`] snapshot, and a
//! [`Collector`] turns consecutive snapshots into rates ([`Sample`]).

pub mod history;
pub mod parse;

use serde::{Deserialize, Serialize};

pub use parse::{parse, Raw};

/// Prints every section the parsers read, each after an `@@name` line.
/// POSIX sh + coreutils/busybox only; slow or missing pieces just leave
/// their section empty (`df` gets a timeout so a hung NFS mount can't stall
/// a sample).
pub const COLLECT: &str = r#"LC_ALL=C; export LC_ALL
echo @@stat; cat /proc/stat 2>/dev/null
echo @@meminfo; cat /proc/meminfo 2>/dev/null
echo @@loadavg; cat /proc/loadavg 2>/dev/null
echo @@uptime; cat /proc/uptime 2>/dev/null
echo @@kernel; uname -sr 2>/dev/null
echo @@hostname; hostname 2>/dev/null || cat /proc/sys/kernel/hostname 2>/dev/null
echo @@netdev; cat /proc/net/dev 2>/dev/null
echo @@diskstats; cat /proc/diskstats 2>/dev/null
echo @@blocks; ls /sys/block 2>/dev/null
echo @@mounts; cat /proc/mounts 2>/dev/null
echo @@df; if command -v timeout >/dev/null 2>&1; then timeout 4 df -kP 2>/dev/null; else df -kP 2>/dev/null; fi
echo @@hwmon
for h in /sys/class/hwmon/hwmon*; do
  [ -d "$h" ] || continue
  n=$(cat "$h/name" 2>/dev/null)
  for t in "$h"/temp*_input; do
    [ -r "$t" ] || continue
    l=$(cat "${t%_input}_label" 2>/dev/null)
    echo "$n|$(cat "$t" 2>/dev/null)|$l"
  done
done
echo @@thermal
for z in /sys/class/thermal/thermal_zone*; do
  [ -r "$z/temp" ] && echo "$(cat "$z/type" 2>/dev/null)|$(cat "$z/temp" 2>/dev/null)"
done
echo @@nvidia
if command -v nvidia-smi >/dev/null 2>&1; then
  q="--query-gpu=index,name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw --format=csv,noheader,nounits"
  if command -v timeout >/dev/null 2>&1; then timeout 3 nvidia-smi $q 2>/dev/null; else nvidia-smi $q 2>/dev/null; fi
fi
echo @@drm
for c in /sys/class/drm/card[0-9] /sys/class/drm/card[0-9][0-9]; do
  d="$c/device"
  [ -r "$d/gpu_busy_percent" ] || continue
  h=$(ls -d "$d"/hwmon/hwmon* 2>/dev/null | head -n 1)
  echo "${c##*/}|$(cat "$d/gpu_busy_percent" 2>/dev/null)|$(cat "$d/mem_info_vram_used" 2>/dev/null)|$(cat "$d/mem_info_vram_total" 2>/dev/null)|$(cat "$h/temp1_input" 2>/dev/null)|$(cat "$h/power1_average" 2>/dev/null)|$(cat "$d/product_name" 2>/dev/null)"
done
echo @@end
"#;

/// Usage of one mounted filesystem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MountUsage {
    pub device: String,
    pub mount: String,
    pub fstype: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub total: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub used: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub avail: u64,
    /// Mounted read-only (`ro` in /proc/mounts).
    #[serde(default)]
    pub read_only: bool,
}

/// One temperature sensor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Temp {
    /// hwmon chip (`coretemp`, `k10temp`, `nvme`, `cpu_thermal`) or thermal zone type.
    pub chip: String,
    /// Sensor label (`Package id 0`, `Tctl`, `Composite`), or "" when none.
    pub label: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub celsius: f32,
}

/// One GPU (NVIDIA through `nvidia-smi`, AMD through sysfs). Fields a
/// driver doesn't report are `None`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    pub name: String,
    /// Busy %.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub util: Option<f32>,
    /// VRAM in bytes.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub mem_used: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub mem_total: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub temp: Option<f32>,
    /// Watts.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub power: Option<f32>,
}

/// Throughput of one network interface or block device, in bytes/s.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Rate {
    pub name: String,
    /// Received (network) or read (disk).
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub rx: f64,
    /// Transmitted (network) or written (disk).
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub tx: f64,
}

/// One processed sample: what the UI shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    /// Unix time, milliseconds.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub ts: f64,
    pub hostname: String,
    pub kernel: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub uptime_secs: f64,
    /// Busy % over all cores since the previous sample (`None` on the first).
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub cpu: Option<f32>,
    /// Busy % per core (empty on the first sample).
    #[cfg_attr(feature = "specta", specta(type = Vec<u32>))]
    pub cores: Vec<f32>,
    #[cfg_attr(feature = "specta", specta(type = [u32; 3]))]
    pub load: [f32; 3],
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub mem_total: u64,
    /// Total minus available (what `free` calls used + shared, minus reclaimable cache).
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub mem_used: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub swap_total: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub swap_used: u64,
    pub mounts: Vec<MountUsage>,
    /// Per interface (loopback left out).
    pub net: Vec<Rate>,
    /// Sum over physical-looking interfaces (no veth/docker/bridges).
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub net_rx: f64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub net_tx: f64,
    /// Per whole disk.
    pub disks: Vec<Rate>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub disk_read: f64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub disk_write: f64,
    pub temps: Vec<Temp>,
    /// Best guess at the CPU temperature, for tiles.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub cpu_temp: Option<f32>,
    pub gpus: Vec<Gpu>,
}

/// Interfaces left out of the network total (virtual / container plumbing).
pub fn is_virtual_iface(name: &str) -> bool {
    const PREFIXES: [&str; 11] =
        ["lo", "veth", "docker", "br-", "virbr", "vnet", "cni", "flannel", "cali", "kube", "lxcbr"];
    // Windows: Hyper-V switches carry the same traffic as the NIC under them;
    // tunnel pseudo-interfaces and WAN miniports aren't links of their own.
    const WINDOWS: [&str; 6] = ["vEthernet", "Hyper-V Virtual", "isatap", "Teredo", "WAN Miniport", "Loopback"];
    PREFIXES.iter().any(|p| name.starts_with(p)) || WINDOWS.iter().any(|p| name.starts_with(p))
}

/// Whole disks counted in the disk total (no loop/ram/zram/dm/md/optical).
pub fn is_physical_disk(name: &str) -> bool {
    const SKIP: [&str; 7] = ["loop", "ram", "zram", "dm-", "md", "sr", "fd"];
    !SKIP.iter().any(|p| name.starts_with(p))
}

/// Picks the sensor that best stands for "CPU temperature".
pub fn cpu_temp(temps: &[Temp]) -> Option<f32> {
    let score = |t: &Temp| -> Option<u8> {
        let chip = t.chip.as_str();
        let label = t.label.to_lowercase();
        match chip {
            "coretemp" if label.starts_with("package") => Some(0),
            "k10temp" | "zenpower" if label == "tctl" || label == "tdie" => Some(0),
            "cpu_thermal" | "cpu-thermal" | "soc_thermal" | "x86_pkg_temp" => Some(1),
            "coretemp" | "k10temp" | "zenpower" => Some(2),
            "acpitz" => Some(4),
            _ if chip.contains("cpu") => Some(3),
            _ => None,
        }
    };
    temps.iter().filter_map(|t| score(t).map(|s| (s, t.celsius))).min_by_key(|(s, _)| *s).map(|(_, c)| c)
}

/// Turns raw snapshots into samples with rates; keeps the previous snapshot.
#[derive(Default)]
pub struct Collector {
    prev: Option<(Raw, f64)>,
}

impl Collector {
    pub fn new() -> Self {
        Self::default()
    }

    /// `ts` is the sample time in unix milliseconds.
    pub fn sample(&mut self, raw: Raw, ts: f64) -> Sample {
        let prev = self.prev.take();
        let dt = prev.as_ref().map(|(_, t)| (ts - t) / 1000.0).filter(|d| *d > 0.05);

        let (cpu, cores) = match (&prev, dt) {
            (Some((p, _)), Some(_)) => {
                let busy = |now: &parse::CpuTimes, before: Option<&parse::CpuTimes>| -> Option<f32> {
                    let before = before?;
                    let total = now.total().checked_sub(before.total())?;
                    let idle = now.idle().checked_sub(before.idle())?;
                    (total > 0).then(|| ((total - idle.min(total)) as f64 / total as f64 * 100.0) as f32)
                };
                let cpu = raw.cpu.as_ref().and_then(|c| busy(c, p.cpu.as_ref()));
                let cores = raw.cores.iter().enumerate().map(|(i, c)| busy(c, p.cores.get(i)).unwrap_or(0.0)).collect();
                (cpu, cores)
            }
            _ => (None, Vec::new()),
        };

        let rate = |now: u64, before: Option<u64>| -> f64 {
            match (before, dt) {
                (Some(b), Some(dt)) if now >= b => (now - b) as f64 / dt,
                _ => 0.0,
            }
        };
        let net: Vec<Rate> = raw
            .net
            .iter()
            .filter(|n| n.name != "lo")
            .map(|n| {
                let p = prev.as_ref().and_then(|(p, _)| p.net.iter().find(|x| x.name == n.name));
                Rate { name: n.name.clone(), rx: rate(n.rx, p.map(|x| x.rx)), tx: rate(n.tx, p.map(|x| x.tx)) }
            })
            .collect();
        let (net_rx, net_tx) =
            net.iter().filter(|n| !is_virtual_iface(&n.name)).fold((0.0, 0.0), |(r, t), n| (r + n.rx, t + n.tx));
        let disks: Vec<Rate> = raw
            .disks
            .iter()
            .filter(|d| (raw.blocks.is_empty() || raw.blocks.contains(&d.name)) && is_physical_disk(&d.name))
            .map(|d| {
                let p = prev.as_ref().and_then(|(p, _)| p.disks.iter().find(|x| x.name == d.name));
                Rate {
                    name: d.name.clone(),
                    rx: rate(d.read_sectors * 512, p.map(|x| x.read_sectors * 512)),
                    tx: rate(d.write_sectors * 512, p.map(|x| x.write_sectors * 512)),
                }
            })
            .collect();
        let (disk_read, disk_write) = disks.iter().fold((0.0, 0.0), |(r, w), d| (r + d.rx, w + d.tx));

        let s = Sample {
            ts,
            hostname: raw.hostname.clone(),
            kernel: raw.kernel.clone(),
            uptime_secs: raw.uptime,
            cpu,
            cores,
            load: raw.load,
            mem_total: raw.mem.total,
            mem_used: raw.mem.used(),
            swap_total: raw.mem.swap_total,
            swap_used: raw.mem.swap_total.saturating_sub(raw.mem.swap_free),
            mounts: raw.mounts.clone(),
            net,
            net_rx,
            net_tx,
            disks,
            disk_read,
            disk_write,
            cpu_temp: cpu_temp(&raw.temps),
            temps: raw.temps.clone(),
            gpus: raw.gpus.clone(),
        };
        self.prev = Some((raw, ts));
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = include_str!("../../tests/fixtures/metrics/x86_server_a.txt");
    const B: &str = include_str!("../../tests/fixtures/metrics/x86_server_b.txt");
    const PI: &str = include_str!("../../tests/fixtures/metrics/pi_busybox.txt");

    #[test]
    fn rates_from_two_snapshots() {
        let mut c = Collector::new();
        let first = c.sample(parse(A), 1_000_000.0);
        assert_eq!(first.cpu, None);
        assert!(first.cores.is_empty());
        assert_eq!(first.net_rx, 0.0);
        let s = c.sample(parse(B), 1_002_000.0);
        // cpu line: +400 total jiffies, +100 idle(+iowait) => 75 % busy.
        assert_eq!(s.cpu, Some(75.0));
        assert_eq!(s.cores.len(), 4);
        assert_eq!(s.cores[0], 90.0);
        // eth0 +2_000_000 rx over 2 s; docker0 / veth excluded from the total.
        assert_eq!(s.net_rx, 1_000_000.0);
        assert_eq!(s.net_tx, 250_000.0);
        assert!(s.net.iter().any(|n| n.name == "docker0") && !s.net.iter().any(|n| n.name == "lo"));
        // nvme0n1 +4096 sectors read = 2 MiB over 2 s; partitions and loop devices ignored.
        assert_eq!(s.disk_read, 1_048_576.0);
        assert_eq!(s.disks.iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), ["nvme0n1", "sda"]);
        assert_eq!(s.cpu_temp, Some(48.0));
    }

    #[test]
    fn busybox_pi_without_hwmon_labels() {
        let s = Collector::new().sample(parse(PI), 0.0);
        assert_eq!(s.cpu_temp, Some(51.6));
        assert_eq!(s.hostname, "pi");
        assert_eq!(s.mounts.iter().map(|m| m.mount.as_str()).collect::<Vec<_>>(), ["/", "/boot/firmware"]);
    }

    #[test]
    fn picks_cpu_sensor() {
        let t = |chip: &str, label: &str, c: f32| Temp { chip: chip.into(), label: label.into(), celsius: c };
        assert_eq!(
            cpu_temp(&[
                t("nvme", "Composite", 40.0),
                t("coretemp", "Core 0", 50.0),
                t("coretemp", "Package id 0", 55.0)
            ]),
            Some(55.0)
        );
        assert_eq!(cpu_temp(&[t("acpitz", "", 30.0), t("k10temp", "Tctl", 61.0)]), Some(61.0));
        assert_eq!(cpu_temp(&[t("nvme", "Composite", 40.0)]), None);
    }
}

#[cfg(all(test, unix))]
mod local_tests {
    /// The collect script runs under `sh` on this machine and parses.
    #[test]
    fn collect_script_runs_locally() {
        let out = std::process::Command::new("sh").arg("-c").arg(super::COLLECT).output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("@@end"), "{text}");
        if cfg!(target_os = "linux") {
            let r = super::parse(&text);
            assert!(r.cpu.is_some() && !r.cores.is_empty());
            assert!(r.mem.total > 0);
            assert!(!r.hostname.is_empty());
        }
    }
}
