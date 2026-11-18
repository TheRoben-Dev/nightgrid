//! Library quests (M12e): a guide's steps checked against a real host,
//! read only. A step names a check with `{check: id}` at the end of its
//! line; the id is one of a fixed set here (some take one plain name, like
//! `container:vaultwarden`). Every check is a fixed command that changes
//! nothing and never uses sudo. What can't be read without root comes back
//! "unknown", never a guess, and output is reduced to a short evidence line
//! (counts and setting values, never file contents).

use serde::{Deserialize, Serialize};

use crate::files::path::shell_quote;
use crate::helper::lookup::check_name;

/// At most this many checks in one run.
pub const MAX_CHECKS: usize = 40;
const MARK: &str = "@@q";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum QuestState {
    Pass,
    Fail,
    /// Couldn't tell (needs root, tool missing, host didn't answer).
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct QuestResult {
    /// The check as written in the guide (`ssh-no-password`, `container:caddy`).
    pub id: String,
    pub state: QuestState,
    /// What was seen, in a few words.
    pub detail: String,
}

/// A check, parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    /// `PasswordAuthentication no` (and keyboard-interactive off).
    SshNoPassword,
    /// `PermitRootLogin no` or `prohibit-password`.
    SshNoRootPassword,
    /// `PermitRootLogin no`.
    SshNoRoot,
    /// The login user has at least one key in `authorized_keys`.
    SshKey,
    /// ufw, firewalld or nftables is active.
    Firewall,
    /// Unattended upgrades, dnf-automatic or rpm-ostree automatic updates.
    AutoUpdates,
    Docker,
    /// The login user may use Docker without sudo.
    DockerGroup,
    /// The clock is synchronised (NTP).
    TimeSync,
    Tailscale,
    /// A systemd timer whose name mentions restic, borg, backup or rsnapshot.
    BackupTimer,
    Swap,
    Fail2ban,
    /// No `/var/run/reboot-required`.
    NoRebootPending,
    /// No database, cache or admin port listens on every address (1.1).
    NoExposedDb,
    /// Docker's unencrypted API (TCP 2375) isn't listening (1.1).
    NoDockerTcp,
    /// No running container's image is older than [`IMAGE_MAX_DAYS`] (1.1).
    ImagesFresh,
    /// No security updates waiting (apt or dnf, from their caches) (1.1).
    SecurityUpdates,
    /// A container with this name is running.
    Container(String),
    /// A systemd unit with this name is active.
    Service(String),
    /// Something listens on this TCP port.
    Port(u16),
    /// This command is installed.
    Command(String),
}

impl Probe {
    pub fn parse(id: &str) -> Result<Probe, String> {
        let id = id.trim();
        if let Some((kind, arg)) = id.split_once(':') {
            let arg = arg.trim();
            return match kind.trim() {
                "container" => check_name(arg).map(|_| Probe::Container(arg.into())),
                "service" => check_name(arg).map(|_| Probe::Service(arg.into())),
                "command"
                    if arg.chars().all(|c| c.is_ascii_alphanumeric() || "-_.+".contains(c))
                        && !arg.is_empty()
                        && arg.len() <= 64 =>
                {
                    Ok(Probe::Command(arg.into()))
                }
                "port" => arg
                    .parse::<u16>()
                    .ok()
                    .filter(|p| *p > 0)
                    .map(Probe::Port)
                    .ok_or_else(|| format!("{arg} isn't a port")),
                _ => Err(format!("unknown check {id:?}")),
            };
        }
        Ok(match id {
            "ssh-no-password" => Probe::SshNoPassword,
            "ssh-no-root-password" => Probe::SshNoRootPassword,
            "ssh-no-root" => Probe::SshNoRoot,
            "ssh-key" => Probe::SshKey,
            "firewall" => Probe::Firewall,
            "auto-updates" => Probe::AutoUpdates,
            "docker" => Probe::Docker,
            "docker-group" => Probe::DockerGroup,
            "time-sync" => Probe::TimeSync,
            "tailscale" => Probe::Tailscale,
            "backup-timer" => Probe::BackupTimer,
            "swap" => Probe::Swap,
            "fail2ban" => Probe::Fail2ban,
            "no-reboot-pending" => Probe::NoRebootPending,
            "no-exposed-db" => Probe::NoExposedDb,
            "no-docker-tcp" => Probe::NoDockerTcp,
            "images-fresh" => Probe::ImagesFresh,
            "security-updates" => Probe::SecurityUpdates,
            _ => return Err(format!("unknown check {id:?}")),
        })
    }

    /// The fixed, read-only shell for this check.
    fn command(&self) -> String {
        match self {
            // sshd's effective order: Include lines expanded in place, first
            // value wins, Match blocks ignored. Unreadable files are marked.
            Probe::SshNoPassword | Probe::SshNoRootPassword | Probe::SshNoRoot => SSHD.into(),
            Probe::SshKey => "f=\"$HOME/.ssh/authorized_keys\"; if [ -r \"$f\" ]; then echo \"keys $(grep -cE '^[^#]*(ssh-|ecdsa-|sk-)' \"$f\")\"; else echo 'keys 0'; fi".into(),
            Probe::Firewall => "for u in ufw firewalld nftables; do echo \"$u $(systemctl is-active $u 2>/dev/null)\"; done; grep -s '^ENABLED=' /etc/ufw/ufw.conf".into(),
            Probe::AutoUpdates => "grep -hs 'Unattended-Upgrade' /etc/apt/apt.conf.d/20auto-upgrades /etc/apt/apt.conf.d/10periodic; echo \"dnf $(systemctl is-enabled dnf-automatic.timer dnf-automatic-install.timer dnf5-automatic.timer 2>/dev/null | tr '\\n' ' ')\"; grep -s '^AutomaticUpdatePolicy' /etc/rpm-ostreed.conf".into(),
            Probe::Docker => "command -v docker >/dev/null 2>&1 && echo 'have docker'; echo \"state $(systemctl is-active docker 2>/dev/null)\"".into(),
            Probe::DockerGroup => "echo \"groups $(id -nG)\"; echo \"uid $(id -u)\"".into(),
            Probe::TimeSync => "echo \"sync $(timedatectl show -p NTPSynchronized --value 2>/dev/null)\"".into(),
            Probe::Tailscale => "if command -v tailscale >/dev/null 2>&1; then tailscale status --peers=false >/dev/null 2>&1; echo \"ts $?\"; else echo 'ts missing'; fi".into(),
            Probe::BackupTimer => "systemctl list-timers --all --no-legend 2>/dev/null | grep -oiE '[A-Za-z0-9@_.-]*(restic|borg|backup|rsnapshot)[A-Za-z0-9@_.-]*\\.timer' | head -5".into(),
            Probe::Swap => "echo \"swaps $(($(wc -l < /proc/swaps 2>/dev/null || echo 1) - 1))\"".into(),
            Probe::Fail2ban => "echo \"state $(systemctl is-active fail2ban 2>/dev/null)\"".into(),
            Probe::NoRebootPending => "if [ -e /var/run/reboot-required ]; then echo 'reboot yes'; else echo 'reboot no'; fi".into(),
            Probe::NoExposedDb | Probe::NoDockerTcp => {
                "if command -v ss >/dev/null 2>&1; then ss -Hltn 2>/dev/null | awk '{print $4}'; else echo '@@noss'; fi".into()
            }
            Probe::ImagesFresh => "echo \"now $(date +%s)\"; if command -v docker >/dev/null 2>&1; then docker ps --format '{{.Image}}' 2>&1 | sort -u | head -60 | while IFS= read -r i; do case \"$i\" in *ermission*|*rror*) echo \"@@denied\"; break ;; esac; printf 'img %s %s\\n' \"$(docker image inspect -f '{{.Created}}' \"$i\" 2>/dev/null | cut -c1-10)\" \"$i\"; done; else echo '@@nodocker'; fi".into(),
            Probe::SecurityUpdates => "if command -v apt-get >/dev/null 2>&1; then echo \"apt $(apt-get -s -o Debug::NoLocking=1 upgrade 2>/dev/null | grep -c '^Inst .*[Ss]ecurity')\"; elif command -v dnf >/dev/null 2>&1; then echo \"dnf $(dnf -q -C updateinfo list --security 2>/dev/null | grep -c .)\"; else echo 'none'; fi".into(),
            Probe::Container(n) => format!(
                "if command -v docker >/dev/null 2>&1; then docker inspect -f '{{{{.State.Status}}}}' {} 2>&1 | head -1; else echo 'no docker'; fi",
                shell_quote(n)
            ),
            Probe::Service(n) => format!("echo \"state $(systemctl is-active {} 2>/dev/null)\"", shell_quote(n)),
            Probe::Port(p) => format!(
                "if command -v ss >/dev/null 2>&1; then ss -Hltn 2>/dev/null | awk '{{print $4}}' | grep -cE ':{p}$'; else echo unknown; fi"
            ),
            Probe::Command(c) => format!("command -v {} >/dev/null 2>&1 && echo yes || echo no", shell_quote(c)),
        }
    }
}

const SSHD: &str = r#"emit() { if [ -r "$1" ]; then cat "$1"; else echo "@@denied $1"; fi; }
if [ ! -e /etc/ssh/sshd_config ]; then echo '@@nosshd'; else
while IFS= read -r l || [ -n "$l" ]; do
  case "$l" in
    [Ii]nclude[[:space:]]*) for p in ${l#*[[:space:]]}; do case "$p" in /*) ;; *) p="/etc/ssh/$p" ;; esac; for g in $p; do [ -e "$g" ] && emit "$g"; done; done ;;
    *) printf '%s\n' "$l" ;;
  esac
done < /etc/ssh/sshd_config
fi"#;

/// The whole run as one script: each check's output under its marker.
pub fn script(probes: &[(String, Probe)]) -> String {
    let mut s = String::new();
    for (id, p) in probes {
        s.push_str(&format!("echo {}\n( {} ) 2>/dev/null\n", shell_quote(&format!("{MARK} {id}")), p.command()));
    }
    s
}

/// Splits [`script`]'s output by marker.
pub fn sections(out: &str) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    for line in out.lines() {
        if let Some(id) = line.strip_prefix(MARK).and_then(|r| r.strip_prefix(' ')) {
            v.push((id.trim().to_owned(), String::new()));
        } else if let Some((_, body)) = v.last_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    v
}

/// First value of an sshd keyword, in order, stopping at `Match`.
/// `Err(file)` when an unreadable file came before any value.
fn sshd_value(cfg: &str, key: &str) -> Result<Option<String>, String> {
    for line in cfg.lines() {
        let l = line.trim();
        if let Some(f) = l.strip_prefix("@@denied ") {
            return Err(f.to_owned());
        }
        if l.starts_with('#') || l.is_empty() {
            continue;
        }
        let mut it = l.splitn(2, |c: char| c.is_whitespace() || c == '=');
        let k = it.next().unwrap_or("");
        if k.eq_ignore_ascii_case("match") {
            break;
        }
        if k.eq_ignore_ascii_case(key) {
            let v = it.next().unwrap_or("").trim_start_matches(|c: char| c.is_whitespace() || c == '=').trim();
            return Ok(Some(v.to_ascii_lowercase()));
        }
    }
    Ok(None)
}

fn r(id: &str, state: QuestState, detail: impl Into<String>) -> QuestResult {
    QuestResult { id: id.to_owned(), state, detail: detail.into() }
}

fn word<'a>(out: &'a str, key: &str) -> Option<&'a str> {
    out.lines().find_map(|l| l.trim().strip_prefix(key).map(str::trim))
}

/// Turns one check's output into a result.
pub fn judge(id: &str, p: &Probe, out: &str) -> QuestResult {
    use QuestState::*;
    match p {
        Probe::SshNoPassword | Probe::SshNoRootPassword | Probe::SshNoRoot => {
            if out.contains("@@nosshd") {
                return r(id, Unknown, "no /etc/ssh/sshd_config");
            }
            let key = if matches!(p, Probe::SshNoPassword) { "passwordauthentication" } else { "permitrootlogin" };
            match sshd_value(out, key) {
                Err(f) => r(id, Unknown, format!("{f} needs root to read")),
                Ok(v) => {
                    let (v, default) = match v {
                        Some(v) => (v, false),
                        None if key == "permitrootlogin" => ("prohibit-password".into(), true),
                        None => ("yes".into(), true),
                    };
                    let shown = format!("{key} {v}{}", if default { " (default)" } else { "" });
                    let ok = match p {
                        Probe::SshNoPassword => {
                            v == "no" && sshd_value(out, "kbdinteractiveauthentication").ok().flatten().is_none_or_no()
                        }
                        Probe::SshNoRootPassword => v == "no" || v == "prohibit-password" || v == "without-password",
                        _ => v == "no",
                    };
                    r(id, if ok { Pass } else { Fail }, shown)
                }
            }
        }
        Probe::SshKey => match word(out, "keys").and_then(|n| n.parse::<u32>().ok()) {
            Some(0) | None => r(id, Fail, "no keys in ~/.ssh/authorized_keys"),
            Some(n) => r(id, Pass, format!("{n} key{} in ~/.ssh/authorized_keys", if n == 1 { "" } else { "s" })),
        },
        Probe::Firewall => {
            let on: Vec<&str> =
                ["ufw", "firewalld", "nftables"].into_iter().filter(|u| word(out, u) == Some("active")).collect();
            let ufw_enabled = out.lines().any(|l| l.trim() == "ENABLED=yes");
            if !on.is_empty() && (on != ["ufw"] || ufw_enabled) {
                r(id, Pass, format!("{} active", on.join(", ")))
            } else if ufw_enabled {
                r(id, Pass, "ufw enabled")
            } else {
                r(id, Fail, "no ufw, firewalld or nftables running")
            }
        }
        Probe::AutoUpdates => {
            let apt = out.lines().any(|l| l.contains("Unattended-Upgrade") && l.contains("\"1\""));
            let dnf = word(out, "dnf").is_some_and(|w| w.split_whitespace().any(|s| s == "enabled"));
            let ostree = out
                .lines()
                .find_map(|l| l.trim().strip_prefix("AutomaticUpdatePolicy="))
                .is_some_and(|v| matches!(v.trim(), "stage" | "apply" | "check"));
            match (apt, dnf, ostree) {
                (true, _, _) => r(id, Pass, "unattended-upgrades on"),
                (_, true, _) => r(id, Pass, "dnf-automatic timer enabled"),
                (_, _, true) => r(id, Pass, "rpm-ostree automatic updates on"),
                _ => r(id, Fail, "no automatic updates set up"),
            }
        }
        Probe::Docker => match (out.contains("have docker"), word(out, "state")) {
            (true, Some("active")) => r(id, Pass, "docker installed and running"),
            (true, s) => r(
                id,
                Fail,
                format!("docker installed, service {}", s.filter(|s| !s.is_empty()).unwrap_or("not running")),
            ),
            (false, _) => r(id, Fail, "docker isn't installed"),
        },
        Probe::DockerGroup => {
            let root = word(out, "uid") == Some("0");
            let docker = word(out, "groups").is_some_and(|g| g.split_whitespace().any(|x| x == "docker"));
            if root || docker {
                r(id, Pass, if root { "logged in as root" } else { "in the docker group" })
            } else {
                r(id, Fail, "not in the docker group")
            }
        }
        Probe::TimeSync => match word(out, "sync") {
            Some("yes") => r(id, Pass, "clock synchronised"),
            Some("no") => r(id, Fail, "clock not synchronised"),
            _ => r(id, Unknown, "timedatectl not available"),
        },
        Probe::Tailscale => match word(out, "ts") {
            Some("0") => r(id, Pass, "tailscale connected"),
            Some("missing") => r(id, Fail, "tailscale isn't installed"),
            _ => r(id, Fail, "tailscale installed but not connected"),
        },
        Probe::BackupTimer => {
            let t: Vec<&str> = out.lines().map(str::trim).filter(|l| l.ends_with(".timer")).collect();
            if t.is_empty() {
                r(id, Fail, "no backup timer")
            } else {
                r(id, Pass, t.join(", "))
            }
        }
        Probe::Swap => match word(out, "swaps").and_then(|n| n.parse::<i32>().ok()) {
            Some(n) if n > 0 => r(id, Pass, format!("{n} swap device{}", if n == 1 { "" } else { "s" })),
            Some(_) => r(id, Fail, "no swap"),
            None => r(id, Unknown, "couldn't read /proc/swaps"),
        },
        Probe::Fail2ban | Probe::Service(_) => {
            let name = match p {
                Probe::Service(n) => n.as_str(),
                _ => "fail2ban",
            };
            match word(out, "state") {
                Some("active") => r(id, Pass, format!("{name} active")),
                Some(s) if !s.is_empty() => r(id, Fail, format!("{name} {s}")),
                _ => r(id, Fail, format!("{name} isn't installed")),
            }
        }
        Probe::NoRebootPending => match word(out, "reboot") {
            Some("no") => r(id, Pass, "no reboot pending"),
            Some("yes") => r(id, Fail, "a reboot is pending"),
            _ => r(id, Unknown, "couldn't tell"),
        },
        Probe::NoExposedDb => {
            if out.contains("@@noss") {
                return r(id, Unknown, "ss isn't available");
            }
            let open: Vec<String> = listening(out)
                .into_iter()
                .filter(|(all, port)| *all && RISKY_PORTS.iter().any(|(p, _)| p == port))
                .map(|(_, port)| {
                    let name = RISKY_PORTS.iter().find(|(p, _)| *p == port).map_or("", |(_, n)| *n);
                    format!("{name} {port}")
                })
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            if open.is_empty() {
                r(id, Pass, "no database or admin port on every address")
            } else {
                r(id, Fail, format!("on every address: {}", open.join(", ")))
            }
        }
        Probe::NoDockerTcp => {
            if out.contains("@@noss") {
                return r(id, Unknown, "ss isn't available");
            }
            if listening(out).iter().any(|(_, p)| *p == 2375) {
                r(id, Fail, "Docker's API listens on TCP 2375 without TLS")
            } else {
                r(id, Pass, "no unencrypted Docker API")
            }
        }
        Probe::ImagesFresh => {
            if out.contains("@@nodocker") {
                return r(id, Pass, "no docker");
            }
            if out.contains("@@denied") {
                return r(id, Unknown, "this login can't use docker");
            }
            let now = word(out, "now").and_then(|n| n.parse::<i64>().ok()).unwrap_or(0);
            let mut old: Vec<(i64, String)> = Vec::new();
            let mut n = 0;
            for l in out.lines().filter_map(|l| l.strip_prefix("img ")) {
                let (date, image) = l.split_once(' ').unwrap_or((l, ""));
                let Some(day) = days_from_iso(date) else { continue };
                n += 1;
                let age = now / 86_400 - day;
                if age > IMAGE_MAX_DAYS {
                    old.push((age, image.trim().to_owned()));
                }
            }
            old.sort_by_key(|o| std::cmp::Reverse(o.0));
            match (n, old.first()) {
                (0, _) => r(id, Pass, "no running containers"),
                (_, None) => r(
                    id,
                    Pass,
                    format!("{n} image{} built in the last {IMAGE_MAX_DAYS} days", if n == 1 { "" } else { "s" }),
                ),
                (_, Some((age, img))) => r(
                    id,
                    Fail,
                    format!("{} of {n} older than {IMAGE_MAX_DAYS} days, oldest {img} ({age} days)", old.len()),
                ),
            }
        }
        Probe::SecurityUpdates => {
            let n = word(out, "apt").or_else(|| word(out, "dnf")).and_then(|n| n.parse::<u32>().ok());
            match n {
                Some(0) => r(id, Pass, "no security updates waiting"),
                Some(n) => r(id, Fail, format!("{n} security update{} waiting", if n == 1 { "" } else { "s" })),
                None => r(id, Unknown, "no apt or dnf to ask"),
            }
        }
        Probe::Container(n) => {
            let l = out.lines().next().unwrap_or("").trim();
            if l == "running" {
                r(id, Pass, format!("{n} running"))
            } else if l == "no docker" {
                r(id, Fail, "docker isn't installed")
            } else if l.contains("permission denied") {
                r(id, Unknown, "this login can't use docker")
            } else if l.contains("No such") || l.is_empty() {
                r(id, Fail, format!("no container named {n}"))
            } else {
                r(id, Fail, format!("{n} {l}"))
            }
        }
        Probe::Port(port) => match out.trim().parse::<u32>() {
            Ok(0) => r(id, Fail, format!("nothing listens on {port}")),
            Ok(_) => r(id, Pass, format!("listening on {port}")),
            Err(_) => r(id, Unknown, "ss isn't available"),
        },
        Probe::Command(c) => match out.trim() {
            "yes" => r(id, Pass, format!("{c} installed")),
            _ => r(id, Fail, format!("{c} isn't installed")),
        },
    }
}

/// Images older than this many days count as outdated.
pub const IMAGE_MAX_DAYS: i64 = 180;

/// Ports that shouldn't listen on every address: databases, caches,
/// search and the Docker API.
pub const RISKY_PORTS: [(u16, &str); 10] = [
    (3306, "MySQL"),
    (5432, "PostgreSQL"),
    (6379, "Redis"),
    (27017, "MongoDB"),
    (9200, "Elasticsearch"),
    (11211, "memcached"),
    (5984, "CouchDB"),
    (8086, "InfluxDB"),
    (2375, "Docker API"),
    (2379, "etcd"),
];

/// `ss -Hltn` local addresses → (listens on every address, port).
fn listening(out: &str) -> Vec<(bool, u16)> {
    out.lines()
        .filter_map(|l| {
            let a = l.trim();
            let (host, port) = a.rsplit_once(':')?;
            let port = port.parse::<u16>().ok()?;
            let host = host.trim_start_matches('[').trim_end_matches(']');
            let host = host.split('%').next().unwrap_or(host);
            Some((matches!(host, "0.0.0.0" | "*" | "::" | ""), port))
        })
        .collect()
}

/// Days since 1970-01-01 for `YYYY-MM-DD`.
pub fn days_from_iso(s: &str) -> Option<i64> {
    let mut it = s.trim().splitn(3, '-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.get(..2)?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || y < 1970 {
        return None;
    }
    // Civil-from-days inverse (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

trait NoneOrNo {
    fn is_none_or_no(&self) -> bool;
}
impl NoneOrNo for Option<String> {
    fn is_none_or_no(&self) -> bool {
        self.as_deref().map_or(true, |v| v == "no")
    }
}

/// Parses the ids, deduplicated, at most [`MAX_CHECKS`]; bad ids come back
/// as unknown results right away.
pub fn plan(ids: &[String]) -> (Vec<(String, Probe)>, Vec<QuestResult>) {
    let mut ok: Vec<(String, Probe)> = Vec::new();
    let mut bad = Vec::new();
    for id in ids.iter().take(MAX_CHECKS) {
        if ok.iter().any(|(i, _)| i == id) || bad.iter().any(|b: &QuestResult| &b.id == id) {
            continue;
        }
        match Probe::parse(id) {
            Ok(p) => ok.push((id.clone(), p)),
            Err(e) => bad.push(r(id, QuestState::Unknown, e)),
        }
    }
    (ok, bad)
}

/// Results for every planned check, in order; a check whose output is
/// missing (the run was cut short) is unknown.
pub fn results(planned: &[(String, Probe)], out: &str) -> Vec<QuestResult> {
    let secs = sections(out);
    planned
        .iter()
        .map(|(id, p)| match secs.iter().find(|(i, _)| i == id) {
            Some((_, body)) => judge(id, p, body),
            None => r(id, QuestState::Unknown, "no answer"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use QuestState::*;

    fn j(id: &str, out: &str) -> QuestResult {
        judge(id, &Probe::parse(id).unwrap(), out)
    }

    #[test]
    fn parses_ids() {
        assert_eq!(Probe::parse("ssh-no-password"), Ok(Probe::SshNoPassword));
        assert_eq!(Probe::parse("container:vaultwarden"), Ok(Probe::Container("vaultwarden".into())));
        assert_eq!(Probe::parse("port:443"), Ok(Probe::Port(443)));
        assert!(Probe::parse("port:0").is_err());
        assert!(Probe::parse("container:a;rm -rf /").is_err());
        assert!(Probe::parse("command:ls /").is_err());
        assert!(Probe::parse("rm -rf /").is_err());
        let (ok, bad) = plan(&["swap".into(), "swap".into(), "nope".into()]);
        assert_eq!(ok.len(), 1);
        assert_eq!(bad[0].state, Unknown);
    }

    #[test]
    fn sshd_first_value_wins_and_includes_count() {
        let cfg = "# comment\nPasswordAuthentication no\nPermitRootLogin yes\nPasswordAuthentication yes\n";
        assert_eq!(j("ssh-no-password", cfg).state, Pass);
        assert_eq!(j("ssh-no-root", cfg).state, Fail);
        assert_eq!(j("ssh-no-root-password", cfg).state, Fail);
        // Defaults: passwords on, root by key only.
        let r = j("ssh-no-password", "Port 22\n");
        assert_eq!((r.state, r.detail.as_str()), (Fail, "passwordauthentication yes (default)"));
        assert_eq!(j("ssh-no-root-password", "").state, Pass);
        // An unreadable include before the setting: can't tell.
        let r = j("ssh-no-password", "@@denied /etc/ssh/sshd_config.d/50-cloud-init.conf\nPasswordAuthentication no\n");
        assert_eq!(r.state, Unknown);
        assert!(r.detail.contains("needs root"));
        // Match blocks don't count.
        assert_eq!(j("ssh-no-password", "Match User bob\nPasswordAuthentication no\n").state, Fail);
        // Keyboard-interactive left on still allows passwords.
        assert_eq!(j("ssh-no-password", "PasswordAuthentication no\nKbdInteractiveAuthentication yes\n").state, Fail);
        assert_eq!(j("ssh-no-password", "passwordauthentication=no\n").state, Pass);
        assert_eq!(j("ssh-no-root", "@@nosshd\n").state, Unknown);
    }

    #[test]
    fn judges_the_rest() {
        assert_eq!(j("ssh-key", "keys 2\n").detail, "2 keys in ~/.ssh/authorized_keys");
        assert_eq!(j("ssh-key", "keys 0\n").state, Fail);
        assert_eq!(j("firewall", "ufw active\nfirewalld inactive\nnftables inactive\nENABLED=yes\n").state, Pass);
        assert_eq!(j("firewall", "ufw active\nfirewalld inactive\nnftables inactive\nENABLED=no\n").state, Fail);
        assert_eq!(j("firewall", "ufw inactive\nfirewalld active\nnftables inactive\n").state, Pass);
        assert_eq!(j("auto-updates", "APT::Periodic::Unattended-Upgrade \"1\";\ndnf \n").state, Pass);
        assert_eq!(j("auto-updates", "dnf enabled \n").state, Pass);
        assert_eq!(j("auto-updates", "dnf disabled \nAutomaticUpdatePolicy=stage\n").state, Pass);
        assert_eq!(j("auto-updates", "APT::Periodic::Unattended-Upgrade \"0\";\ndnf \n").state, Fail);
        assert_eq!(j("docker", "have docker\nstate active\n").state, Pass);
        assert_eq!(j("docker", "state \n").detail, "docker isn't installed");
        assert_eq!(j("docker-group", "groups robin wheel docker\nuid 1000\n").state, Pass);
        assert_eq!(j("docker-group", "groups robin\nuid 1000\n").state, Fail);
        assert_eq!(j("time-sync", "sync yes\n").state, Pass);
        assert_eq!(j("time-sync", "sync \n").state, Unknown);
        assert_eq!(j("tailscale", "ts 0\n").state, Pass);
        assert_eq!(j("tailscale", "ts 1\n").state, Fail);
        assert_eq!(j("backup-timer", "restic-backup.timer\n").detail, "restic-backup.timer");
        assert_eq!(j("backup-timer", "").state, Fail);
        assert_eq!(j("swap", "swaps 1\n").state, Pass);
        assert_eq!(j("swap", "swaps 0\n").state, Fail);
        assert_eq!(j("fail2ban", "state inactive\n").detail, "fail2ban inactive");
        assert_eq!(j("no-reboot-pending", "reboot yes\n").state, Fail);
        assert_eq!(j("container:caddy", "running\n").state, Pass);
        assert_eq!(j("container:caddy", "Error: No such object: caddy\n").detail, "no container named caddy");
        assert_eq!(j("container:caddy", "permission denied while trying to connect\n").state, Unknown);
        assert_eq!(j("container:caddy", "exited\n").detail, "caddy exited");
        assert_eq!(j("service:caddy", "state active\n").state, Pass);
        assert_eq!(j("port:443", "1\n").state, Pass);
        assert_eq!(j("port:443", "0\n").state, Fail);
        assert_eq!(j("command:restic", "no\n").state, Fail);
    }

    #[test]
    fn judges_posture_checks() {
        let ss = "127.0.0.1:5432\n0.0.0.0:22\n[::]:6379\n*:3306\n192.168.1.5:9200\n";
        let r = j("no-exposed-db", ss);
        assert_eq!(r.state, Fail);
        assert_eq!(r.detail, "on every address: MySQL 3306, Redis 6379");
        assert_eq!(j("no-exposed-db", "127.0.0.1:5432\n0.0.0.0:22\n").state, Pass);
        assert_eq!(j("no-exposed-db", "@@noss\n").state, Unknown);
        assert_eq!(j("no-docker-tcp", "0.0.0.0:2375\n").state, Fail);
        assert_eq!(j("no-docker-tcp", "0.0.0.0:2376\n").state, Pass);
        let now = days_from_iso("2026-11-18").unwrap() * 86_400;
        let out = format!("now {now}\nimg 2026-10-01 caddy:2\nimg 2025-01-02 old/app:1\nimg  broken\n");
        let r = j("images-fresh", &out);
        assert_eq!(r.state, Fail);
        assert!(r.detail.starts_with("1 of 2 older than 180 days, oldest old/app:1"), "{}", r.detail);
        assert_eq!(j("images-fresh", &format!("now {now}\nimg 2026-10-01 caddy:2\n")).state, Pass);
        assert_eq!(j("images-fresh", "now 1\n@@nodocker\n").state, Pass);
        assert_eq!(j("images-fresh", "now 1\n@@denied\n").state, Unknown);
        assert_eq!(j("security-updates", "apt 0\n").state, Pass);
        assert_eq!(j("security-updates", "dnf 4\n").detail, "4 security updates waiting");
        assert_eq!(j("security-updates", "none\n").state, Unknown);
        assert_eq!(days_from_iso("1970-01-01"), Some(0));
        assert_eq!(days_from_iso("2000-03-01"), Some(11_017));
        assert_eq!(days_from_iso("2024-13-01"), None);
    }

    #[test]
    fn one_script_and_its_sections() {
        let (planned, _) = plan(&["swap".into(), "container:caddy".into(), "ssh-no-root".into()]);
        let s = script(&planned);
        assert!(s.contains("echo '@@q swap'"));
        assert!(s.contains("docker inspect -f '{{.State.Status}}' 'caddy'"));
        assert!(!s.contains("sudo"));
        for bad in ["rm ", " > /", "tee ", "sed -i", "chmod", "systemctl start", "systemctl stop", "docker run"] {
            assert!(!s.contains(bad), "{bad}");
        }
        let out = "@@q swap\nswaps 1\n@@q container:caddy\nrunning\n";
        let res = results(&planned, out);
        assert_eq!(res.iter().map(|r| r.state).collect::<Vec<_>>(), vec![Pass, Pass, Unknown]);
        assert_eq!(res[2].detail, "no answer");
    }

    /// Every check's script parses in `sh`.
    #[test]
    fn scripts_parse_in_sh() {
        let ids = [
            "ssh-no-password",
            "ssh-key",
            "firewall",
            "auto-updates",
            "docker",
            "docker-group",
            "time-sync",
            "tailscale",
            "backup-timer",
            "swap",
            "fail2ban",
            "no-reboot-pending",
            "no-exposed-db",
            "no-docker-tcp",
            "images-fresh",
            "security-updates",
            "container:x",
            "service:x",
            "port:22",
            "command:x",
        ];
        let (planned, bad) = plan(&ids.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert!(bad.is_empty());
        let s = script(&planned);
        let o = std::process::Command::new("sh").arg("-n").arg("-c").arg(&s).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }

    /// Every `{check: …}` in the Library names a real check, on a step line.
    #[test]
    fn library_checks_exist() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/kb");
        let mut seen = 0;
        for shelf in std::fs::read_dir(&root).unwrap().flatten() {
            if !shelf.path().is_dir() {
                continue;
            }
            for f in std::fs::read_dir(shelf.path()).unwrap().flatten() {
                let text = std::fs::read_to_string(f.path()).unwrap_or_default();
                for line in text.lines().filter(|l| l.contains("{check:")) {
                    let id = line.rsplit_once("{check:").unwrap().1.trim_end().trim_end_matches('}').trim();
                    assert!(Probe::parse(id).is_ok(), "{}: {id}", f.path().display());
                    let t = line.trim_start();
                    assert!(
                        t.starts_with("- [ ] ") || t.starts_with("- [x] "),
                        "{}: not a step: {line}",
                        f.path().display()
                    );
                    assert!(line.trim_end().ends_with('}'), "{}: the check goes at the end", f.path().display());
                    seen += 1;
                }
            }
        }
        assert!(seen >= 20, "only {seen} checks in the Library");
    }
}
