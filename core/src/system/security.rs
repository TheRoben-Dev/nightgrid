//! Security hygiene audit and listening ports (M10d). One unprivileged
//! script reads what a user can see: listening sockets (`ss`, or `netstat`),
//! the sshd config files, which firewall/intrusion services are active,
//! automatic updates, reboot-required, extra uid-0 accounts and Docker's
//! published ports. The checks and the score are computed here; nothing is
//! changed on the host. Suggested fixes are text for the user to run.

use serde::Serialize;

use super::{SystemError, SystemResult};
use crate::ssh::client::Connection;

/// One listening socket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Listener {
    /// `tcp` or `udp`.
    pub proto: String,
    /// Bound address as reported (`0.0.0.0`, `::`, `127.0.0.1`, `192.168.1.5`).
    pub addr: String,
    pub port: u16,
    pub scope: Scope,
    /// Process name, when visible to this user.
    pub process: Option<String>,
    /// Container publishing it, for Docker ports.
    pub container: Option<String>,
    /// Well-known service on this port (`ssh`, `postgres`), if any.
    pub service: Option<String>,
}

/// Who can reach a socket, judged from the bound address alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    /// Loopback only.
    Local,
    /// A VPN address (Tailscale/CGNAT 100.64.0.0/10, or a `wg`/`tailscale` style range we can tell).
    Vpn,
    /// One specific address (often the LAN).
    Address,
    /// Every interface (`0.0.0.0`, `::`, `*`).
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
    /// Couldn't tell (not readable without root, tool missing).
    Unknown,
    /// Shown for context; not scored.
    Info,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub id: String,
    pub title: String,
    pub status: CheckStatus,
    pub detail: String,
    /// What to do about it, in words, with commands in backticks. The user
    /// runs them; Nightgrid doesn't.
    pub fix: Option<String>,
    /// Points this check can cost.
    pub weight: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SecurityReport {
    /// 0–100.
    pub score: u32,
    /// `A` … `F`.
    pub grade: String,
    pub checks: Vec<Check>,
    pub listeners: Vec<Listener>,
    /// Process names are only visible for our own processes without root.
    pub processes_partial: bool,
}

/// Everything in one unprivileged round trip.
pub fn audit_command() -> String {
    super::sudo::sh(
        r##"LC_ALL=C; export LC_ALL
echo @@uid; id -u
echo @@ss
if command -v ss >/dev/null 2>&1; then ss -H -tulnp 2>/dev/null || ss -tuln 2>/dev/null | tail -n +2
elif command -v netstat >/dev/null 2>&1; then echo NETSTAT; netstat -tuln 2>/dev/null
else echo NONE; fi
echo @@sshd
for f in /etc/ssh/sshd_config.d/*.conf /etc/ssh/sshd_config; do [ -r "$f" ] && { echo "#file $f"; cat "$f"; }; done
echo @@services
for s in ufw firewalld nftables fail2ban crowdsec unattended-upgrades apt-daily-upgrade.timer dnf-automatic.timer dnf-automatic-install.timer; do echo "$s $(systemctl is-active "$s" 2>/dev/null || echo unknown) $(systemctl is-enabled "$s" 2>/dev/null || echo unknown)"; done
echo @@ufw; cat /etc/ufw/ufw.conf 2>/dev/null
echo @@aptauto; cat /etc/apt/apt.conf.d/20auto-upgrades 2>/dev/null
echo @@reboot; [ -e /var/run/reboot-required ] && echo yes
echo @@uid0; awk -F: '$3 == 0 { print $1 }' /etc/passwd 2>/dev/null
echo @@docker; command -v docker >/dev/null 2>&1 && docker ps --format '{{.Names}}|{{.Ports}}' 2>/dev/null
echo @@end
"##,
    )
}

pub async fn audit(conn: &Connection) -> SystemResult<SecurityReport> {
    let (_, out, _) = conn.exec(&audit_command()).await.map_err(SystemError::other)?;
    Ok(parse_audit(&String::from_utf8_lossy(&out)))
}

fn sections(out: &str) -> Vec<(&str, Vec<&str>)> {
    let mut v: Vec<(&str, Vec<&str>)> = Vec::new();
    for line in out.lines() {
        if let Some(name) = line.strip_prefix("@@") {
            v.push((name.trim(), Vec::new()));
        } else if let Some(last) = v.last_mut() {
            last.1.push(line);
        }
    }
    v
}

/// Well-known ports. Databases and caches are flagged when they listen on
/// every interface.
const SERVICES: &[(u16, &str, bool)] = &[
    (21, "ftp", false),
    (22, "ssh", false),
    (23, "telnet", false),
    (25, "smtp", false),
    (53, "dns", false),
    (80, "http", false),
    (111, "rpcbind", false),
    (139, "netbios", false),
    (443, "https", false),
    (445, "smb", false),
    (631, "cups", false),
    (873, "rsync", false),
    (1883, "mqtt", false),
    (2049, "nfs", false),
    (2375, "docker api (no TLS)", true),
    (3000, "web app", false),
    (3306, "mysql", true),
    (3389, "rdp", false),
    (5432, "postgres", true),
    (5900, "vnc", false),
    (5984, "couchdb", true),
    (6379, "redis", true),
    (8006, "proxmox", false),
    (8080, "http-alt", false),
    (8086, "influxdb", true),
    (8443, "https-alt", false),
    (9000, "portainer/minio", false),
    (9090, "prometheus", false),
    (9100, "node-exporter", false),
    (9200, "elasticsearch", true),
    (11211, "memcached", true),
    (27017, "mongodb", true),
    (32400, "plex", false),
];

pub fn service_name(port: u16) -> Option<&'static str> {
    SERVICES.iter().find(|(p, _, _)| *p == port).map(|(_, n, _)| *n)
}

pub(crate) fn is_sensitive(port: u16) -> bool {
    SERVICES.iter().any(|(p, _, s)| *p == port && *s)
}

pub fn scope_of(addr: &str) -> Scope {
    let a = addr.trim_matches(|c| c == '[' || c == ']');
    match a {
        "0.0.0.0" | "::" | "*" | "" => return Scope::All,
        "::1" | "localhost" => return Scope::Local,
        _ => {}
    }
    if a.starts_with("127.") || a.starts_with("::ffff:127.") {
        return Scope::Local;
    }
    let v4: Vec<u8> = a.trim_start_matches("::ffff:").split('.').filter_map(|p| p.parse().ok()).collect();
    if v4.len() == 4 && v4[0] == 100 && (64..128).contains(&v4[1]) {
        return Scope::Vpn;
    }
    // Tailscale's IPv6 range.
    if a.to_ascii_lowercase().starts_with("fd7a:115c:a1e0:") {
        return Scope::Vpn;
    }
    Scope::Address
}

/// `0.0.0.0:22`, `[::]:80`, `127.0.0.53%lo:53`, `*:22` → (addr, port).
fn split_addr(s: &str) -> Option<(String, u16)> {
    let i = s.rfind(':')?;
    let port = s[i + 1..].parse().ok()?;
    let mut addr = &s[..i];
    if let Some(p) = addr.find('%') {
        addr = &addr[..p];
    }
    Some((addr.trim_matches(|c| c == '[' || c == ']').to_owned(), port))
}

/// `users:(("sshd",pid=812,fd=3),("sshd",pid=900,fd=3))` → `sshd`.
fn process_of(s: &str) -> Option<String> {
    let start = s.find("((\"")? + 3;
    let end = s[start..].find('"')? + start;
    Some(s[start..end].to_owned())
}

pub fn parse_listeners(lines: &[&str]) -> Vec<Listener> {
    let netstat = lines.first().is_some_and(|l| l.trim() == "NETSTAT");
    let mut out: Vec<Listener> = Vec::new();
    for line in lines {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (proto, local, rest) = if netstat {
            // `tcp 0 0 0.0.0.0:22 0.0.0.0:* LISTEN`
            if f.len() < 4 || !(f[0].starts_with("tcp") || f[0].starts_with("udp")) {
                continue;
            }
            if f[0].starts_with("tcp") && !line.contains("LISTEN") {
                continue;
            }
            (&f[0][..3], f[3], "")
        } else {
            // `tcp LISTEN 0 4096 0.0.0.0:22 0.0.0.0:* users:(...)`
            if f.len() < 5 || !(f[0] == "tcp" || f[0] == "udp") {
                continue;
            }
            if f[0] == "tcp" && f[1] != "LISTEN" {
                continue;
            }
            (f[0], f[4], f.get(6).copied().unwrap_or(""))
        };
        let Some((addr, port)) = split_addr(local) else { continue };
        let l = Listener {
            proto: proto.to_owned(),
            scope: scope_of(&addr),
            addr,
            port,
            process: process_of(rest),
            container: None,
            service: service_name(port).map(str::to_owned),
        };
        // `0.0.0.0` and `::` for the same port are one service.
        if !out.iter().any(|o| o.proto == l.proto && o.port == l.port && o.scope == l.scope && o.process == l.process) {
            out.push(l);
        }
    }
    out
}

/// `docker ps` lines `name|0.0.0.0:8080->80/tcp, :::8080->80/tcp, 5432/tcp`
/// → published host ports.
pub fn parse_docker(lines: &[&str]) -> Vec<Listener> {
    let mut out: Vec<Listener> = Vec::new();
    for line in lines {
        let Some((name, ports)) = line.split_once('|') else { continue };
        for p in ports.split(',').map(str::trim) {
            let Some((host, cont)) = p.split_once("->") else { continue };
            let proto = cont.rsplit_once('/').map_or("tcp", |(_, pr)| pr);
            let Some((addr, port)) = split_addr(host) else { continue };
            let addr = if addr.is_empty() || addr == "::" { "0.0.0.0".to_owned() } else { addr };
            let l = Listener {
                proto: proto.to_owned(),
                scope: scope_of(&addr),
                addr,
                port,
                process: Some("docker-proxy".into()),
                container: Some(name.trim().to_owned()),
                service: service_name(port).map(str::to_owned),
            };
            if !out.iter().any(|o| o.proto == l.proto && o.port == l.port && o.container == l.container) {
                out.push(l);
            }
        }
    }
    out
}

/// The effective value of an sshd option: first occurrence wins, `Match`
/// blocks are skipped (they only apply to some connections).
pub fn sshd_option(lines: &[&str], key: &str) -> Option<String> {
    let mut in_match = false;
    for l in lines {
        let t = l.trim();
        if t.starts_with("#file ") {
            in_match = false;
            continue;
        }
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let mut it = t.splitn(2, |c: char| c.is_whitespace() || c == '=');
        let k = it.next().unwrap_or("");
        if k.eq_ignore_ascii_case("match") {
            in_match = true;
            continue;
        }
        if in_match {
            continue;
        }
        if k.eq_ignore_ascii_case(key) {
            return it.next().map(|v| v.trim().trim_start_matches('=').trim().to_ascii_lowercase());
        }
    }
    None
}

struct Svc {
    active: bool,
    enabled: bool,
    known: bool,
}

fn services<'a>(lines: &'a [&'a str]) -> impl Fn(&str) -> Svc + 'a {
    move |name: &str| {
        let row = lines.iter().find_map(|l| {
            let mut it = l.split_whitespace();
            (it.next()? == name)
                .then(|| (it.next().unwrap_or("unknown").to_owned(), it.next().unwrap_or("unknown").to_owned()))
        });
        match row {
            Some((a, e)) => Svc {
                active: a == "active",
                enabled: e == "enabled" || e == "static" || e == "enabled-runtime",
                known: a != "unknown" || e != "unknown",
            },
            None => Svc { active: false, enabled: false, known: false },
        }
    }
}

pub fn parse_audit(out: &str) -> SecurityReport {
    let secs = sections(out);
    let get = |k: &str| secs.iter().find(|(n, _)| *n == k).map(|(_, l)| l.as_slice()).unwrap_or(&[]);
    let root = get("uid").first().is_some_and(|l| l.trim() == "0");
    let ss = get("ss");
    let have_sockets = !ss.iter().any(|l| l.trim() == "NONE");
    let mut listeners = parse_listeners(ss);
    let docker = parse_docker(get("docker"));
    // A published Docker port shows up in `ss` as docker-proxy (or nothing,
    // with the userland proxy off); keep the container's entry.
    listeners.retain(|l| !docker.iter().any(|d| d.port == l.port && d.proto == l.proto));
    listeners.extend(docker.iter().cloned());
    listeners.sort_by(|a, b| a.port.cmp(&b.port).then(a.proto.cmp(&b.proto)));
    let processes_partial = !root && listeners.iter().any(|l| l.process.is_none());

    let sshd = get("sshd");
    let svc = services(get("services"));
    let mut checks = Vec::new();
    let check = |id: &str, title: &str, status: CheckStatus, detail: String, fix: Option<&str>, weight: u32| Check {
        id: id.into(),
        title: title.into(),
        status,
        detail,
        fix: fix.map(str::to_owned),
        weight,
    };

    // SSH.
    let ssh_open = listeners.iter().any(|l| l.port == 22 && l.scope == Scope::All)
        || sshd_option(sshd, "port")
            .is_some_and(|p| listeners.iter().any(|l| l.port.to_string() == p && l.scope == Scope::All));
    if sshd.iter().all(|l| l.trim().is_empty() || l.starts_with("#file")) {
        checks.push(check(
            "ssh-root",
            "Root can't log in over SSH",
            CheckStatus::Unknown,
            "The sshd config isn't readable.".into(),
            None,
            20,
        ));
        checks.push(check(
            "ssh-password",
            "SSH needs keys, not passwords",
            CheckStatus::Unknown,
            "The sshd config isn't readable.".into(),
            None,
            15,
        ));
    } else {
        let root_login = sshd_option(sshd, "permitrootlogin").unwrap_or_else(|| "prohibit-password".into());
        let (st, detail) = match root_login.as_str() {
            "no" => (CheckStatus::Pass, "PermitRootLogin no.".to_owned()),
            "prohibit-password" | "without-password" | "forced-commands-only" => {
                (CheckStatus::Pass, format!("PermitRootLogin {root_login}: root needs a key."))
            }
            other => (CheckStatus::Fail, format!("PermitRootLogin {other}: root can log in with a password.")),
        };
        checks.push(check(
            "ssh-root",
            "Root can't log in over SSH",
            st,
            detail,
            Some("Set `PermitRootLogin no` (or `prohibit-password`) in a file under `/etc/ssh/sshd_config.d/`, check it with `sudo sshd -t`, then `sudo systemctl reload ssh`. Keep your current session open until a new login works."),
            20,
        ));
        let pw = sshd_option(sshd, "passwordauthentication").unwrap_or_else(|| "yes".into());
        let kbd = sshd_option(sshd, "kbdinteractiveauthentication")
            .or_else(|| sshd_option(sshd, "challengeresponseauthentication"))
            .unwrap_or_else(|| "yes".into());
        let (st, detail) = if pw == "no" && kbd == "no" {
            (CheckStatus::Pass, "Password and keyboard-interactive logins are off.".to_owned())
        } else if pw == "no" {
            (CheckStatus::Warn, "PasswordAuthentication is off, but keyboard-interactive (which can ask for a password through PAM) is on.".to_owned())
        } else {
            (
                if ssh_open { CheckStatus::Fail } else { CheckStatus::Warn },
                format!(
                    "Password logins are allowed{}.",
                    if ssh_open { " and SSH listens on every interface" } else { "" }
                ),
            )
        };
        checks.push(check(
            "ssh-password",
            "SSH needs keys, not passwords",
            st,
            detail,
            Some("Make sure your key works first, then set `PasswordAuthentication no` and `KbdInteractiveAuthentication no` under `/etc/ssh/sshd_config.d/`, run `sudo sshd -t` and `sudo systemctl reload ssh`."),
            15,
        ));
    }

    // Firewall.
    let ufw_conf = get("ufw").iter().any(|l| l.trim().eq_ignore_ascii_case("enabled=yes"));
    let ufw = svc("ufw");
    let firewalld = svc("firewalld");
    let nft = svc("nftables");
    let ufw_on = ufw_conf && (ufw.active || !ufw.known);
    let fw = if ufw_on {
        Some("ufw")
    } else if firewalld.active {
        Some("firewalld")
    } else if nft.active {
        Some("nftables")
    } else {
        None
    };
    checks.push(match fw {
        Some(name) => check("firewall", "A firewall is on", CheckStatus::Pass, format!("{name} is active."), None, 15),
        None if !ufw.known && !firewalld.known && !nft.known && get("ufw").is_empty() => {
            check("firewall", "A firewall is on", CheckStatus::Unknown, "Couldn't tell which firewall is in use (no systemd?).".into(), None, 15)
        }
        None => check(
            "firewall",
            "A firewall is on",
            CheckStatus::Warn,
            "No ufw, firewalld or nftables service is active. iptables rules may still exist; reading them needs root.".into(),
            Some("On Debian/Ubuntu: `sudo ufw allow OpenSSH` first, then `sudo ufw enable`. On Fedora/RHEL: `sudo systemctl enable --now firewalld`."),
            15,
        ),
    });

    // Databases and caches on every interface.
    let exposed: Vec<&Listener> = listeners.iter().filter(|l| l.scope == Scope::All && is_sensitive(l.port)).collect();
    checks.push(if !have_sockets {
        check("exposed-services", "Databases aren't open to the network", CheckStatus::Unknown, "Neither ss nor netstat is installed.".into(), None, 20)
    } else if exposed.is_empty() {
        check("exposed-services", "Databases aren't open to the network", CheckStatus::Pass, "No database, cache or Docker API port listens on every interface.".into(), None, 20)
    } else {
        let names: Vec<String> = exposed
            .iter()
            .map(|l| {
                format!(
                    "{} on {}{}",
                    l.service.as_deref().unwrap_or("?"),
                    l.port,
                    l.container.as_ref().map(|c| format!(" (container {c})")).unwrap_or_default()
                )
            })
            .collect();
        check(
            "exposed-services",
            "Databases aren't open to the network",
            CheckStatus::Fail,
            format!("Listening on every interface: {}.", names.join(", ")),
            Some("Bind them to `127.0.0.1` (or a Docker network without a published port), e.g. `ports: [\"127.0.0.1:5432:5432\"]` in compose, or `bind-address = 127.0.0.1` / `listen_addresses = 'localhost'` in the server's config."),
            20,
        )
    });

    // Docker publishes ports around ufw.
    let published_all: Vec<&Listener> =
        listeners.iter().filter(|l| l.container.is_some() && l.scope == Scope::All).collect();
    if ufw_on && !published_all.is_empty() {
        let ports: Vec<String> = published_all.iter().map(|l| l.port.to_string()).collect();
        checks.push(check(
            "docker-ufw",
            "Docker ports respect the firewall",
            CheckStatus::Warn,
            format!("ufw is on, but Docker publishes {} on every interface; Docker's iptables rules bypass ufw.", ports.join(", ")),
            Some("Publish only what must be reachable, bind the rest to `127.0.0.1:` in compose, or use the `DOCKER-USER` chain (e.g. ufw-docker)."),
            10,
        ));
    }

    // Brute-force protection.
    let f2b = svc("fail2ban").active || svc("crowdsec").active;
    checks.push(if f2b {
        check("intrusion", "Brute-force protection", CheckStatus::Pass, "fail2ban or CrowdSec is running.".into(), None, 5)
    } else {
        let pw_on = checks.iter().any(|c| c.id == "ssh-password" && c.status != CheckStatus::Pass);
        check(
            "intrusion",
            "Brute-force protection",
            if ssh_open && pw_on { CheckStatus::Warn } else { CheckStatus::Info },
            "Neither fail2ban nor CrowdSec is running.".into(),
            Some("With key-only SSH this matters less. Otherwise `sudo apt install fail2ban` (its default jail covers sshd)."),
            5,
        )
    });

    // Automatic updates.
    let apt_auto = get("aptauto").iter().any(|l| l.contains("Unattended-Upgrade") && l.contains("\"1\""));
    let uu = svc("unattended-upgrades");
    let apt_timer = svc("apt-daily-upgrade.timer");
    let dnf = svc("dnf-automatic.timer").enabled || svc("dnf-automatic-install.timer").enabled;
    let auto = (apt_auto && (uu.enabled || uu.active) && (apt_timer.enabled || !apt_timer.known)) || dnf;
    checks.push(if auto {
        check("auto-updates", "Security updates install themselves", CheckStatus::Pass, "unattended-upgrades or dnf-automatic is set up.".into(), None, 10)
    } else if !uu.known && !apt_timer.known && !svc("dnf-automatic.timer").known {
        check("auto-updates", "Security updates install themselves", CheckStatus::Unknown, "Couldn't tell (not apt or dnf, or no systemd).".into(), None, 10)
    } else {
        check(
            "auto-updates",
            "Security updates install themselves",
            CheckStatus::Warn,
            "No automatic security updates found.".into(),
            Some("Debian/Ubuntu: `sudo apt install unattended-upgrades && sudo dpkg-reconfigure -plow unattended-upgrades`. Fedora: `sudo dnf install dnf-automatic && sudo systemctl enable --now dnf-automatic-install.timer`. Or review updates in the Updates tab."),
            10,
        )
    });

    // Pending reboot.
    let reboot = get("reboot").iter().any(|l| l.trim() == "yes");
    checks.push(check(
        "reboot",
        "No reboot pending",
        if reboot { CheckStatus::Warn } else { CheckStatus::Pass },
        if reboot {
            "Updates installed a new kernel or library; it isn't in use until a reboot.".into()
        } else {
            "Nothing is waiting for a reboot.".into()
        },
        reboot.then_some("Reboot when it suits you (Updates tab → Reboot)."),
        5,
    ));

    // Extra uid-0 accounts.
    let uid0: Vec<&str> = get("uid0").iter().map(|l| l.trim()).filter(|l| !l.is_empty() && *l != "root").collect();
    checks.push(if get("uid0").is_empty() {
        check("uid0", "Only root has uid 0", CheckStatus::Unknown, "/etc/passwd isn't readable.".into(), None, 20)
    } else if uid0.is_empty() {
        check("uid0", "Only root has uid 0", CheckStatus::Pass, "No other account has root's uid.".into(), None, 20)
    } else {
        check(
            "uid0",
            "Only root has uid 0",
            CheckStatus::Fail,
            format!("Also uid 0: {}. That's root under another name.", uid0.join(", ")),
            Some("If you didn't create it, treat the host as compromised. Otherwise give the account its own uid and use sudo."),
            20,
        )
    });

    let (score, grade) = score(&checks);
    SecurityReport { score, grade, checks, listeners, processes_partial }
}

/// 100 minus the weight of each failed check and half the weight of each
/// warning; unknown and info checks don't count.
pub fn score(checks: &[Check]) -> (u32, String) {
    let lost: u32 = checks
        .iter()
        .map(|c| match c.status {
            CheckStatus::Fail => c.weight,
            CheckStatus::Warn => c.weight / 2,
            _ => 0,
        })
        .sum();
    let s = 100u32.saturating_sub(lost);
    let g = match s {
        90.. => "A",
        75..=89 => "B",
        60..=74 => "C",
        40..=59 => "D",
        _ => "F",
    };
    (s, g.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "@@uid\n1000\n@@ss\n\
tcp   LISTEN 0      4096   0.0.0.0:22        0.0.0.0:*\n\
tcp   LISTEN 0      4096      [::]:22           [::]:*\n\
tcp   LISTEN 0      511    127.0.0.1:5432    0.0.0.0:*\n\
udp   UNCONN 0      0      127.0.0.53%lo:53  0.0.0.0:*\n\
tcp   LISTEN 0      4096   100.101.5.7:9100  0.0.0.0:*    users:((\"node_exporter\",pid=900,fd=3))\n\
@@sshd\n#file /etc/ssh/sshd_config.d/10-hardening.conf\nPasswordAuthentication no\nKbdInteractiveAuthentication no\n\
#file /etc/ssh/sshd_config\nInclude /etc/ssh/sshd_config.d/*.conf\nPasswordAuthentication yes\nMatch User backup\n  PermitRootLogin yes\n\
@@services\nufw active enabled\nfirewalld inactive disabled\nnftables inactive disabled\nfail2ban active enabled\ncrowdsec unknown unknown\n\
unattended-upgrades active enabled\napt-daily-upgrade.timer active enabled\ndnf-automatic.timer unknown unknown\ndnf-automatic-install.timer unknown unknown\n\
@@ufw\nENABLED=yes\nLOGLEVEL=low\n@@aptauto\nAPT::Periodic::Update-Package-Lists \"1\";\nAPT::Periodic::Unattended-Upgrade \"1\";\n\
@@reboot\n@@uid0\nroot\n@@docker\n@@end\n";

    #[test]
    fn a_tidy_host_scores_full_marks() {
        let r = parse_audit(GOOD);
        let failing: Vec<_> =
            r.checks.iter().filter(|c| !matches!(c.status, CheckStatus::Pass)).map(|c| &c.id).collect();
        assert!(failing.is_empty(), "{failing:?}");
        assert_eq!((r.score, r.grade.as_str()), (100, "A"));
        assert_eq!(
            r.listeners.iter().map(|l| (l.port, l.scope)).collect::<Vec<_>>(),
            [(22, Scope::All), (53, Scope::Local), (5432, Scope::Local), (9100, Scope::Vpn),]
        );
        assert_eq!(r.listeners[3].process.as_deref(), Some("node_exporter"));
        assert_eq!(r.listeners[0].service.as_deref(), Some("ssh"));
        assert!(r.processes_partial);
    }

    #[test]
    fn a_messy_host_gets_findings() {
        let out = "@@uid\n0\n@@ss\n\
tcp LISTEN 0 128 0.0.0.0:22 0.0.0.0:* users:((\"sshd\",pid=1,fd=3))\n\
tcp LISTEN 0 128 0.0.0.0:6379 0.0.0.0:* users:((\"redis-server\",pid=2,fd=6))\n\
tcp LISTEN 0 128 0.0.0.0:8080 0.0.0.0:* users:((\"docker-proxy\",pid=3,fd=4))\n\
@@sshd\n#file /etc/ssh/sshd_config\nPermitRootLogin yes\n\
@@services\nufw active enabled\nfirewalld unknown unknown\nnftables inactive disabled\nfail2ban inactive disabled\ncrowdsec unknown unknown\n\
unattended-upgrades inactive disabled\napt-daily-upgrade.timer active enabled\ndnf-automatic.timer unknown unknown\ndnf-automatic-install.timer unknown unknown\n\
@@ufw\nENABLED=yes\n@@aptauto\n@@reboot\nyes\n@@uid0\nroot\ntoor\n\
@@docker\nweb|0.0.0.0:8080->80/tcp, :::8080->80/tcp\ndb|5432/tcp\ncache|0.0.0.0:27017->27017/tcp\n@@end\n";
        let r = parse_audit(out);
        let st = |id: &str| r.checks.iter().find(|c| c.id == id).map(|c| c.status);
        assert_eq!(st("ssh-root"), Some(CheckStatus::Fail));
        assert_eq!(st("ssh-password"), Some(CheckStatus::Fail));
        assert_eq!(st("firewall"), Some(CheckStatus::Pass));
        assert_eq!(st("exposed-services"), Some(CheckStatus::Fail));
        assert_eq!(st("docker-ufw"), Some(CheckStatus::Warn));
        assert_eq!(st("intrusion"), Some(CheckStatus::Warn));
        assert_eq!(st("auto-updates"), Some(CheckStatus::Warn));
        assert_eq!(st("reboot"), Some(CheckStatus::Warn));
        assert_eq!(st("uid0"), Some(CheckStatus::Fail));
        let exp = r.checks.iter().find(|c| c.id == "exposed-services").unwrap();
        assert!(
            exp.detail.contains("redis on 6379") && exp.detail.contains("mongodb on 27017 (container cache)"),
            "{}",
            exp.detail
        );
        // 100 - 20 - 15 - 20 - 5 - 2 - 5 - 2 - 20 = 11
        assert_eq!((r.score, r.grade.as_str()), (11, "F"));
        let web = r.listeners.iter().find(|l| l.port == 8080).unwrap();
        assert_eq!(web.container.as_deref(), Some("web"));
        assert_eq!(r.listeners.iter().filter(|l| l.port == 8080).count(), 1);
        assert!(!r.processes_partial);
    }

    #[test]
    fn unreadable_bits_are_unknown_not_failures() {
        let r = parse_audit(
            "@@uid\n1000\n@@ss\nNONE\n@@sshd\n@@services\n@@ufw\n@@aptauto\n@@reboot\n@@uid0\n@@docker\n@@end\n",
        );
        let unknown = r.checks.iter().filter(|c| c.status == CheckStatus::Unknown).count();
        assert!(unknown >= 5, "{:?}", r.checks);
        assert_eq!(r.score, 100);
    }

    #[test]
    fn netstat_fallback_and_scopes() {
        let l = parse_listeners(&[
            "NETSTAT",
            "Active Internet connections (only servers)",
            "Proto Recv-Q Send-Q Local Address           Foreign Address         State",
            "tcp        0      0 0.0.0.0:22              0.0.0.0:*               LISTEN",
            "tcp6       0      0 :::80                   :::*                    LISTEN",
            "udp        0      0 192.168.1.10:123        0.0.0.0:*",
        ]);
        assert_eq!(
            l.iter().map(|x| (x.proto.as_str(), x.port, x.scope)).collect::<Vec<_>>(),
            [("tcp", 22, Scope::All), ("tcp", 80, Scope::All), ("udp", 123, Scope::Address),]
        );
        assert_eq!(scope_of("::1"), Scope::Local);
        assert_eq!(scope_of("100.64.0.1"), Scope::Vpn);
        assert_eq!(scope_of("100.128.0.1"), Scope::Address);
        assert_eq!(scope_of("*"), Scope::All);
    }

    #[test]
    fn sshd_first_match_and_match_blocks() {
        let lines = [
            "#file a",
            "# PasswordAuthentication yes",
            "passwordauthentication=no",
            "#file b",
            "PasswordAuthentication yes",
            "Match all",
            "PermitRootLogin yes",
        ];
        assert_eq!(sshd_option(&lines, "PasswordAuthentication").as_deref(), Some("no"));
        assert_eq!(sshd_option(&lines, "PermitRootLogin"), None);
    }

    #[test]
    fn script_runs_under_sh() {
        let out = std::process::Command::new("sh").arg("-c").arg(audit_command()).output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("@@end"), "{text}");
        let r = parse_audit(&text);
        assert!(r.score <= 100 && !r.checks.is_empty());
    }
}
