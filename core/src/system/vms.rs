//! Virtual machines (M10h): libvirt domains (`virsh`) and Proxmox VE guests
//! (`qm` for VMs, `pct` for containers) on a host, with start, shutdown,
//! reboot and force stop, and a console command for the terminal.
//!
//! The list is read as the login user first (libvirt's group is enough);
//! Proxmox's tools and a locked-down libvirt need root, which goes through
//! [`super::sudo`] like the other System tabs.

use serde::{Deserialize, Serialize};

use super::sudo::{self, SudoAuth};
use super::{SystemError, SystemResult};
use crate::files::path::shell_quote;
use crate::ssh::client::Connection;

const MARK: &str = "@@NGVM@@";
const VIRSH: &str = "virsh -c qemu:///system";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum VmKind {
    /// A libvirt / KVM domain.
    Libvirt,
    /// A Proxmox VE virtual machine (`qm`).
    Qemu,
    /// A Proxmox VE container (`pct`).
    Lxc,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Vm {
    pub kind: VmKind,
    /// Domain name (libvirt) or VMID (Proxmox).
    pub id: String,
    pub name: String,
    /// running, stopped, paused, … (lower case, "shut off" → "stopped").
    pub state: String,
    pub vcpus: Option<u32>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub memory_mb: Option<u64>,
    pub autostart: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct VmReport {
    pub vms: Vec<Vm>,
    pub libvirt: bool,
    pub proxmox: bool,
    /// A tool is installed but this login may not use it: read as root.
    pub needs_root: bool,
    /// Messages from the tools (permission errors, a daemon that's down).
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum VmAction {
    Start,
    /// Asks the guest to shut down (ACPI).
    Shutdown,
    Reboot,
    /// Pulls the plug.
    Stop,
}

impl VmAction {
    /// Needs a confirmation naming the guest and host (rule 5).
    pub fn is_destructive(self) -> bool {
        !matches!(self, VmAction::Start)
    }
}

pub fn valid_id(kind: VmKind, id: &str) -> bool {
    match kind {
        VmKind::Qemu | VmKind::Lxc => !id.is_empty() && id.len() <= 10 && id.chars().all(|c| c.is_ascii_digit()),
        VmKind::Libvirt => {
            !id.is_empty()
                && id.len() <= 200
                && !id.starts_with('-')
                && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+' | ':'))
        }
    }
}

pub fn scan_command() -> String {
    format!(
        "export LC_ALL=C\n\
         if command -v qm >/dev/null 2>&1; then echo '{MARK} qm'; qm list 2>&1; echo '{MARK} pct'; pct list 2>&1; fi\n\
         if command -v virsh >/dev/null 2>&1; then echo '{MARK} virsh'; {VIRSH} list --all 2>&1\n\
         for d in $({VIRSH} list --all --name 2>/dev/null); do echo \"{MARK} dominfo $d\"; {VIRSH} dominfo \"$d\" 2>/dev/null; done; fi\n\
         echo '{MARK} end'\n"
    )
}

fn denied(text: &str) -> bool {
    let l = text.to_ascii_lowercase();
    l.contains("permission denied")
        || l.contains("authentication unavailable")
        || l.contains("authentication failed")
        || l.contains("only root can")
        || l.contains("must be root")
        || l.contains("ipcc_send_rec")
}

pub fn parse_scan(out: &str) -> VmReport {
    let mut r = VmReport { vms: Vec::new(), libvirt: false, proxmox: false, needs_root: false, notes: Vec::new() };
    let mut section = String::new();
    let mut body: Vec<&str> = Vec::new();
    let flush = |section: &str, body: &[&str], r: &mut VmReport| {
        let text = body.join("\n");
        match section.split_whitespace().next().unwrap_or("") {
            "qm" | "pct" => {
                r.proxmox = true;
                if denied(&text) {
                    r.needs_root = true;
                    if section == "qm" {
                        r.notes.push("Proxmox's qm and pct need root.".into());
                    }
                    return;
                }
                let kind = if section == "qm" { VmKind::Qemu } else { VmKind::Lxc };
                r.vms.extend(body.iter().filter_map(|l| parse_pve_line(kind, l)));
            }
            "virsh" => {
                r.libvirt = true;
                if denied(&text) {
                    r.needs_root = true;
                    r.notes.push("libvirt refused this login (add it to the libvirt group, or read as root).".into());
                    return;
                }
                if text.contains("failed to connect") {
                    r.notes.push("libvirtd isn't answering on qemu:///system.".into());
                    return;
                }
                r.vms.extend(body.iter().filter_map(|l| parse_virsh_line(l)));
            }
            "dominfo" => {
                let name = section.split_once(' ').map(|(_, n)| n.trim()).unwrap_or("");
                if let Some(vm) = r.vms.iter_mut().find(|v| v.kind == VmKind::Libvirt && v.id == name) {
                    for l in body {
                        let Some((k, v)) = l.split_once(':') else { continue };
                        let v = v.trim();
                        match k.trim() {
                            "CPU(s)" => vm.vcpus = v.parse().ok(),
                            "Max memory" => {
                                vm.memory_mb = v
                                    .split_whitespace()
                                    .next()
                                    .and_then(|n| n.parse::<u64>().ok())
                                    .map(|kib| kib / 1024)
                            }
                            "Autostart" => vm.autostart = Some(v == "enable"),
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    };
    for line in out.lines() {
        if let Some(rest) = line.strip_prefix(MARK) {
            flush(&section, &body, &mut r);
            section = rest.trim().to_owned();
            body.clear();
        } else {
            body.push(line);
        }
    }
    flush(&section, &body, &mut r);
    r.vms.sort_by(|a, b| (a.state != "running", &a.name).cmp(&(b.state != "running", &b.name)));
    r
}

/// `       100 ubuntu    running    2048    32.00 1234` (qm) or
/// `101        running                 pihole` (pct, lock column may be empty).
fn parse_pve_line(kind: VmKind, line: &str) -> Option<Vm> {
    let w: Vec<&str> = line.split_whitespace().collect();
    let id = *w.first()?;
    if !id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    match kind {
        VmKind::Qemu => Some(Vm {
            kind,
            id: id.to_owned(),
            name: w.get(1)?.to_string(),
            state: w.get(2)?.to_string(),
            vcpus: None,
            memory_mb: w.get(3).and_then(|m| m.parse().ok()),
            autostart: None,
        }),
        _ => Some(Vm {
            kind,
            id: id.to_owned(),
            name: if w.len() >= 3 { w[w.len() - 1].to_owned() } else { id.to_owned() },
            state: w.get(1)?.to_string(),
            vcpus: None,
            memory_mb: None,
            autostart: None,
        }),
    }
}

/// ` 1    web      running` / ` -    test     shut off`.
fn parse_virsh_line(line: &str) -> Option<Vm> {
    let w: Vec<&str> = line.split_whitespace().collect();
    if w.len() < 3 || !(w[0] == "-" || w[0].chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    let state = w[2..].join(" ");
    let state = match state.as_str() {
        "shut off" => "stopped".to_owned(),
        "in shutdown" => "stopping".to_owned(),
        s => s.to_owned(),
    };
    Some(Vm {
        kind: VmKind::Libvirt,
        id: w[1].to_owned(),
        name: w[1].to_owned(),
        state,
        vcpus: None,
        memory_mb: None,
        autostart: None,
    })
}

pub fn action_command(kind: VmKind, id: &str, action: VmAction) -> SystemResult<String> {
    if !valid_id(kind, id) {
        return Err(SystemError::other(format!("invalid guest id {id:?}")));
    }
    let q = shell_quote(id);
    Ok(match kind {
        VmKind::Libvirt => {
            let verb = match action {
                VmAction::Start => "start",
                VmAction::Shutdown => "shutdown",
                VmAction::Reboot => "reboot",
                VmAction::Stop => "destroy",
            };
            format!("LC_ALL=C {VIRSH} {verb} {q} 2>&1")
        }
        VmKind::Qemu | VmKind::Lxc => {
            let tool = if kind == VmKind::Qemu { "qm" } else { "pct" };
            let verb = match action {
                VmAction::Start => "start",
                VmAction::Shutdown => "shutdown",
                VmAction::Reboot => "reboot",
                VmAction::Stop => "stop",
            };
            format!("{tool} {verb} {q} 2>&1")
        }
    })
}

/// The console command for a terminal tab (the PTY handles sudo's prompt).
pub fn console_command(kind: VmKind, id: &str, as_root: bool) -> SystemResult<String> {
    if !valid_id(kind, id) {
        return Err(SystemError::other(format!("invalid guest id {id:?}")));
    }
    let q = shell_quote(id);
    let cmd = match kind {
        VmKind::Libvirt => format!("{VIRSH} console {q}"),
        VmKind::Qemu => format!("qm terminal {q}"),
        VmKind::Lxc => format!("pct enter {q}"),
    };
    Ok(if as_root { format!("sudo {cmd}") } else { cmd })
}

pub async fn scan(conn: &Connection, auth: &SudoAuth) -> SystemResult<VmReport> {
    let o = sudo::run(conn, auth, &scan_command()).await?;
    let mut r = parse_scan(&o.stdout);
    if !matches!(auth, SudoAuth::User) {
        // Read as root: a remaining refusal isn't about the login.
        r.needs_root = false;
    }
    Ok(r)
}

pub async fn act(conn: &Connection, auth: &SudoAuth, kind: VmKind, id: &str, action: VmAction) -> SystemResult<()> {
    let cmd = action_command(kind, id, action)?;
    let o = sudo::run(conn, auth, &cmd).await?;
    if o.code != 0 {
        let msg = format!("{}{}", o.stdout, o.stderr);
        return Err(SystemError::other(msg.trim().chars().take(400).collect::<String>()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proxmox_and_libvirt() {
        let out = "@@NGVM@@ qm\n      VMID NAME                 STATUS     MEM(MB)    BOOTDISK(GB) PID       \n\
                   \x20      100 ubuntu               running    2048              32.00 1234      \n\
                   \x20      102 win11                stopped    8192              64.00 0         \n\
                   @@NGVM@@ pct\nVMID       Status     Lock         Name                \n\
                   101        running                 pihole              \n\
                   103        stopped     backup      media               \n\
                   @@NGVM@@ virsh\n Id   Name     State\n-------------------------\n 1    web      running\n -    test     shut off\n\n\
                   @@NGVM@@ dominfo web\nId:             1\nName:           web\nCPU(s):         2\nMax memory:     2097152 KiB\nAutostart:      enable\n\
                   @@NGVM@@ dominfo test\nCPU(s):         4\nMax memory:     4194304 KiB\nAutostart:      disable\n@@NGVM@@ end\n";
        let r = parse_scan(out);
        assert!(r.proxmox && r.libvirt && !r.needs_root);
        assert_eq!(r.vms.len(), 6, "{:#?}", r.vms);
        let ubuntu = r.vms.iter().find(|v| v.id == "100").unwrap();
        assert_eq!(
            (ubuntu.kind, ubuntu.name.as_str(), ubuntu.state.as_str(), ubuntu.memory_mb),
            (VmKind::Qemu, "ubuntu", "running", Some(2048))
        );
        let media = r.vms.iter().find(|v| v.id == "103").unwrap();
        assert_eq!((media.kind, media.name.as_str(), media.state.as_str()), (VmKind::Lxc, "media", "stopped"));
        let pihole = r.vms.iter().find(|v| v.id == "101").unwrap();
        assert_eq!(pihole.name, "pihole");
        let web = r.vms.iter().find(|v| v.id == "web").unwrap();
        assert_eq!((web.vcpus, web.memory_mb, web.autostart), (Some(2), Some(2048), Some(true)));
        let test = r.vms.iter().find(|v| v.id == "test").unwrap();
        assert_eq!(test.state, "stopped");
        assert_eq!(r.vms[0].state, "running", "running guests first");
    }

    #[test]
    fn notices_permission_problems() {
        let out = "@@NGVM@@ qm\nipcc_send_rec[1] failed: Permission denied\n@@NGVM@@ pct\nipcc_send_rec[1] failed: Permission denied\n\
                   @@NGVM@@ virsh\nerror: failed to connect to the hypervisor\nerror: authentication unavailable: no polkit agent available\n@@NGVM@@ end\n";
        let r = parse_scan(out);
        assert!(r.needs_root);
        assert!(r.vms.is_empty());
        assert_eq!(r.notes.len(), 2);
    }

    #[test]
    fn commands_are_checked() {
        assert_eq!(action_command(VmKind::Qemu, "100", VmAction::Stop).unwrap(), "qm stop '100' 2>&1");
        assert_eq!(
            action_command(VmKind::Libvirt, "web-1", VmAction::Stop).unwrap(),
            "LC_ALL=C virsh -c qemu:///system destroy 'web-1' 2>&1"
        );
        assert!(action_command(VmKind::Qemu, "100; reboot", VmAction::Start).is_err());
        assert!(action_command(VmKind::Libvirt, "-x", VmAction::Start).is_err());
        assert_eq!(console_command(VmKind::Lxc, "101", true).unwrap(), "sudo pct enter '101'");
        assert!(VmAction::Shutdown.is_destructive() && !VmAction::Start.is_destructive());
    }

    #[test]
    fn script_runs_under_sh() {
        let out = std::process::Command::new("sh").arg("-c").arg(scan_command()).output().unwrap();
        assert!(out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout).contains("@@NGVM@@ end"));
    }
}
