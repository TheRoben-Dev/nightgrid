//! Proxmox VE part 5 (M10r): a node's network. Proxmox stages every change
//! in `/etc/network/interfaces.new` and only applies it on request
//! (`ifreload`), so edits here are staged, shown as a diff, then applied
//! with a rollback armed on the node: if the change cuts this computer off,
//! the node restores the old file by itself.

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{explain, get, s, sections, valid_node};
use crate::files::path::shell_quote;

pub const ROLLBACK_DIR: &str = "/var/lib/nightgrid/pve-net-rollback";
pub const ROLLBACK_UNIT: &str = "nightgrid-pve-net-rollback";

// ---------------------------------------------------------------- types

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveIface {
    pub iface: String,
    /// bridge, bond, eth, vlan, OVSBridge…
    pub kind: String,
    pub active: bool,
    pub autostart: bool,
    /// The NIC is physically there (eth).
    pub exists: bool,
    pub cidr: String,
    pub gateway: String,
    pub cidr6: String,
    pub gateway6: String,
    /// Bridge ports or bond slaves, space separated.
    pub ports: String,
    pub vlan_aware: bool,
    pub bond_mode: String,
    pub vlan_id: Option<u32>,
    pub vlan_raw_device: String,
    pub mtu: Option<u32>,
    pub comments: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveNetwork {
    pub ifaces: Vec<PveIface>,
    /// `diff -u` of the staged changes, empty when nothing waits.
    pub diff: String,
    /// A rollback from an earlier apply is still armed.
    pub rollback_armed: bool,
    pub notes: Vec<String>,
}

/// A bridge, bond or VLAN to create or change (or an eth's address and comment).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveIfaceSpec {
    pub iface: String,
    pub kind: String,
    pub cidr: String,
    pub gateway: String,
    pub cidr6: String,
    pub gateway6: String,
    /// Bridge ports or bond slaves, space separated.
    pub ports: String,
    pub vlan_aware: bool,
    pub bond_mode: String,
    pub vlan_id: Option<u32>,
    pub vlan_raw_device: String,
    pub mtu: Option<u32>,
    pub autostart: bool,
    pub comments: String,
}

// ---------------------------------------------------------------- validation

pub fn valid_iface(n: &str) -> bool {
    (2..=20).contains(&n.len())
        && n.starts_with(|c: char| c.is_ascii_alphabetic())
        && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}

fn valid_ports(p: &str) -> bool {
    p.split_whitespace().count() <= 16 && p.split_whitespace().all(valid_iface)
}

fn valid_v4(a: &str) -> bool {
    let p: Vec<&str> = a.split('.').collect();
    p.len() == 4 && p.iter().all(|x| !x.is_empty() && x.len() <= 3 && x.parse::<u8>().is_ok())
}

fn valid_v6(a: &str) -> bool {
    a.contains(':') && a.len() <= 45 && a.chars().all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
}

fn valid_cidr(c: &str, v6: bool) -> bool {
    c.split_once('/').is_some_and(|(a, n)| {
        let max = if v6 { 128 } else { 32 };
        (if v6 { valid_v6(a) } else { valid_v4(a) }) && n.parse::<u8>().is_ok_and(|n| (1..=max).contains(&n))
    })
}

pub const BOND_MODES: &[&str] =
    &["balance-rr", "active-backup", "balance-xor", "broadcast", "802.3ad", "balance-tlb", "balance-alb"];
const KINDS: &[&str] = &["bridge", "bond", "vlan", "eth"];

fn check(s: &PveIfaceSpec) -> Result<(), String> {
    if !valid_iface(&s.iface) || !KINDS.contains(&s.kind.as_str()) {
        return Err("invalid interface".into());
    }
    if s.kind == "bridge" && !(s.iface.starts_with("vmbr") && s.iface[4..].parse::<u16>().is_ok_and(|n| n <= 9999)) {
        return Err("a Linux bridge is named vmbr plus a number (vmbr1)".into());
    }
    if !s.cidr.is_empty() && !valid_cidr(&s.cidr, false) {
        return Err("IPv4: an address with prefix, like 192.168.1.20/24".into());
    }
    if !s.gateway.is_empty() && (!valid_v4(&s.gateway) || s.cidr.is_empty()) {
        return Err("gateway: an IPv4 address, only with an IPv4 address set".into());
    }
    if !s.cidr6.is_empty() && !valid_cidr(&s.cidr6, true) {
        return Err("IPv6: an address with prefix".into());
    }
    if !s.gateway6.is_empty() && (!valid_v6(&s.gateway6) || s.cidr6.is_empty()) {
        return Err("IPv6 gateway: only with an IPv6 address set".into());
    }
    if !valid_ports(&s.ports) {
        return Err("ports: interface names separated by spaces".into());
    }
    if s.kind == "bond" && (s.ports.trim().is_empty() || !BOND_MODES.contains(&s.bond_mode.as_str())) {
        return Err("a bond needs at least one slave and a mode".into());
    }
    if s.kind == "vlan" {
        let id_ok = s.vlan_id.is_some_and(|v| (1..=4094).contains(&v));
        let named = s
            .iface
            .rsplit_once('.')
            .is_some_and(|(dev, id)| valid_iface(dev) && id.parse::<u32>().is_ok_and(|v| (1..=4094).contains(&v)));
        if !named && !(id_ok && valid_iface(&s.vlan_raw_device)) {
            return Err("a VLAN is named device.tag (vmbr0.20), or needs a tag and a raw device".into());
        }
    }
    if s.mtu.is_some_and(|m| !(1280..=65520).contains(&m)) {
        return Err("MTU: 1280 to 65520".into());
    }
    if s.comments.len() > 200 || s.comments.chars().any(|c| c.is_control()) {
        return Err("comment: one line, up to 200 characters".into());
    }
    Ok(())
}

// ---------------------------------------------------------------- scripts

/// The node's interfaces, the staged diff and whether a rollback is armed.
pub fn network_script(node: &str) -> Option<String> {
    valid_node(node).then(|| {
        format!(
            "{}\necho '@@diff'; [ -f /etc/network/interfaces.new ] && diff -u /etc/network/interfaces /etc/network/interfaces.new; \
echo '@@armed'; systemctl is-active --quiet {ROLLBACK_UNIT}.timer 2>/dev/null && echo yes; \
for p in {ROLLBACK_DIR}/*/pid; do [ -f \"$p\" ] && kill -0 \"$(cat \"$p\")\" 2>/dev/null && echo yes; done; true",
            get(&format!("/nodes/{node}/network"), "")
        )
    })
}

fn params(s: &PveIfaceSpec, edit: bool) -> (Vec<(&'static str, String)>, Vec<&'static str>) {
    let mut set: Vec<(&'static str, String)> = vec![("type", s.kind.clone())];
    let mut del: Vec<&'static str> = Vec::new();
    let opt = |k: &'static str, v: &str, set: &mut Vec<(&'static str, String)>, del: &mut Vec<&'static str>| {
        if v.trim().is_empty() {
            if edit {
                del.push(k);
            }
        } else {
            set.push((k, v.trim().to_owned()));
        }
    };
    let mtu = s.mtu.map(|m| m.to_string()).unwrap_or_default();
    for (k, v) in [
        ("cidr", &s.cidr),
        ("gateway", &s.gateway),
        ("cidr6", &s.cidr6),
        ("gateway6", &s.gateway6),
        ("comments", &s.comments),
        ("mtu", &mtu),
    ] {
        opt(k, v, &mut set, &mut del);
    }
    match s.kind.as_str() {
        "bridge" => {
            opt("bridge_ports", &s.ports, &mut set, &mut del);
            if s.vlan_aware {
                set.push(("bridge_vlan_aware", "1".into()));
            } else if edit {
                del.push("bridge_vlan_aware");
            }
        }
        "bond" => {
            set.push(("slaves", s.ports.trim().to_owned()));
            set.push(("bond_mode", s.bond_mode.clone()));
        }
        "vlan" if !s.iface.contains('.') => {
            set.push(("vlan-id", s.vlan_id.unwrap_or(0).to_string()));
            set.push(("vlan-raw-device", s.vlan_raw_device.clone()));
        }
        _ => {}
    }
    set.push(("autostart", if s.autostart { "1".into() } else { "0".into() }));
    (set, del)
}

/// Stages a new interface (nothing changes until apply).
pub fn iface_create_script(node: &str, s: &PveIfaceSpec) -> Result<String, String> {
    if !valid_node(node) {
        return Err("invalid node".into());
    }
    check(s)?;
    if s.kind == "eth" {
        return Err("physical interfaces can't be created".into());
    }
    let (set, _) = params(s, false);
    let mut c =
        format!("pvesh create {} --iface {}", shell_quote(&format!("/nodes/{node}/network")), shell_quote(&s.iface));
    for (k, v) in set {
        c.push_str(&format!(" --{k} {}", shell_quote(&v)));
    }
    c.push_str(" --output-format json 2>&1 && echo @@ok");
    Ok(c)
}

/// Stages a change to an interface.
pub fn iface_update_script(node: &str, s: &PveIfaceSpec) -> Result<String, String> {
    if !valid_node(node) {
        return Err("invalid node".into());
    }
    check(s)?;
    let (set, del) = params(s, true);
    let mut c = format!("pvesh set {}", shell_quote(&format!("/nodes/{node}/network/{}", s.iface)));
    for (k, v) in set {
        c.push_str(&format!(" --{k} {}", shell_quote(&v)));
    }
    if !del.is_empty() {
        c.push_str(&format!(" --delete {}", del.join(",")));
    }
    c.push_str(" --output-format json 2>&1 && echo @@ok");
    Ok(c)
}

/// Stages removing an interface.
pub fn iface_delete_script(node: &str, iface: &str) -> Option<String> {
    (valid_node(node) && valid_iface(iface)).then(|| {
        format!(
            "pvesh delete {} --output-format json 2>&1 && echo @@ok",
            shell_quote(&format!("/nodes/{node}/network/{iface}"))
        )
    })
}

/// Drops every staged change.
pub fn revert_script(node: &str) -> Option<String> {
    valid_node(node).then(|| {
        format!(
            "pvesh delete {} --output-format json 2>&1 && echo @@ok",
            shell_quote(&format!("/nodes/{node}/network"))
        )
    })
}

/// What an apply printed: the task it started, or why it didn't.
pub fn parse_apply(out: &str) -> Result<Option<String>, String> {
    if !out.lines().any(|l| l.starts_with("@@armed")) {
        return Err(explain(out));
    }
    if out.lines().any(|l| l.trim() == "@@failed") {
        let why: String = out.lines().filter(|l| !l.starts_with("@@")).collect::<Vec<_>>().join("\n");
        return Err(format!("{} (the old config was restored)", explain(&why)));
    }
    let rest: String = out.lines().filter(|l| !l.starts_with("@@")).collect::<Vec<_>>().join("\n");
    Ok(super::parse_upid(&rest).ok())
}

pub fn valid_token(t: &str) -> bool {
    !t.is_empty() && t.len() <= 12 && t.bytes().all(|b| b.is_ascii_digit())
}

const RESTORE: &str =
    "D=$(dirname \"$0\"); cp -a \"$D/interfaces\" /etc/network/interfaces; rm -f /etc/network/interfaces.new; \
ifreload -a >/dev/null 2>&1; rm -f \"$D/pid\"; echo restored\n";

/// Saves the live config, arms a restore in `secs`, then applies the staged
/// changes (Proxmox's own apply, `ifreload`). Prints `@@armed …` and the task id.
pub fn apply_script(node: &str, token: &str, secs: u32) -> Option<String> {
    if !valid_node(node) || !valid_token(token) {
        return None;
    }
    let restore = base64::engine::general_purpose::STANDARD.encode(RESTORE);
    let secs = secs.clamp(30, 900);
    Some(format!(
        "[ -f /etc/network/interfaces.new ] || {{ echo 'no staged changes to apply' >&2; exit 2; }}; \
umask 077; D={ROLLBACK_DIR}/{token}; mkdir -p \"$D\" || exit 2; \
(cd {ROLLBACK_DIR} && ls -1t | tail -n +6 | while IFS= read -r f; do [ \"$f\" = {token} ] || rm -rf -- \"$f\"; done); \
cp -a /etc/network/interfaces \"$D/interfaces\" || {{ echo 'could not save the current network config' >&2; exit 2; }}; \
printf '%s' '{restore}' | base64 -d > \"$D/restore.sh\"; \
if command -v systemd-run >/dev/null 2>&1 && [ -d /run/systemd/system ]; then \
systemctl stop {ROLLBACK_UNIT}.timer >/dev/null 2>&1; systemctl reset-failed {ROLLBACK_UNIT}.timer {ROLLBACK_UNIT}.service >/dev/null 2>&1; \
systemd-run --quiet --unit={ROLLBACK_UNIT} --on-active={secs}s /bin/sh \"$D/restore.sh\" && echo '@@armed systemd' || {{ echo 'could not arm the rollback' >&2; exit 2; }}; \
else setsid nohup sh -c \"sleep {secs}; sh '$D/restore.sh'\" >/dev/null 2>&1 < /dev/null & echo $! > \"$D/pid\"; echo '@@armed sleep'; fi; \
if o=$(pvesh set {} --output-format json 2>&1); then echo \"$o\"; else echo \"$o\" >&2; sh \"$D/restore.sh\" >/dev/null 2>&1; \
systemctl stop {ROLLBACK_UNIT}.timer >/dev/null 2>&1; [ -f \"$D/pid\" ] && kill \"$(cat \"$D/pid\")\" 2>/dev/null; echo @@failed; fi",
        shell_quote(&format!("/nodes/{node}/network"))
    ))
}

/// Keeps the applied change: disarms the rollback.
pub fn confirm_script(token: &str) -> Option<String> {
    valid_token(token).then(|| {
        format!(
            "D={ROLLBACK_DIR}/{token}; systemctl stop {ROLLBACK_UNIT}.timer >/dev/null 2>&1; \
if [ -f \"$D/pid\" ]; then kill \"$(cat \"$D/pid\")\" 2>/dev/null; rm -f \"$D/pid\"; fi; echo @@ok"
        )
    })
}

/// Restores the saved config now and disarms the timer.
pub fn rollback_script(token: &str) -> Option<String> {
    valid_token(token).then(|| {
        format!(
            "D={ROLLBACK_DIR}/{token}; [ -f \"$D/restore.sh\" ] || {{ echo 'no saved config to restore' >&2; exit 2; }}; \
systemctl stop {ROLLBACK_UNIT}.timer >/dev/null 2>&1; if [ -f \"$D/pid\" ]; then kill \"$(cat \"$D/pid\")\" 2>/dev/null; fi; \
sh \"$D/restore.sh\" && echo @@ok"
        )
    })
}

// ---------------------------------------------------------------- parsing

fn num(v: &Value, k: &str) -> Option<u32> {
    s(v, k).trim().parse().ok()
}

fn flag(v: &Value, k: &str) -> bool {
    matches!(s(v, k).as_str(), "1" | "true") || v[k].as_bool() == Some(true)
}

fn order(kind: &str) -> u8 {
    match kind {
        "bridge" | "OVSBridge" => 0,
        "bond" | "OVSBond" => 1,
        "vlan" | "OVSIntPort" => 2,
        "eth" => 3,
        _ => 4,
    }
}

pub fn parse_network(out: &str) -> Result<PveNetwork, String> {
    let secs = sections(out);
    let (_, all) = secs.first().ok_or_else(|| explain(out))?;
    let body = all.split("@@diff").next().unwrap_or(all);
    let v: Value = serde_json::from_str(body.trim()).map_err(|_| explain(body))?;
    let items = v.as_array().ok_or_else(|| explain(body))?;
    let mut ifaces: Vec<PveIface> = items
        .iter()
        .map(|i| PveIface {
            iface: s(i, "iface"),
            kind: s(i, "type"),
            active: flag(i, "active"),
            autostart: flag(i, "autostart"),
            exists: flag(i, "exists"),
            cidr: s(i, "cidr"),
            gateway: s(i, "gateway"),
            cidr6: s(i, "cidr6"),
            gateway6: s(i, "gateway6"),
            ports: {
                let p = s(i, "bridge_ports");
                if p.is_empty() {
                    s(i, "slaves")
                } else {
                    p
                }
            },
            vlan_aware: flag(i, "bridge_vlan_aware"),
            bond_mode: s(i, "bond_mode"),
            vlan_id: num(i, "vlan-id"),
            vlan_raw_device: s(i, "vlan-raw-device"),
            mtu: num(i, "mtu"),
            comments: s(i, "comments").trim().to_owned(),
        })
        .filter(|i| !i.iface.is_empty())
        .collect();
    ifaces.sort_by(|a, b| order(&a.kind).cmp(&order(&b.kind)).then_with(|| a.iface.cmp(&b.iface)));
    let rest = out.split_once("@@diff").map(|(_, r)| r).unwrap_or("");
    let (diff, armed) = rest.split_once("@@armed").unwrap_or((rest, ""));
    Ok(PveNetwork {
        ifaces,
        diff: diff.trim_matches('\n').to_owned(),
        rollback_armed: armed.lines().any(|l| l.trim() == "yes"),
        notes: Vec::new(),
    })
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxmox::MARK;

    fn spec(iface: &str, kind: &str) -> PveIfaceSpec {
        PveIfaceSpec {
            iface: iface.into(),
            kind: kind.into(),
            cidr: String::new(),
            gateway: String::new(),
            cidr6: String::new(),
            gateway6: String::new(),
            ports: String::new(),
            vlan_aware: false,
            bond_mode: String::new(),
            vlan_id: None,
            vlan_raw_device: String::new(),
            mtu: None,
            autostart: true,
            comments: String::new(),
        }
    }

    #[test]
    fn parses_interfaces_diff_and_rollback() {
        let out = format!(
            "{MARK} /nodes/pve1/network\n{}\n@@diff\n--- /etc/network/interfaces\n+++ /etc/network/interfaces.new\n@@ -1 +1,2 @@\n+auto vmbr2\n@@armed\nyes\n",
            include_str!("fixtures/network_all.json")
        );
        let n = parse_network(&out).unwrap();
        assert_eq!(
            n.ifaces.iter().map(|i| i.iface.as_str()).collect::<Vec<_>>(),
            ["vmbr0", "vmbr1", "bond0", "vmbr0.20", "enp3s0", "enp4s0", "wlp5s0"]
        );
        let v0 = &n.ifaces[0];
        assert_eq!(
            (v0.cidr.as_str(), v0.gateway.as_str(), v0.ports.as_str(), v0.comments.as_str()),
            ("192.168.1.20/24", "192.168.1.1", "bond0", "LAN")
        );
        assert!(n.ifaces[1].vlan_aware);
        assert_eq!((n.ifaces[2].ports.as_str(), n.ifaces[2].bond_mode.as_str()), ("enp3s0 enp4s0", "active-backup"));
        assert_eq!((n.ifaces[3].vlan_id, n.ifaces[3].vlan_raw_device.as_str()), (Some(20), "vmbr0"));
        assert!(!n.ifaces[6].active && n.ifaces[6].exists);
        assert!(n.diff.starts_with("--- /etc/network/interfaces") && n.diff.ends_with("+auto vmbr2"));
        assert!(n.rollback_armed);
        let clean = parse_network(&format!("{MARK} /nodes/pve1/network\n[]\n@@diff\n@@armed\n")).unwrap();
        assert!(clean.diff.is_empty() && !clean.rollback_armed);
        assert!(parse_network("pvesh: command not found").is_err());
        assert!(network_script("pve1")
            .unwrap()
            .contains("diff -u /etc/network/interfaces /etc/network/interfaces.new"));
    }

    #[test]
    fn stages_bridges_bonds_and_vlans() {
        let mut b = spec("vmbr2", "bridge");
        b.ports = "enp4s0".into();
        b.vlan_aware = true;
        b.cidr = "10.0.0.2/24".into();
        let c = iface_create_script("pve1", &b).unwrap();
        assert!(
            c.starts_with("pvesh create '/nodes/pve1/network' --iface 'vmbr2' --type 'bridge' --cidr '10.0.0.2/24'")
        );
        assert!(c.contains("--bridge_ports 'enp4s0' --bridge_vlan_aware '1' --autostart '1'"));
        let u = iface_update_script("pve1", &b).unwrap();
        assert!(u.starts_with("pvesh set '/nodes/pve1/network/vmbr2' --type 'bridge'"));
        assert!(u.contains("--delete gateway,cidr6,gateway6,comments,mtu"));
        let mut bond = spec("bond1", "bond");
        assert!(iface_create_script("pve1", &bond).is_err(), "needs slaves");
        bond.ports = "enp3s0 enp4s0".into();
        bond.bond_mode = "802.3ad".into();
        assert!(iface_create_script("pve1", &bond).unwrap().contains("--slaves 'enp3s0 enp4s0' --bond_mode '802.3ad'"));
        let v = spec("vmbr0.30", "vlan");
        assert!(iface_create_script("pve1", &v).is_ok());
        let mut v2 = spec("vlan30", "vlan");
        assert!(iface_create_script("pve1", &v2).is_err());
        v2.vlan_id = Some(30);
        v2.vlan_raw_device = "vmbr0".into();
        assert!(iface_create_script("pve1", &v2).unwrap().contains("--vlan-id '30' --vlan-raw-device 'vmbr0'"));
        assert!(iface_create_script("pve1", &spec("br0", "bridge")).is_err(), "bridges are vmbrN");
        assert!(iface_create_script("pve1", &spec("enp9s0", "eth")).is_err());
        let mut bad = spec("vmbr3", "bridge");
        bad.gateway = "10.0.0.1".into();
        assert!(iface_create_script("pve1", &bad).is_err(), "gateway without address");
        bad.gateway.clear();
        bad.ports = "enp3s0; reboot".into();
        assert!(iface_create_script("pve1", &bad).is_err());
        assert!(iface_delete_script("pve1", "vmbr2").unwrap().contains("pvesh delete '/nodes/pve1/network/vmbr2'"));
        assert!(iface_delete_script("pve1", "x;y").is_none());
        assert_eq!(
            revert_script("pve1").unwrap(),
            "pvesh delete '/nodes/pve1/network' --output-format json 2>&1 && echo @@ok"
        );
    }

    #[test]
    fn applies_with_a_rollback() {
        let a = apply_script("pve1", "1700000000", 90).unwrap();
        assert!(a.contains("D=/var/lib/nightgrid/pve-net-rollback/1700000000"));
        assert!(a.contains("--on-active=90s"));
        assert!(a.contains("if o=$(pvesh set '/nodes/pve1/network' --output-format json 2>&1); then"));
        assert!(a.ends_with("echo @@failed; fi"));
        assert!(apply_script("pve1", "17x", 90).is_none());
        let upid = "UPID:pve1:0001A2B3:0C1D2E3F:67000000:srvreload:networking:root@pam:";
        assert_eq!(parse_apply(&format!("@@armed systemd\n\"{upid}\"")).unwrap().as_deref(), Some(upid));
        assert!(parse_apply("@@armed systemd\nerror: bad config\n@@failed").unwrap_err().contains("restored"));
        assert!(parse_apply("no staged changes to apply").is_err());
        assert!(confirm_script("1700000000").unwrap().contains("systemctl stop nightgrid-pve-net-rollback.timer"));
        assert!(rollback_script("1700000000").unwrap().contains("sh \"$D/restore.sh\""));
        let restore = String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(a.split("printf '%s' '").nth(1).unwrap().split('\'').next().unwrap())
                .unwrap(),
        )
        .unwrap();
        assert!(restore.contains("cp -a \"$D/interfaces\" /etc/network/interfaces") && restore.contains("ifreload -a"));
        // The generated shell parses (Windows runners have no /bin/sh).
        if !cfg!(unix) {
            return;
        }
        for script in
            [a, confirm_script("1").unwrap(), rollback_script("1").unwrap(), network_script("pve1").unwrap(), restore]
        {
            let ok = std::process::Command::new("/bin/sh").arg("-n").arg("-c").arg(&script).status().unwrap().success();
            assert!(ok, "{script}");
        }
    }
}
