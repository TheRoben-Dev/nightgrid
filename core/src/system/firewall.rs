//! Firewall manager (M10v): reads ufw, firewalld or nftables, and changes ufw
//! and firewalld rules behind a safety net. Every change first snapshots the
//! firewall's config on the host and arms a rollback there (a transient
//! systemd timer, or a detached `sleep` without systemd) that restores the
//! snapshot unless Nightgrid cancels it after the user confirms that the
//! connection still works. The host restores itself even if Nightgrid can't
//! reach it any more.

use std::net::IpAddr;

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::files::path::shell_quote;
use crate::{Error, Result};

fn invalid(field: &'static str, reason: impl Into<String>) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

/// Where snapshots and restore scripts live on the host (root only).
pub const ROLLBACK_DIR: &str = "/var/lib/nightgrid/fw-rollback";
/// The transient unit that restores the snapshot.
pub const ROLLBACK_UNIT: &str = "nightgrid-fw-rollback";
/// Seconds before an unconfirmed change is rolled back, by default.
pub const DEFAULT_ROLLBACK_SECS: u32 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Backend {
    Ufw,
    Firewalld,
    Nftables,
    Iptables,
    /// Windows Defender Firewall (Windows hosts).
    Windows,
    None,
}

/// One ufw rule as `ufw show added` prints it, without the leading `ufw`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UfwRule {
    /// `allow from 192.168.1.0/24 to any port 22 proto tcp comment 'lan'`
    pub spec: String,
    /// `allow`, `deny`, `reject`, `limit`.
    pub action: String,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UfwState {
    /// `deny`, `allow` or `reject` for incoming traffic.
    pub default_in: String,
    pub default_out: String,
    pub rules: Vec<UfwRule>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Zone {
    pub name: String,
    pub active: bool,
    pub default: bool,
    pub target: String,
    pub interfaces: Vec<String>,
    pub sources: Vec<String>,
    pub services: Vec<String>,
    pub ports: Vec<String>,
    pub rich_rules: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FirewallReport {
    pub backend: Backend,
    pub active: bool,
    pub ufw: Option<UfwState>,
    pub zones: Vec<Zone>,
    /// `nft list ruleset` or `iptables-save` (read-only view), clipped.
    pub raw: String,
    /// A rollback is armed on the host right now.
    pub rollback_armed: bool,
    /// Windows Defender Firewall's profiles and rules (Windows hosts).
    pub windows: Option<crate::win::firewall::WinFirewall>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum UfwAction {
    Allow,
    Deny,
    Reject,
    Limit,
}

impl UfwAction {
    fn word(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Reject => "reject",
            Self::Limit => "limit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Proto {
    Tcp,
    Udp,
    Any,
}

/// One change to the firewall.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum FwChange {
    UfwAdd {
        action: UfwAction,
        port: String,
        proto: Proto,
        from: Option<String>,
        comment: Option<String>,
    },
    /// A rule from the report, by its spec.
    UfwDelete {
        spec: String,
    },
    UfwEnable {
        enabled: bool,
    },
    /// The incoming default: `allow`, `deny` or `reject`.
    UfwDefault {
        policy: String,
    },
    FirewalldService {
        zone: String,
        service: String,
        add: bool,
    },
    FirewalldPort {
        zone: String,
        port: String,
        proto: Proto,
        add: bool,
    },
    /// Accept traffic from a source address (optionally to one port).
    FirewalldSource {
        zone: String,
        source: String,
        port: Option<String>,
        proto: Proto,
        add: bool,
    },
    /// Windows: an inbound allow rule for a port (optionally from one source).
    WinAllow {
        port: String,
        proto: Proto,
        from: Option<String>,
        comment: Option<String>,
    },
    /// Windows: a rule from the report, by its unique name.
    WinDelete {
        name: String,
    },
    WinRuleEnable {
        name: String,
        enabled: bool,
    },
    /// Windows: every profile on or off.
    WinEnable {
        enabled: bool,
    },
    /// Windows: the inbound default for every profile, `block` or `allow`.
    WinDefault {
        policy: String,
    },
}

impl FwChange {
    pub fn backend(&self) -> Backend {
        match self {
            Self::UfwAdd { .. } | Self::UfwDelete { .. } | Self::UfwEnable { .. } | Self::UfwDefault { .. } => {
                Backend::Ufw
            }
            Self::WinAllow { .. }
            | Self::WinDelete { .. }
            | Self::WinRuleEnable { .. }
            | Self::WinEnable { .. }
            | Self::WinDefault { .. } => Backend::Windows,
            _ => Backend::Firewalld,
        }
    }

    /// Plain words, for confirmations and logs.
    pub fn describe(&self) -> String {
        match self {
            Self::UfwAdd { action, port, proto, from, .. } => format!(
                "{} {}{}{}",
                action.word(),
                port,
                if *proto == Proto::Any { String::new() } else { format!("/{}", proto_word(*proto)) },
                from.as_deref().map(|f| format!(" from {f}")).unwrap_or_default()
            ),
            Self::UfwDelete { spec } => format!("delete rule `{spec}`"),
            Self::UfwEnable { enabled } => if *enabled { "turn ufw on" } else { "turn ufw off" }.into(),
            Self::UfwDefault { policy } => format!("set the incoming default to {policy}"),
            Self::FirewalldService { zone, service, add } => format!(
                "{} service {service} {} zone {zone}",
                if *add { "add" } else { "remove" },
                if *add { "to" } else { "from" }
            ),
            Self::FirewalldPort { zone, port, proto, add } => {
                format!("{} port {port}/{} in zone {zone}", if *add { "open" } else { "close" }, proto_word(*proto))
            }
            Self::FirewalldSource { zone, source, port, add, .. } => format!(
                "{} {source}{} in zone {zone}",
                if *add { "accept" } else { "stop accepting" },
                port.as_deref().map(|p| format!(" on port {p}")).unwrap_or_default()
            ),
            Self::WinAllow { port, proto, from, .. } => format!(
                "allow {port}{}{}",
                if *proto == Proto::Any { String::new() } else { format!("/{}", proto_word(*proto)) },
                from.as_deref().map(|f| format!(" from {f}")).unwrap_or_default()
            ),
            Self::WinDelete { name } => format!("delete rule {name}"),
            Self::WinRuleEnable { name, enabled } => {
                format!("{} rule {name}", if *enabled { "enable" } else { "disable" })
            }
            Self::WinEnable { enabled } => {
                if *enabled { "turn Windows Defender Firewall on" } else { "turn Windows Defender Firewall off" }.into()
            }
            Self::WinDefault { policy } => format!("set the incoming default to {policy}"),
        }
    }
}

fn proto_word(p: Proto) -> &'static str {
    match p {
        Proto::Tcp => "tcp",
        Proto::Udp => "udp",
        Proto::Any => "any",
    }
}

// ---- validation -------------------------------------------------------------

/// `22`, `8000:8100` (ufw) or `8000-8100` (firewalld).
fn valid_port(p: &str, range_sep: char) -> bool {
    let ok = |n: &str| n.parse::<u16>().is_ok_and(|v| v > 0);
    match p.split_once(range_sep) {
        Some((a, b)) => ok(a) && ok(b) && a.parse::<u16>().ok() < b.parse::<u16>().ok(),
        None => ok(p),
    }
}

/// An address or a CIDR network.
pub(crate) fn proto_name(p: Proto) -> &'static str {
    proto_word(p)
}

pub(crate) fn valid_port_range(p: &str) -> bool {
    valid_port(p, '-')
}

pub(crate) fn valid_comment_text(s: &str) -> bool {
    valid_comment(s)
}

pub fn valid_source(s: &str) -> bool {
    match s.split_once('/') {
        Some((ip, bits)) => ip
            .parse::<IpAddr>()
            .is_ok_and(|a| bits.parse::<u8>().is_ok_and(|b| b <= if a.is_ipv4() { 32 } else { 128 })),
        None => s.parse::<IpAddr>().is_ok(),
    }
}

fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn valid_comment(s: &str) -> bool {
    s.len() <= 60 && s.chars().all(|c| c.is_ascii_alphanumeric() || " ._-".contains(c))
}

/// Splits a ufw spec into words, keeping a quoted comment together.
fn spec_words(spec: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote = false;
    for c in spec.chars() {
        match c {
            '\'' => quote = !quote,
            c if c.is_whitespace() && !quote => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The shell command for one change (run as root, after the snapshot).
pub fn change_command(c: &FwChange) -> Result<String> {
    Ok(match c {
        FwChange::UfwAdd { action, port, proto, from, comment } => {
            let port = port.trim();
            if !valid_port(port, ':') {
                return Err(invalid("port", "a port (1-65535) or a range like 8000:8100"));
            }
            if port.contains(':') && *proto == Proto::Any {
                return Err(invalid("proto", "a port range needs tcp or udp"));
            }
            let from = from.as_deref().map(str::trim).filter(|f| !f.is_empty() && *f != "any");
            if from.is_some_and(|f| !valid_source(f)) {
                return Err(invalid("from", "an IP address or network such as 192.168.1.0/24"));
            }
            let mut cmd = format!("ufw {}", action.word());
            if *proto != Proto::Any {
                cmd.push_str(&format!(" proto {}", proto_word(*proto)));
            }
            cmd.push_str(&format!(" from {} to any port {port}", from.unwrap_or("any")));
            if let Some(cm) = comment.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
                if !valid_comment(cm) {
                    return Err(invalid("comment", "letters, digits, spaces and . _ - (60 at most)"));
                }
                cmd.push_str(&format!(" comment {}", shell_quote(cm)));
            }
            cmd
        }
        FwChange::UfwDelete { spec } => {
            let words = spec_words(spec);
            let cut = words.iter().position(|w| w == "comment").unwrap_or(words.len());
            let words = &words[..cut];
            if !matches!(words.first().map(String::as_str), Some("allow" | "deny" | "reject" | "limit")) {
                return Err(invalid("spec", "not a ufw rule"));
            }
            if words.iter().any(|w| !w.chars().all(|c| c.is_ascii_alphanumeric() || "./:,_-".contains(c))) {
                return Err(invalid("spec", "unexpected characters"));
            }
            format!("ufw --force delete {}", words.iter().map(|w| shell_quote(w)).collect::<Vec<_>>().join(" "))
        }
        FwChange::UfwEnable { enabled } => if *enabled { "ufw --force enable" } else { "ufw --force disable" }.into(),
        FwChange::UfwDefault { policy } => {
            if !matches!(policy.as_str(), "allow" | "deny" | "reject") {
                return Err(invalid("policy", "allow, deny or reject"));
            }
            format!("ufw default {policy} incoming")
        }
        FwChange::FirewalldService { zone, service, add } => {
            if !valid_name(zone) || !valid_name(service) {
                return Err(invalid("service", "a zone and service name"));
            }
            format!(
                "firewall-cmd -q --permanent --zone={zone} --{}-service={service} && firewall-cmd -q --reload",
                if *add { "add" } else { "remove" }
            )
        }
        FwChange::FirewalldPort { zone, port, proto, add } => {
            if !valid_name(zone) || !valid_port(port.trim(), '-') || *proto == Proto::Any {
                return Err(invalid("port", "a zone, a port or range like 8000-8100, and tcp or udp"));
            }
            format!(
                "firewall-cmd -q --permanent --zone={zone} --{}-port={}/{} && firewall-cmd -q --reload",
                if *add { "add" } else { "remove" },
                port.trim(),
                proto_word(*proto)
            )
        }
        FwChange::FirewalldSource { zone, source, port, proto, add } => {
            let source = source.trim();
            if !valid_name(zone) || !valid_source(source) {
                return Err(invalid("source", "a zone and an IP address or network"));
            }
            let family = if source.split('/').next().is_some_and(|ip| ip.contains(':')) { "ipv6" } else { "ipv4" };
            let mut rule = format!("rule family=\"{family}\" source address=\"{source}\"");
            if let Some(p) = port.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
                if !valid_port(p, '-') || *proto == Proto::Any {
                    return Err(invalid("port", "a port or range like 8000-8100, and tcp or udp"));
                }
                rule.push_str(&format!(" port port=\"{p}\" protocol=\"{}\"", proto_word(*proto)));
            }
            rule.push_str(" accept");
            format!(
                "firewall-cmd -q --permanent --zone={zone} --{}-rich-rule={} && firewall-cmd -q --reload",
                if *add { "add" } else { "remove" },
                shell_quote(&rule)
            )
        }
        FwChange::WinAllow { .. }
        | FwChange::WinDelete { .. }
        | FwChange::WinRuleEnable { .. }
        | FwChange::WinEnable { .. }
        | FwChange::WinDefault { .. } => {
            // Validates; Windows changes run as PowerShell (win::firewall).
            crate::win::firewall::change_script(c)?;
            String::new()
        }
    })
}

// ---- reading ----------------------------------------------------------------

/// Reads the firewall (as root). Sections marked `@@name`.
pub const READ_COMMAND: &str = r#"LC_ALL=C; export LC_ALL
echo @@detect
if command -v ufw >/dev/null 2>&1; then echo ufw; fi
if command -v firewall-cmd >/dev/null 2>&1; then echo "firewalld $(firewall-cmd --state 2>/dev/null)"; fi
if command -v nft >/dev/null 2>&1; then echo nft; fi
if command -v iptables-save >/dev/null 2>&1; then echo iptables; fi
echo @@armed
if systemctl is-active --quiet nightgrid-fw-rollback.timer 2>/dev/null; then echo yes; fi
for p in /var/lib/nightgrid/fw-rollback/*/pid; do [ -f "$p" ] && kill -0 "$(cat "$p")" 2>/dev/null && echo yes; done
if command -v ufw >/dev/null 2>&1; then
  echo @@ufwstatus; ufw status verbose 2>&1
  echo @@ufwadded; ufw show added 2>&1
  echo @@ufwdefault; cat /etc/default/ufw 2>/dev/null
fi
if command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state >/dev/null 2>&1; then
  echo @@fwdefault; firewall-cmd --get-default-zone 2>/dev/null
  echo @@fwactive; firewall-cmd --get-active-zones 2>/dev/null
  for z in $( (firewall-cmd --get-default-zone; firewall-cmd --get-active-zones | grep -v '^[[:space:]]') 2>/dev/null | sort -u); do echo "@@zone $z"; firewall-cmd --zone="$z" --list-all 2>/dev/null; done
fi
echo @@raw
if command -v nft >/dev/null 2>&1; then nft list ruleset 2>&1 | head -c 65536; elif command -v iptables-save >/dev/null 2>&1; then iptables-save 2>&1 | head -c 65536; fi
echo @@end
"#;

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

fn ufw_policy(word: &str) -> String {
    match word.trim_matches('"').to_ascii_uppercase().as_str() {
        "DROP" => "deny".into(),
        "REJECT" => "reject".into(),
        "ACCEPT" => "allow".into(),
        other => other.to_ascii_lowercase(),
    }
}

pub fn parse_read(out: &str) -> FirewallReport {
    let secs = sections(out);
    let get = |k: &str| secs.iter().find(|(n, _)| *n == k).map(|(_, l)| l.clone()).unwrap_or_default();
    let detect = get("detect");
    let has = |w: &str| detect.iter().any(|l| l.split_whitespace().next() == Some(w));
    let firewalld_running = detect.iter().any(|l| l.trim() == "firewalld running");
    let rollback_armed = get("armed").iter().any(|l| l.trim() == "yes");

    let ufw_status = get("ufwstatus");
    let ufw_active = ufw_status.iter().any(|l| l.trim() == "Status: active");
    let ufw = has("ufw").then(|| {
        let mut default_in = String::new();
        let mut default_out = String::new();
        if let Some(d) = ufw_status.iter().find_map(|l| l.strip_prefix("Default:")) {
            for part in d.split(',') {
                let part = part.trim();
                if let Some(p) = part.strip_suffix("(incoming)") {
                    default_in = p.trim().to_owned();
                } else if let Some(p) = part.strip_suffix("(outgoing)") {
                    default_out = p.trim().to_owned();
                }
            }
        }
        for l in get("ufwdefault") {
            if default_in.is_empty() {
                if let Some(v) = l.strip_prefix("DEFAULT_INPUT_POLICY=") {
                    default_in = ufw_policy(v);
                }
            }
            if default_out.is_empty() {
                if let Some(v) = l.strip_prefix("DEFAULT_OUTPUT_POLICY=") {
                    default_out = ufw_policy(v);
                }
            }
        }
        let rules = get("ufwadded")
            .iter()
            .filter_map(|l| l.trim().strip_prefix("ufw "))
            .map(|spec| {
                let words = spec_words(spec);
                let action = words.first().cloned().unwrap_or_default();
                let comment = words.iter().position(|w| w == "comment").and_then(|i| words.get(i + 1)).cloned();
                UfwRule { spec: spec.to_owned(), action, comment }
            })
            .collect();
        UfwState { default_in, default_out, rules }
    });

    let default_zone = get("fwdefault").first().map(|s| s.trim().to_owned()).unwrap_or_default();
    let active_zones: Vec<String> = get("fwactive")
        .iter()
        .filter(|l| !l.starts_with(char::is_whitespace))
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.is_empty())
        .collect();
    let mut zones = Vec::new();
    for (name, lines) in secs.iter().filter(|(n, _)| n.starts_with("zone ")) {
        let zname = name.trim_start_matches("zone ").trim().to_owned();
        let mut z = Zone {
            active: active_zones.contains(&zname),
            default: zname == default_zone,
            name: zname,
            ..Default::default()
        };
        let mut in_rich = false;
        for l in lines {
            let t = l.trim();
            if let Some((k, v)) = t.split_once(':').filter(|(k, _)| !k.contains(' ') || *k == "rich rules") {
                in_rich = k == "rich rules";
                let list = || v.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
                match k {
                    "target" => z.target = v.trim().to_owned(),
                    "interfaces" => z.interfaces = list(),
                    "sources" => z.sources = list(),
                    "services" => z.services = list(),
                    "ports" => z.ports = list(),
                    "rich rules" if !v.trim().is_empty() => z.rich_rules.push(v.trim().to_owned()),
                    _ => {}
                }
            } else if in_rich && t.starts_with("rule ") {
                z.rich_rules.push(t.to_owned());
            }
        }
        zones.push(z);
    }
    zones.sort_by_key(|z| (!z.default, !z.active, z.name.clone()));

    let backend = if ufw.is_some() && (ufw_active || !firewalld_running) {
        Backend::Ufw
    } else if firewalld_running {
        Backend::Firewalld
    } else if has("nft") {
        Backend::Nftables
    } else if has("iptables") {
        Backend::Iptables
    } else {
        Backend::None
    };
    let active = match backend {
        Backend::Ufw => ufw_active,
        Backend::Firewalld => true,
        _ => false,
    };
    let raw: String = get("raw").join("\n");
    FirewallReport { backend, active, ufw, zones, raw, rollback_armed, windows: None }
}

// ---- lock-out check -----------------------------------------------------------

/// Ports a ufw spec opens: `None` when it names no port (every port).
fn spec_ports(spec: &str) -> Option<Vec<(u16, u16)>> {
    let words = spec_words(spec);
    let cut = words.iter().position(|w| w == "comment").unwrap_or(words.len());
    let words = &words[..cut];
    let token = match words.iter().position(|w| w == "port") {
        Some(i) => words.get(i + 1)?.clone(),
        None => {
            // `allow 22/tcp`, `allow OpenSSH`, or `allow from X` (no port).
            let w = words.get(1)?;
            if matches!(w.as_str(), "in" | "out") {
                words.get(2)?.clone()
            } else if w == "from" || w == "proto" {
                return None;
            } else {
                w.clone()
            }
        }
    };
    let token = token.split('/').next().unwrap_or("").to_owned();
    let lower = token.to_ascii_lowercase();
    if lower == "openssh" || lower == "ssh" {
        return Some(vec![(22, 22)]);
    }
    let mut out = Vec::new();
    for part in token.split(',') {
        let (a, b) = part.split_once(':').unwrap_or((part, part));
        if let (Ok(a), Ok(b)) = (a.parse(), b.parse()) {
            out.push((a, b));
        }
    }
    Some(out)
}

fn spec_covers(spec: &str, port: u16) -> bool {
    match spec_ports(spec) {
        None => true,
        Some(r) => r.iter().any(|(a, b)| (*a..=*b).contains(&port)),
    }
}

fn ufw_allows(ufw: &UfwState, port: u16, without: Option<&str>) -> bool {
    ufw.default_in == "allow"
        || ufw.rules.iter().any(|r| {
            (r.action == "allow" || r.action == "limit")
                && Some(r.spec.as_str()) != without
                && spec_covers(&r.spec, port)
        })
}

/// Why a change could cut off SSH on `ssh_port`, if it could. Such changes
/// need the host name typed (security rule 5); the rollback covers the rest.
pub fn lockout_risk(r: &FirewallReport, c: &FwChange, ssh_port: u16) -> Option<String> {
    let msg = |what: &str| Some(format!("{what}: Nightgrid connects on port {ssh_port} and could be locked out."));
    match (c, r.ufw.as_ref()) {
        (FwChange::UfwEnable { enabled: true }, Some(u)) if !ufw_allows(u, ssh_port, None) => {
            msg("No ufw rule allows SSH")
        }
        (FwChange::UfwDefault { policy }, Some(u))
            if policy != "allow"
                && r.active
                && !u
                    .rules
                    .iter()
                    .any(|x| (x.action == "allow" || x.action == "limit") && spec_covers(&x.spec, ssh_port)) =>
        {
            msg("No rule allows SSH once incoming traffic is denied by default")
        }
        (FwChange::UfwDelete { spec }, Some(u))
            if spec_covers(spec, ssh_port) && !ufw_allows(u, ssh_port, Some(spec)) =>
        {
            msg("This is the rule that lets SSH in")
        }
        (FwChange::UfwAdd { action: UfwAction::Deny | UfwAction::Reject, port, .. }, _)
            if spec_covers(&format!("x {port}"), ssh_port) =>
        {
            msg("This blocks the SSH port")
        }
        (FwChange::FirewalldService { service, add: false, .. }, _) if service == "ssh" && ssh_port == 22 => {
            msg("This removes the ssh service")
        }
        (FwChange::FirewalldPort { port, add: false, .. }, _) => {
            let (a, b) = port.split_once('-').unwrap_or((port, port));
            match (a.parse::<u16>(), b.parse::<u16>()) {
                (Ok(a), Ok(b)) if (a..=b).contains(&ssh_port) => msg("This closes the SSH port"),
                _ => None,
            }
        }
        _ => None,
    }
}

// ---- snapshot, rollback, confirm -----------------------------------------------

fn restore_script(b: Backend) -> Result<&'static str> {
    Ok(match b {
        Backend::Ufw => "D=$(dirname \"$0\"); cp -a \"$D/ufw/.\" /etc/ufw/; [ -f \"$D/default-ufw\" ] && cp -a \"$D/default-ufw\" /etc/default/ufw; \
if grep -qi '^ENABLED=yes' /etc/ufw/ufw.conf; then if ufw status | grep -q 'Status: active'; then ufw reload >/dev/null 2>&1; else ufw --force enable >/dev/null 2>&1; fi; \
else ufw --force disable >/dev/null 2>&1; fi; rm -f \"$D/pid\"; echo restored\n",
        Backend::Firewalld => "D=$(dirname \"$0\"); rm -rf /etc/firewalld.ng-restore && cp -a \"$D/firewalld\" /etc/firewalld.ng-restore && rm -rf /etc/firewalld && mv /etc/firewalld.ng-restore /etc/firewalld; \
firewall-cmd --reload >/dev/null 2>&1; rm -f \"$D/pid\"; echo restored\n",
        _ => return Err(invalid("backend", "only ufw and firewalld can be changed")),
    })
}

/// A rollback token: the unix time the change was made.
pub fn valid_token(t: &str) -> bool {
    !t.is_empty() && t.len() <= 12 && t.chars().all(|c| c.is_ascii_digit())
}

/// Root script: snapshot, arm the rollback for `secs`, apply `change`. On
/// failure the snapshot is restored at once. Prints `@@armed <how>` and
/// `@@applied` or `@@failed <code>`.
pub fn apply_command(change: &FwChange, token: &str, secs: u32) -> Result<String> {
    if !valid_token(token) {
        return Err(invalid("token", "digits"));
    }
    let backend = change.backend();
    let cmd = change_command(change)?;
    let restore = base64::engine::general_purpose::STANDARD.encode(restore_script(backend)?);
    let snapshot = match backend {
        Backend::Ufw => "mkdir -p \"$D/ufw\" && cp -a /etc/ufw/. \"$D/ufw/\" && { [ ! -f /etc/default/ufw ] || cp -a /etc/default/ufw \"$D/default-ufw\"; }",
        _ => "cp -a /etc/firewalld \"$D/firewalld\"",
    };
    let secs = secs.clamp(30, 900);
    Ok(format!(
        "umask 077; D={ROLLBACK_DIR}/{token}; mkdir -p \"$D\" || exit 2; \
(cd {ROLLBACK_DIR} && ls -1t | tail -n +6 | while IFS= read -r f; do [ \"$f\" = {token} ] || rm -rf -- \"$f\"; done); \
{snapshot} || {{ echo 'could not save the current rules' >&2; exit 2; }}; \
printf '%s' '{restore}' | base64 -d > \"$D/restore.sh\"; \
if command -v systemd-run >/dev/null 2>&1 && [ -d /run/systemd/system ]; then \
systemctl stop {ROLLBACK_UNIT}.timer >/dev/null 2>&1; systemctl reset-failed {ROLLBACK_UNIT}.timer {ROLLBACK_UNIT}.service >/dev/null 2>&1; \
systemd-run --quiet --unit={ROLLBACK_UNIT} --on-active={secs}s /bin/sh \"$D/restore.sh\" && echo '@@armed systemd' || {{ echo 'could not arm the rollback' >&2; exit 2; }}; \
else setsid nohup sh -c \"sleep {secs}; sh '$D/restore.sh'\" >/dev/null 2>&1 < /dev/null & echo $! > \"$D/pid\"; echo '@@armed sleep'; fi; \
if {cmd} >\"$D/out\" 2>&1; then echo @@applied; else c=$?; cat \"$D/out\" >&2; sh \"$D/restore.sh\" >/dev/null 2>&1; \
systemctl stop {ROLLBACK_UNIT}.timer >/dev/null 2>&1; echo \"@@failed $c\"; fi"
    ))
}

/// Root script that cancels the armed rollback (keeps the change).
pub fn confirm_command(token: &str) -> Result<String> {
    if !valid_token(token) {
        return Err(invalid("token", "digits"));
    }
    Ok(format!(
        "D={ROLLBACK_DIR}/{token}; systemctl stop {ROLLBACK_UNIT}.timer >/dev/null 2>&1; \
if [ -f \"$D/pid\" ]; then kill \"$(cat \"$D/pid\")\" 2>/dev/null; rm -f \"$D/pid\"; fi; echo kept"
    ))
}

/// Root script that restores the snapshot now and disarms the timer.
pub fn rollback_command(token: &str) -> Result<String> {
    if !valid_token(token) {
        return Err(invalid("token", "digits"));
    }
    Ok(format!(
        "D={ROLLBACK_DIR}/{token}; [ -f \"$D/restore.sh\" ] || {{ echo 'no snapshot to restore' >&2; exit 2; }}; \
systemctl stop {ROLLBACK_UNIT}.timer >/dev/null 2>&1; if [ -f \"$D/pid\" ]; then kill \"$(cat \"$D/pid\")\" 2>/dev/null; fi; sh \"$D/restore.sh\""
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const UFW: &str = "@@detect\nufw\nnft\n@@armed\n@@ufwstatus\nStatus: active\nLogging: on (low)\nDefault: deny (incoming), allow (outgoing), disabled (routed)\nNew profiles: skip\n\
@@ufwadded\nAdded user rules (see 'ufw status' for running firewall):\nufw allow 22/tcp\nufw allow from 192.168.1.0/24 to any port 80,443 proto tcp comment 'lan web'\nufw deny 23\n\
@@ufwdefault\nDEFAULT_INPUT_POLICY=\"DROP\"\n@@raw\ntable inet filter {\n}\n@@end\n";

    const FWD: &str = "@@detect\nfirewalld running\nnft\n@@armed\nyes\n@@fwdefault\npublic\n@@fwactive\npublic\n  interfaces: eth0\ntrusted\n  sources: 10.0.0.0/8\n\
@@zone public\npublic (active)\n  target: default\n  icmp-block-inversion: no\n  interfaces: eth0\n  sources: \n  services: dhcpv6-client ssh cockpit\n  ports: 8080/tcp 5353/udp\n  protocols: \n  forward: yes\n  masquerade: no\n  forward-ports: \n  source-ports: \n  icmp-blocks: \n  rich rules: \n\trule family=\"ipv4\" source address=\"192.168.1.20\" accept\n\
@@zone trusted\ntrusted (active)\n  target: ACCEPT\n  sources: 10.0.0.0/8\n  services: \n  ports: \n  rich rules: \n@@raw\n@@end\n";

    #[test]
    fn reads_ufw() {
        let r = parse_read(UFW);
        assert_eq!(r.backend, Backend::Ufw);
        assert!(r.active && !r.rollback_armed);
        let u = r.ufw.as_ref().unwrap();
        assert_eq!((u.default_in.as_str(), u.default_out.as_str()), ("deny", "allow"));
        assert_eq!(u.rules.len(), 3);
        assert_eq!(u.rules[1].comment.as_deref(), Some("lan web"));
        assert_eq!(u.rules[2].action, "deny");
        assert!(r.raw.contains("table inet filter"));
        // Inactive ufw falls back to /etc/default/ufw for the policy.
        let r = parse_read(&UFW.replace(
            "Status: active\nLogging: on (low)\nDefault: deny (incoming), allow (outgoing), disabled (routed)\n",
            "Status: inactive\n",
        ));
        assert!(!r.active);
        assert_eq!(r.ufw.unwrap().default_in, "deny");
    }

    #[test]
    fn reads_firewalld() {
        let r = parse_read(FWD);
        assert_eq!(r.backend, Backend::Firewalld);
        assert!(r.active && r.rollback_armed);
        assert_eq!(r.zones.len(), 2);
        let z = &r.zones[0];
        assert!(z.default && z.active);
        assert_eq!(z.services, vec!["dhcpv6-client", "ssh", "cockpit"]);
        assert_eq!(z.ports, vec!["8080/tcp", "5353/udp"]);
        assert_eq!(z.interfaces, vec!["eth0"]);
        assert_eq!(z.rich_rules, vec!["rule family=\"ipv4\" source address=\"192.168.1.20\" accept"]);
        assert_eq!(r.zones[1].target, "ACCEPT");
        assert_eq!(r.zones[1].sources, vec!["10.0.0.0/8"]);
    }

    #[test]
    fn builds_change_commands() {
        let add = FwChange::UfwAdd {
            action: UfwAction::Allow,
            port: "8443".into(),
            proto: Proto::Tcp,
            from: Some("192.168.1.0/24".into()),
            comment: Some("dash board".into()),
        };
        assert_eq!(
            change_command(&add).unwrap(),
            "ufw allow proto tcp from 192.168.1.0/24 to any port 8443 comment 'dash board'"
        );
        let bad = |c: FwChange| assert!(change_command(&c).is_err(), "{c:?}");
        bad(FwChange::UfwAdd {
            action: UfwAction::Allow,
            port: "0".into(),
            proto: Proto::Tcp,
            from: None,
            comment: None,
        });
        bad(FwChange::UfwAdd {
            action: UfwAction::Allow,
            port: "1000:2000".into(),
            proto: Proto::Any,
            from: None,
            comment: None,
        });
        bad(FwChange::UfwAdd {
            action: UfwAction::Allow,
            port: "22".into(),
            proto: Proto::Tcp,
            from: Some("1.2.3.4; rm".into()),
            comment: None,
        });
        bad(FwChange::UfwAdd {
            action: UfwAction::Allow,
            port: "22".into(),
            proto: Proto::Tcp,
            from: None,
            comment: Some("it's".into()),
        });
        assert_eq!(
            change_command(&FwChange::UfwDelete {
                spec: "allow from 192.168.1.0/24 to any port 80,443 proto tcp comment 'lan web'".into()
            })
            .unwrap(),
            "ufw --force delete 'allow' 'from' '192.168.1.0/24' 'to' 'any' 'port' '80,443' 'proto' 'tcp'"
        );
        bad(FwChange::UfwDelete { spec: "status; reboot".into() });
        bad(FwChange::UfwDelete { spec: "allow 22$(reboot)".into() });
        bad(FwChange::UfwDefault { policy: "open".into() });
        assert!(change_command(&FwChange::FirewalldService {
            zone: "public".into(),
            service: "http".into(),
            add: true
        })
        .unwrap()
        .contains("--add-service=http"));
        assert!(change_command(&FwChange::FirewalldPort {
            zone: "public".into(),
            port: "8000-8100".into(),
            proto: Proto::Tcp,
            add: false
        })
        .unwrap()
        .contains("--remove-port=8000-8100/tcp"));
        let src = change_command(&FwChange::FirewalldSource {
            zone: "public".into(),
            source: "fd00::/8".into(),
            port: Some("22".into()),
            proto: Proto::Tcp,
            add: true,
        })
        .unwrap();
        assert!(src.contains("--add-rich-rule='rule family=\"ipv6\" source address=\"fd00::/8\" port port=\"22\" protocol=\"tcp\" accept'"), "{src}");
        bad(FwChange::FirewalldService { zone: "public; x".into(), service: "http".into(), add: true });
    }

    #[test]
    fn spots_lockouts() {
        let r = parse_read(UFW);
        let ssh_rule = "allow 22/tcp".to_owned();
        assert!(lockout_risk(&r, &FwChange::UfwDelete { spec: ssh_rule.clone() }, 22).is_some());
        assert!(lockout_risk(&r, &FwChange::UfwDelete { spec: "deny 23".into() }, 22).is_none());
        assert!(lockout_risk(&r, &FwChange::UfwDelete { spec: ssh_rule.clone() }, 2222).is_none());
        assert!(lockout_risk(
            &r,
            &FwChange::UfwAdd {
                action: UfwAction::Deny,
                port: "22".into(),
                proto: Proto::Tcp,
                from: None,
                comment: None
            },
            22
        )
        .is_some());
        assert!(lockout_risk(&r, &FwChange::UfwEnable { enabled: true }, 22).is_none());
        let mut no_ssh = r.clone();
        no_ssh.ufw.as_mut().unwrap().rules.remove(0);
        assert!(lockout_risk(&no_ssh, &FwChange::UfwEnable { enabled: true }, 22).is_some());
        assert!(lockout_risk(&no_ssh, &FwChange::UfwDefault { policy: "deny".into() }, 22).is_some());
        // A from-rule with no port covers every port.
        no_ssh.ufw.as_mut().unwrap().rules.push(UfwRule {
            spec: "allow from 100.64.0.0/10".into(),
            action: "allow".into(),
            comment: None,
        });
        assert!(lockout_risk(&no_ssh, &FwChange::UfwEnable { enabled: true }, 22).is_none());
        let f = parse_read(FWD);
        assert!(lockout_risk(
            &f,
            &FwChange::FirewalldService { zone: "public".into(), service: "ssh".into(), add: false },
            22
        )
        .is_some());
        assert!(lockout_risk(
            &f,
            &FwChange::FirewalldPort { zone: "public".into(), port: "20-30".into(), proto: Proto::Tcp, add: false },
            22
        )
        .is_some());
        assert!(lockout_risk(
            &f,
            &FwChange::FirewalldPort { zone: "public".into(), port: "8080".into(), proto: Proto::Tcp, add: false },
            22
        )
        .is_none());
        assert_eq!(spec_ports("allow OpenSSH"), Some(vec![(22, 22)]));
        assert_eq!(spec_ports("limit in 1000:2000/udp"), Some(vec![(1000, 2000)]));
    }

    #[test]
    fn rollback_scripts() {
        let c = FwChange::UfwEnable { enabled: true };
        let s = apply_command(&c, "1700000000", 120).unwrap();
        assert!(s.contains("D=/var/lib/nightgrid/fw-rollback/1700000000"));
        assert!(s.contains("--on-active=120s"));
        assert!(s.contains("if ufw --force enable >\"$D/out\""));
        assert!(apply_command(&c, "17; rm", 120).is_err());
        assert!(apply_command(&c, "1", 5).unwrap().contains("--on-active=30s"));
        assert!(confirm_command("1").unwrap().contains("systemctl stop nightgrid-fw-rollback.timer"));
        assert!(rollback_command("1").unwrap().contains("sh \"$D/restore.sh\""));
        assert!(apply_command(
            &FwChange::FirewalldService { zone: "public".into(), service: "http".into(), add: true },
            "1",
            60
        )
        .unwrap()
        .contains("cp -a /etc/firewalld"));
    }
}
