//! What this computer can run: memory, CPU and GPUs, read locally (no
//! network), and which DRIFT model fits it.
//!
//! Detection uses what the OS already has: `sysctl` on macOS, `/proc` on
//! Linux, PowerShell's CIM on Windows and `nvidia-smi` where it exists. The
//! fit rules are pure so they can be tested.

use super::model::ModelSpec;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gpu {
    pub name: String,
    /// Dedicated memory in bytes (0 when unknown).
    pub vram: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hardware {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub cores: u32,
    /// Total memory in bytes (0 when unknown).
    pub memory: u64,
    /// GPU and CPU share memory (Apple Silicon).
    pub unified: bool,
    pub gpus: Vec<Gpu>,
    /// This build runs the model on the GPU (Metal); otherwise on the CPU.
    pub gpu_build: bool,
}

/// How well a model fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Fits with room to spare.
    Good,
    /// Runs, but leaves little memory for everything else.
    Tight,
    /// Not enough memory.
    No,
    /// Memory couldn't be read.
    Unknown,
}

impl Fit {
    pub fn as_str(self) -> &'static str {
        match self {
            Fit::Good => "good",
            Fit::Tight => "tight",
            Fit::No => "no",
            Fit::Unknown => "unknown",
        }
    }
}

const GB: u64 = 1_000_000_000;

/// Memory a model needs while answering: the file plus the context cache and
/// llama.cpp's buffers (8k context).
pub fn needed(spec: &ModelSpec) -> u64 {
    spec.bytes + spec.overhead
}

/// Memory the model may use. On Apple Silicon Metal can wire about two
/// thirds of RAM (three quarters above 36 GB). On a CPU build the model sits
/// in RAM next to the OS, the app and everything else.
pub fn budget(hw: &Hardware) -> (u64, u64) {
    let m = hw.memory;
    if hw.unified && hw.gpu_build {
        let wired = if m > 36 * GB { m / 4 * 3 } else { m / 3 * 2 };
        // (comfortable, the most it can take)
        (wired.saturating_sub(GB), wired)
    } else {
        (m / 2, m / 10 * 7)
    }
}

pub fn fit(hw: &Hardware, spec: &ModelSpec) -> Fit {
    if hw.memory == 0 {
        return Fit::Unknown;
    }
    let (good, max) = budget(hw);
    let need = needed(spec);
    if need <= good {
        Fit::Good
    } else if need <= max {
        Fit::Tight
    } else {
        Fit::No
    }
}

/// The window every model gets (what `overhead` was measured for).
pub const BASE_CONTEXT: u32 = 8192;
/// Windows tried, biggest first.
const CONTEXTS: [u32; 3] = [16384, 12288, BASE_CONTEXT];

/// The context window to load `spec` with: 16k or 12k when the bigger cache
/// still fits the comfortable budget, else 8k. A bigger window costs only
/// memory (the cache is allocated up front); prompts aren't slower until
/// they're actually longer. On a CPU build the window stays at 8k, where
/// reading a long prompt is slow enough that the extra room wouldn't help.
pub fn context_for(hw: &Hardware, spec: &ModelSpec) -> u32 {
    if hw.memory == 0 || !hw.gpu_build {
        return BASE_CONTEXT;
    }
    // Keep a gigabyte of the comfortable budget free: the window is a nice
    // to have, the rest of the computer isn't.
    let room = budget(hw).0.saturating_sub(GB);
    CONTEXTS
        .into_iter()
        .find(|&n| needed(spec) + u64::from(n - BASE_CONTEXT) * spec.kv_per_token <= room)
        .unwrap_or(BASE_CONTEXT)
}

/// The biggest model that fits comfortably (the smallest when none does or
/// memory is unknown). `models` is ordered small to big.
pub fn recommend<'a>(hw: &Hardware, models: &'a [ModelSpec]) -> &'a ModelSpec {
    models.iter().rev().find(|m| fit(hw, m) == Fit::Good).unwrap_or(&models[0])
}

/// One line on why, for the settings card.
pub fn reason(hw: &Hardware, spec: &ModelSpec) -> String {
    let need = needed(spec) as f64 / GB as f64;
    let have = hw.memory as f64 / GB as f64;
    let place = if hw.unified && hw.gpu_build { "of unified memory the GPU can use" } else { "of RAM" };
    let (good, max) = budget(hw);
    match fit(hw, spec) {
        Fit::Unknown => format!("needs about {need:.1} GB; this computer's memory couldn't be read"),
        Fit::Good => {
            format!("needs about {need:.1} GB, about {:.0} GB {place} is free for it", good as f64 / GB as f64)
        }
        Fit::Tight => format!(
            "needs about {need:.1} GB of the {:.0} GB {place}: it runs, but other apps get little room",
            max as f64 / GB as f64
        ),
        Fit::No => format!("needs about {need:.1} GB; this computer has {have:.0} GB in all"),
    }
}

// ── parsers (pure) ─────────────────────────────────────────────────────

/// `MemTotal:  16314880 kB` from /proc/meminfo.
pub fn parse_meminfo_total(s: &str) -> u64 {
    s.lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))
        .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
        .map_or(0, |kb| kb * 1024)
}

/// The first `model name` from /proc/cpuinfo (ARM boards may have none).
pub fn parse_cpuinfo_model(s: &str) -> String {
    s.lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            matches!(k.trim(), "model name" | "Model" | "Hardware").then(|| v.trim().to_owned())
        })
        .unwrap_or_default()
}

/// `nvidia-smi --query-gpu=name,memory.total --format=csv,noheader,nounits`:
/// `NVIDIA GeForce RTX 3060, 12288` (MiB).
pub fn parse_nvidia_smi(s: &str) -> Vec<Gpu> {
    s.lines()
        .filter_map(|l| {
            let (name, mib) = l.rsplit_once(',')?;
            let mib: u64 = mib.trim().parse().ok()?;
            Some(Gpu { name: name.trim().to_owned(), vram: mib * 1024 * 1024 })
        })
        .collect()
}

/// `key=value` lines from the Windows PowerShell probe.
pub fn parse_kv(s: &str) -> Vec<(String, String)> {
    s.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            Some((k.trim().to_owned(), v.trim().to_owned()))
        })
        .collect()
}

/// Windows: memory, CPU and video adapters. AdapterRAM is a 32-bit field
/// and tops out at 4 GB, so it's only used when nvidia-smi says nothing.
pub fn from_windows_kv(kv: &[(String, String)]) -> (u64, String, Vec<Gpu>) {
    let mut mem = 0;
    let mut cpu = String::new();
    let mut gpus = Vec::new();
    for (k, v) in kv {
        match k.as_str() {
            "mem" => mem = v.parse().unwrap_or(0),
            "cpu" if cpu.is_empty() => cpu = v.clone(),
            "gpu" => {
                let (name, ram) = v.rsplit_once('|').unwrap_or((v, "0"));
                gpus.push(Gpu { name: name.trim().to_owned(), vram: ram.trim().parse().unwrap_or(0) });
            }
            _ => {}
        }
    }
    (mem, cpu, gpus)
}

// ── detection ──────────────────────────────────────────────────────────

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let mut c = std::process::Command::new(cmd);
    c.args(args).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashing up.
        c.creation_flags(0x0800_0000);
    }
    let out = c.output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn nvidia() -> Vec<Gpu> {
    run("nvidia-smi", &["--query-gpu=name,memory.total", "--format=csv,noheader,nounits"])
        .map(|s| parse_nvidia_smi(&s))
        .unwrap_or_default()
}

/// Reads this computer's hardware. Blocking (runs a few small programs).
pub fn detect(gpu_build: bool) -> Hardware {
    let mut hw = Hardware {
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        cores: std::thread::available_parallelism().map_or(0, |n| n.get() as u32),
        gpu_build,
        ..Default::default()
    };
    if cfg!(target_os = "macos") {
        hw.memory = run("sysctl", &["-n", "hw.memsize"]).and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        hw.cpu = run("sysctl", &["-n", "machdep.cpu.brand_string"]).map(|s| s.trim().to_owned()).unwrap_or_default();
        hw.unified = hw.arch == "aarch64";
        if hw.unified {
            hw.gpus.push(Gpu {
                name: format!("{} GPU (shares memory)", if hw.cpu.is_empty() { "Apple" } else { &hw.cpu }),
                vram: 0,
            });
        }
    } else if cfg!(target_os = "linux") {
        hw.memory = std::fs::read_to_string("/proc/meminfo").map(|s| parse_meminfo_total(&s)).unwrap_or(0);
        hw.cpu = std::fs::read_to_string("/proc/cpuinfo").map(|s| parse_cpuinfo_model(&s)).unwrap_or_default();
        hw.gpus = nvidia();
    } else if cfg!(windows) {
        let script = "$c=Get-CimInstance Win32_ComputerSystem;'mem='+$c.TotalPhysicalMemory;\
                      Get-CimInstance Win32_Processor|%{'cpu='+$_.Name};\
                      Get-CimInstance Win32_VideoController|%{'gpu='+$_.Name+'|'+$_.AdapterRAM}";
        if let Some(out) = run("powershell", &["-NoProfile", "-NonInteractive", "-Command", script]) {
            let (mem, cpu, gpus) = from_windows_kv(&parse_kv(&out));
            hw.memory = mem;
            hw.cpu = cpu;
            hw.gpus = gpus;
        }
        let nv = nvidia();
        if !nv.is_empty() {
            hw.gpus.retain(|g| !g.name.to_ascii_lowercase().contains("nvidia"));
            hw.gpus.extend(nv);
        }
    }
    hw
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helper::model::MODELS;

    fn mac(gb: u64) -> Hardware {
        Hardware {
            memory: gb * 1024 * 1024 * 1024,
            unified: true,
            gpu_build: true,
            arch: "aarch64".into(),
            ..Default::default()
        }
    }
    fn pc(gb: u64) -> Hardware {
        Hardware { memory: gb * 1024 * 1024 * 1024, ..Default::default() }
    }

    #[test]
    fn context_grows_only_with_room() {
        let by_tier = |t: &str| MODELS.iter().find(|m| m.tier == t).unwrap();
        let (light, balanced, best) = (by_tier("light"), by_tier("balanced"), by_tier("best"));
        // 8 GB Mac: the light model fits with 8k only.
        assert_eq!(context_for(&mac(8), light), 8192);
        // 16 GB Mac: room for 16k with either the light or the balanced model.
        assert_eq!(context_for(&mac(16), light), 16384);
        assert_eq!(context_for(&mac(16), balanced), 16384);
        // The best model on a 32 GB Mac gets 16k too; on 24 GB it doesn't fit, so 8k.
        assert_eq!(context_for(&mac(32), best), 16384);
        assert_eq!(context_for(&mac(24), best), 8192);
        // CPU builds and unknown memory stay at 8k.
        assert_eq!(context_for(&pc(64), light), 8192);
        assert_eq!(context_for(&Hardware::default(), light), 8192);
        // Never more than the comfortable budget.
        for gb in [8, 16, 24, 32, 64] {
            for m in MODELS {
                let n = context_for(&mac(gb), m);
                if n > 8192 {
                    assert!(needed(m) + u64::from(n - 8192) * m.kv_per_token <= budget(&mac(gb)).0);
                }
            }
        }
    }

    #[test]
    fn recommends_by_memory() {
        let pick = |hw: &Hardware| recommend(hw, MODELS).tier;
        assert_eq!(pick(&mac(8)), "light");
        assert_eq!(pick(&mac(16)), "balanced");
        assert_eq!(pick(&mac(24)), "balanced");
        assert_eq!(pick(&mac(32)), "best");
        assert_eq!(pick(&mac(64)), "best");
        assert_eq!(pick(&pc(8)), "light");
        assert_eq!(pick(&pc(16)), "balanced");
        assert_eq!(pick(&pc(32)), "balanced");
        assert_eq!(pick(&pc(48)), "best");
        assert_eq!(pick(&Hardware::default()), "light");
    }

    #[test]
    fn marks_what_wont_fit() {
        let best = MODELS.iter().find(|m| m.tier == "best").unwrap();
        assert_eq!(fit(&mac(16), best), Fit::No);
        assert_eq!(fit(&mac(8), &MODELS[0]), Fit::Good);
        assert_eq!(fit(&Hardware::default(), &MODELS[0]), Fit::Unknown);
        assert!(reason(&mac(16), best).contains("has 17 GB in all"));
    }

    #[test]
    fn parses_probes() {
        assert_eq!(parse_meminfo_total("MemTotal:       16314880 kB\nMemFree: 1 kB\n"), 16_314_880 * 1024);
        assert_eq!(parse_meminfo_total("nothing"), 0);
        assert_eq!(
            parse_cpuinfo_model("processor\t: 0\nmodel name\t: AMD Ryzen 7 5800X 8-Core Processor\n"),
            "AMD Ryzen 7 5800X 8-Core Processor"
        );
        let g = parse_nvidia_smi("NVIDIA GeForce RTX 3060, 12288\nbad line\n");
        assert_eq!(g, vec![Gpu { name: "NVIDIA GeForce RTX 3060".into(), vram: 12288 * 1024 * 1024 }]);
        let (mem, cpu, gpus) = from_windows_kv(&parse_kv(
            "mem=34276728832\r\ncpu=AMD Ryzen Z1 Extreme\r\ngpu=AMD Radeon Graphics|536870912\r\n",
        ));
        assert_eq!(mem, 34_276_728_832);
        assert_eq!(cpu, "AMD Ryzen Z1 Extreme");
        assert_eq!(gpus[0].vram, 536_870_912);
    }
}
