//! DRIFT's read-only checks on a host (D-151).
//!
//! DRIFT may look things up on a saved host when the user allowed it for that
//! host ("DRIFT may run read-only checks" in the host editor). It never sends
//! a command line of its own: it asks for a [`Lookup`] (a kind plus at most
//! one name or path), and this module turns that into a fixed command from an
//! allow-list. Names and paths are checked against a strict character set and
//! single-quoted, nothing runs through sudo, and every output is clipped and
//! has secrets masked before it leaves this side.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::redact::line_has_secret;

/// Most lookups one question may run (including the model's follow-ups).
pub const MAX_LOOKUPS: usize = 8;
/// Characters of output kept per lookup (the prompt is small).
pub const MAX_OUTPUT: usize = 1500;
/// Bytes a file read may return before clipping.
const FILE_BYTES: usize = 6000;
const MASK: &str = "••••";

/// What DRIFT may look up. Each kind maps to one fixed command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum LookupKind {
    /// Uptime, load, disk use and memory in one go.
    Overview,
    Disk,
    Memory,
    /// Load and the busiest processes by CPU.
    Processes,
    /// Listening TCP/UDP ports.
    Ports,
    /// Addresses and routes.
    Network,
    /// systemd units in the failed state.
    FailedUnits,
    /// Error-level journal lines from the last day.
    Errors,
    /// All containers with image and status.
    Containers,
    /// One container's state (never its environment).
    Container,
    ContainerLogs,
    /// `systemctl status` of one unit.
    Service,
    ServiceLogs,
    /// One file under an allowed path (start of it; the end for logs).
    File,
    /// Compose projects and their files.
    ComposeProjects,
    /// OS, kernel, hostname, virtualization and whether a reboot is pending.
    System,
    /// Kernel warnings and errors (OOM kills, disk and network errors).
    KernelLog,
    /// systemd timers: when each last ran and runs next.
    Timers,
    /// Cron jobs of the login user and /etc/cron.*.
    Cron,
    /// Mounted filesystems (network shares included).
    Mounts,
    /// Block devices, RAID and ZFS pool health.
    Storage,
    /// Temperatures, fans and GPU use, where tools exist.
    Sensors,
    /// Who is logged in and recent logins.
    Logins,
    /// Pending package updates (read from the package cache).
    Updates,
    /// Running services.
    Services,
    /// Connection counts and the busiest remote peers.
    Connections,
    /// CPU, memory and I/O per running container.
    DockerStats,
    /// Docker events of the last hour (restarts, OOM, health changes).
    DockerEvents,
    /// Space used by images, containers and volumes.
    DockerDisk,
}

impl LookupKind {
    pub const ALL: [LookupKind; 29] = [
        Self::Overview,
        Self::Disk,
        Self::Memory,
        Self::Processes,
        Self::Ports,
        Self::Network,
        Self::FailedUnits,
        Self::Errors,
        Self::Containers,
        Self::Container,
        Self::ContainerLogs,
        Self::Service,
        Self::ServiceLogs,
        Self::File,
        Self::ComposeProjects,
        Self::System,
        Self::KernelLog,
        Self::Timers,
        Self::Cron,
        Self::Mounts,
        Self::Storage,
        Self::Sensors,
        Self::Logins,
        Self::Updates,
        Self::Services,
        Self::Connections,
        Self::DockerStats,
        Self::DockerEvents,
        Self::DockerDisk,
    ];

    /// Whether the kind needs a target (a name or a path).
    pub fn needs_target(self) -> bool {
        matches!(self, Self::Container | Self::ContainerLogs | Self::Service | Self::ServiceLogs | Self::File)
    }

    /// Logs keep their newest lines when clipped; everything else its start.
    fn keeps_tail(self) -> bool {
        matches!(self, Self::ContainerLogs | Self::ServiceLogs | Self::Errors | Self::KernelLog | Self::DockerEvents)
    }
}

/// One check DRIFT wants to run.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Lookup {
    pub kind: LookupKind,
    /// Container or unit name, or a file path, for kinds that take one.
    #[serde(default)]
    pub target: Option<String>,
}

impl Lookup {
    pub fn new(kind: LookupKind, target: Option<&str>) -> Self {
        Self { kind, target: target.map(str::to_string) }
    }
}

/// What a check returned, as shown to the user and given to the model.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LookupResult {
    pub lookup: Lookup,
    /// The exact command that ran (empty when it was refused).
    pub command: String,
    pub ok: bool,
    /// Masked and clipped output, or why it couldn't run.
    pub output: String,
    pub clipped: bool,
}

static NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9_.@:-]{0,127}$").unwrap());
static PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(/|~/)[A-Za-z0-9_./@+~-]{0,400}$").unwrap());

/// Validates a container or unit name.
pub fn check_name(n: &str) -> Result<(), String> {
    if NAME.is_match(n) {
        Ok(())
    } else {
        Err(format!("\"{}\" isn't a plain container or service name", clean(n)))
    }
}

fn clean(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(80).collect()
}

/// Paths DRIFT may read by default; a host can add its own prefixes.
pub const DEFAULT_PATHS: &[&str] = &["/etc/", "/var/log/", "/opt/", "/srv/"];
/// File names DRIFT may read anywhere (compose files; `.env` with values masked).
const ANYWHERE: &[&str] = &["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml", ".env"];
/// Never readable, whatever the host allows.
const DENY_PREFIX: &[&str] = &["/proc/", "/sys/", "/dev/", "/run/secrets/", "/etc/ssl/private/", "/var/lib/docker/"];
const DENY_DIRS: &[&str] =
    &[".ssh", ".gnupg", ".aws", ".kube", ".docker", ".password-store", ".config/gh", "private", "secrets", "sudoers.d"];
const DENY_NAMES: &[&str] = &[
    "shadow",
    "shadow-",
    "gshadow",
    "gshadow-",
    "sudoers",
    ".netrc",
    ".pgpass",
    ".git-credentials",
    ".htpasswd",
    "htpasswd",
    "acme.json",
    "authorized_keys",
    "known_hosts",
    "master.key",
];
const DENY_EXT: &[&str] =
    &[".key", ".pem", ".p12", ".pfx", ".kdbx", ".jks", ".keystore", ".gpg", ".asc", ".age", ".crt", ".token"];

/// Whether `path` may be read: under an allowed prefix (or a compose/.env
/// file), and nothing that holds keys or passwords. `extra` are the host's
/// own prefixes (one per entry).
pub fn path_allowed(path: &str, extra: &[String]) -> Result<(), String> {
    if !PATH.is_match(path) || path.split('/').any(|c| c == "..") {
        return Err(format!("\"{}\" isn't a plain absolute path", clean(path)));
    }
    let lower = path.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or("");
    let denied = DENY_PREFIX.iter().any(|p| lower.starts_with(p))
        || lower.split('/').any(|c| DENY_DIRS.contains(&c))
        || lower.contains("/.config/gh/")
        || DENY_NAMES.contains(&name)
        || name.starts_with("sudoers")
        || name.starts_with("id_")
        || name.ends_with("_rsa")
        || name.ends_with("_ed25519")
        || name.ends_with("_ecdsa")
        || name.contains("secret")
        || name.contains("credential")
        || name.contains("password")
        || DENY_EXT.iter().any(|e| name.ends_with(e));
    if denied {
        return Err(format!("{} may hold keys or passwords, so I don't read it", clean(path)));
    }
    let under = |p: &str| {
        let p = p.trim();
        !p.is_empty() && {
            let p = p.trim_end_matches('/');
            path == p || path.starts_with(&format!("{p}/"))
        }
    };
    let env_like = name == ".env" || name.ends_with(".env");
    if DEFAULT_PATHS.iter().any(|p| under(p)) || extra.iter().any(|p| under(p)) || ANYWHERE.contains(&name) || env_like
    {
        Ok(())
    } else {
        Err(format!(
            "{} isn't in a folder DRIFT may read ({} or the host's own list)",
            clean(path),
            DEFAULT_PATHS.join(", ")
        ))
    }
}

/// `'x'`, or `"$HOME"/'x'` for `~/x` (the name was validated: no quotes).
fn quote(s: &str) -> String {
    match s.strip_prefix("~/") {
        Some(rest) => format!("\"$HOME\"/'{rest}'"),
        None => format!("'{s}'"),
    }
}

/// Resolves symlinks first, so a link can't point a read at a key.
pub fn resolve_command(path: &str) -> String {
    format!("readlink -f -- {} 2>/dev/null || realpath -- {} 2>/dev/null", quote(path), quote(path))
}

/// The fixed command for a lookup, or why it isn't allowed. For files, pass
/// the path `resolve_command` returned (or the asked one when it printed nothing).
pub fn command(l: &Lookup, extra: &[String]) -> Result<String, String> {
    let target = l.target.as_deref().map(str::trim).filter(|t| !t.is_empty());
    if l.kind.needs_target() && target.is_none() {
        return Err("this check needs a name or a path".into());
    }
    let t = target.unwrap_or("");
    match l.kind {
        LookupKind::File => path_allowed(t, extra)?,
        k if k.needs_target() => check_name(t)?,
        _ => {}
    }
    let q = quote(t);
    let cmd: String = match l.kind {
        LookupKind::Overview => "uptime; echo; df -hP 2>/dev/null | grep -vE '^(tmpfs|devtmpfs|overlay|shm|udev|efivarfs|none) '; echo; free -m 2>/dev/null || vm_stat 2>/dev/null | head -n 8".into(),
        LookupKind::Disk => "df -hP; echo; df -iP 2>/dev/null | grep -vE '^(tmpfs|devtmpfs|overlay|shm|udev) '".into(),
        LookupKind::Memory => "free -m 2>/dev/null || vm_stat | head -n 8; echo; ps aux | head -n 1; ps aux | sort -nrk 6 | head -n 10".into(),
        LookupKind::Processes => "uptime; echo; ps aux | head -n 1; ps aux | sort -nrk 3 | head -n 12".into(),
        LookupKind::Ports => "ss -tuln 2>/dev/null || netstat -an | grep -i listen".into(),
        LookupKind::Network => "ip -br addr 2>/dev/null || ifconfig | grep -E 'flags|inet'; echo; ip route 2>/dev/null || netstat -rn".into(),
        LookupKind::FailedUnits => "systemctl --failed --no-pager --no-legend --plain".into(),
        LookupKind::Errors => "journalctl -p err -n 40 --no-pager -o short-iso --since '24 hours ago'".into(),
        LookupKind::Containers => "docker ps -a --format '{{.Names}}\\t{{.Image}}\\t{{.Status}}'".into(),
        LookupKind::Container => format!(
            "docker inspect --format 'name={{{{.Name}}}} image={{{{.Config.Image}}}} state={{{{.State.Status}}}} health={{{{if .State.Health}}}}{{{{.State.Health.Status}}}}{{{{else}}}}none{{{{end}}}} restarts={{{{.RestartCount}}}} policy={{{{.HostConfig.RestartPolicy.Name}}}} exit={{{{.State.ExitCode}}}} oom={{{{.State.OOMKilled}}}} started={{{{.State.StartedAt}}}} finished={{{{.State.FinishedAt}}}} error={{{{.State.Error}}}}{{{{println}}}}ports={{{{json .NetworkSettings.Ports}}}}{{{{println}}}}mounts={{{{range .Mounts}}}}{{{{.Source}}}}:{{{{.Destination}}}} {{{{end}}}}' {q}"
        ),
        LookupKind::ContainerLogs => format!("docker logs --tail 50 --timestamps {q} 2>&1"),
        LookupKind::Service => format!("systemctl status {q} --no-pager -l -n 0"),
        LookupKind::ServiceLogs => format!("journalctl -u {q} -n 50 --no-pager -o short-iso"),
        LookupKind::File => {
            let lower = t.to_ascii_lowercase();
            if lower.starts_with("/var/log/") || lower.ends_with(".log") {
                format!("tail -n 60 -- {q}")
            } else {
                format!("head -c {FILE_BYTES} -- {q}")
            }
        }
        LookupKind::ComposeProjects => "docker compose ls --all 2>/dev/null || docker-compose ls --all".into(),
        LookupKind::System => "uname -srvm; grep -E '^(PRETTY_NAME|VERSION)=' /etc/os-release 2>/dev/null || sw_vers 2>/dev/null; hostname; systemd-detect-virt 2>/dev/null; uptime; [ -f /var/run/reboot-required ] && echo 'reboot required'; ls /var/run/reboot-required.pkgs >/dev/null 2>&1 && cat /var/run/reboot-required.pkgs | head -n 10; true".into(),
        LookupKind::KernelLog => "journalctl -k -p warning -n 40 --no-pager -o short-iso --since '48 hours ago' 2>/dev/null || dmesg -T --level=err,warn 2>/dev/null | tail -n 40".into(),
        LookupKind::Timers => "systemctl list-timers --all --no-pager".into(),
        LookupKind::Cron => "crontab -l 2>&1 | grep -v '^#'; echo; ls -l /etc/cron.d /etc/cron.daily /etc/cron.hourly /etc/cron.weekly 2>/dev/null; grep -v '^#' /etc/crontab 2>/dev/null".into(),
        LookupKind::Mounts => "findmnt -rno TARGET,SOURCE,FSTYPE,OPTIONS -t noproc,sysfs,cgroup2,cgroup,tmpfs,devtmpfs,overlay,squashfs,debugfs,tracefs,mqueue,hugetlbfs,securityfs,pstore,bpf,configfs,fusectl,autofs,binfmt_misc,efivarfs,devpts,nsfs,ramfs,rpc_pipefs 2>/dev/null || mount".into(),
        LookupKind::Storage => "lsblk -o NAME,SIZE,TYPE,FSTYPE,MOUNTPOINT,MODEL 2>/dev/null || diskutil list; echo; cat /proc/mdstat 2>/dev/null; zpool status -x 2>/dev/null; zpool list 2>/dev/null; btrfs filesystem show 2>/dev/null | head -n 20; true".into(),
        LookupKind::Sensors => "sensors 2>/dev/null || cat /sys/class/thermal/thermal_zone*/temp 2>/dev/null; nvidia-smi --query-gpu=name,temperature.gpu,utilization.gpu,memory.used,memory.total --format=csv 2>/dev/null; true".into(),
        LookupKind::Logins => "who; echo; last -n 15 2>/dev/null; echo; lastb -n 5 2>/dev/null; true".into(),
        LookupKind::Updates => "apt list --upgradable 2>/dev/null | head -n 40 || true; command -v dnf >/dev/null && dnf -q --cacheonly check-update 2>/dev/null | head -n 40; command -v rpm-ostree >/dev/null && rpm-ostree status 2>/dev/null | head -n 20; true".into(),
        LookupKind::Services => "systemctl list-units --type=service --state=running --no-pager --no-legend --plain".into(),
        LookupKind::Connections => "ss -s 2>/dev/null; echo; ss -tn state established 2>/dev/null | awk 'NR>1{p=$4; sub(/:[0-9]+$/, \"\", p); print p}' | sort | uniq -c | sort -nr | head -n 10; true".into(),
        LookupKind::DockerStats => "docker stats --no-stream --format '{{.Name}}\\t{{.CPUPerc}}\\t{{.MemUsage}}\\t{{.NetIO}}\\t{{.BlockIO}}'".into(),
        LookupKind::DockerEvents => "docker events --since 1h --until 0s --format '{{.Time}} {{.Type}} {{.Action}} {{.Actor.Attributes.name}}' 2>&1 | grep -vE ' (exec_create|exec_start|exec_die) ' | tail -n 40".into(),
        LookupKind::DockerDisk => "docker system df".into(),
    };
    // Synology keeps docker in /usr/local/bin, outside non-interactive PATH.
    Ok(if cmd.contains("docker") { format!("{}{cmd}", crate::docker::PATH_FIX) } else { cmd })
}

/// Lists container and service names, to match the question's words against.
pub const NAMES_COMMAND: &str = "PATH=\"$PATH:/usr/local/bin\"; echo '#containers'; docker ps -a --format '{{.Names}}' 2>/dev/null; echo '#units'; systemctl list-units --all --type=service --plain --no-legend --no-pager 2>/dev/null | awk '{print $1}'";

/// `(containers, units)` from [`NAMES_COMMAND`] output.
pub fn parse_names(out: &str) -> (Vec<String>, Vec<String>) {
    let (mut c, mut u) = (Vec::new(), Vec::new());
    let mut into = 0;
    for l in out.lines().map(str::trim) {
        match l {
            "#containers" => into = 1,
            "#units" => into = 2,
            "" => {}
            n if NAME.is_match(n) => match into {
                1 => c.push(n.to_string()),
                2 => u.push(n.to_string()),
                _ => {}
            },
            _ => {}
        }
    }
    (c, u)
}

/// Checks for the containers and services the question's words name: the
/// state and recent logs of each (at most two targets).
pub fn targets(terms: &[String], containers: &[String], units: &[String]) -> Vec<Lookup> {
    let mut out = Vec::new();
    let mut hits = 0;
    for t in terms.iter().map(|t| t.to_ascii_lowercase()) {
        if hits >= 2 || t.len() < 3 {
            continue;
        }
        let pick = |names: &[String]| -> Option<String> {
            names
                .iter()
                .find(|n| n.to_ascii_lowercase() == t)
                .or_else(|| {
                    names.iter().find(|n| {
                        let n = n.to_ascii_lowercase();
                        let base = n.strip_suffix(".service").unwrap_or(&n);
                        base == t || base.split(['-', '_', '.', '@']).any(|w| w == t)
                    })
                })
                .cloned()
        };
        if let Some(c) = pick(containers) {
            out.push(Lookup::new(LookupKind::Container, Some(&c)));
            out.push(Lookup::new(LookupKind::ContainerLogs, Some(&c)));
            hits += 1;
        } else if let Some(u) = pick(units) {
            out.push(Lookup::new(LookupKind::Service, Some(&u)));
            out.push(Lookup::new(LookupKind::ServiceLogs, Some(&u)));
            hits += 1;
        }
    }
    out
}

static KEY_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?(-----END [A-Z ]*PRIVATE KEY-----|\z)").unwrap()
});
static BEARER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(bearer|token|basic)\s+[A-Za-z0-9._~+/=-]{12,}").unwrap());
static URL_PASS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\b[a-z][a-z0-9+.-]*://[^:/\s@]+:)[^@\s]+@").unwrap());
static ASSIGN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(\b[A-Za-z0-9_.-]*(?:pass(?:word|wd)?|secret|token|api[_-]?key|access[_-]?key|private[_-]?key|auth|credential|pwd)[A-Za-z0-9_.-]*\s*[:=]\s*)("[^"\n]*"|'[^'\n]*'|[^\s,;]+)"#).unwrap()
});
static ENV_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*(?:export\s+)?[A-Za-z_][A-Za-z0-9_.-]*\s*=\s*)(.+)$").unwrap());

/// Masks secrets in command output: private keys, bearer tokens, URL
/// passwords and `key=value` for secret-ish names. A line that still looks
/// like it holds a secret is dropped whole.
pub fn mask(text: &str) -> String {
    let t = KEY_BLOCK.replace_all(text, "[private key removed]");
    let t = BEARER.replace_all(&t, format!("$1 {MASK}"));
    let t = URL_PASS.replace_all(&t, format!("${{1}}{MASK}@"));
    let t = ASSIGN.replace_all(&t, format!("${{1}}{MASK}"));
    t.lines()
        .map(|l| if line_has_secret(l) { "[line hidden: it looked like it held a secret]" } else { l })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Masks every value in an env file (`KEY=value` → `KEY=••••`).
pub fn mask_env(text: &str) -> String {
    text.lines()
        .map(|l| {
            if l.trim_start().starts_with('#') {
                l.to_string()
            } else {
                ENV_LINE.replace(l, format!("${{1}}{MASK}")).into_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Masks and clips one lookup's output for the prompt.
pub fn finish(l: &Lookup, out: &str) -> (String, bool) {
    let out = out.replace('\r', "");
    let name = l.target.as_deref().unwrap_or("").rsplit('/').next().unwrap_or("").to_ascii_lowercase();
    let masked = if l.kind == LookupKind::File && (name == ".env" || name.ends_with(".env")) {
        mask_env(&out)
    } else {
        mask(&out)
    };
    let masked = masked.trim_end().to_string();
    let n = masked.chars().count();
    if n <= MAX_OUTPUT {
        return (masked, false);
    }
    let kept: String = if l.kind.keeps_tail() {
        let skip = n - MAX_OUTPUT;
        let s: String = masked.chars().skip(skip).collect();
        // Start at a whole line.
        s.split_once('\n').map(|(_, r)| r.to_string()).unwrap_or(s)
    } else {
        let s: String = masked.chars().take(MAX_OUTPUT).collect();
        s.rsplit_once('\n').map(|(l, _)| l.to_string()).unwrap_or(s)
    };
    (kept, true)
}

/// Extra readable prefixes from the host's setting (one per line).
pub fn extra_paths(setting: Option<&str>) -> Vec<String> {
    setting
        .unwrap_or("")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && (l.starts_with('/') || l.starts_with("~/")))
        .map(str::to_string)
        .collect()
}

/// De-duplicates and caps a batch of lookups (named checks first).
pub fn plan(mut asked: Vec<Lookup>, found: Vec<Lookup>) -> Vec<Lookup> {
    asked.extend(found);
    let mut out: Vec<Lookup> = Vec::new();
    for l in asked {
        if !out.contains(&l) {
            out.push(l);
        }
    }
    // A container/service check makes the general list redundant.
    if out.iter().any(|l| l.kind == LookupKind::Container) {
        out.retain(|l| l.kind != LookupKind::Containers);
    }
    if out.iter().any(|l| l.kind == LookupKind::Service) {
        out.retain(|l| l.kind != LookupKind::FailedUnits);
    }
    // Targeted checks are worth more than the general ones.
    out.sort_by_key(|l| !l.kind.needs_target());
    out.truncate(MAX_LOOKUPS);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(kind: LookupKind, t: Option<&str>) -> Lookup {
        Lookup::new(kind, t)
    }

    #[test]
    fn every_command_is_read_only_and_never_elevated() {
        for k in LookupKind::ALL {
            let t = match k {
                LookupKind::File => Some("/etc/hostname"),
                k if k.needs_target() => Some("web"),
                _ => None,
            };
            let c = command(&l(k, t), &[]).unwrap();
            for bad in [
                "sudo",
                "doas",
                " su ",
                "pkexec",
                "run0",
                " rm ",
                "docker restart",
                "systemctl restart",
                " stop ",
                " kill",
                "exec ",
                " > ",
                ">>",
                "tee ",
                "sed -i",
            ] {
                assert!(!c.contains(bad), "{k:?}: {c} contains {bad}");
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn every_command_parses_in_sh() {
        for k in LookupKind::ALL {
            let t = match k {
                LookupKind::File => Some("~/app/compose.yaml"),
                k if k.needs_target() => Some("web"),
                _ => None,
            };
            let c = command(&l(k, t), &[]).unwrap();
            let ok = std::process::Command::new("sh").args(["-n", "-c", &c]).status().unwrap().success();
            assert!(ok, "{k:?}: {c}");
        }
        assert!(std::process::Command::new("sh").args(["-n", "-c", NAMES_COMMAND]).status().unwrap().success());
        assert!(std::process::Command::new("sh")
            .args(["-n", "-c", &resolve_command("/etc/x")])
            .status()
            .unwrap()
            .success());
    }

    /// `cargo test -p nightgrid-core --lib lookup::tests::print -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn print() {
        for k in LookupKind::ALL {
            let t = match k {
                LookupKind::File => Some("/etc/hostname"),
                k if k.needs_target() => Some("web"),
                _ => None,
            };
            println!("{}", command(&l(k, t), &[]).unwrap());
        }
    }

    #[test]
    fn targets_are_validated_and_quoted() {
        assert!(command(&l(LookupKind::Container, Some("web; rm -rf /")), &[]).is_err());
        assert!(command(&l(LookupKind::Service, Some("$(id)")), &[]).is_err());
        assert!(command(&l(LookupKind::ServiceLogs, Some("nginx'")), &[]).is_err());
        assert!(command(&l(LookupKind::ContainerLogs, None), &[]).is_err());
        assert_eq!(
            command(&l(LookupKind::ServiceLogs, Some("nginx.service")), &[]).unwrap(),
            "journalctl -u 'nginx.service' -n 50 --no-pager -o short-iso"
        );
        assert!(command(&l(LookupKind::Container, Some("web")), &[]).unwrap().contains("{{.State.Status}}"));
        assert!(!command(&l(LookupKind::Container, Some("web")), &[]).unwrap().contains("Env"));
        assert_eq!(
            command(&l(LookupKind::File, Some("~/app/compose.yaml")), &[]).unwrap(),
            "head -c 6000 -- \"$HOME\"/'app/compose.yaml'"
        );
        assert_eq!(
            command(&l(LookupKind::File, Some("/var/log/syslog")), &[]).unwrap(),
            "tail -n 60 -- '/var/log/syslog'"
        );
    }

    #[test]
    fn paths_are_allow_listed() {
        let extra = vec!["/data/apps".to_string()];
        for ok in [
            "/etc/nginx/nginx.conf",
            "/var/log/syslog",
            "/opt/stack/compose.yaml",
            "/home/me/app/docker-compose.yml",
            "/home/me/app/.env",
            "/data/apps/config.yml",
            "~/stack/.env",
        ] {
            assert!(path_allowed(ok, &extra).is_ok(), "{ok}");
        }
        for bad in [
            "/etc/shadow",
            "/etc/gshadow",
            "/etc/sudoers",
            "/etc/sudoers.d/me",
            "/etc/ssl/private/site.key",
            "/etc/letsencrypt/live/x/privkey.pem",
            "/home/me/.ssh/id_ed25519",
            "/home/me/.ssh/config",
            "~/.docker/config.json",
            "/opt/traefik/acme.json",
            "/proc/1/environ",
            "/etc/../root/x",
            "/home/me/notes.txt",
            "/data/appsX/y",
            "relative/path",
            "/etc/x'y",
            "/srv/app/secrets.yml",
            "/srv/app/db_password",
        ] {
            assert!(path_allowed(bad, &extra).is_err(), "{bad}");
        }
    }

    #[test]
    fn masks_secrets() {
        let m = mask("DB_PASSWORD=hunter22\nAuthorization: Bearer abcdef0123456789\nurl postgres://u:pw1234@db/x\nfine line\n-----BEGIN OPENSSH PRIVATE KEY-----\nAAA\n-----END OPENSSH PRIVATE KEY-----");
        assert!(
            !m.contains("hunter22") && !m.contains("abcdef0123456789") && !m.contains("pw1234") && !m.contains("AAA"),
            "{m}"
        );
        assert!(m.contains("fine line"));
        let e = mask_env("# comment\nAPP_PORT=8080\nexport DB_URL=postgres://x\n");
        assert_eq!(e, "# comment\nAPP_PORT=••••\nexport DB_URL=••••");
        let (out, _) = finish(&l(LookupKind::File, Some("/opt/app/.env")), "A=1\nB=2");
        assert_eq!(out, "A=••••\nB=••••");
    }

    #[test]
    fn clips_logs_from_the_end() {
        let long: String = (0..400).map(|i| format!("line {i}\n")).collect();
        let (out, clipped) = finish(&l(LookupKind::ContainerLogs, Some("web")), &long);
        assert!(clipped && out.ends_with("line 399") && out.chars().count() <= MAX_OUTPUT);
        let (out, clipped) = finish(&l(LookupKind::Disk, None), &long);
        assert!(clipped && out.starts_with("line 0") && out.chars().count() <= MAX_OUTPUT);
    }

    #[test]
    fn finds_named_containers_and_units() {
        let (c, u) = parse_names("#containers\nweb\nnginx-proxy\n#units\nssh.service\nnginx.service\ncron.service\n");
        assert_eq!(c, ["web", "nginx-proxy"]);
        let got = targets(&["nginx".into(), "cron".into(), "web".into()], &c, &u);
        assert_eq!(got[0], l(LookupKind::Container, Some("nginx-proxy")));
        assert_eq!(got[2], l(LookupKind::Service, Some("cron.service")));
        assert_eq!(got.len(), 4, "two targets at most");
    }

    #[test]
    fn plans_are_capped_and_deduped() {
        let p = plan(
            vec![l(LookupKind::Containers, None), l(LookupKind::Disk, None), l(LookupKind::Disk, None)],
            vec![l(LookupKind::Container, Some("web")), l(LookupKind::ContainerLogs, Some("web"))],
        );
        assert_eq!(
            p,
            vec![
                l(LookupKind::Container, Some("web")),
                l(LookupKind::ContainerLogs, Some("web")),
                l(LookupKind::Disk, None)
            ]
        );
        let many: Vec<_> = LookupKind::ALL.iter().filter(|k| !k.needs_target()).map(|k| l(*k, None)).collect();
        assert_eq!(plan(many, vec![]).len(), MAX_LOOKUPS);
    }

    #[test]
    fn extra_paths_from_setting() {
        assert_eq!(extra_paths(Some("/data\n# note\nrelative\n~/apps\n")), ["/data", "~/apps"]);
    }
}
