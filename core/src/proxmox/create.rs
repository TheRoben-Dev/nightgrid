//! Proxmox VE part 3 (M10r): make guests. New VMs and containers, clones,
//! templates, destroy, and the ISO images and container templates they start
//! from (listed, downloaded to a storage by the node itself).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::backup::{valid_storage, valid_volid};
use super::{create, explain, f, get, opt_s, s, sections, valid_kind, valid_node, valid_vmid};
use crate::files::path::shell_quote;

// ---------------------------------------------------------------- types

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveBridge {
    pub name: String,
    pub cidr: Option<String>,
    pub vlan_aware: bool,
    pub ports: String,
    pub comment: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveVolume {
    pub volid: String,
    /// iso or vztmpl.
    pub content: String,
    pub size: f64,
    /// The file name part of the volume id.
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveAppliance {
    pub template: String,
    pub headline: String,
    pub os: String,
    /// system, turnkeylinux, mail, …
    pub section: String,
    pub version: String,
}

/// What the create forms need from one node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveCreateInfo {
    pub next_id: u32,
    pub bridges: Vec<PveBridge>,
    pub isos: Vec<PveVolume>,
    pub templates: Vec<PveVolume>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveVmSpec {
    pub node: String,
    pub vmid: u32,
    pub name: String,
    pub cores: u32,
    pub memory_mb: u32,
    pub disk_storage: String,
    pub disk_gb: u32,
    /// Install ISO (volume id), or none.
    pub iso: Option<String>,
    pub bridge: String,
    pub vlan: Option<u32>,
    /// l26 (Linux), win11, win10, other.
    pub ostype: String,
    /// UEFI (OVMF) with an EFI disk; Windows 11 also gets a TPM.
    pub uefi: bool,
    pub start: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveCtSpec {
    pub node: String,
    pub vmid: u32,
    pub hostname: String,
    /// Container template (volume id).
    pub template: String,
    pub storage: String,
    pub disk_gb: u32,
    pub cores: u32,
    pub memory_mb: u32,
    pub swap_mb: u32,
    pub bridge: String,
    pub vlan: Option<u32>,
    /// `dhcp`, or an address with prefix like `192.168.1.50/24`.
    pub ip: String,
    pub gateway: Option<String>,
    /// OpenSSH public keys for root, one per line (no password is ever set).
    pub ssh_keys: String,
    pub unprivileged: bool,
    /// Lets the container run Docker and systemd's newer features.
    pub nesting: bool,
    pub start: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveCloneSpec {
    pub node: String,
    pub kind: String,
    pub vmid: u32,
    pub newid: u32,
    pub name: String,
    /// A full copy; otherwise a linked clone (templates only, shares the base disk).
    pub full: bool,
    /// Where a full clone's disks go (default: same as the source).
    pub storage: Option<String>,
}

// ---------------------------------------------------------- validation

/// A DNS-style guest name: letters, digits, `-` and `.`; up to 63.
pub fn valid_guest_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 63
        && !n.starts_with(['-', '.'])
        && !n.ends_with(['-', '.'])
        && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

pub fn valid_bridge(b: &str) -> bool {
    (1..=15).contains(&b.len())
        && b.starts_with(|c: char| c.is_ascii_alphabetic())
        && b.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

fn valid_ipv4(a: &str) -> bool {
    let p: Vec<&str> = a.split('.').collect();
    p.len() == 4 && p.iter().all(|x| !x.is_empty() && x.len() <= 3 && x.parse::<u8>().is_ok())
}

fn valid_cidr(c: &str) -> bool {
    c.split_once('/').is_some_and(|(a, n)| valid_ipv4(a) && n.parse::<u8>().is_ok_and(|n| (1..=32).contains(&n)))
}

/// OpenSSH public keys, one per line.
pub fn valid_ssh_keys(k: &str) -> bool {
    k.len() <= 16 * 1024
        && k.lines().filter(|l| !l.trim().is_empty()).all(|l| {
            let mut w = l.split_whitespace();
            matches!(w.next(), Some(t) if t.starts_with("ssh-") || t.starts_with("ecdsa-") || t.starts_with("sk-"))
                && w.next()
                    .is_some_and(|b| b.len() >= 16 && b.chars().all(|c| c.is_ascii_alphanumeric() || "+/=".contains(c)))
        })
        && !k.chars().any(|c| c.is_control() && c != '\n' && c != '\r')
}

pub const OSTYPES: &[&str] = &["l26", "win11", "win10", "other"];

fn net(bridge: &str, vlan: Option<u32>, model: &str) -> String {
    let mut n = format!("{model},bridge={bridge}");
    if let Some(v) = vlan {
        n.push_str(&format!(",tag={v}"));
    }
    n
}

fn valid_vlan(v: Option<u32>) -> bool {
    v.map_or(true, |v| (1..=4094).contains(&v))
}

// ------------------------------------------------------------- scripts

/// Bridges, ISOs and CT templates on `node`, plus the next free id.
/// `iso_storages` / `tmpl_storages` are the storages with that content there.
pub fn create_info_script(node: &str, iso_storages: &[String], tmpl_storages: &[String]) -> Option<String> {
    if !valid_node(node) || !iso_storages.iter().chain(tmpl_storages).all(|s| valid_storage(s)) {
        return None;
    }
    let mut o =
        format!("{}\n{}\n", get("/cluster/nextid", ""), get(&format!("/nodes/{node}/network"), " --type any_bridge"));
    for st in iso_storages {
        o.push_str(&get(&format!("/nodes/{node}/storage/{st}/content"), " --content iso"));
        o.push('\n');
    }
    for st in tmpl_storages {
        o.push_str(&get(&format!("/nodes/{node}/storage/{st}/content"), " --content vztmpl"));
        o.push('\n');
    }
    Some(o)
}

pub fn parse_create_info(out: &str) -> PveCreateInfo {
    let mut r = PveCreateInfo::default();
    for (path, body) in sections(out) {
        let Ok(v) = serde_json::from_str::<Value>(body.trim()) else {
            r.notes.push(format!("{path}: {}", explain(&body)));
            continue;
        };
        if path == "/cluster/nextid" {
            r.next_id = match &v {
                Value::String(s) => s.parse().unwrap_or(0),
                Value::Number(n) => n.as_u64().unwrap_or(0) as u32,
                _ => 0,
            };
        } else if path.ends_with("/network") {
            r.bridges = v
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|b| PveBridge {
                            name: s(b, "iface"),
                            cidr: opt_s(b, "cidr"),
                            vlan_aware: super::b(b, "bridge_vlan_aware"),
                            ports: s(b, "bridge_ports"),
                            comment: s(b, "comments").trim().to_owned(),
                            active: super::b(b, "active"),
                        })
                        .filter(|b| !b.name.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            r.bridges.sort_by(|a, b| a.name.cmp(&b.name));
        } else {
            for i in v.as_array().into_iter().flatten() {
                let volid = s(i, "volid");
                if !valid_volid(&volid) {
                    continue;
                }
                let vol = PveVolume {
                    name: volid.rsplit('/').next().unwrap_or(&volid).to_owned(),
                    content: s(i, "content"),
                    size: f(i, "size"),
                    volid,
                };
                if vol.content == "iso" {
                    r.isos.push(vol);
                } else if vol.content == "vztmpl" {
                    r.templates.push(vol);
                }
            }
        }
    }
    r.isos.sort_by(|a, b| a.name.cmp(&b.name));
    r.templates.sort_by(|a, b| a.name.cmp(&b.name));
    r
}

/// A new VM. Disks use VirtIO SCSI (single) with discard and iothread, the
/// NIC VirtIO, the guest agent on; UEFI adds an EFI disk, Windows 11 a TPM.
pub fn vm_create_script(v: &PveVmSpec) -> Result<String, String> {
    if !valid_node(&v.node) || !valid_vmid(v.vmid) {
        return Err("invalid node or id".into());
    }
    if !valid_guest_name(&v.name) {
        return Err("name: letters, digits, - and . only".into());
    }
    if !(1..=512).contains(&v.cores) || !(64..=4_194_304).contains(&v.memory_mb) || !(1..=65_536).contains(&v.disk_gb) {
        return Err("cores, memory or disk size out of range".into());
    }
    if !valid_storage(&v.disk_storage)
        || !valid_bridge(&v.bridge)
        || !valid_vlan(v.vlan)
        || !OSTYPES.contains(&v.ostype.as_str())
    {
        return Err("invalid storage, bridge, VLAN or OS type".into());
    }
    if v.iso.as_deref().is_some_and(|i| !valid_volid(i)) {
        return Err("invalid ISO".into());
    }
    let st = &v.disk_storage;
    let windows = v.ostype.starts_with("win");
    let uefi = v.uefi || v.ostype == "win11";
    let mut p: Vec<(&str, String)> = vec![
        ("vmid", v.vmid.to_string()),
        ("name", v.name.clone()),
        ("ostype", v.ostype.clone()),
        ("cores", v.cores.to_string()),
        ("sockets", "1".into()),
        ("cpu", "x86-64-v2-AES".into()),
        ("memory", v.memory_mb.to_string()),
        ("scsihw", "virtio-scsi-single".into()),
        ("scsi0", format!("{st}:{},discard=on,iothread=1", v.disk_gb)),
        ("net0", net(&v.bridge, v.vlan, "virtio")),
        ("agent", "1".into()),
    ];
    let mut boot = String::from("order=scsi0");
    if let Some(iso) = &v.iso {
        p.push(("ide2", format!("{iso},media=cdrom")));
        boot.push_str(";ide2");
    }
    p.push(("boot", boot));
    if uefi {
        p.push(("bios", "ovmf".into()));
        p.push(("machine", "q35".into()));
        p.push(("efidisk0", format!("{st}:1,efitype=4m,pre-enrolled-keys={}", if windows { 1 } else { 0 })));
    }
    if v.ostype == "win11" {
        p.push(("tpmstate0", format!("{st}:1,version=v2.0")));
    }
    if v.start {
        p.push(("start", "1".into()));
    }
    Ok(create(&format!("/nodes/{}/qemu", v.node), &p))
}

/// A new container. Root gets the SSH keys; no password is set.
pub fn ct_create_script(c: &PveCtSpec) -> Result<String, String> {
    if !valid_node(&c.node) || !valid_vmid(c.vmid) {
        return Err("invalid node or id".into());
    }
    if !valid_guest_name(&c.hostname) {
        return Err("host name: letters, digits, - and . only".into());
    }
    if !valid_volid(&c.template) || !valid_storage(&c.storage) || !valid_bridge(&c.bridge) || !valid_vlan(c.vlan) {
        return Err("invalid template, storage, bridge or VLAN".into());
    }
    if !(1..=512).contains(&c.cores)
        || !(16..=4_194_304).contains(&c.memory_mb)
        || c.swap_mb > 4_194_304
        || !(1..=65_536).contains(&c.disk_gb)
    {
        return Err("cores, memory, swap or disk size out of range".into());
    }
    let ip = c.ip.trim();
    if ip != "dhcp" && !valid_cidr(ip) {
        return Err("IP: dhcp or an address with prefix, like 192.168.1.50/24".into());
    }
    if c.gateway.as_deref().is_some_and(|g| !valid_ipv4(g)) || (ip == "dhcp" && c.gateway.is_some()) {
        return Err("gateway: an IPv4 address, only with a static IP".into());
    }
    if c.ssh_keys.trim().is_empty() || !valid_ssh_keys(&c.ssh_keys) {
        return Err("add at least one SSH public key (ssh-ed25519 …) so you can log in".into());
    }
    let mut n = net(&c.bridge, c.vlan, "name=eth0");
    n.push_str(&format!(",ip={ip}"));
    if let Some(g) = &c.gateway {
        n.push_str(&format!(",gw={g}"));
    }
    let mut p: Vec<(&str, String)> = vec![
        ("vmid", c.vmid.to_string()),
        ("hostname", c.hostname.clone()),
        ("ostemplate", c.template.clone()),
        ("storage", c.storage.clone()),
        ("rootfs", format!("{}:{}", c.storage, c.disk_gb)),
        ("cores", c.cores.to_string()),
        ("memory", c.memory_mb.to_string()),
        ("swap", c.swap_mb.to_string()),
        ("net0", n),
        ("ssh-public-keys", c.ssh_keys.trim().to_owned()),
        ("unprivileged", if c.unprivileged { "1" } else { "0" }.into()),
    ];
    if c.nesting {
        p.push(("features", "nesting=1".into()));
    }
    if c.start {
        p.push(("start", "1".into()));
    }
    Ok(create(&format!("/nodes/{}/lxc", c.node), &p))
}

pub fn clone_script(c: &PveCloneSpec) -> Result<String, String> {
    if !valid_node(&c.node) || !valid_kind(&c.kind) || !valid_vmid(c.vmid) || !valid_vmid(c.newid) || c.vmid == c.newid
    {
        return Err("invalid source or new id".into());
    }
    if !valid_guest_name(&c.name) {
        return Err("name: letters, digits, - and . only".into());
    }
    if c.storage.as_deref().is_some_and(|s| !valid_storage(s)) || (c.storage.is_some() && !c.full) {
        return Err("a target storage only goes with a full clone".into());
    }
    let mut p: Vec<(&str, String)> = vec![("newid", c.newid.to_string())];
    p.push((if c.kind == "qemu" { "name" } else { "hostname" }, c.name.clone()));
    p.push(("full", if c.full { "1" } else { "0" }.into()));
    if let Some(st) = &c.storage {
        p.push(("storage", st.clone()));
    }
    Ok(create(&format!("/nodes/{}/{}/{}/clone", c.node, c.kind, c.vmid), &p))
}

/// Turns a stopped guest into a template (one way).
pub fn template_script(node: &str, kind: &str, vmid: u32) -> Option<String> {
    (valid_node(node) && valid_kind(kind) && valid_vmid(vmid))
        .then(|| create(&format!("/nodes/{node}/{kind}/{vmid}/template"), &[]))
}

/// Deletes a guest with its disks and removes it from jobs and HA.
pub fn destroy_script(node: &str, kind: &str, vmid: u32) -> Option<String> {
    (valid_node(node) && valid_kind(kind) && valid_vmid(vmid)).then(|| {
        format!(
            "pvesh delete {} --purge 1 --destroy-unreferenced-disks 1 --output-format json 2>&1",
            shell_quote(&format!("/nodes/{node}/{kind}/{vmid}"))
        )
    })
}

/// An `http(s)://` URL without spaces or quotes.
pub fn valid_url(u: &str) -> bool {
    (u.starts_with("https://") || u.starts_with("http://"))
        && u.len() <= 2048
        && u.len() > 10
        && !u.chars().any(|c| c.is_whitespace() || c.is_control() || c == '"' || c == '\'' || c == '`')
}

/// A file name for a downloaded ISO.
pub fn valid_iso_name(n: &str) -> bool {
    n.len() <= 255
        && n.to_ascii_lowercase().ends_with(".iso")
        && n.len() > 4
        && !n.starts_with('.')
        && n.chars().all(|c| c.is_ascii_alphanumeric() || "._-+".contains(c))
}

/// The node downloads an ISO from `url` into `storage` (optionally checked).
pub fn iso_download_script(
    node: &str,
    storage: &str,
    url: &str,
    filename: &str,
    sha256: Option<&str>,
) -> Option<String> {
    if !valid_node(node) || !valid_storage(storage) || !valid_url(url) || !valid_iso_name(filename) {
        return None;
    }
    let mut p: Vec<(&str, String)> =
        vec![("url", url.to_owned()), ("filename", filename.to_owned()), ("content", "iso".into())];
    if let Some(h) = sha256 {
        let h = h.trim().to_ascii_lowercase();
        if h.len() != 64 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        p.push(("checksum", h));
        p.push(("checksum-algorithm", "sha256".into()));
    }
    Some(create(&format!("/nodes/{node}/storage/{storage}/download-url"), &p))
}

/// The container templates Proxmox offers (its appliance index).
pub fn appliances_script(node: &str) -> Option<String> {
    valid_node(node).then(|| get(&format!("/nodes/{node}/aplinfo"), ""))
}

pub fn parse_appliances(out: &str) -> Result<Vec<PveAppliance>, String> {
    let (_, body) = sections(out).into_iter().next().ok_or_else(|| explain(out))?;
    let v: Value = serde_json::from_str(body.trim()).map_err(|_| explain(&body))?;
    let mut a: Vec<PveAppliance> = v
        .as_array()
        .into_iter()
        .flatten()
        .map(|i| PveAppliance {
            template: s(i, "template"),
            headline: s(i, "headline"),
            os: s(i, "os"),
            section: s(i, "section"),
            version: s(i, "version"),
        })
        .filter(|a| valid_template_name(&a.template))
        .collect();
    a.sort_by(|x, y| (x.section != "system", &x.template).cmp(&(y.section != "system", &y.template)));
    Ok(a)
}

pub fn valid_template_name(t: &str) -> bool {
    (5..=255).contains(&t.len())
        && !t.starts_with('.')
        && t.chars().all(|c| c.is_ascii_alphanumeric() || "._-+".contains(c))
}

pub fn appliance_download_script(node: &str, storage: &str, template: &str) -> Option<String> {
    (valid_node(node) && valid_storage(storage) && valid_template_name(template)).then(|| {
        create(&format!("/nodes/{node}/aplinfo"), &[("storage", storage.to_owned()), ("template", template.to_owned())])
    })
}

#[cfg(test)]
mod tests {
    use super::super::MARK;
    use super::*;

    fn out(parts: &[(&str, &str)]) -> String {
        parts.iter().map(|(p, b)| format!("{MARK} {p}\n{b}\n")).collect()
    }

    const KEY: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIDummyKeyForTestsOnly0123456789abcdef me@laptop";

    fn vm() -> PveVmSpec {
        PveVmSpec {
            node: "pve1".into(),
            vmid: 103,
            name: "web".into(),
            cores: 2,
            memory_mb: 4096,
            disk_storage: "local-lvm".into(),
            disk_gb: 32,
            iso: Some("local:iso/ubuntu-24.04.1-live-server-amd64.iso".into()),
            bridge: "vmbr0".into(),
            vlan: None,
            ostype: "l26".into(),
            uefi: false,
            start: false,
        }
    }

    fn ct() -> PveCtSpec {
        PveCtSpec {
            node: "pve1".into(),
            vmid: 104,
            hostname: "dns2".into(),
            template: "local:vztmpl/debian-12-standard_12.7-1_amd64.tar.zst".into(),
            storage: "local-lvm".into(),
            disk_gb: 8,
            cores: 1,
            memory_mb: 512,
            swap_mb: 512,
            bridge: "vmbr0".into(),
            vlan: Some(20),
            ip: "192.168.1.50/24".into(),
            gateway: Some("192.168.1.1".into()),
            ssh_keys: KEY.into(),
            unprivileged: true,
            nesting: true,
            start: true,
        }
    }

    #[test]
    fn parses_create_info() {
        let r = parse_create_info(&out(&[
            ("/cluster/nextid", include_str!("fixtures/nextid.json")),
            ("/nodes/pve1/network", include_str!("fixtures/network.json")),
            ("/nodes/pve1/storage/local/content", include_str!("fixtures/storage_iso.json")),
            ("/nodes/pve1/storage/local/content", include_str!("fixtures/storage_vztmpl.json")),
            ("/nodes/pve1/storage/nas/content", "storage 'nas' is not online"),
        ]));
        assert_eq!(r.next_id, 103);
        assert_eq!(r.bridges.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["vmbr0", "vmbr1"]);
        assert!(r.bridges[1].vlan_aware && r.bridges[0].cidr.as_deref() == Some("192.168.1.20/24"));
        assert_eq!(r.isos.len(), 3);
        assert_eq!(r.isos[0].name, "Win11_24H2_English_x64.iso");
        assert_eq!(r.templates.len(), 2);
        assert_eq!(r.notes.len(), 1);
    }

    #[test]
    fn builds_a_vm() {
        let s = vm_create_script(&vm()).unwrap();
        assert!(s.starts_with("pvesh create '/nodes/pve1/qemu' --vmid '103' --name 'web' --ostype 'l26'"));
        assert!(s.contains("--scsi0 'local-lvm:32,discard=on,iothread=1'"));
        assert!(
            s.contains("--ide2 'local:iso/ubuntu-24.04.1-live-server-amd64.iso,media=cdrom' --boot 'order=scsi0;ide2'")
        );
        assert!(s.contains("--net0 'virtio,bridge=vmbr0'") && !s.contains("ovmf") && !s.contains("--start"));
        let w = vm_create_script(&PveVmSpec { ostype: "win11".into(), vlan: Some(30), start: true, ..vm() }).unwrap();
        assert!(w.contains("--bios 'ovmf' --machine 'q35' --efidisk0 'local-lvm:1,efitype=4m,pre-enrolled-keys=1' --tpmstate0 'local-lvm:1,version=v2.0'"));
        assert!(
            w.contains("--net0 'virtio,bridge=vmbr0,tag=30'") && w.ends_with("--start '1' --output-format json 2>&1")
        );
        assert!(vm_create_script(&PveVmSpec { name: "bad name".into(), ..vm() }).is_err());
        assert!(vm_create_script(&PveVmSpec { vlan: Some(5000), ..vm() }).is_err());
        assert!(vm_create_script(&PveVmSpec { iso: Some("local:iso/x;rm".into()), ..vm() }).is_err());
        assert!(vm_create_script(&PveVmSpec { iso: None, ..vm() }).unwrap().contains("--boot 'order=scsi0'"));
    }

    #[test]
    fn builds_a_container() {
        let s = ct_create_script(&ct()).unwrap();
        assert!(s.contains("--rootfs 'local-lvm:8'"));
        assert!(s.contains("--net0 'name=eth0,bridge=vmbr0,tag=20,ip=192.168.1.50/24,gw=192.168.1.1'"));
        assert!(s.contains(&format!("--ssh-public-keys '{KEY}'")));
        assert!(s.contains("--unprivileged '1' --features 'nesting=1' --start '1'"));
        assert!(!s.contains("password"));
        assert!(ct_create_script(&PveCtSpec { ssh_keys: String::new(), ..ct() }).unwrap_err().contains("SSH"));
        assert!(ct_create_script(&PveCtSpec { ssh_keys: "not a key".into(), ..ct() }).is_err());
        assert!(ct_create_script(&PveCtSpec { ip: "192.168.1.300/24".into(), ..ct() }).is_err());
        assert!(ct_create_script(&PveCtSpec { ip: "dhcp".into(), ..ct() }).is_err(), "gateway with dhcp");
        assert!(ct_create_script(&PveCtSpec { ip: "dhcp".into(), gateway: None, ..ct() })
            .unwrap()
            .contains("ip=dhcp'"));
    }

    #[test]
    fn builds_clone_template_destroy_and_downloads() {
        let c = PveCloneSpec {
            node: "pve1".into(),
            kind: "qemu".into(),
            vmid: 9000,
            newid: 105,
            name: "app1".into(),
            full: false,
            storage: None,
        };
        assert_eq!(
            clone_script(&c).unwrap(),
            "pvesh create '/nodes/pve1/qemu/9000/clone' --newid '105' --name 'app1' --full '0' --output-format json 2>&1"
        );
        assert!(clone_script(&PveCloneSpec { storage: Some("local-lvm".into()), ..c.clone() }).is_err());
        assert!(clone_script(&PveCloneSpec { full: true, storage: Some("local-lvm".into()), ..c.clone() })
            .unwrap()
            .contains("--storage 'local-lvm'"));
        assert!(clone_script(&PveCloneSpec { kind: "lxc".into(), ..c.clone() }).unwrap().contains("--hostname 'app1'"));
        assert!(clone_script(&PveCloneSpec { newid: 9000, ..c }).is_err());
        assert_eq!(
            template_script("pve1", "lxc", 201).unwrap(),
            "pvesh create '/nodes/pve1/lxc/201/template' --output-format json 2>&1"
        );
        assert_eq!(
            destroy_script("pve1", "qemu", 101).unwrap(),
            "pvesh delete '/nodes/pve1/qemu/101' --purge 1 --destroy-unreferenced-disks 1 --output-format json 2>&1"
        );
        let d = iso_download_script("pve1", "local", "https://example.com/os.iso", "os.iso", Some(&"ab".repeat(32)))
            .unwrap();
        assert!(d.contains("'/nodes/pve1/storage/local/download-url' --url 'https://example.com/os.iso' --filename 'os.iso' --content 'iso' --checksum"));
        assert!(iso_download_script("pve1", "local", "ftp://x/os.iso", "os.iso", None).is_none());
        assert!(iso_download_script("pve1", "local", "https://example.com/a'b.iso", "os.iso", None).is_none());
        assert!(iso_download_script("pve1", "local", "https://example.com/os.iso", "../os.iso", None).is_none());
        assert!(iso_download_script("pve1", "local", "https://example.com/os.iso", "os.iso", Some("xyz")).is_none());
        assert!(appliance_download_script("pve1", "local", "debian-12-standard_12.7-1_amd64.tar.zst")
            .unwrap()
            .contains("--template 'debian-12-standard_12.7-1_amd64.tar.zst'"));
        assert!(appliance_download_script("pve1", "local", "a b").is_none());
    }

    #[test]
    fn parses_appliances() {
        let a = parse_appliances(&out(&[("/nodes/pve1/aplinfo", include_str!("fixtures/aplinfo.json"))])).unwrap();
        assert_eq!(a.len(), 3);
        assert_eq!(a[2].section, "turnkeylinux");
        assert_eq!(a[0].template, "debian-12-standard_12.7-1_amd64.tar.zst");
        assert!(valid_ssh_keys(&format!("{KEY}\n\n{KEY}")));
        assert!(!valid_ssh_keys("ssh-rsa short"));
    }
}
