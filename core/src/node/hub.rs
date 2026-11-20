//! Nightgrid Hub and node setup (D-175): the app's side of nightgrid-node's
//! hub profile (its `docs/HUB.md` and app-integration spec 0020) and of the
//! node-to-node link (spec 0016).
//!
//! One always-on host per network runs `nightgrid-hub` next to its node. The
//! app makes other nodes members of that hub: it pairs each member with the
//! hub's collector over the link (`peers.set`), names the hub on the member
//! (`hub.member.set`), lets root on the member serve it (`serve add sync`)
//! and gives the hub its member list (`fleet.members.set`). The hub then
//! pulls the members' history read-only and raises "host offline" while the
//! app is closed. Nothing here changes how the app syncs each host itself.
//!
//! Also the root-owned config files of both (`/etc/nightgrid-node/config.toml`,
//! `/etc/nightgrid-hub/config.toml`): reading, the few edits the setup needs
//! (comment-preserving, through `toml_edit`), and saving with a rollback when
//! the service doesn't come back.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use toml_edit::{value, Array, DocumentMut, Item, Table};

use crate::files::path::shell_quote;

pub const HUB_SOCKET: &str = "/run/nightgrid-hub/hub.sock";
pub const HUB_GROUP: &str = "nightgrid-hub";
pub const NODE_CONFIG: &str = "/etc/nightgrid-node/config.toml";
pub const HUB_CONFIG: &str = "/etc/nightgrid-hub/config.toml";
/// The node's link port for `listen = "tailnet"`.
pub const LINK_PORT: u16 = 7419;
/// The hub collector's port for `listen = "tailnet"`.
pub const COLLECT_PORT: u16 = 7420;
/// Members need this node version (`hub.member` 1).
pub const MEMBER_MIN_VERSION: &str = "0.26.0";
/// The hub's `[collect] listen` takes a list (so it can answer LAN-only
/// members' pings next to the tailnet address) from this version.
pub const LISTEN_LIST_MIN_VERSION: &str = "0.28.0";
/// The link's default `[peers] networks` (and the collector's).
pub const TAILNET_NETWORKS: &[&str] = &["100.64.0.0/10", "fd7a:115c:a1e0::/48"];

/// `nightgrid-hub-v0.26.0-x86_64-linux`.
pub fn asset_name(tag: &str, arch: &str) -> String {
    format!("nightgrid-hub-{tag}-{arch}-linux")
}

/// Which program a config, log or status belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum NodePart {
    Node,
    Hub,
}

impl NodePart {
    pub fn binary(self) -> &'static str {
        match self {
            Self::Node => "nightgrid-node",
            Self::Hub => "nightgrid-hub",
        }
    }

    pub fn config(self) -> &'static str {
        match self {
            Self::Node => NODE_CONFIG,
            Self::Hub => HUB_CONFIG,
        }
    }

    /// Its systemd units, main one first.
    pub fn units(self) -> &'static [&'static str] {
        match self {
            Self::Node => &[
                "nightgrid-node.service",
                "nightgrid-node-peer.service",
                "nightgrid-node-checks.service",
                "nightgrid-node-netwatch.service",
                "nightgrid-node-status.service",
                "nightgrid-node-helper.socket",
                "nightgrid-node-health.socket",
            ],
            Self::Hub => &["nightgrid-hub.service", "nightgrid-hub-collect.service"],
        }
    }

    /// Config sections whose `networks` are copied into a unit by an
    /// `allow` (which has to run again after they change), with that
    /// `allow`'s argument and unit.
    fn network_grants(self) -> &'static [(&'static str, &'static str, &'static str)] {
        match self {
            Self::Node => &[
                ("peers", "peers", "nightgrid-node-peer.service"),
                ("checks", "checks", "nightgrid-node-checks.service"),
                ("statuspage", "statuspage", "nightgrid-node-status.service"),
            ],
            Self::Hub => &[("collect", "collect", "nightgrid-hub-collect.service")],
        }
    }
}

// ---- probing the hub ----

/// What the app finds about the hub on a host. One read-only script.
pub const PROBE: &str = r#"u=$(id -un 2>/dev/null)
command -v nightgrid-hub >/dev/null 2>&1 && echo "@@bin yes" || echo "@@bin no"
[ -S __SOCK__ ] && echo "@@sock yes" || echo "@@sock no"
[ -w __SOCK__ ] && echo "@@writable yes" || echo "@@writable no"
getent group __GROUP__ 2>/dev/null | cut -d: -f4 | tr ',' '\n' | grep -qx "$u" && echo "@@member yes" || echo "@@member no"
echo "@@unit $(systemctl is-active nightgrid-hub.service 2>/dev/null)"
echo "@@collect $(systemctl is-active nightgrid-hub-collect.service 2>/dev/null)"
nightgrid-hub version 2>/dev/null | sed 's/^nightgrid-hub /@@version /'
"#;

pub fn probe_script() -> String {
    PROBE.replace("__SOCK__", HUB_SOCKET).replace("__GROUP__", HUB_GROUP)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HubProbe {
    pub installed: bool,
    pub socket: bool,
    pub writable: bool,
    pub member: bool,
    /// `systemctl is-active` of the hub and its collector.
    pub unit: String,
    pub collect: String,
    pub version: String,
}

impl HubProbe {
    /// The socket can be used from this login, directly or through `sg`.
    pub fn usable(&self) -> bool {
        self.socket && (self.writable || self.member)
    }
}

pub fn parse_probe(out: &str) -> HubProbe {
    let mut p = HubProbe::default();
    for l in out.lines() {
        let Some(rest) = l.strip_prefix("@@") else { continue };
        let (k, v) = rest.split_once(' ').unwrap_or((rest, ""));
        let v = v.trim();
        match k {
            "bin" => p.installed = v == "yes",
            "sock" => p.socket = v == "yes",
            "writable" => p.writable = v == "yes",
            "member" => p.member = v == "yes",
            "unit" => p.unit = v.into(),
            "collect" => p.collect = v.into(),
            "version" => p.version = v.into(),
            _ => {}
        }
    }
    p
}

/// One request to the hub's socket with its own CLI (`nightgrid-hub query`),
/// through `sg` when this login predates the group.
pub fn query_command(p: &HubProbe, method: &str, params: &serde_json::Value) -> String {
    let inner = format!("nightgrid-hub query {} {}", shell_quote(method), shell_quote(&params.to_string()));
    if p.writable {
        inner
    } else {
        format!("sg {HUB_GROUP} -c {}", shell_quote(&inner))
    }
}

/// The install (or update) as root.
pub fn install_command(binary: &str, user: &str) -> String {
    format!("{} install --json --user {}", shell_quote(binary), shell_quote(user))
}

pub fn uninstall_command(purge: bool) -> String {
    format!("nightgrid-hub uninstall --json{}", if purge { " --purge" } else { "" })
}

// ---- protocol types (the parts the app uses) ----

/// `peers.get` / `peers.set` entry, and a hub member.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct NodePeer {
    pub id: String,
    pub name: String,
    pub address: String,
    pub fingerprint: String,
    /// Only on peers: this host follows that peer's UPS shutdown.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub follow_ups: bool,
}

/// `peers.get` result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct NodePeerList {
    pub peers: Vec<NodePeer>,
    pub interval: u32,
    pub serve: Vec<String>,
}

/// One reading in `peers.latest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct NodePeerReading {
    pub id: String,
    pub at: f64,
    pub ok: bool,
    pub rtt_ms: Option<f64>,
    pub version: Option<String>,
    pub hostname: Option<String>,
    pub error: Option<String>,
}

/// `peers.latest` result.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct NodePeersLatest {
    /// `ok`, `off` (link not allowed) or `denied` (it stopped).
    pub access: String,
    pub fingerprint: Option<String>,
    pub listening: Option<String>,
    pub readings: Vec<NodePeerReading>,
}

/// A member in `fleet.overview`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct HubMemberState {
    pub id: String,
    pub name: String,
    /// `new`, `ok`, `stale`, `unreachable`, `consent` or `version`.
    pub state: String,
    pub last_contact: Option<f64>,
    pub version: Option<String>,
    pub hostname: Option<String>,
    pub alerts: u32,
    pub error: Option<String>,
}

/// `fleet.overview` result.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct HubOverview {
    pub hub_id: String,
    pub fingerprint: Option<String>,
    pub listening: Option<String>,
    /// Every address the collector answers on (0.28.0+; empty before).
    pub listen_all: Vec<String>,
    /// `ok`, `off` (never ran: `allow collect`) or `denied` (stopped).
    pub collector: String,
    pub store_bytes: f64,
    pub members: Vec<HubMemberState>,
}

/// `fleet.members.get` / `.set`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Members {
    pub members: Vec<NodePeer>,
}

/// The `--json` report of `nightgrid-hub install` / `allow collect`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct HubReport {
    pub action: String,
    pub version: String,
    pub steps: Vec<String>,
    pub users: Vec<String>,
    pub warnings: Vec<String>,
    pub hub_id: Option<String>,
    pub fingerprint: Option<String>,
    pub listening: Option<String>,
}

/// The last JSON line of a `--json` command; plain text becomes steps.
pub fn parse_report(out: &str) -> HubReport {
    out.lines()
        .rev()
        .filter(|l| l.trim_start().starts_with('{'))
        .find_map(|l| serde_json::from_str(l.trim()).ok())
        .unwrap_or_else(|| HubReport {
            steps: out.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect(),
            ..Default::default()
        })
}

/// `{"hubId": …}` both ways.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HubMember {
    pub hub_id: Option<String>,
}

/// Adds or replaces `p` (by id) in a peer list.
pub fn upsert(list: &mut Vec<NodePeer>, p: NodePeer) {
    list.retain(|x| x.id != p.id && x.fingerprint != p.fingerprint);
    list.push(p);
}

/// A name the node accepts (1–80 characters).
pub fn peer_name(s: &str) -> String {
    let t: String = s.trim().chars().take(80).collect();
    if t.is_empty() {
        "host".into()
    } else {
        t
    }
}

// ---- addresses and networks ----

/// The IP of an `ip:port` / `[v6]:port` address.
pub fn addr_ip(addr: &str) -> Option<IpAddr> {
    addr.parse::<std::net::SocketAddr>().ok().map(|s| s.ip()).or_else(|| addr.parse().ok())
}

/// `ip:port`, bracketing IPv6.
pub fn join_addr(ip: IpAddr, port: u16) -> String {
    std::net::SocketAddr::new(ip, port).to_string()
}

pub fn is_tailnet(ip: IpAddr) -> bool {
    TAILNET_NETWORKS.iter().any(|n| in_cidr(ip, n))
}

/// `ip` is inside `cidr` (`10.0.0.0/8`, `fd00::/8`, or a bare address).
pub fn in_cidr(ip: IpAddr, cidr: &str) -> bool {
    let (net, bits) = match cidr.split_once('/') {
        Some((n, b)) => (n, b.parse::<u32>().ok()),
        None => (cidr, None),
    };
    let Ok(net) = net.trim().parse::<IpAddr>() else { return false };
    match (ip, net) {
        (IpAddr::V4(a), IpAddr::V4(n)) => {
            let bits = bits.unwrap_or(32).min(32);
            let mask = if bits == 0 { 0 } else { u32::MAX << (32 - bits) };
            u32::from(a) & mask == u32::from(n) & mask
        }
        (IpAddr::V6(a), IpAddr::V6(n)) => {
            let bits = bits.unwrap_or(128).min(128);
            let mask = if bits == 0 { 0 } else { u128::MAX << (128 - bits) };
            u128::from(a) & mask == u128::from(n) & mask
        }
        _ => false,
    }
}

pub fn covered(ip: IpAddr, networks: &[String]) -> bool {
    networks.iter().any(|n| in_cidr(ip, n))
}

/// A single-address network for `ip`.
pub fn host_cidr(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(_) => format!("{ip}/32"),
        IpAddr::V6(_) => format!("{ip}/128"),
    }
}

/// The source address in `ip -o route get X` output (`… src 192.168.1.5 …`).
pub fn parse_route_src(out: &str) -> Option<IpAddr> {
    let mut it = out.split_whitespace();
    while let Some(w) = it.next() {
        if w == "src" {
            return it.next()?.parse().ok();
        }
    }
    None
}

/// The first address `getent ahosts` lists, IPv4 first.
pub fn parse_getent(out: &str) -> Option<IpAddr> {
    let ips: Vec<IpAddr> = out.lines().filter_map(|l| l.split_whitespace().next()?.parse().ok()).collect();
    ips.iter().find(|i| i.is_ipv4()).or(ips.first()).copied()
}

/// Run on the hub host: where it reaches `target` from, and `target`'s
/// address as it resolves there.
pub fn route_script(target: &str) -> String {
    let t = shell_quote(target);
    format!(
        "a=$(getent ahosts {t} 2>/dev/null | awk '{{print $1; exit}}'); [ -n \"$a\" ] || a={t}; echo \"@@ip $a\"; ip -o route get \"$a\" 2>/dev/null | sed 's/^/@@route /'"
    )
}

/// `(target ip, source ip)` from [`route_script`].
pub fn parse_route_script(out: &str) -> (Option<IpAddr>, Option<IpAddr>) {
    let mut ip = None;
    let mut src = None;
    for l in out.lines() {
        if let Some(v) = l.strip_prefix("@@ip ") {
            ip = parse_getent(v);
        } else if let Some(v) = l.strip_prefix("@@route ") {
            src = src.or_else(|| parse_route_src(v));
        }
    }
    (ip, src)
}

/// The host's own addresses (`hostname -I`-like), one per line.
pub const ADDRS_SCRIPT: &str = "ip -o addr show 2>/dev/null | awk '{print $4}' | cut -d/ -f1";

pub fn parse_addrs(out: &str) -> Vec<IpAddr> {
    out.lines().filter_map(|l| l.trim().parse().ok()).collect()
}

// ---- config files ----

pub fn parse_config(text: &str) -> Result<DocumentMut, String> {
    text.parse::<DocumentMut>().map_err(|e| format!("not valid TOML: {}", e.to_string().trim()))
}

/// The top-level sections each program accepts (both refuse unknown ones).
fn known_sections(part: NodePart) -> &'static [&'static str] {
    match part {
        NodePart::Node => &[],
        NodePart::Hub => &["server", "store", "collect", "alerts"],
    }
}

/// A first check before a save: valid TOML and, for the hub (which has no
/// `doctor`), only sections it knows. The node's own `doctor --config`
/// checks its file on the host before it is put in place.
pub fn precheck(part: NodePart, text: &str) -> Result<(), String> {
    let doc = parse_config(text)?;
    let known = known_sections(part);
    if !known.is_empty() {
        for (k, _) in doc.iter() {
            if !known.contains(&k) {
                return Err(format!("[{k}] isn't a {} section ({})", part.binary(), known.join(", ")));
            }
        }
    }
    Ok(())
}

fn table<'a>(doc: &'a mut DocumentMut, section: &str) -> &'a mut Table {
    if !doc.contains_table(section) {
        doc[section] = Item::Table(Table::new());
    }
    doc[section].as_table_mut().expect("just made a table")
}

pub fn get_str(doc: &DocumentMut, section: &str, key: &str) -> Option<String> {
    doc.get(section)?.get(key)?.as_str().map(str::to_owned)
}

/// A string list, or `default` when the key isn't set.
pub fn get_list(doc: &DocumentMut, section: &str, key: &str, default: &[&str]) -> Vec<String> {
    match doc.get(section).and_then(|s| s.get(key)).and_then(Item::as_array) {
        Some(a) => a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect(),
        None => default.iter().map(|s| (*s).to_owned()).collect(),
    }
}

pub fn set_str(doc: &mut DocumentMut, section: &str, key: &str, v: &str) {
    table(doc, section)[key] = value(v);
}

pub fn set_list(doc: &mut DocumentMut, section: &str, key: &str, list: &[String]) {
    let mut a = Array::new();
    for s in list {
        a.push(s.as_str());
    }
    table(doc, section)[key] = value(a);
}

/// The hub's `[collect] listen` entries: one string (before 0.28.0) or a
/// list; `tailnet` when unset.
pub fn get_listen(doc: &DocumentMut) -> Vec<String> {
    match doc.get("collect").and_then(|s| s.get("listen")) {
        Some(i) if i.as_str().is_some() => vec![i.as_str().unwrap_or_default().to_owned()],
        Some(i) => {
            i.as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default()
        }
        None => vec!["tailnet".into()],
    }
}

/// Adds `addr` (`ip:port`) to the hub's `[collect] listen`, keeping what
/// is there, so the collector also answers pings on it. `true` when the
/// file changed.
pub fn add_listen(doc: &mut DocumentMut, addr: &str) -> bool {
    let mut l = get_listen(doc);
    if l.iter().any(|e| e == addr) {
        return false;
    }
    l.retain(|e| !e.is_empty());
    l.push(addr.to_owned());
    set_list(doc, "collect", "listen", &l);
    true
}

/// Adds `cidr` to `[section] networks` unless an entry already covers it.
/// `true` when the file changed.
pub fn add_network(doc: &mut DocumentMut, section: &str, ip: IpAddr) -> bool {
    let mut nets = get_list(doc, section, "networks", TAILNET_NETWORKS);
    if covered(ip, &nets) {
        return false;
    }
    nets.push(host_cidr(ip));
    set_list(doc, section, "networks", &nets);
    true
}

/// The `allow` commands to run again after a save, for each section whose
/// `networks` changed and whose unit root already allowed.
pub fn reapply_after(part: NodePart, old: &str, new: &str) -> Vec<(&'static str, &'static str)> {
    let (Ok(a), Ok(b)) = (parse_config(old), parse_config(new)) else { return vec![] };
    part.network_grants()
        .iter()
        .filter(|(section, _, _)| get_list(&a, section, "networks", &[]) != get_list(&b, section, "networks", &[]))
        .map(|(_, grant, unit)| (*grant, *unit))
        .collect()
}

/// Saves a config as root from `tmp` (already uploaded): keeps the old file
/// as `config.toml.bak`, puts the new one in place, restarts what's running,
/// runs `allow` again where networks changed, and puts the old file back
/// (and restarts again) if the main service doesn't stay up.
pub fn save_script(part: NodePart, tmp: &str, reapply: &[(&str, &str)]) -> String {
    let cfg = part.config();
    let bin = part.binary();
    let units = part.units().iter().filter(|u| u.ends_with(".service")).copied().collect::<Vec<_>>().join(" ");
    let main = part.units()[0];
    let tmp = shell_quote(tmp);
    let mut s = format!(
        "set -u\nd=$(dirname {cfg}); mkdir -p \"$d\"\n[ -f {cfg} ] && cp -p {cfg} {cfg}.bak\ninstall -m 0644 -o root -g root {tmp} {cfg} || exit 1\necho \"@@step saved {cfg} (the old one is {cfg}.bak)\"\n"
    );
    if part == NodePart::Node {
        s.push_str(&format!(
            "if ! {bin} doctor --config {cfg} --json >/dev/null 2>&1; then {bin} doctor --config {cfg} 2>&1 | grep -i config | sed 's/^/@@check /'; fi\n"
        ));
    }
    s.push_str(&format!("systemctl try-restart {units} 2>&1\necho \"@@step restarted what was running\"\n"));
    for (grant, unit) in reapply {
        s.push_str(&format!(
            "if [ -f /etc/systemd/system/{unit} ]; then {bin} allow {grant} --json >/dev/null 2>&1 && echo \"@@step ran allow {grant} again for its networks\" || echo \"@@warn allow {grant} failed\"; fi\n"
        ));
    }
    s.push_str(&format!(
        "sleep 3\nif ! systemctl is-active --quiet {main}; then\n  echo \"@@failed {main} didn't stay up with the new config\"\n  journalctl -u {main} -n 8 --no-pager -o cat 2>/dev/null | sed 's/^/@@log /'\n  if [ -f {cfg}.bak ]; then cp -p {cfg}.bak {cfg}; systemctl try-restart {units}; echo \"@@step put the old config back\"; fi\n  exit 3\nfi\n"
    ));
    s
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ConfigSaveReport {
    pub ok: bool,
    pub steps: Vec<String>,
    pub warnings: Vec<String>,
    /// Why it was rolled back, with the service's last log lines.
    pub failure: Option<String>,
}

pub fn parse_save(out: &str, code: i32) -> ConfigSaveReport {
    let mut r = ConfigSaveReport { ok: code == 0, ..Default::default() };
    let mut log = Vec::new();
    for l in out.lines() {
        if let Some(v) = l.strip_prefix("@@step ") {
            r.steps.push(v.into());
        } else if let Some(v) = l.strip_prefix("@@warn ").or_else(|| l.strip_prefix("@@check ")) {
            r.warnings.push(v.trim().into());
        } else if let Some(v) = l.strip_prefix("@@failed ") {
            r.failure = Some(v.into());
        } else if let Some(v) = l.strip_prefix("@@log ") {
            log.push(v.to_owned());
        }
    }
    if !log.is_empty() {
        let f = r.failure.get_or_insert_with(|| "the service didn't stay up".into());
        f.push_str(":\n");
        f.push_str(&log.join("\n"));
    }
    if !r.ok && r.failure.is_none() {
        r.failure = Some("the save failed".into());
    }
    r
}

/// The last `lines` journal lines of a program's units.
pub fn logs_command(part: NodePart, lines: u32) -> String {
    let units: String = part.units().iter().map(|u| format!(" -u {u}")).collect();
    format!("journalctl{units} -n {} --no-pager -o short-iso 2>&1", lines.clamp(10, 2000))
}

/// `systemctl is-active` for each unit, one `unit state` per line.
pub fn units_script(part: NodePart) -> String {
    part.units().iter().map(|u| format!("echo \"{u} $(systemctl is-active {u} 2>/dev/null)\"\n")).collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NodeUnit {
    pub unit: String,
    /// `active`, `inactive`, `failed`, … Units that were never installed
    /// are left out.
    pub state: String,
}

pub fn parse_units(out: &str) -> Vec<NodeUnit> {
    out.lines()
        .filter_map(|l| {
            let (u, s) = l.trim().split_once(' ').unwrap_or((l.trim(), ""));
            let s = s.trim();
            ((!u.is_empty() && !s.is_empty() && s != "inactive")
                || u.ends_with("node.service")
                || u.ends_with("hub.service"))
            .then(|| NodeUnit { unit: u.into(), state: if s.is_empty() { "unknown".into() } else { s.into() } })
        })
        .collect()
}

/// One finding of `nightgrid-node doctor --json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct DoctorFinding {
    pub check: String,
    /// `ok`, `info`, `warn` or `fail`.
    pub level: String,
    pub detail: String,
}

pub fn parse_doctor(out: &str) -> Vec<DoctorFinding> {
    #[derive(Deserialize, Default)]
    #[serde(default)]
    struct R {
        findings: Vec<DoctorFinding>,
    }
    out.lines()
        .rev()
        .filter(|l| l.trim_start().starts_with('{'))
        .find_map(|l| serde_json::from_str::<R>(l.trim()).ok())
        .map(|r| r.findings)
        .unwrap_or_default()
}

/// `0.26.0` or newer.
pub fn at_least(version: &str, min: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v')
            .split(['-', '+'])
            .next()
            .unwrap_or("")
            .split('.')
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    let (a, b) = (parts(version), parts(min));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    true
}

/// How a member's link has to change so the hub can reach it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkPlan {
    /// The member's link address the hub dials.
    pub member_addr: String,
    /// New `[peers] listen` on the member (it had no tailnet address).
    pub listen: Option<String>,
    /// Add to the member's `[peers] networks`: where the hub connects from.
    pub member_network: Option<IpAddr>,
    /// Add to the hub's `[collect] networks`: the member's link address.
    pub hub_network: Option<IpAddr>,
}

/// Decides the link between a member and its hub.
///
/// - `listening`: the member's link address now (`peers.latest`), if any.
/// - `seen_ip`: the member's address as the hub host resolves it, and
///   `member_addrs` the member's own addresses (to check it's really its).
/// - `hub_src`: where the hub host connects to the member from.
/// - `member_networks` / `hub_networks`: the configs' lists now.
pub fn plan_link(
    listening: Option<&str>,
    seen_ip: Option<IpAddr>,
    member_addrs: &[IpAddr],
    hub_src: Option<IpAddr>,
    member_networks: &[String],
    hub_networks: &[String],
) -> Result<LinkPlan, String> {
    let mut plan = LinkPlan::default();
    let ip = match listening.and_then(addr_ip) {
        Some(ip) => {
            plan.member_addr = listening.unwrap_or_default().to_owned();
            ip
        }
        None => {
            // No tailnet address on the member: listen on the LAN address
            // the hub host reaches it at.
            let ip = seen_ip
                .filter(|ip| member_addrs.is_empty() || member_addrs.contains(ip))
                .filter(|ip| !ip.is_loopback() && !ip.is_unspecified())
                .ok_or(
                    "this host's link has no tailnet address, and its LAN address couldn't be found from the hub host",
                )?;
            let addr = join_addr(ip, LINK_PORT);
            plan.listen = Some(addr.clone());
            plan.member_addr = addr;
            ip
        }
    };
    if let Some(src) = hub_src {
        if !src.is_loopback() && !covered(src, member_networks) {
            plan.member_network = Some(src);
        }
    }
    if !covered(ip, hub_networks) && !ip.is_loopback() {
        plan.hub_network = Some(ip);
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn probe_and_query() {
        let p = parse_probe(
            "@@bin yes\n@@sock yes\n@@writable no\n@@member yes\n@@unit active\n@@collect inactive\n@@version 0.26.0\n",
        );
        assert!(p.installed && p.usable());
        assert_eq!((p.unit.as_str(), p.collect.as_str(), p.version.as_str()), ("active", "inactive", "0.26.0"));
        let q = query_command(&p, "fleet.overview", &serde_json::Value::Null);
        assert_eq!(q, "sg nightgrid-hub -c 'nightgrid-hub query '\\''fleet.overview'\\'' '\\''null'\\'''");
        let q =
            query_command(&HubProbe { writable: true, ..p }, "fleet.members.set", &serde_json::json!({"members": []}));
        assert_eq!(q, "nightgrid-hub query 'fleet.members.set' '{\"members\":[]}'");
        for s in
            [probe_script(), units_script(NodePart::Node), route_script("nas.lan"), logs_command(NodePart::Hub, 50)]
        {
            let out = std::process::Command::new("sh").arg("-n").arg("-c").arg(&s).output().unwrap();
            assert!(out.status.success(), "{s}");
        }
    }

    #[test]
    fn cidrs_and_routes() {
        assert!(in_cidr(ip("100.101.1.2"), "100.64.0.0/10"));
        assert!(!in_cidr(ip("192.168.0.5"), "100.64.0.0/10"));
        assert!(in_cidr(ip("192.168.0.5"), "192.168.0.5"));
        assert!(in_cidr(ip("fd7a:115c:a1e0::5"), "fd7a:115c:a1e0::/48"));
        assert!(!in_cidr(ip("10.0.0.1"), "fd00::/8"));
        assert!(in_cidr(ip("10.1.2.3"), "0.0.0.0/0"));
        assert!(is_tailnet(ip("100.64.0.7")));
        assert_eq!(
            parse_route_src("192.168.0.20 dev eth0 src 192.168.0.10 uid 1000 \\    cache"),
            Some(ip("192.168.0.10"))
        );
        assert_eq!(parse_route_src("garbage"), None);
        let (t, s) = parse_route_script("@@ip 192.168.0.20\n@@route 192.168.0.20 dev eth0 src 192.168.0.10 uid 0\n");
        assert_eq!((t, s), (Some(ip("192.168.0.20")), Some(ip("192.168.0.10"))));
        assert_eq!(parse_getent("::1 STREAM localhost\n127.0.0.1 STREAM localhost\n"), Some(ip("127.0.0.1")));
        assert_eq!(join_addr(ip("fd00::1"), 7419), "[fd00::1]:7419");
        assert_eq!(addr_ip("[fd00::1]:7419"), Some(ip("fd00::1")));
        assert_eq!(addr_ip("100.64.0.2:7420"), Some(ip("100.64.0.2")));
        assert_eq!(parse_addrs("127.0.0.1\n192.168.0.20\nfe80::1\n").len(), 3);
    }

    #[test]
    fn link_plans() {
        let tail: Vec<String> = TAILNET_NETWORKS.iter().map(|s| (*s).to_owned()).collect();
        // Both on the tailnet: nothing to change.
        let p = plan_link(Some("100.64.0.9:7419"), None, &[], Some(ip("100.64.0.2")), &tail, &tail).unwrap();
        assert_eq!(p, LinkPlan { member_addr: "100.64.0.9:7419".into(), ..Default::default() });
        // A LAN-only member behind the hub host.
        let lan = [ip("192.168.0.20"), ip("127.0.0.1")];
        let p = plan_link(None, Some(ip("192.168.0.20")), &lan, Some(ip("192.168.0.10")), &tail, &tail).unwrap();
        assert_eq!(p.listen.as_deref(), Some("192.168.0.20:7419"));
        assert_eq!(p.member_addr, "192.168.0.20:7419");
        assert_eq!(p.member_network, Some(ip("192.168.0.10")));
        assert_eq!(p.hub_network, Some(ip("192.168.0.20")));
        // Already allowed: nothing to add.
        let nets = vec!["192.168.0.0/24".to_owned()];
        let p = plan_link(Some("192.168.0.20:7419"), None, &[], Some(ip("192.168.0.10")), &nets, &nets).unwrap();
        assert_eq!((p.listen, p.member_network, p.hub_network), (None, None, None));
        // The address the hub sees isn't the member's: refuse to guess.
        assert!(plan_link(None, Some(ip("10.9.9.9")), &lan, None, &tail, &tail).is_err());
        assert!(plan_link(None, None, &lan, None, &tail, &tail).is_err());
    }

    #[test]
    fn config_edits_keep_comments() {
        let text = "# mine\n[peers]\n# where\nlisten = \"tailnet\"\nserve = [\"ping\"]\n";
        let mut doc = parse_config(text).unwrap();
        set_str(&mut doc, "peers", "listen", "192.168.0.20:7419");
        assert!(add_network(&mut doc, "peers", ip("192.168.0.10")));
        assert!(!add_network(&mut doc, "peers", ip("192.168.0.10")));
        assert!(!add_network(&mut doc, "peers", ip("100.64.0.3")));
        let out = doc.to_string();
        assert!(out.contains("# mine") && out.contains("# where"), "{out}");
        assert!(out.contains("listen = \"192.168.0.20:7419\""), "{out}");
        let nets = get_list(&parse_config(&out).unwrap(), "peers", "networks", &[]);
        assert_eq!(nets, vec!["100.64.0.0/10", "fd7a:115c:a1e0::/48", "192.168.0.10/32"]);
        let mut hub = parse_config("").unwrap();
        assert!(add_network(&mut hub, "collect", ip("192.168.0.20")));
        assert_eq!(get_str(&hub, "collect", "listen"), None);
        assert_eq!(reapply_after(NodePart::Node, text, &out), vec![("peers", "nightgrid-node-peer.service")]);
        assert!(reapply_after(NodePart::Node, text, text).is_empty());
        assert_eq!(
            reapply_after(NodePart::Hub, "", &hub.to_string()),
            vec![("collect", "nightgrid-hub-collect.service")]
        );
    }

    #[test]
    fn prechecks_and_saves() {
        assert!(precheck(NodePart::Hub, "[store]\nmax_gb = 4\n").is_ok());
        assert!(precheck(NodePart::Hub, "[peers]\n").unwrap_err().contains("isn't a nightgrid-hub section"));
        assert!(precheck(NodePart::Node, "[metrics\n").unwrap_err().contains("TOML"));
        for part in [NodePart::Node, NodePart::Hub] {
            let s = save_script(part, "/tmp/ng x/config.toml", &[("peers", "nightgrid-node-peer.service")]);
            let out = std::process::Command::new("sh").arg("-n").arg("-c").arg(&s).output().unwrap();
            assert!(out.status.success(), "{s}");
        }
        let r = parse_save("@@step saved\n@@failed nightgrid-hub.service didn't stay up\n@@log bad key\n@@step put the old config back\n", 3);
        assert!(!r.ok);
        assert_eq!(r.steps.len(), 2);
        assert!(r.failure.unwrap().ends_with(":\nbad key"));
        assert!(parse_save("@@step saved\n", 0).ok);
    }

    #[test]
    fn reports_units_and_versions() {
        let r = parse_report("{\"action\":\"allow\",\"version\":\"0.26.0\",\"steps\":[],\"users\":[],\"warnings\":[\"w\"],\"hubId\":\"ab\",\"fingerprint\":\"cd\",\"listening\":\"100.64.0.2:7420\"}\n");
        assert_eq!((r.hub_id.as_deref(), r.listening.as_deref()), (Some("ab"), Some("100.64.0.2:7420")));
        let u = parse_units("nightgrid-node.service active\nnightgrid-node-peer.service inactive\nnightgrid-node-checks.service failed\n");
        assert_eq!(
            u.iter().map(|u| u.unit.as_str()).collect::<Vec<_>>(),
            ["nightgrid-node.service", "nightgrid-node-checks.service"]
        );
        let f = parse_doctor(
            "{\"version\":\"0.26.0\",\"findings\":[{\"check\":\"config\",\"level\":\"ok\",\"detail\":\"fine\"}]}",
        );
        assert_eq!(f[0].check, "config");
        assert!(at_least("0.26.0", MEMBER_MIN_VERSION) && at_least("v0.27.1", MEMBER_MIN_VERSION));
        assert!(!at_least("0.25.9", MEMBER_MIN_VERSION));
        let mut l = vec![NodePeer { id: "a".into(), fingerprint: "1".into(), ..Default::default() }];
        upsert(&mut l, NodePeer { id: "a".into(), fingerprint: "2".into(), name: "hub".into(), ..Default::default() });
        assert_eq!((l.len(), l[0].fingerprint.as_str()), (1, "2"));
        assert_eq!(peer_name("  "), "host");
        assert_eq!(asset_name("v0.26.0", "aarch64"), "nightgrid-hub-v0.26.0-aarch64-linux");
    }

    #[test]
    fn collector_listens_on_lan_too() {
        // Unset: tailnet, and adding a LAN address keeps it.
        let mut d = parse_config("[collect]\ninterval_secs = 60\n").unwrap();
        assert_eq!(get_listen(&d), ["tailnet"]);
        assert!(add_listen(&mut d, "192.168.0.10:7420"));
        assert_eq!(get_listen(&d), ["tailnet", "192.168.0.10:7420"]);
        assert!(!add_listen(&mut d, "192.168.0.10:7420"), "already there");
        // One string (the 0.26 shape) becomes a list.
        let mut d = parse_config("# mine\n[collect]\nlisten = \"tailnet\"\n").unwrap();
        assert!(add_listen(&mut d, "192.168.0.10:7420"));
        let out = d.to_string();
        assert!(out.contains("listen = [\"tailnet\", \"192.168.0.10:7420\"]"), "{out}");
        assert!(out.contains("# mine"), "{out}");
        // An empty entry is dropped, others kept.
        let mut d = parse_config("[collect]\nlisten = [\"\", \"lan\"]\n").unwrap();
        assert!(add_listen(&mut d, "10.0.0.2:7420"));
        assert_eq!(get_listen(&d), ["lan", "10.0.0.2:7420"]);
        assert!(at_least("0.32.0", LISTEN_LIST_MIN_VERSION) && !at_least("0.26.0", LISTEN_LIST_MIN_VERSION));
        let o: HubOverview = serde_json::from_str("{\"hubId\":\"h\",\"listening\":\"100.64.0.2:7420\"}").unwrap();
        assert!(o.listen_all.is_empty());
    }
}
