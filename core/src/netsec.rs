//! Network security checks (D-145): what a scan already shows (router port
//! forwards to risky services, UPnP left on, a second DHCP server, public
//! addresses) and an opt-in web check of the devices it found (expired or
//! self-signed certificates, admin pages of gear that ships with default
//! passwords, logins over plain HTTP).
//!
//! The web check connects to a few web ports per device and reads one page
//! or one certificate. It never sends a password or tries a login.

use std::net::IpAddr;

use base64::Engine as _;
use serde::Serialize;

use crate::files::path::shell_quote;
use crate::netmap::{Discovery, ExposureKind};
use crate::nettools::{port_name, Tone};
use crate::ssh::client::Connection;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NetFinding {
    pub ip: Option<String>,
    pub port: Option<u16>,
    pub level: Tone,
    pub title: String,
    pub detail: String,
}

fn finding(
    ip: Option<&str>,
    port: Option<u16>,
    level: Tone,
    title: impl Into<String>,
    detail: impl Into<String>,
) -> NetFinding {
    NetFinding { ip: ip.map(str::to_owned), port, level, title: title.into(), detail: detail.into() }
}

/// Ports that should never be forwarded from the internet as they are.
const RISKY_FORWARD: &[u16] =
    &[21, 22, 23, 139, 445, 2375, 3306, 3389, 5432, 5900, 5901, 6379, 8006, 9000, 9090, 9200, 11211, 27017, 1883];

/// Findings from what a discovery already saw.
pub fn discovery_findings(d: &Discovery) -> Vec<NetFinding> {
    let mut out = Vec::new();
    let mut upnp = false;
    for e in &d.exposures {
        match e.kind {
            ExposureKind::Upnp => {
                upnp = true;
                let (ip, port) = e.target.rsplit_once(':').map_or((e.target.as_str(), None), |(h, p)| (h, p.parse::<u16>().ok()));
                if let Some(p) = port.filter(|p| RISKY_FORWARD.contains(p)) {
                    let what = port_name(p);
                    out.push(finding(
                        Some(ip),
                        Some(p),
                        Tone::Bad,
                        format!("The router forwards port {} from the internet to {ip}:{p}{}", e.port.unwrap_or(p), if what.is_empty() { String::new() } else { format!(" ({what})") }),
                        "Anyone on the internet can reach this service and try passwords or known bugs. Remove the forward, or reach it over Tailscale or a VPN instead.",
                    ));
                }
            }
            ExposureKind::PublicAddress => out.push(finding(
                None,
                None,
                Tone::Warn,
                format!("The scanning machine has a public address ({})", e.public),
                "Every service listening on all interfaces is reachable from the internet unless a firewall blocks it. Check System → Firewall.",
            )),
            _ => {}
        }
    }
    if upnp {
        out.push(finding(
            d.gateway.as_deref(),
            None,
            Tone::Info,
            "The router accepts UPnP port forwards",
            "Any device on the LAN (including a compromised one) can open ports to the internet. Turn UPnP off on the router if nothing needs it.",
        ));
    }
    let gw = d.gateway.as_deref();
    let mut servers: Vec<&String> = d.dhcp.iter().collect();
    servers.sort();
    servers.dedup();
    if servers.len() > 1 {
        out.push(finding(
            None,
            None,
            Tone::Bad,
            format!("More than one DHCP server answered: {}", servers.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")),
            "Devices take whichever answers first, so some get the wrong gateway or DNS. Keep one DHCP server per network.",
        ));
    } else if let (Some(s), Some(g)) = (servers.first(), gw) {
        if s.as_str() != g {
            out.push(finding(
                Some(s),
                None,
                Tone::Warn,
                format!("Addresses come from {s}, not the router ({g})"),
                "Fine if that's your DHCP server (Pi-hole, AdGuard, a server). If you don't know it, a device is handing out addresses and could redirect traffic.",
            ));
        }
    }
    out
}

/// TLS ports and web ports the web check tries on each device.
pub const TLS_PORTS: &[u16] = &[443, 8443, 5001, 9443, 8006, 10443];
pub const WEB_PORTS: &[u16] = &[80, 8080, 8000, 5000, 81, 8081];
pub const MAX_DEVICES: usize = 64;

const CERT: &str = "@@NGC@@";
const PAGE: &str = "@@NGH@@";

const WEB_PY: &str = r#"
import base64, socket, ssl, sys, concurrent.futures as cf
tls = [int(p) for p in sys.argv[1].split(",")]
web = [int(p) for p in sys.argv[2].split(",")]
hosts = sys.argv[3:]
def cert(h, p):
    try:
        s = socket.create_connection((h, p), 1.5)
    except Exception:
        return None
    try:
        c = ssl.create_default_context(); c.check_hostname = False; c.verify_mode = ssl.CERT_NONE
        w = c.wrap_socket(s, server_hostname=h); w.settimeout(2)
        der = w.getpeercert(True); w.close()
        return "@@NGC@@\t%s\t%d\t%s" % (h, p, base64.b64encode(der).decode()) if der else None
    except Exception:
        return None
def page(h, p):
    try:
        s = socket.create_connection((h, p), 1.5)
    except Exception:
        return None
    s.settimeout(2)
    b = b""
    try:
        s.sendall(b"GET / HTTP/1.0\r\nHost: " + h.encode() + b"\r\nUser-Agent: nightgrid\r\n\r\n")
        while len(b) < 16384:
            d = s.recv(4096)
            if not d:
                break
            b += d
    except Exception:
        pass
    s.close()
    if not b.startswith(b"HTTP/"):
        return None
    return "@@NGH@@\t%s\t%d\t%s" % (h, p, base64.b64encode(b[:16384]).decode())
jobs = [(cert, h, p) for h in hosts for p in tls] + [(page, h, p) for h in hosts for p in web]
with cf.ThreadPoolExecutor(32) as ex:
    for r in ex.map(lambda j: j[0](j[1], j[2]), jobs):
        if r:
            print(r, flush=True)
"#;

/// The web check script for these addresses (validated: IP addresses only, at most 64).
pub fn web_script(ips: &[String]) -> Result<String, String> {
    if ips.is_empty() {
        return Err("no devices to check".into());
    }
    if ips.len() > MAX_DEVICES {
        return Err(format!("at most {MAX_DEVICES} devices at a time"));
    }
    for ip in ips {
        if ip.parse::<IpAddr>().is_err() {
            return Err(format!("{ip:?} isn't an address"));
        }
    }
    let list = |p: &[u16]| p.iter().map(u16::to_string).collect::<Vec<_>>().join(",");
    Ok(format!(
        "if command -v python3 >/dev/null 2>&1; then python3 -c {} {} {} {}; else echo '@@NGNOTE@@ the web check needs python3 on the scanning machine'; fi",
        shell_quote(WEB_PY),
        list(TLS_PORTS),
        list(WEB_PORTS),
        ips.iter().map(|i| shell_quote(i)).collect::<Vec<_>>().join(" ")
    ))
}

/// Admin pages of gear known to ship with a default password, by what the page says.
const ADMIN_PAGES: &[(&str, &str)] = &[
    ("routeros", "MikroTik router"),
    ("mikrotik", "MikroTik router"),
    ("dd-wrt", "DD-WRT router"),
    ("luci", "OpenWrt router"),
    ("openwrt", "OpenWrt router"),
    ("tp-link", "TP-Link device"),
    ("netgear", "Netgear device"),
    ("asuswrt", "ASUS router"),
    ("linksys", "Linksys router"),
    ("fritz!box", "FRITZ!Box"),
    ("hp embedded web server", "HP printer"),
    ("epson", "Epson printer"),
    ("brother", "Brother printer"),
    ("hikvision", "Hikvision camera"),
    ("doc/page/login.asp", "Hikvision camera"),
    ("netsurveillance", "IP camera (XMEye)"),
    ("dahua", "Dahua camera"),
    ("reolink", "Reolink camera"),
    ("ubiquiti", "Ubiquiti device"),
    ("unifi", "UniFi controller"),
    ("phpmyadmin", "phpMyAdmin"),
    ("webmin", "Webmin"),
    ("apache tomcat", "Tomcat manager"),
    ("jenkins", "Jenkins"),
    ("grafana", "Grafana"),
    ("portainer", "Portainer"),
];

fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim())
    })
}

fn title(body: &str) -> Option<String> {
    let lower = body.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let open = lower[start..].find('>')? + start + 1;
    let end = lower[open..].find("</title>")? + open;
    let t: String = body[open..end].split_whitespace().collect::<Vec<_>>().join(" ");
    (!t.is_empty()).then(|| t.chars().take(80).collect())
}

/// Turns the web check's output into findings. `now` is unix seconds.
pub fn parse_web(out: &str, now: i64) -> Vec<NetFinding> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let mut found = Vec::new();
    for l in out.lines() {
        let f: Vec<&str> = l.splitn(4, '\t').collect();
        if f.len() != 4 {
            continue;
        }
        let (ip, Ok(port)) = (f[1], f[2].parse::<u16>()) else { continue };
        let Ok(bytes) = b64.decode(f[3].trim()) else { continue };
        if f[0] == CERT {
            let Ok((_, c)) = x509_parser::parse_x509_certificate(&bytes) else { continue };
            let subject = c.subject().to_string();
            let issuer = c.issuer().to_string();
            let end = c.validity().not_after.timestamp();
            let days = (end - now).div_euclid(86_400);
            if days < 0 {
                found.push(finding(
                    Some(ip),
                    Some(port),
                    Tone::Bad,
                    format!("Certificate expired {} days ago", -days),
                    format!("{subject}. Browsers refuse it and services that check it fail."),
                ));
            } else if days < 14 {
                found.push(finding(
                    Some(ip),
                    Some(port),
                    Tone::Warn,
                    format!("Certificate expires in {days} days"),
                    format!("{subject}, issued by {issuer}."),
                ));
            }
            if subject == issuer {
                found.push(finding(Some(ip), Some(port), Tone::Info, "Self-signed certificate", format!("{subject}. Browsers warn about it; fine on a LAN if you know it, but it can't prove it's the real device.")));
            }
        } else if f[0] == PAGE {
            let text = String::from_utf8_lossy(&bytes);
            let (head, body) = text.split_once("\r\n\r\n").or_else(|| text.split_once("\n\n")).unwrap_or((&text, ""));
            let status = head.lines().next().and_then(|s| s.split_whitespace().nth(1)).unwrap_or("");
            let redirect_tls = header(head, "location").is_some_and(|v| v.to_ascii_lowercase().starts_with("https://"));
            let lower = format!("{} {}", header(head, "server").unwrap_or(""), body).to_ascii_lowercase();
            let name = ADMIN_PAGES.iter().find(|(sig, _)| lower.contains(sig)).map(|(_, n)| *n);
            let page = title(body).map_or(String::new(), |t| format!(" (\"{t}\")"));
            if let Some(n) = name {
                found.push(finding(Some(ip), Some(port), Tone::Info, format!("Admin page of a {n}{page}"), "This kind of device ships with a default password. Make sure it was changed, and that the page isn't forwarded to the internet."));
            }
            if !redirect_tls
                && (lower.contains("type=\"password\"")
                    || lower.contains("type='password'")
                    || lower.contains("type=password"))
            {
                found.push(finding(
                    Some(ip),
                    Some(port),
                    Tone::Warn,
                    format!("Login form over plain HTTP{page}"),
                    "Passwords typed here cross the network readable. Use the device's HTTPS address if it has one.",
                ));
            } else if status == "401"
                && header(head, "www-authenticate").is_some_and(|v| v.to_ascii_lowercase().starts_with("basic"))
            {
                found.push(finding(
                    Some(ip),
                    Some(port),
                    Tone::Warn,
                    "Password prompt over plain HTTP (basic auth)",
                    "The password is sent with every request, readable on the network.",
                ));
            }
        }
    }
    found.sort_by(|a, b| b.level.cmp(&a.level).then(a.ip.cmp(&b.ip)).then(a.port.cmp(&b.port)));
    found.dedup();
    found
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

/// Runs the web check from a host.
pub async fn web_check(conn: &Connection, ips: &[String]) -> Result<Vec<NetFinding>, String> {
    let sh = web_script(ips)?;
    let (_, out, _) = tokio::time::timeout(std::time::Duration::from_secs(150), conn.exec(&sh))
        .await
        .map_err(|_| "the web check took too long".to_string())?
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out);
    if let Some(n) = text.lines().find_map(|l| l.strip_prefix("@@NGNOTE@@")) {
        return Err(n.trim().to_owned());
    }
    Ok(parse_web(&text, unix_now()))
}

/// Runs the web check from this computer.
#[cfg(unix)]
pub async fn web_check_local(ips: &[String]) -> Result<Vec<NetFinding>, String> {
    let sh = web_script(ips)?;
    let run = tokio::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(&sh)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(std::time::Duration::from_secs(150), run)
        .await
        .map_err(|_| "the web check took too long".to_string())?
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    if let Some(n) = text.lines().find_map(|l| l.strip_prefix("@@NGNOTE@@")) {
        return Err(n.trim().to_owned());
    }
    Ok(parse_web(&text, unix_now()))
}

#[cfg(not(unix))]
pub async fn web_check_local(_ips: &[String]) -> Result<Vec<NetFinding>, String> {
    Err("The web check runs on macOS and Linux. On Windows, run it from one of your hosts.".into())
}

/// What the scheduled network watch reports (D-145), from one scan.
/// `expected` are the keys saved as the expected map; a device that isn't
/// there counts as missing only when the scan covered its subnet (it is in
/// `d.missing`). New devices are untrusted ones first seen in the last day.
pub fn watch_conditions(
    d: &Discovery,
    expected: &[String],
    cfg: &crate::settings::NetWatchSettings,
    scanner: &str,
    now: i64,
) -> Vec<crate::alerts::Condition> {
    use crate::alerts::{AlertKind, Condition, Severity};
    let mut out = Vec::new();
    if cfg.alert_new {
        for x in d.devices.iter().filter(|x| !x.trusted && !x.routed) {
            let recent = match x.first_seen {
                Some(t) => now - t <= 86_400,
                None => x.is_new,
            };
            if !recent {
                continue;
            }
            let key = crate::device_labels::key_for(x.mac.as_deref(), &x.ip);
            let what = x.names.first().cloned().or_else(|| x.vendor.clone()).unwrap_or_else(|| x.kind.clone());
            out.push(Condition {
                key: format!("netwatch:new:{key}"),
                kind: AlertKind::NetNewDevice,
                severity: Severity::Warn,
                title: format!("{} ({what})", x.ip),
                detail: format!("A device {scanner} hasn't seen before joined the network. Mark it trusted on the network map if it's yours."),
                host_id: None,
            });
        }
    }
    if cfg.alert_missing && !expected.is_empty() {
        for m in d.missing.iter().filter(|m| expected.contains(&m.key)) {
            let what = m.name.clone().or_else(|| m.vendor.clone()).unwrap_or_else(|| m.kind.clone());
            out.push(Condition {
                key: format!("netwatch:missing:{}", m.key),
                kind: AlertKind::NetMissing,
                severity: Severity::Warn,
                title: format!("{what} ({})", m.ip),
                detail: format!("On the expected network map, but it didn't answer {scanner}'s scan."),
                host_id: None,
            });
        }
    }
    if cfg.alert_risks {
        for f in d.findings.iter().filter(|f| f.level >= Tone::Warn) {
            let at = match (&f.ip, f.port) {
                (Some(ip), Some(p)) => format!("{ip}:{p}"),
                (Some(ip), None) => ip.clone(),
                _ => String::new(),
            };
            out.push(Condition {
                key: format!("netwatch:risk:{at}:{}", f.title),
                kind: AlertKind::NetRisk,
                severity: if f.level == Tone::Bad { Severity::Critical } else { Severity::Warn },
                title: f.title.clone(),
                detail: f.detail.clone(),
                host_id: None,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::netmap::parse_discovery;

    #[test]
    fn findings_from_a_scan() {
        let mut d = parse_discovery(
            "@@NGNET@@ route\ndefault via 192.168.1.1 dev eth0\n@@NGNET@@ upnp\nExternalIPAddress = 203.0.113.7\n \
             0 TCP  3389->192.168.1.50:3389  'rdp' '' 0\n 1 TCP 32400->192.168.1.40:32400 'plex' '' 0\n@@NGNET@@ dhcp\n\
             option dhcp-server-identifier 192.168.1.5;\n@@NGNET@@ end\n",
        );
        let f = discovery_findings(&d);
        assert_eq!(f[0].level, Tone::Bad);
        assert!(f[0].title.contains("192.168.1.50:3389 (RDP)"));
        assert!(f.iter().any(|x| x.title == "The router accepts UPnP port forwards"));
        assert!(f.iter().any(|x| x.title == "Addresses come from 192.168.1.5, not the router (192.168.1.1)"));
        d.dhcp.push("192.168.1.99".into());
        assert!(discovery_findings(&d)
            .iter()
            .any(|x| x.level == Tone::Bad && x.title.starts_with("More than one DHCP")));
        assert_eq!(d.findings.len(), 3);
    }

    #[test]
    fn web_check_input_and_pages() {
        assert!(web_script(&["192.168.1.1".into(), "$(id)".into()]).is_err());
        assert!(web_script(&[]).is_err());
        let s = web_script(&["192.168.1.1".into()]).unwrap();
        assert!(s.contains("'192.168.1.1'") && s.contains("443,8443"));
        let b64 = base64::engine::general_purpose::STANDARD;
        let page = "HTTP/1.1 200 OK\r\nServer: lighttpd\r\n\r\n<html><title>Hikvision Login</title><input type=\"password\" name=p></html>";
        let auth = "HTTP/1.0 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"x\"\r\n\r\n";
        let redirect = "HTTP/1.1 302 Found\r\nLocation: https://192.168.1.9/\r\n\r\n<input type=password>";
        let out = format!(
            "@@NGH@@\t192.168.1.7\t80\t{}\n@@NGH@@\t192.168.1.8\t80\t{}\n@@NGH@@\t192.168.1.9\t80\t{}\nnoise\n",
            b64.encode(page),
            b64.encode(auth),
            b64.encode(redirect)
        );
        let f = parse_web(&out, 0);
        let titles: Vec<&str> = f.iter().map(|x| x.title.as_str()).collect();
        assert!(titles.contains(&"Login form over plain HTTP (\"Hikvision Login\")"));
        assert!(titles.contains(&"Admin page of a Hikvision camera (\"Hikvision Login\")"));
        assert!(titles.contains(&"Password prompt over plain HTTP (basic auth)"));
        assert!(!f.iter().any(|x| x.ip.as_deref() == Some("192.168.1.9")));
    }

    #[test]
    fn watch_reports_new_missing_and_risks() {
        let mut d = parse_discovery(
            "@@NGNET@@ route\ndefault via 192.168.1.1 dev eth0\n@@NGNET@@ upnp\nExternalIPAddress = 203.0.113.7\n \
             0 TCP  3389->192.168.1.50:3389  'rdp' '' 0\n@@NGNET@@ neigh\n\
             192.168.1.60 dev eth0 lladdr aa:bb:cc:00:00:60 REACHABLE\n@@NGNET@@ end\n",
        );
        for x in &mut d.devices {
            x.is_new = true;
            x.first_seen = Some(1000);
        }
        d.missing.push(crate::netmap::MissingDevice {
            key: "aa:bb:cc:00:00:61".into(),
            ip: "192.168.1.61".into(),
            name: Some("lamp".into()),
            vendor: None,
            kind: "iot".into(),
            last_seen: 10,
            trusted: true,
        });
        let cfg = crate::settings::NetWatchSettings::default();
        let c = watch_conditions(&d, &["aa:bb:cc:00:00:61".into()], &cfg, "lab", 2000);
        assert!(c.iter().any(|x| x.key == "netwatch:new:aa:bb:cc:00:00:60"));
        assert!(c.iter().any(|x| x.key == "netwatch:missing:aa:bb:cc:00:00:61" && x.title == "lamp (192.168.1.61)"));
        assert!(c
            .iter()
            .any(|x| x.kind == crate::alerts::AlertKind::NetRisk && x.severity == crate::alerts::Severity::Critical));
        // Old news and nothing expected: quiet.
        let c = watch_conditions(&d, &[], &cfg, "lab", 1000 + 90_000);
        assert!(c.iter().all(|x| x.kind == crate::alerts::AlertKind::NetRisk));
    }
}
