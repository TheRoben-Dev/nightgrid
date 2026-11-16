//! Proxmox VE part 4 (M10r): a guest's hardware and options. Reads the
//! configuration with its pending changes (`/pending`), edits it through the
//! config API with the digest it was read at, grows disks, and lists the
//! node's PCI and USB devices for passthrough.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{explain, get, parse_upid, s, sections, valid_kind, valid_node, valid_vmid};
use crate::files::path::shell_quote;

// ---------------------------------------------------------------- types

/// One configuration key: what runs now, and what waits for a restart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveConfigEntry {
    pub key: String,
    /// Current value (none for a key only added as pending).
    pub value: Option<String>,
    /// New value that applies at the next restart.
    pub pending: Option<String>,
    /// Removal that applies at the next restart.
    pub delete: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveDisk {
    /// scsi0, virtio1, ide2, rootfs, mp0, efidisk0, tpmstate0…
    pub key: String,
    /// storage:volume, or "none" for an empty CD drive.
    pub volid: String,
    pub size: String,
    pub cdrom: bool,
    /// Container mount point path (mp*), or empty.
    pub mountpoint: String,
    /// The full value, options included.
    pub value: String,
    pub pending: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveNic {
    pub key: String,
    /// virtio, e1000… for VMs; veth for containers.
    pub model: String,
    pub mac: String,
    pub bridge: String,
    pub vlan: Option<u32>,
    pub firewall: bool,
    /// Container only: ip=… (dhcp, CIDR or empty).
    pub ip: String,
    pub value: String,
    pub pending: bool,
    pub delete: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveConfig {
    pub kind: String,
    /// Changes are sent with it, so a config someone else changed meanwhile is refused.
    pub digest: String,
    pub entries: Vec<PveConfigEntry>,
    pub disks: Vec<PveDisk>,
    pub nics: Vec<PveNic>,
    /// hostpci* and usb* entries.
    pub devices: Vec<PveConfigEntry>,
    /// Detached disks (unused*): still on the storage.
    pub unused: Vec<PveConfigEntry>,
    /// Any change waits for a restart.
    pub pending: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PvePci {
    /// 0000:01:00.0
    pub id: String,
    pub vendor: String,
    pub device: String,
    pub class: String,
    /// -1 when the IOMMU is off.
    pub iommu_group: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveUsb {
    /// vendor:product, as `usbN: host=` takes it.
    pub id: String,
    /// Bus-port path (1-2), for passing a port instead of a device.
    pub path: String,
    pub manufacturer: String,
    pub product: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveHostDevices {
    pub pci: Vec<PvePci>,
    pub usb: Vec<PveUsb>,
    /// IOMMU looks off (every PCI group is -1).
    pub iommu_off: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveSetting {
    pub key: String,
    pub value: String,
}

// ---------------------------------------------------------------- keys

fn indexed(k: &str, prefix: &str, max: u32) -> bool {
    k.strip_prefix(prefix).is_some_and(|n| {
        !n.is_empty()
            && n.len() <= 3
            && n.bytes().all(|b| b.is_ascii_digit())
            && n.parse::<u32>().is_ok_and(|n| n <= max)
    })
}

const PLAIN: &[&str] = &[
    "cores",
    "sockets",
    "memory",
    "balloon",
    "swap",
    "cpu",
    "cpulimit",
    "cpuunits",
    "numa",
    "onboot",
    "startup",
    "tags",
    "description",
    "name",
    "hostname",
    "boot",
    "agent",
    "protection",
    "features",
    "nameserver",
    "searchdomain",
    "efidisk0",
    "tpmstate0",
    "vga",
    "machine",
    "bios",
];

fn disk_key(k: &str) -> bool {
    indexed(k, "scsi", 30)
        || indexed(k, "virtio", 15)
        || indexed(k, "sata", 5)
        || indexed(k, "ide", 3)
        || indexed(k, "mp", 255)
        || k == "rootfs"
}

/// A key the editor may set. Anything else stays a web-UI or shell job.
pub fn settable_key(k: &str) -> bool {
    PLAIN.contains(&k) || disk_key(k) || indexed(k, "net", 31) || indexed(k, "hostpci", 15) || indexed(k, "usb", 14)
}

/// A key the editor may remove (detached disks included).
pub fn deletable_key(k: &str) -> bool {
    (settable_key(k) && !matches!(k, "rootfs" | "memory" | "cores" | "name" | "hostname")) || indexed(k, "unused", 255)
}

fn valid_value(k: &str, v: &str) -> bool {
    let max = if k == "description" { 8192 } else { 1024 };
    !v.is_empty()
        && v.len() <= max
        && !v.chars().any(|c| c.is_control() && !(k == "description" && (c == '\n' || c == '\t')))
}

fn valid_digest(d: &str) -> bool {
    d.is_empty() || (d.len() <= 64 && d.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Keys that remove data when deleted (a detached disk) or that detach a disk.
pub fn is_destructive_delete(k: &str) -> bool {
    indexed(k, "unused", 255) || disk_key(k) || k == "efidisk0" || k == "tpmstate0"
}

// ---------------------------------------------------------------- scripts

fn base(node: &str, kind: &str, vmid: u32) -> Option<String> {
    (valid_node(node) && valid_kind(kind) && valid_vmid(vmid)).then(|| format!("/nodes/{node}/{kind}/{vmid}"))
}

/// The configuration with pending changes.
pub fn config_script(node: &str, kind: &str, vmid: u32) -> Option<String> {
    Some(get(&format!("{}/pending", base(node, kind, vmid)?), ""))
}

/// Sets and removes keys in one call, refused by Proxmox if `digest` is stale.
pub fn config_set_script(
    node: &str,
    kind: &str,
    vmid: u32,
    digest: &str,
    set: &[PveSetting],
    delete: &[String],
) -> Result<String, String> {
    let path = format!("{}/config", base(node, kind, vmid).ok_or("invalid guest")?);
    if set.is_empty() && delete.is_empty() {
        return Err("nothing to change".into());
    }
    if !valid_digest(digest) {
        return Err("invalid digest".into());
    }
    let mut c = format!("pvesh set {}", shell_quote(&path));
    for p in set {
        if !settable_key(&p.key) || !valid_value(&p.key, &p.value) {
            return Err(format!("{}: not a setting this editor changes, or an invalid value", p.key));
        }
        if delete.contains(&p.key) || set.iter().filter(|q| q.key == p.key).count() > 1 {
            return Err(format!("{} appears twice", p.key));
        }
        c.push_str(&format!(" --{} {}", p.key, shell_quote(&p.value)));
    }
    if !delete.is_empty() {
        if let Some(bad) = delete.iter().find(|k| !deletable_key(k)) {
            return Err(format!("{bad} can't be removed here"));
        }
        c.push_str(&format!(" --delete {}", shell_quote(&delete.join(","))));
    }
    if !digest.is_empty() {
        c.push_str(&format!(" --digest {digest}"));
    }
    c.push_str(" --output-format json 2>&1 && echo @@ok");
    Ok(c)
}

/// Grows a disk by `grow_gb`. Shrinking isn't something Proxmox does.
pub fn resize_script(node: &str, kind: &str, vmid: u32, disk: &str, grow_gb: u32) -> Option<String> {
    let path = format!("{}/resize", base(node, kind, vmid)?);
    (disk_key(disk) && (1..=65536).contains(&grow_gb)).then(|| {
        format!(
            "pvesh set {} --disk {disk} --size +{grow_gb}G --output-format json 2>&1 && echo @@ok",
            shell_quote(&path)
        )
    })
}

/// The task a resize started, if this Proxmox version runs it as one.
pub fn parse_resize(out: &str) -> Result<Option<String>, String> {
    if !out.lines().any(|l| l.trim() == "@@ok") {
        return Err(explain(out));
    }
    let body: String = out.lines().filter(|l| l.trim() != "@@ok").collect::<Vec<_>>().join("\n");
    Ok(parse_upid(&body).ok())
}

pub fn host_devices_script(node: &str) -> Option<String> {
    valid_node(node).then(|| {
        [get(&format!("/nodes/{node}/hardware/pci"), ""), get(&format!("/nodes/{node}/hardware/usb"), "")].join("\n")
    })
}

// ---------------------------------------------------------------- parsing

fn val(v: &Value, k: &str) -> Option<String> {
    match &v[k] {
        Value::Null => None,
        Value::String(_) | Value::Number(_) => Some(s(v, k)),
        Value::Bool(b) => Some(if *b { "1".into() } else { "0".into() }),
        _ => None,
    }
}

/// Splits `a=b,c,d=e` into pairs (a bare item gets an empty key).
pub fn options(v: &str) -> Vec<(String, String)> {
    v.split(',')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (k.to_owned(), v.to_owned()),
            None => (String::new(), p.to_owned()),
        })
        .collect()
}

fn opt<'a>(o: &'a [(String, String)], k: &str) -> Option<&'a str> {
    o.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
}

fn parse_disk(e: &PveConfigEntry) -> Option<PveDisk> {
    let value = e.pending.clone().or_else(|| e.value.clone())?;
    let o = options(&value);
    let volid = o.first().map(|(k, v)| if k.is_empty() { v.clone() } else { format!("{k}={v}") })?;
    Some(PveDisk {
        key: e.key.clone(),
        size: opt(&o, "size").unwrap_or("").to_owned(),
        cdrom: opt(&o, "media") == Some("cdrom"),
        mountpoint: opt(&o, "mp").unwrap_or("").to_owned(),
        volid,
        value,
        pending: e.pending.is_some() || e.delete,
    })
}

const MODELS: &[&str] = &["virtio", "e1000", "e1000e", "rtl8139", "vmxnet3"];

fn parse_nic(e: &PveConfigEntry, kind: &str) -> Option<PveNic> {
    let value = e.pending.clone().or_else(|| e.value.clone())?;
    let o = options(&value);
    let (model, mac) = if kind == "lxc" {
        (opt(&o, "type").unwrap_or("veth").to_owned(), opt(&o, "hwaddr").unwrap_or("").to_owned())
    } else {
        o.iter()
            .find(|(k, _)| MODELS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .or_else(|| opt(&o, "model").map(|m| (m.to_owned(), opt(&o, "macaddr").unwrap_or("").to_owned())))
            .unwrap_or_default()
    };
    Some(PveNic {
        key: e.key.clone(),
        model,
        mac,
        bridge: opt(&o, "bridge").unwrap_or("").to_owned(),
        vlan: opt(&o, "tag").and_then(|t| t.parse().ok()),
        firewall: opt(&o, "firewall") == Some("1"),
        ip: opt(&o, "ip").unwrap_or("").to_owned(),
        value,
        pending: e.pending.is_some(),
        delete: e.delete,
    })
}

/// Natural order: scsi2 before scsi10.
fn key_order(k: &str) -> (String, u32) {
    let split = k.find(|c: char| c.is_ascii_digit()).unwrap_or(k.len());
    (k[..split].to_owned(), k[split..].parse().unwrap_or(0))
}

pub fn parse_config(out: &str, kind: &str) -> Result<PveConfig, String> {
    let (_, body) = sections(out).into_iter().next().ok_or_else(|| explain(out))?;
    let v: Value = serde_json::from_str(body.trim()).map_err(|_| explain(&body))?;
    let items = v.as_array().ok_or_else(|| explain(&body))?;
    let mut entries: Vec<PveConfigEntry> = items
        .iter()
        .map(|i| PveConfigEntry {
            key: s(i, "key"),
            value: val(i, "value"),
            pending: val(i, "pending"),
            delete: !matches!(i["delete"], Value::Null) && val(i, "delete").as_deref() != Some("0"),
        })
        .filter(|e| !e.key.is_empty())
        .collect();
    entries.sort_by_key(|e| key_order(&e.key));
    let digest = entries.iter().find(|e| e.key == "digest").and_then(|e| e.value.clone()).unwrap_or_default();
    let disks = entries
        .iter()
        .filter(|e| disk_key(&e.key) || e.key == "efidisk0" || e.key == "tpmstate0")
        .filter_map(parse_disk)
        .collect();
    let nics = entries.iter().filter(|e| indexed(&e.key, "net", 31)).filter_map(|e| parse_nic(e, kind)).collect();
    let devices =
        entries.iter().filter(|e| indexed(&e.key, "hostpci", 15) || indexed(&e.key, "usb", 14)).cloned().collect();
    let unused = entries.iter().filter(|e| indexed(&e.key, "unused", 255)).cloned().collect();
    let pending = entries.iter().any(|e| e.pending.is_some() || e.delete);
    Ok(PveConfig { kind: kind.to_owned(), digest, entries, disks, nics, devices, unused, pending })
}

pub fn parse_host_devices(out: &str) -> PveHostDevices {
    let mut d = PveHostDevices::default();
    for (path, body) in sections(out) {
        let Ok(v) = serde_json::from_str::<Value>(body.trim()) else {
            d.notes.push(format!("{path}: {}", explain(&body)));
            continue;
        };
        let items = v.as_array().cloned().unwrap_or_default();
        if path.ends_with("/pci") {
            d.pci = items
                .iter()
                .map(|i| PvePci {
                    id: s(i, "id"),
                    vendor: s(i, "vendor_name"),
                    device: s(i, "device_name"),
                    class: s(i, "class"),
                    iommu_group: i["iommugroup"].as_i64().unwrap_or(-1) as i32,
                })
                // Bridges and host internals (class 0x06…) aren't passthrough candidates.
                .filter(|p| !p.id.is_empty() && !p.class.starts_with("0x06"))
                .collect();
            d.pci.sort_by(|a, b| a.id.cmp(&b.id));
        } else if path.ends_with("/usb") {
            d.usb = items
                .iter()
                .filter(|i| s(i, "class") != "9")
                .map(|i| PveUsb {
                    id: format!("{}:{}", s(i, "vendid"), s(i, "prodid")),
                    path: s(i, "usbpath"),
                    manufacturer: s(i, "manufacturer"),
                    product: s(i, "product"),
                })
                .collect();
        }
    }
    d.iommu_off = !d.pci.is_empty() && d.pci.iter().all(|p| p.iommu_group < 0);
    d
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxmox::MARK;

    fn wrap(path: &str, body: &str) -> String {
        format!("{MARK} {path}\n{body}\n")
    }

    #[test]
    fn parses_a_vm_config_with_pending_changes() {
        let c = parse_config(&wrap("/nodes/pve1/qemu/100/pending", include_str!("fixtures/pending_qemu.json")), "qemu")
            .unwrap();
        assert_eq!(c.digest, "3f5c0a9e2b7d41c8e6f0a1b2c3d4e5f60718293a");
        assert!(c.pending);
        let cores = c.entries.iter().find(|e| e.key == "cores").unwrap();
        assert_eq!((cores.value.as_deref(), cores.pending.as_deref()), (Some("4"), Some("6")));
        assert_eq!(c.disks.iter().map(|d| d.key.as_str()).collect::<Vec<_>>(), ["ide2", "scsi0", "scsi1"]);
        let cd = &c.disks[0];
        assert!(cd.cdrom);
        assert_eq!(cd.volid, "local:iso/debian-12.7.0-amd64-netinst.iso");
        assert_eq!(c.disks[1].size, "32G");
        assert_eq!(c.nics.len(), 2);
        assert_eq!(
            (c.nics[0].model.as_str(), c.nics[0].mac.as_str(), c.nics[0].firewall),
            ("virtio", "BC:24:11:5A:3B:01", true)
        );
        assert_eq!((c.nics[1].vlan, c.nics[1].delete), (Some(20), true));
        assert_eq!(c.devices.iter().map(|d| d.key.as_str()).collect::<Vec<_>>(), ["hostpci0", "usb0"]);
        assert_eq!(c.unused[0].value.as_deref(), Some("local-lvm:vm-100-disk-2"));
        assert!(c.entries.iter().any(|e| e.key == "description" && e.value.as_deref().unwrap().contains('\n')));
    }

    #[test]
    fn parses_a_container_config() {
        let c = parse_config(&wrap("/nodes/pve2/lxc/200/pending", include_str!("fixtures/pending_lxc.json")), "lxc")
            .unwrap();
        assert_eq!(
            c.disks.iter().map(|d| (d.key.as_str(), d.mountpoint.as_str())).collect::<Vec<_>>(),
            [("mp0", "/srv/data"), ("rootfs", "")]
        );
        let n = &c.nics[0];
        assert_eq!((n.model.as_str(), n.mac.as_str(), n.ip.as_str()), ("veth", "BC:24:11:00:20:00", "192.168.1.53/24"));
        assert!(c.pending, "swap is pending");
        assert!(parse_config("Configuration file 'nodes/pve1/qemu-server/999.conf' does not exist", "qemu").is_err());
    }

    #[test]
    fn lists_passthrough_candidates() {
        let out = [
            wrap("/nodes/pve1/hardware/pci", include_str!("fixtures/hardware_pci.json")),
            wrap("/nodes/pve1/hardware/usb", include_str!("fixtures/hardware_usb.json")),
        ]
        .join("");
        let d = parse_host_devices(&out);
        assert_eq!(
            d.pci.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            ["0000:01:00.0", "0000:01:00.1", "0000:05:00.0"]
        );
        assert_eq!(d.pci[0].iommu_group, 14);
        assert!(!d.iommu_off);
        assert_eq!(d.usb.iter().map(|u| u.id.as_str()).collect::<Vec<_>>(), ["1050:0407", "10c4:ea60"]);
        assert_eq!(d.usb[1].path, "2-1");
        let off = parse_host_devices(&wrap(
            "/nodes/pve1/hardware/pci",
            r#"[{"id":"0000:01:00.0","class":"0x030000","iommugroup":-1}]"#,
        ));
        assert!(off.iommu_off);
    }

    fn set(k: &str, v: &str) -> PveSetting {
        PveSetting { key: k.into(), value: v.into() }
    }

    #[test]
    fn builds_config_edits_safely() {
        let c = config_set_script(
            "pve1",
            "qemu",
            100,
            "3f5c0a",
            &[set("cores", "6"), set("description", "two\nlines")],
            &["net1".into(), "unused0".into()],
        )
        .unwrap();
        assert!(c.starts_with("pvesh set '/nodes/pve1/qemu/100/config' --cores '6' --description 'two\nlines'"));
        assert!(c.contains("--delete 'net1,unused0' --digest 3f5c0a"));
        assert!(c.ends_with("&& echo @@ok"));
        // Not on the list, or dangerous values.
        assert!(config_set_script("pve1", "qemu", 100, "", &[set("args", "-device x")], &[]).is_err());
        assert!(config_set_script("pve1", "qemu", 100, "", &[set("hookscript", "local:snippets/x")], &[]).is_err());
        assert!(config_set_script("pve1", "qemu", 100, "", &[set("cores", "4\n--delete x")], &[]).is_err());
        assert!(config_set_script("pve1", "qemu", 100, "", &[set("net99", "virtio,bridge=vmbr0")], &[]).is_err());
        assert!(config_set_script("pve1", "qemu", 100, "", &[], &["rootfs".into()]).is_err());
        assert!(config_set_script("pve1", "qemu", 100, "", &[set("net0", "virtio,bridge=vmbr0")], &["net0".into()])
            .is_err());
        assert!(config_set_script("pve1", "qemu", 100, "zz; reboot", &[set("cores", "2")], &[]).is_err());
        assert!(config_set_script("pve1", "qemu", 100, "", &[], &[]).is_err());
        assert!(config_set_script("pve1", "lxc", 200, "", &[set("mp1", "local-lvm:16,mp=/data")], &[]).is_ok());
        assert!(is_destructive_delete("unused3") && is_destructive_delete("scsi1") && !is_destructive_delete("net1"));
    }

    #[test]
    fn grows_disks() {
        assert_eq!(
            resize_script("pve1", "qemu", 100, "scsi0", 10).unwrap(),
            "pvesh set '/nodes/pve1/qemu/100/resize' --disk scsi0 --size +10G --output-format json 2>&1 && echo @@ok"
        );
        assert!(resize_script("pve1", "lxc", 200, "rootfs", 4).is_some());
        assert!(resize_script("pve1", "qemu", 100, "net0", 4).is_none());
        assert!(resize_script("pve1", "qemu", 100, "scsi0", 0).is_none());
        let upid = "UPID:pve1:0001A2B3:0C1D2E3F:67000000:resize:100:root@pam:";
        assert_eq!(parse_resize(&format!("\"{upid}\"\n@@ok")).unwrap().as_deref(), Some(upid));
        assert_eq!(parse_resize("null\n@@ok").unwrap(), None);
        assert!(parse_resize("unable to resize: shrinking disks is not supported").is_err());
    }

    #[test]
    fn reads_options() {
        assert_eq!(
            options("virtio=AA:BB,bridge=vmbr0,firewall=1"),
            [("virtio".into(), "AA:BB".into()), ("bridge".into(), "vmbr0".into()), ("firewall".into(), "1".into())]
        );
        assert_eq!(options("none,media=cdrom")[0], (String::new(), "none".into()));
    }
}
