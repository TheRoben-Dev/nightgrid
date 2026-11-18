//! Sandbox clones (M12c): a throwaway copy of a guest, cut off from the
//! network, to try an upgrade or a risky change on. Made in two steps: the
//! clone (a Proxmox task), then `isolate`, which rewrites the copy's network
//! cards, drops passthrough devices and host bind mounts, marks it with tags
//! and arms an expiry timer on the node.
//!
//! Everything about a sandbox lives on Proxmox itself, in its tags:
//! `nightgrid-sandbox`, `ng-sbx-of-<source vmid>` and `ng-sbx-until-<unix>`.
//! The expiry is a transient systemd timer on the node, so it fires while
//! Nightgrid is closed. It checks the guest still carries the exact tags it
//! was armed for before it destroys anything, so a reused id or an extended
//! sandbox is never touched by a stale timer.

use serde::{Deserialize, Serialize};

use super::backup::{valid_snapname, valid_storage};
use super::create::{valid_bridge, valid_guest_name};
use super::hardware::{options, PveConfig, PveSetting};
use super::{create, valid_kind, valid_node, valid_vmid};
use crate::files::path::shell_quote;

pub const TAG: &str = "nightgrid-sandbox";
pub const OF: &str = "ng-sbx-of-";
pub const UNTIL: &str = "ng-sbx-until-";
/// A sandbox lives at most 30 days per arming; "never" is `hours = 0`.
pub const MAX_HOURS: u32 = 720;

// ---------------------------------------------------------------- types

/// How the copy is kept off the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PveIsolation {
    /// Every network card has its cable pulled (`link_down=1`).
    Cut,
    /// Every network card moves to one bridge (ideally one with no ports),
    /// without its VLAN tag, so sandboxes can talk to each other only.
    Bridge,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveSandboxSpec {
    pub node: String,
    pub kind: String,
    /// The guest being copied.
    pub vmid: u32,
    pub newid: u32,
    pub name: String,
    /// Copy this snapshot instead of the current state.
    pub snapshot: Option<String>,
    /// A full copy; a linked clone needs a template as the source.
    pub full: bool,
    pub storage: Option<String>,
    pub isolation: PveIsolation,
    /// Only with `Bridge`.
    pub bridge: Option<String>,
    /// Destroyed after this many hours; 0 keeps it until you act.
    pub hours: u32,
    pub start: bool,
}

/// What `isolate` changes on the copy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveSandboxPlan {
    pub set: Vec<PveSetting>,
    pub delete: Vec<String>,
    /// Plain sentences for the person: what was cut, moved or dropped.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveSandboxDone {
    pub notes: Vec<String>,
    /// Unix time it expires, if it does.
    pub until: Option<f64>,
    /// The start task, when asked to start.
    pub upid: Option<String>,
}

// ---------------------------------------------------------- validation

pub fn check(s: &PveSandboxSpec) -> Result<(), String> {
    if !valid_node(&s.node) || !valid_kind(&s.kind) || !valid_vmid(s.vmid) || !valid_vmid(s.newid) || s.vmid == s.newid
    {
        return Err("invalid source or new id".into());
    }
    if !valid_guest_name(&s.name) {
        return Err("name: letters, digits, - and . only".into());
    }
    if s.snapshot.as_deref().is_some_and(|n| !valid_snapname(n)) {
        return Err("invalid snapshot name".into());
    }
    if s.snapshot.is_some() && !s.full {
        return Err("a copy of a snapshot is always a full clone".into());
    }
    if s.storage.as_deref().is_some_and(|x| !valid_storage(x)) || (s.storage.is_some() && !s.full) {
        return Err("a target storage only goes with a full clone".into());
    }
    match (s.isolation, s.bridge.as_deref()) {
        (PveIsolation::Bridge, Some(b)) if valid_bridge(b) => {}
        (PveIsolation::Bridge, _) => return Err("pick the bridge sandboxes go on".into()),
        (PveIsolation::Cut, Some(_)) => return Err("a bridge only goes with bridge isolation".into()),
        (PveIsolation::Cut, None) => {}
    }
    if s.hours > MAX_HOURS {
        return Err(format!("at most {MAX_HOURS} hours; extend it later if you need longer"));
    }
    Ok(())
}

// ------------------------------------------------------------- scripts

/// Starts the clone; prints the task's UPID.
pub fn clone_script(s: &PveSandboxSpec) -> Result<String, String> {
    check(s)?;
    let mut p: Vec<(&str, String)> = vec![("newid", s.newid.to_string())];
    p.push((if s.kind == "qemu" { "name" } else { "hostname" }, s.name.clone()));
    p.push(("full", if s.full { "1" } else { "0" }.into()));
    if let Some(st) = &s.storage {
        p.push(("storage", st.clone()));
    }
    if let Some(sn) = &s.snapshot {
        p.push(("snapname", sn.clone()));
    }
    p.push(("description", format!("Nightgrid sandbox of {} {}.", if s.kind == "qemu" { "VM" } else { "CT" }, s.vmid)));
    Ok(create(&format!("/nodes/{}/{}/{}/clone", s.node, s.kind, s.vmid), &p))
}

fn unit(vmid: u32) -> String {
    format!("nightgrid-sandbox-{vmid}")
}

fn stop_timer(vmid: u32) -> String {
    let u = unit(vmid);
    format!(
        "systemctl stop {u}.timer {u}.service >/dev/null 2>&1; systemctl reset-failed {u}.timer {u}.service >/dev/null 2>&1"
    )
}

/// What the timer runs: stop and destroy the guest, only if it still
/// carries the tags this timer was armed for.
pub fn expire_body(node: &str, kind: &str, vmid: u32, until: u64) -> Option<String> {
    (valid_node(node) && valid_kind(kind) && valid_vmid(vmid)).then(|| {
        format!(
            "p=/nodes/{node}/{kind}/{vmid}\n\
             c=$(pvesh get \"$p/config\" --output-format json 2>/dev/null) || exit 0\n\
             case \"$c\" in *{TAG}*) ;; *) exit 0 ;; esac\n\
             case \"$c\" in *{UNTIL}{until}*) ;; *) exit 0 ;; esac\n\
             pvesh create \"$p/status/stop\" >/dev/null 2>&1\n\
             i=0\n\
             while [ $i -lt 90 ]; do\n\
             pvesh get \"$p/status/current\" --output-format json 2>/dev/null | grep -q '\"status\":\"stopped\"' && break\n\
             sleep 2; i=$((i+1))\n\
             done\n\
             pvesh delete \"$p\" --purge 1 --destroy-unreferenced-disks 1\n"
        )
    })
}

/// Arms (or re-arms) the expiry timer on the node this runs on. A node
/// restart drops it; the view shows such a sandbox as overdue.
pub fn arm_script(node: &str, kind: &str, vmid: u32, until: u64, now: u64) -> Option<String> {
    let body = expire_body(node, kind, vmid, until)?;
    let secs = until.saturating_sub(now).max(60);
    Some(format!(
        "{}\nsystemd-run --quiet --unit={} --description={} --on-active={secs}s /bin/sh -c {} 2>&1 && echo @@ok",
        stop_timer(vmid),
        unit(vmid),
        shell_quote(&format!("Nightgrid sandbox {vmid} expiry")),
        shell_quote(&body)
    ))
}

/// Drops the expiry timer (keep, never-expire, or destroy now).
pub fn disarm_script(vmid: u32) -> Option<String> {
    valid_vmid(vmid).then(|| format!("{}; echo @@ok", stop_timer(vmid)))
}

pub fn parse_ok(out: &str) -> Result<(), String> {
    if out.lines().any(|l| l.trim() == "@@ok") {
        Ok(())
    } else {
        Err(super::explain(out))
    }
}

// --------------------------------------------------------------- tags

fn is_ours(t: &str) -> bool {
    t == TAG || t.starts_with(OF) || t.starts_with(UNTIL)
}

/// The tags of a config entry, as Proxmox stores them (`a;b`).
fn tags_of(cfg: &PveConfig) -> Vec<String> {
    cfg.entries
        .iter()
        .find(|e| e.key == "tags")
        .and_then(|e| e.pending.clone().or_else(|| e.value.clone()))
        .unwrap_or_default()
        .split([';', ',', ' '])
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The guest's own tags with ours replaced: `source` and `until` set the
/// sandbox marks, `None` for both removes them all (keep it).
pub fn retag(cfg: &PveConfig, source: Option<u32>, until: Option<u64>, sandbox: bool) -> String {
    let mut t: Vec<String> = tags_of(cfg).into_iter().filter(|t| !is_ours(t)).collect();
    if sandbox {
        t.push(TAG.into());
        if let Some(s) = source.or_else(|| source_of(cfg)) {
            t.push(format!("{OF}{s}"));
        }
        if let Some(u) = until {
            t.push(format!("{UNTIL}{u}"));
        }
    }
    t.join(";")
}

/// The source vmid recorded in the tags.
pub fn source_of(cfg: &PveConfig) -> Option<u32> {
    tags_of(cfg).iter().find_map(|t| t.strip_prefix(OF).and_then(|n| n.parse().ok()))
}

pub fn is_sandbox(cfg: &PveConfig) -> bool {
    tags_of(cfg).iter().any(|t| t == TAG)
}

// --------------------------------------------------------------- plan

fn join(o: &[(String, String)]) -> String {
    o.iter().map(|(k, v)| if k.is_empty() { v.clone() } else { format!("{k}={v}") }).collect::<Vec<_>>().join(",")
}

/// Sets `k=v` in an option list, replacing an earlier value.
fn put(o: &mut Vec<(String, String)>, k: &str, v: &str) {
    match o.iter_mut().find(|(a, _)| a == k) {
        Some(p) => p.1 = v.to_owned(),
        None => o.push((k.to_owned(), v.to_owned())),
    }
}

/// What to change on a fresh copy so it can't reach the network or the
/// source's hardware and data.
pub fn plan(cfg: &PveConfig, s: &PveSandboxSpec, until: Option<u64>) -> PveSandboxPlan {
    let mut p = PveSandboxPlan::default();
    for n in cfg.nics.iter().filter(|n| !n.delete) {
        let mut o = options(&n.value);
        match s.isolation {
            PveIsolation::Cut => {
                put(&mut o, "link_down", "1");
                p.notes.push(format!("{}: cable pulled (link down)", n.key));
            }
            PveIsolation::Bridge => {
                let b = s.bridge.clone().unwrap_or_default();
                put(&mut o, "bridge", &b);
                o.retain(|(k, _)| k != "tag" && k != "trunks" && k != "link_down");
                p.notes.push(format!(
                    "{}: moved from {} to {b}",
                    n.key,
                    if n.bridge.is_empty() { "?" } else { &n.bridge }
                ));
            }
        }
        p.set.push(PveSetting { key: n.key.clone(), value: join(&o) });
    }
    if cfg.nics.iter().all(|n| n.delete) {
        p.notes.push("It has no network cards.".into());
    }
    // Passthrough devices belong to the source: two guests can't hold them.
    for d in &cfg.devices {
        if !d.delete {
            p.delete.push(d.key.clone());
            p.notes.push(format!("{}: passthrough removed", d.key));
        }
    }
    // Container bind mounts point at the node's own folders, shared with the
    // source: a sandbox writing there would change real data.
    if s.kind == "lxc" {
        for d in &cfg.disks {
            if d.key.starts_with("mp") && d.volid.starts_with('/') {
                p.delete.push(d.key.clone());
                p.notes.push(format!("{}: bind mount of {} removed", d.key, d.volid));
            }
        }
    }
    p.set.push(PveSetting { key: "onboot".into(), value: "0".into() });
    p.set.push(PveSetting { key: "tags".into(), value: retag(cfg, Some(s.vmid), until, true) });
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxmox::hardware::parse_config;
    use crate::proxmox::MARK;

    fn cfg(json: &str, kind: &str) -> PveConfig {
        parse_config(&format!("{MARK} /nodes/pve1/{kind}/9000/pending\n{json}\n"), kind).unwrap()
    }

    fn spec(kind: &str, vmid: u32) -> PveSandboxSpec {
        PveSandboxSpec {
            node: "pve1".into(),
            kind: kind.into(),
            vmid,
            newid: 9000,
            name: "media-sandbox".into(),
            snapshot: None,
            full: true,
            storage: None,
            isolation: PveIsolation::Cut,
            bridge: None,
            hours: 24,
            start: false,
        }
    }

    fn val<'a>(p: &'a PveSandboxPlan, k: &str) -> &'a str {
        &p.set.iter().find(|s| s.key == k).unwrap().value
    }

    #[test]
    fn checks() {
        assert!(check(&spec("qemu", 100)).is_ok());
        let mut s = spec("qemu", 100);
        s.newid = 100;
        assert!(check(&s).is_err());
        s = spec("qemu", 100);
        s.snapshot = Some("before-upgrade".into());
        s.full = false;
        assert!(check(&s).unwrap_err().contains("full"));
        s = spec("qemu", 100);
        s.isolation = PveIsolation::Bridge;
        assert!(check(&s).is_err());
        s.bridge = Some("vmbr9".into());
        assert!(check(&s).is_ok());
        s.bridge = Some("vmbr9; rm -rf /".into());
        assert!(check(&s).is_err());
        s = spec("qemu", 100);
        s.hours = MAX_HOURS + 1;
        assert!(check(&s).is_err());
        s = spec("lxc", 200);
        s.name = "bad name".into();
        assert!(check(&s).is_err());
    }

    #[test]
    fn clone_from_snapshot() {
        let mut s = spec("qemu", 100);
        s.snapshot = Some("before-upgrade".into());
        let c = clone_script(&s).unwrap();
        assert!(c.starts_with("pvesh create '/nodes/pve1/qemu/100/clone'"));
        assert!(c.contains("--newid '9000'"));
        assert!(c.contains("--name 'media-sandbox'"));
        assert!(c.contains("--snapname 'before-upgrade'"));
        assert!(c.contains("--full '1'"));
        let mut s = spec("lxc", 200);
        s.name = "pihole-sandbox".into();
        assert!(clone_script(&s).unwrap().contains("--hostname 'pihole-sandbox'"));
    }

    #[test]
    fn vm_plan_cuts_cables_and_drops_passthrough() {
        let c = cfg(include_str!("fixtures/pending_qemu.json"), "qemu");
        let p = plan(&c, &spec("qemu", 100), Some(1_800_000_000));
        // net1 is pending removal in the fixture, so only net0 is rewritten.
        assert_eq!(val(&p, "net0"), "virtio=BC:24:11:5A:3B:01,bridge=vmbr0,firewall=1,link_down=1");
        assert!(!p.set.iter().any(|s| s.key == "net1"));
        assert_eq!(p.delete, vec!["hostpci0".to_owned(), "usb0".to_owned()]);
        assert_eq!(val(&p, "onboot"), "0");
        assert_eq!(val(&p, "tags"), "media;prod;nightgrid-sandbox;ng-sbx-of-100;ng-sbx-until-1800000000");
        assert!(p.notes.iter().any(|n| n.contains("passthrough")));
    }

    #[test]
    fn ct_plan_moves_bridge_and_drops_bind_mounts() {
        let json = include_str!("fixtures/pending_lxc.json")
            .replace("\"tags\",\"value\":\"dns\"", "\"tags\",\"value\":\"dns;ng-sbx-until-5\"")
            .replace(
                "{\"key\":\"unprivileged\"",
                "{\"key\":\"mp1\",\"value\":\"/srv/media,mp=/media\"},{\"key\":\"unprivileged\"",
            );
        let c = cfg(&json, "lxc");
        let mut s = spec("lxc", 200);
        s.isolation = PveIsolation::Bridge;
        s.bridge = Some("vmbr9".into());
        let p = plan(&c, &s, None);
        let net0 = val(&p, "net0");
        assert!(net0.contains("bridge=vmbr9"));
        assert!(net0.contains("ip=192.168.1.53/24"));
        assert!(!net0.contains("vmbr0"));
        // mp0 is a storage volume (copied by the clone); mp1 is a host folder.
        assert_eq!(p.delete, vec!["mp1".to_owned()]);
        // Old sandbox marks are replaced, and no expiry means no until tag.
        assert_eq!(val(&p, "tags"), "dns;nightgrid-sandbox;ng-sbx-of-200");
    }

    #[test]
    fn keep_removes_only_our_tags() {
        let c = cfg(
            r#"[{"key":"digest","value":"ab"},{"key":"tags","value":"web;nightgrid-sandbox;ng-sbx-of-101;ng-sbx-until-9"}]"#,
            "qemu",
        );
        assert!(is_sandbox(&c));
        assert_eq!(source_of(&c), Some(101));
        assert_eq!(retag(&c, None, None, false), "web");
        assert_eq!(retag(&c, None, Some(42), true), "web;nightgrid-sandbox;ng-sbx-of-101;ng-sbx-until-42");
    }

    #[test]
    fn timer_guards_on_tags() {
        let a = arm_script("pve1", "qemu", 9000, 1_000_600, 1_000_000).unwrap();
        assert!(a.contains("--unit=nightgrid-sandbox-9000"));
        assert!(a.contains("--on-active=600s"));
        assert!(a.contains("systemctl stop nightgrid-sandbox-9000.timer"));
        let b = expire_body("pve1", "qemu", 9000, 1_000_600).unwrap();
        assert!(b.contains("*nightgrid-sandbox*"));
        assert!(b.contains("*ng-sbx-until-1000600*"));
        assert!(b.contains("pvesh delete \"$p\" --purge 1"));
        // An expiry already in the past still waits a minute.
        assert!(arm_script("pve1", "lxc", 9000, 5, 1_000).unwrap().contains("--on-active=60s"));
        assert!(arm_script("pve1;x", "lxc", 9000, 5, 1).is_none());
        assert!(parse_ok("noise\n@@ok\n").is_ok());
        assert!(parse_ok("Failed to start transient timer unit").is_err());
    }
}
