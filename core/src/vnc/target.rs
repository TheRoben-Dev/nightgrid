//! Where a guest's screen is, and how to reach it without opening a port.
//!
//! - libvirt: `virsh domdisplay` names the VNC listener (normally on the
//!   host's loopback); Nightgrid opens an SSH `direct-tcpip` channel to it, or
//!   `direct-streamlocal` for a Unix socket.
//! - Proxmox VE: `qm vncproxy` relays the VM's VNC socket on stdin/stdout. It
//!   requires a one-time password in `LC_PVE_TICKET` (valid 30 s), which
//!   Nightgrid makes up per session and sends on stdin, never on the command line.
//! - RDP: the guest's address from libvirt's DHCP leases / guest agent or the
//!   Proxmox guest agent, reached through a local SSH tunnel.

use serde::Serialize;

use crate::files::path::shell_quote;
use crate::system::vms::{valid_id, VmKind};
use crate::system::{SystemError, SystemResult};

const VIRSH: &str = "LC_ALL=C virsh -c qemu:///system";

/// A guest's display as the host describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Display {
    Tcp { host: String, port: u16, password: Option<String> },
    Unix { path: String, password: Option<String> },
}

/// The command that prints a libvirt guest's displays (VNC first, with its
/// password when this login may read it, then any type).
pub fn libvirt_display_command(id: &str) -> SystemResult<String> {
    if !valid_id(VmKind::Libvirt, id) {
        return Err(SystemError::other(format!("invalid guest id {id:?}")));
    }
    let q = shell_quote(id);
    Ok(format!(
        "{VIRSH} domstate {q} 2>&1 | head -n 1 | sed 's/^/@state /'; \
         {VIRSH} domdisplay --type vnc --include-password {q} 2>/dev/null || {VIRSH} domdisplay --type vnc {q} 2>/dev/null; \
         {VIRSH} domdisplay {q} 2>&1 | sed 's/^/@any /'"
    ))
}

/// Reads the output of [`libvirt_display_command`].
pub fn parse_libvirt_display(out: &str) -> Result<Display, String> {
    let mut state = None;
    let mut any = None;
    for line in out.lines().map(str::trim) {
        if let Some(s) = line.strip_prefix("@state ") {
            state = Some(s.to_string());
        } else if let Some(s) = line.strip_prefix("@any ") {
            if !s.is_empty() && any.is_none() {
                any = Some(s.to_string());
            }
        } else if line.starts_with("vnc") {
            return parse_vnc_uri(line);
        }
    }
    match state.as_deref() {
        Some(s) if s.starts_with("error") => return Err(s.trim_start_matches("error: ").to_string()),
        Some(s) if s != "running" && s != "paused" => return Err(format!("the VM isn't running ({s})")),
        _ => {}
    }
    match any {
        Some(a) if a.starts_with("spice") => Err("this VM's display is SPICE: switch its graphics to VNC (virsh edit, <graphics type='vnc' listen='127.0.0.1'/>) to view it here".into()),
        Some(a) if a.starts_with("error") => Err(a.trim_start_matches("error: ").to_string()),
        _ => Err("this VM has no VNC display".into()),
    }
}

/// `vnc://[:password@]host:display` or `vnc+unix://[:password@]/path`.
pub fn parse_vnc_uri(uri: &str) -> Result<Display, String> {
    let (scheme, rest) = uri.split_once("://").ok_or("unexpected display address")?;
    let (password, rest) = match rest.rsplit_once('@') {
        Some((user, r)) => (user.split_once(':').map(|(_, p)| p.to_string()).filter(|p| !p.is_empty()), r),
        None => (None, rest),
    };
    if scheme.ends_with("unix") {
        let path = rest.split('?').next().unwrap_or(rest);
        if !path.starts_with('/') {
            return Err("unexpected display socket".into());
        }
        return Ok(Display::Unix { path: path.to_string(), password });
    }
    let (host, display) = rest.trim_end_matches('/').rsplit_once(':').ok_or("unexpected display address")?;
    let display: u16 = display.parse().map_err(|_| "unexpected display number")?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let host = match host {
        "" | "0.0.0.0" | "::" | "localhost" => "127.0.0.1",
        h => h,
    };
    // libvirt prints the display number for VNC (port 5900 + n).
    let port = if display < 5900 { 5900 + display } else { display };
    Ok(Display::Tcp { host: host.to_string(), port, password })
}

/// Checks a Proxmox VM before starting `qm vncproxy`.
pub fn qm_status_command(id: &str) -> SystemResult<String> {
    if !valid_id(VmKind::Qemu, id) {
        return Err(SystemError::other(format!("invalid guest id {id:?}")));
    }
    Ok(format!("qm status {} 2>&1", shell_quote(id)))
}

pub fn parse_qm_status(out: &str) -> Result<(), String> {
    let t = out.trim();
    match t.strip_prefix("status: ") {
        Some("running") | Some("paused") => Ok(()),
        Some(s) => Err(format!("the VM isn't running ({s})")),
        None => Err(if t.is_empty() { "couldn't read the VM's status".into() } else { t.chars().take(300).collect() }),
    }
}

/// The relay command; reads the one-time password from its first stdin line.
pub fn qm_vncproxy_command(id: &str) -> SystemResult<String> {
    if !valid_id(VmKind::Qemu, id) {
        return Err(SystemError::other(format!("invalid guest id {id:?}")));
    }
    Ok(format!("read -r t; LC_PVE_TICKET=\"$t\" exec qm vncproxy {}", shell_quote(id)))
}

/// A random password for one `qm vncproxy` session (VNC only uses 8 characters).
pub fn session_password() -> String {
    const ABC: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789";
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).expect("system randomness");
    b.iter().map(|x| ABC[usize::from(*x) % ABC.len()] as char).collect()
}

/// Whether a guest kind has a graphical console.
pub fn has_display(kind: VmKind) -> bool {
    !matches!(kind, VmKind::Lxc)
}

/// The guest's addresses, for RDP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GuestAddrs {
    pub addrs: Vec<String>,
    /// Why none were found.
    pub note: Option<String>,
}

pub fn guest_addr_command(kind: VmKind, id: &str) -> SystemResult<String> {
    if !valid_id(kind, id) {
        return Err(SystemError::other(format!("invalid guest id {id:?}")));
    }
    let q = shell_quote(id);
    Ok(match kind {
        VmKind::Libvirt => format!(
            "for s in lease agent arp; do {VIRSH} domifaddr --source $s {q} 2>/dev/null | sed \"s/^/@$s /\"; done"
        ),
        VmKind::Qemu => format!("qm guest cmd {q} network-get-interfaces 2>&1"),
        VmKind::Lxc => format!("pct exec {q} -- ip -4 -o addr show scope global 2>&1"),
    })
}

fn usable(ip: &str) -> bool {
    match ip.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(a)) => !a.is_loopback() && !a.is_link_local() && !a.is_unspecified(),
        Ok(std::net::IpAddr::V6(a)) => !a.is_loopback() && !a.is_unspecified() && (a.segments()[0] & 0xffc0) != 0xfe80,
        Err(_) => false,
    }
}

fn push(out: &mut Vec<String>, ip: &str) {
    let ip = ip.split('/').next().unwrap_or(ip).trim();
    if usable(ip) && !out.iter().any(|a| a == ip) {
        out.push(ip.to_string());
    }
}

pub fn parse_guest_addrs(kind: VmKind, out: &str) -> GuestAddrs {
    let mut addrs = Vec::new();
    match kind {
        VmKind::Libvirt => {
            for line in out.lines() {
                let f: Vec<&str> = line.split_whitespace().collect();
                // "@lease vnet0 52:54:00:.. ipv4 192.168.122.45/24"
                if let Some(i) = f.iter().position(|w| *w == "ipv4" || *w == "ipv6") {
                    if let Some(ip) = f.get(i + 1) {
                        push(&mut addrs, ip);
                    }
                }
            }
        }
        VmKind::Qemu => {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(out.trim()) {
                for nic in v.as_array().into_iter().flatten() {
                    for a in nic.get("ip-addresses").and_then(|a| a.as_array()).into_iter().flatten() {
                        if let Some(ip) = a.get("ip-address").and_then(|x| x.as_str()) {
                            push(&mut addrs, ip);
                        }
                    }
                }
            } else {
                let t = out.trim();
                return GuestAddrs {
                    addrs,
                    note: Some(
                        if t.contains("not running") || t.contains("No QEMU guest agent") || t.contains("guest agent") {
                            "the QEMU guest agent isn't running in the VM: enter the address yourself".into()
                        } else {
                            format!("couldn't ask the guest agent: {}", t.chars().take(200).collect::<String>())
                        },
                    ),
                };
            }
        }
        VmKind::Lxc => {
            for line in out.lines() {
                let f: Vec<&str> = line.split_whitespace().collect();
                if let Some(i) = f.iter().position(|w| *w == "inet") {
                    if let Some(ip) = f.get(i + 1) {
                        push(&mut addrs, ip);
                    }
                }
            }
        }
    }
    // IPv4 first: RDP clients and Windows firewalls handle it most reliably.
    addrs.sort_by_key(|a| a.contains(':'));
    let note = addrs
        .is_empty()
        .then(|| "no address found (no DHCP lease or guest agent): enter the address yourself".to_string());
    GuestAddrs { addrs, note }
}

/// A host name or IP address the user typed for RDP.
pub fn valid_addr(a: &str) -> bool {
    !a.is_empty()
        && a.len() <= 253
        && !a.starts_with('-')
        && a.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_'))
}

/// A `.rdp` file pointing at the local end of the tunnel. Holds no secrets.
pub fn rdp_file(port: u16) -> String {
    format!("full address:s:127.0.0.1:{port}\r\nprompt for credentials:i:1\r\nscreen mode id:i:1\r\nsmart sizing:i:1\r\ndynamic resolution:i:1\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vnc_uris() {
        assert_eq!(
            parse_vnc_uri("vnc://127.0.0.1:0").unwrap(),
            Display::Tcp { host: "127.0.0.1".into(), port: 5900, password: None }
        );
        assert_eq!(
            parse_vnc_uri("vnc://localhost:3").unwrap(),
            Display::Tcp { host: "127.0.0.1".into(), port: 5903, password: None }
        );
        assert_eq!(
            parse_vnc_uri("vnc://:s3cret@192.168.1.5:1").unwrap(),
            Display::Tcp { host: "192.168.1.5".into(), port: 5901, password: Some("s3cret".into()) }
        );
        assert_eq!(
            parse_vnc_uri("vnc://[::]:2").unwrap(),
            Display::Tcp { host: "127.0.0.1".into(), port: 5902, password: None }
        );
        assert_eq!(
            parse_vnc_uri("vnc+unix:///var/lib/libvirt/qemu/web.vnc").unwrap(),
            Display::Unix { path: "/var/lib/libvirt/qemu/web.vnc".into(), password: None }
        );
        assert!(parse_vnc_uri("garbage").is_err());
    }

    #[test]
    fn libvirt_display_output() {
        let out = "@state running\nvnc://127.0.0.1:0\n@any vnc://127.0.0.1:0\n";
        assert!(matches!(parse_libvirt_display(out).unwrap(), Display::Tcp { port: 5900, .. }));
        let e = parse_libvirt_display(
            "@state shut off\n@any error: Requested operation is not valid: domain is not running\n",
        )
        .unwrap_err();
        assert!(e.contains("isn't running"), "{e}");
        let e = parse_libvirt_display("@state running\n@any spice://127.0.0.1:5900\n").unwrap_err();
        assert!(e.contains("SPICE"), "{e}");
        let e = parse_libvirt_display("@state error: failed to get domain 'x'\n@any error: failed to get domain 'x'\n")
            .unwrap_err();
        assert!(e.contains("failed to get domain"), "{e}");
        assert!(libvirt_display_command("web; rm -rf /").is_err());
        assert!(libvirt_display_command("web-1").unwrap().contains("'web-1'"));
    }

    #[test]
    fn proxmox_commands() {
        assert_eq!(qm_vncproxy_command("101").unwrap(), "read -r t; LC_PVE_TICKET=\"$t\" exec qm vncproxy '101'");
        assert!(qm_vncproxy_command("1; id").is_err());
        assert!(parse_qm_status("status: running\n").is_ok());
        assert!(parse_qm_status("status: stopped\n").unwrap_err().contains("stopped"));
        assert!(parse_qm_status("Configuration file 'nodes/pve/qemu-server/9.conf' does not exist\n")
            .unwrap_err()
            .contains("does not exist"));
        let a = session_password();
        assert_eq!(a.len(), 8);
        assert_ne!(a, session_password());
        assert!(!has_display(VmKind::Lxc) && has_display(VmKind::Qemu));
    }

    #[test]
    fn guest_addresses() {
        let out = "@lease  Name       MAC address          Protocol     Address\n@lease -------------------------------------------\n@lease  vnet0      52:54:00:aa:bb:cc    ipv4         192.168.122.45/24\n@agent  lo  00:00:00:00:00:00 ipv4 127.0.0.1/8\n@agent  Ethernet  52:54:00:aa:bb:cc  ipv6  fe80::1/64\n@agent  Ethernet  52:54:00:aa:bb:cc  ipv6  fd00::5/64\n@agent  Ethernet  52:54:00:aa:bb:cc  ipv4  192.168.122.45/24\n";
        let g = parse_guest_addrs(VmKind::Libvirt, out);
        assert_eq!(g.addrs, vec!["192.168.122.45", "fd00::5"]);
        assert!(g.note.is_none());

        let qm = r#"[{"name":"lo","ip-addresses":[{"ip-address":"127.0.0.1","ip-address-type":"ipv4"}]},{"name":"Ethernet","ip-addresses":[{"ip-address":"fe80::2","ip-address-type":"ipv6"},{"ip-address":"192.168.1.60","ip-address-type":"ipv4"}]}]"#;
        assert_eq!(parse_guest_addrs(VmKind::Qemu, qm).addrs, vec!["192.168.1.60"]);
        let g = parse_guest_addrs(VmKind::Qemu, "QEMU guest agent is not running\n");
        assert!(g.addrs.is_empty() && g.note.unwrap().contains("guest agent"));
        assert!(parse_guest_addrs(VmKind::Libvirt, "").note.is_some());
    }

    #[test]
    fn rdp_file_and_addresses() {
        let f = rdp_file(50123);
        assert!(f.starts_with("full address:s:127.0.0.1:50123\r\n"));
        assert!(!f.contains("password"));
        assert!(valid_addr("192.168.1.60") && valid_addr("win11.lan") && valid_addr("fd00::5"));
        assert!(!valid_addr("-oProxy") && !valid_addr("a b") && !valid_addr(""));
    }
}
