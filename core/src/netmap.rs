//! Network map (M10i): what one host can see of its network, and what of it
//! faces the internet.
//!
//! Discovery runs on a host the user picks, only when they click: its
//! interfaces and default gateway, the neighbour (ARP/NDP) table, an optional
//! ping sweep of its own subnet (/24 or smaller), mDNS announcements
//! (`avahi-browse`), Tailscale peers, UPnP port mappings on the router
//! (`upnpc`), Tailscale Funnel and cloudflared tunnels. Nothing is sent to an
//! outside service (rule 7): "faces the internet" is judged from what the
//! host itself reports, never by probing from outside.
//!
//! The UI joins this with the saved hosts, their listening ports (security
//! audit) and reverse-proxy routes to draw the map.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};

use serde::Serialize;

use crate::ssh::client::Connection;

const MARK: &str = "@@NGNET@@";
/// Largest subnet the sweep walks (a /24).
pub const SWEEP_MAX_HOSTS: u32 = 254;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Iface {
    pub name: String,
    pub addr: String,
    pub prefix: u8,
    pub kind: AddrKind,
}

/// What kind of address this is, for exposure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum AddrKind {
    Loopback,
    /// RFC 1918, ULA (fc00::/7) or link-local.
    Private,
    /// Tailscale / CGNAT 100.64.0.0/10 or Tailscale's fd7a:115c:a1e0::/48.
    Tailnet,
    /// Anything else: routable on the internet.
    Public,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Seen {
    /// In the neighbour table.
    Neighbour,
    /// Answered the ping sweep.
    Ping,
    /// Announced over mDNS.
    Mdns,
    /// A Tailscale peer.
    Tailscale,
    /// The default gateway.
    Gateway,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub ip: String,
    pub mac: Option<String>,
    /// Names from mDNS or Tailscale.
    pub names: Vec<String>,
    /// mDNS service types (`_http._tcp`, `_smb._tcp`, …).
    pub services: Vec<String>,
    pub seen: Vec<Seen>,
    /// Tailscale: OS and whether it's online.
    pub os: Option<String>,
    pub online: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ExposureKind {
    /// The host has a public address on an interface.
    PublicAddress,
    /// The router forwards a port here (UPnP).
    Upnp,
    /// Tailscale Funnel publishes a port.
    Funnel,
    /// A cloudflared tunnel serves a hostname.
    Tunnel,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Exposure {
    pub kind: ExposureKind,
    /// Port on this host it lands on, when known.
    pub port: Option<u16>,
    /// Public side: an address, `external:port`, or a hostname.
    pub public: String,
    /// Internal side: `ip:port`, a URL or an interface.
    pub target: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Discovery {
    pub ifaces: Vec<Iface>,
    pub gateway: Option<String>,
    pub devices: Vec<Device>,
    pub exposures: Vec<Exposure>,
    /// The subnet that was swept, when asked.
    pub swept: Option<String>,
    /// Tools that were missing or failed, in words.
    pub notes: Vec<String>,
}

pub fn addr_kind(ip: &IpAddr) -> AddrKind {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            if v.is_loopback() {
                AddrKind::Loopback
            } else if o[0] == 100 && (64..128).contains(&o[1]) {
                AddrKind::Tailnet
            } else if v.is_private() || v.is_link_local() || v.is_unspecified() || v.is_broadcast() || o[0] >= 224 {
                AddrKind::Private
            } else {
                AddrKind::Public
            }
        }
        IpAddr::V6(v) => {
            let s = v.segments();
            if v.is_loopback() {
                AddrKind::Loopback
            } else if s[0] == 0xfd7a && s[1] == 0x115c && s[2] == 0xa1e0 {
                AddrKind::Tailnet
            } else if (s[0] & 0xfe00) == 0xfc00
                || (s[0] & 0xffc0) == 0xfe80
                || v.is_unspecified()
                || (s[0] & 0xff00) == 0xff00
            {
                AddrKind::Private
            } else if (s[0] & 0xe000) == 0x2000 {
                AddrKind::Public
            } else {
                AddrKind::Private
            }
        }
    }
}

/// The /24-or-smaller IPv4 subnet of `iface` to sweep, as (network, size).
pub fn sweep_range(iface: &Iface) -> Option<(Ipv4Addr, u32)> {
    let ip: Ipv4Addr = iface.addr.parse().ok()?;
    if addr_kind(&IpAddr::V4(ip)) != AddrKind::Private || !(24..=30).contains(&iface.prefix) {
        return None;
    }
    let mask = u32::MAX << (32 - u32::from(iface.prefix));
    let net = u32::from(ip) & mask;
    let hosts = (!mask).saturating_sub(1).min(SWEEP_MAX_HOSTS);
    Some((Ipv4Addr::from(net), hosts))
}

/// The discovery script. `sweep` is the subnet to ping (from [`sweep_range`]
/// of an earlier run), or none.
pub fn discover_command(sweep: Option<(Ipv4Addr, u32)>) -> String {
    // `t N cmd`: `timeout` where it exists (macOS and BSDs don't ship it).
    let mut s = String::from(
        "export LC_ALL=C\nt() { if command -v timeout >/dev/null 2>&1; then timeout \"$@\"; else shift; \"$@\"; fi; }\n\
         case $(uname 2>/dev/null) in Darwin|*BSD) pw='-t 1' ;; *) pw='-W 1' ;; esac\n",
    );
    let sec = |s: &mut String, name: &str, body: &str| {
        s.push_str(&format!("echo '{MARK} {name}'\n{body}\n"));
    };
    sec(&mut s, "addr", "ip -o addr show 2>/dev/null || ifconfig 2>/dev/null");
    sec(
        &mut s,
        "route",
        "ip route show default 2>/dev/null || netstat -rn 2>/dev/null | awk '$1==\"default\"||$1==\"0.0.0.0\"'",
    );
    if let Some((net, n)) = sweep {
        let o = net.octets();
        let base = u32::from(net);
        let last = Ipv4Addr::from(base + n);
        // Parallel single pings, 1 s each; answers fill the neighbour table too.
        sec(
            &mut s,
            &format!("sweep {net}/{}", 32 - (n + 1).next_power_of_two().trailing_zeros()),
            &format!(
                "if command -v fping >/dev/null 2>&1; then fping -a -q -r 0 -t 300 -g {}.{}.{}.{} {} 2>/dev/null; \
                 else i=1; while [ $i -le {n} ]; do ip=\"{}.{}.{}.$(( {} + i ))\"; (ping -c 1 $pw \"$ip\" >/dev/null 2>&1 && echo \"$ip\") & \
                 i=$((i+1)); if [ $((i % 64)) -eq 0 ]; then wait; fi; done; wait; fi",
                o[0],
                o[1],
                o[2],
                o[3] + 1,
                last,
                o[0],
                o[1],
                o[2],
                o[3]
            ),
        );
    }
    sec(&mut s, "neigh", "ip neigh show 2>/dev/null || arp -an 2>/dev/null");
    sec(
        &mut s,
        "mdns",
        "if command -v avahi-browse >/dev/null 2>&1; then t 5 avahi-browse -aprt 2>/dev/null | grep '^=' | head -n 400; else echo 'missing'; fi",
    );
    sec(
        &mut s,
        "tailscale",
        "if command -v tailscale >/dev/null 2>&1; then t 5 tailscale status --json 2>/dev/null | head -c 400000; else echo 'missing'; fi",
    );
    sec(
        &mut s,
        "funnel",
        "command -v tailscale >/dev/null 2>&1 && t 5 tailscale funnel status 2>/dev/null | head -n 50",
    );
    sec(
        &mut s,
        "upnp",
        "if command -v upnpc >/dev/null 2>&1; then t 8 upnpc -l 2>/dev/null | head -n 120; else echo 'missing'; fi",
    );
    sec(
        &mut s,
        "cloudflared",
        "if pgrep -x cloudflared >/dev/null 2>&1 || command -v cloudflared >/dev/null 2>&1; then echo running; \
         for f in /etc/cloudflared/config.yml /etc/cloudflared/config.yaml \"$HOME/.cloudflared/config.yml\" \"$HOME/.cloudflared/config.yaml\"; do \
         [ -r \"$f\" ] && grep -E '^[[:space:]-]*(hostname|service):' \"$f\" | head -n 100; done; fi",
    );
    sec(&mut s, "end", "");
    s
}

fn sections(out: &str) -> Vec<(String, Vec<&str>)> {
    let mut v: Vec<(String, Vec<&str>)> = Vec::new();
    for line in out.lines() {
        if let Some(name) = line.strip_prefix(MARK) {
            v.push((name.trim().to_owned(), Vec::new()));
        } else if let Some(last) = v.last_mut() {
            last.1.push(line);
        }
    }
    v
}

/// `2: eth0    inet 192.168.1.5/24 brd … scope global eth0` (ip -o addr), or
/// BSD ifconfig blocks.
pub fn parse_addrs(lines: &[&str]) -> Vec<Iface> {
    let mut out = Vec::new();
    let mut bsd_iface = String::new();
    for l in lines {
        let w: Vec<&str> = l.split_whitespace().collect();
        if w.len() >= 4 && w[0].ends_with(':') && w[0][..w[0].len() - 1].chars().all(|c| c.is_ascii_digit()) {
            // ip -o addr
            let name = w[1].trim_end_matches(':').to_owned();
            if let Some(i) = w.iter().position(|x| *x == "inet" || *x == "inet6") {
                if let Some((a, p)) = w.get(i + 1).and_then(|c| c.split_once('/')) {
                    if let (Ok(ip), Ok(prefix)) = (a.parse::<IpAddr>(), p.parse::<u8>()) {
                        out.push(Iface { name, addr: ip.to_string(), prefix, kind: addr_kind(&ip) });
                    }
                }
            }
        } else if !l.starts_with([' ', '\t']) && l.contains(": flags=") {
            bsd_iface = l.split(':').next().unwrap_or("").to_owned();
        } else if w.first().is_some_and(|x| *x == "inet" || *x == "inet6") && !bsd_iface.is_empty() {
            let a = w.get(1).map(|a| a.split('%').next().unwrap_or(a)).unwrap_or("");
            let Ok(ip) = a.parse::<IpAddr>() else { continue };
            let prefix = match ip {
                IpAddr::V4(_) => w
                    .iter()
                    .position(|x| *x == "netmask")
                    .and_then(|i| w.get(i + 1))
                    .and_then(|m| u32::from_str_radix(m.trim_start_matches("0x"), 16).ok())
                    .map_or(24, |m| m.count_ones() as u8),
                IpAddr::V6(_) => w
                    .iter()
                    .position(|x| *x == "prefixlen")
                    .and_then(|i| w.get(i + 1))
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(64),
            };
            out.push(Iface { name: bsd_iface.clone(), addr: ip.to_string(), prefix, kind: addr_kind(&ip) });
        }
    }
    out
}

/// `default via 192.168.1.1 dev eth0` or `default 192.168.1.1 UGScg en0`.
pub fn parse_gateway(lines: &[&str]) -> Option<String> {
    lines.iter().find_map(|l| {
        let w: Vec<&str> = l.split_whitespace().collect();
        let i = w.iter().position(|x| *x == "via").map(|i| i + 1).unwrap_or(1);
        w.get(i).filter(|a| a.parse::<IpAddr>().is_ok()).map(|a| (*a).to_owned())
    })
}

/// `192.168.1.20 dev eth0 lladdr aa:bb:… REACHABLE` or BSD
/// `? (192.168.1.20) at aa:bb:… on en0`. Failed/incomplete entries are skipped.
pub fn parse_neigh(lines: &[&str]) -> Vec<(String, Option<String>)> {
    lines
        .iter()
        .filter_map(|l| {
            if l.contains("FAILED") || l.contains("INCOMPLETE") || l.contains("incomplete") {
                return None;
            }
            let w: Vec<&str> = l.split_whitespace().collect();
            let (ip, mac) = if let Some(p) = w.iter().find(|x| x.starts_with('(') && x.ends_with(')')) {
                let mac = w.iter().position(|x| *x == "at").and_then(|i| w.get(i + 1)).map(|m| (*m).to_owned());
                (p.trim_matches(['(', ')']).to_owned(), mac)
            } else {
                let mac = w.iter().position(|x| *x == "lladdr").and_then(|i| w.get(i + 1)).map(|m| (*m).to_owned());
                ((*w.first()?).to_owned(), mac)
            };
            let parsed: IpAddr = ip.parse().ok()?;
            if parsed.is_multicast() || matches!(parsed, IpAddr::V6(v) if (v.segments()[0] & 0xffc0) == 0xfe80) {
                return None;
            }
            Some((ip, mac.map(|m| m.to_ascii_lowercase())))
        })
        .collect()
}

/// `=;eth0;IPv4;Living Room TV;_googlecast._tcp;local;tv.local;192.168.1.30;8009;…`
pub fn parse_mdns(lines: &[&str]) -> Vec<(String, String, String)> {
    lines
        .iter()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(';').collect();
            if f.first() != Some(&"=") || f.len() < 9 {
                return None;
            }
            let ip = f[7];
            ip.parse::<IpAddr>().ok()?;
            let name = unescape_avahi(f[3]);
            Some((ip.to_owned(), name, f[4].to_owned()))
        })
        .collect()
}

/// avahi escapes as `\032` (decimal) for spaces and such.
fn unescape_avahi(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 3 < b.len() && b[i + 1..i + 4].iter().all(u8::is_ascii_digit) {
            let n: u32 = s[i + 1..i + 4].parse().unwrap_or(32);
            out.push(u8::try_from(n).unwrap_or(b' '));
            i += 4;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Peers from `tailscale status --json`: (ip, name, os, online).
pub fn parse_tailscale(json: &str) -> Vec<(String, String, Option<String>, Option<bool>)> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return Vec::new() };
    let Some(peers) = v.get("Peer").and_then(|p| p.as_object()) else { return Vec::new() };
    peers
        .values()
        .filter_map(|p| {
            let ip = p.get("TailscaleIPs")?.as_array()?.iter().find_map(|x| x.as_str().filter(|s| s.contains('.')))?;
            let name = p
                .get("HostName")
                .and_then(|x| x.as_str())
                .filter(|s| !s.is_empty())
                .or_else(|| p.get("DNSName").and_then(|x| x.as_str()).map(|d| d.split('.').next().unwrap_or(d)))?;
            Some((
                ip.to_owned(),
                name.to_owned(),
                p.get("OS").and_then(|x| x.as_str()).filter(|s| !s.is_empty()).map(str::to_owned),
                p.get("Online").and_then(|x| x.as_bool()),
            ))
        })
        .collect()
}

/// One UPnP mapping: protocol, external port, internal address, internal port, description.
pub type UpnpMapping = (String, u16, String, u16, String);

/// `upnpc -l` lines: ` 0 TCP  8443->192.168.1.5:443  'nas' '' 0`, plus
/// `ExternalIPAddress = 203.0.113.7`.
pub fn parse_upnp(lines: &[&str]) -> (Option<String>, Vec<UpnpMapping>) {
    let mut external = None;
    let mut maps = Vec::new();
    for l in lines {
        if let Some(ip) = l.split_once("ExternalIPAddress =").map(|(_, r)| r.trim()) {
            external = Some(ip.to_owned());
            continue;
        }
        let w: Vec<&str> = l.split_whitespace().collect();
        if w.len() < 3 || !w[0].chars().all(|c| c.is_ascii_digit()) || !matches!(w[1], "TCP" | "UDP") {
            continue;
        }
        let Some((ext, int)) = w[2].split_once("->") else { continue };
        let Some((iip, iport)) = int.rsplit_once(':') else { continue };
        let (Ok(ext), Ok(iport)) = (ext.parse::<u16>(), iport.parse::<u16>()) else { continue };
        let desc = l.split('\'').nth(1).unwrap_or("").to_owned();
        maps.push((w[1].to_owned(), ext, iip.to_owned(), iport, desc));
    }
    (external, maps)
}

/// `tailscale funnel status`: `https://box.tail1234.ts.net (Funnel on)` then
/// `|-- / proxy http://127.0.0.1:8080`.
pub fn parse_funnel(lines: &[&str]) -> Vec<Exposure> {
    let mut out = Vec::new();
    let mut site: Option<String> = None;
    for l in lines {
        let t = l.trim();
        if t.starts_with("https://") || t.starts_with("tcp://") {
            site = t.contains("Funnel on").then(|| t.split_whitespace().next().unwrap_or(t).to_owned());
        } else if let (Some(s), Some(rest)) = (&site, t.strip_prefix("|--")) {
            let w: Vec<&str> = rest.split_whitespace().collect();
            let target = w.last().copied().unwrap_or("").to_owned();
            let port = target.rsplit_once(':').and_then(|(_, p)| p.trim_end_matches('/').parse().ok());
            out.push(Exposure {
                kind: ExposureKind::Funnel,
                port,
                public: format!("{s}{}", w.first().copied().filter(|p| *p != "/").unwrap_or("")),
                target,
                detail: "Tailscale Funnel: public on the internet".into(),
            });
        }
    }
    out
}

/// cloudflared config: `- hostname: app.example.com` / `service: http://localhost:8080`.
pub fn parse_cloudflared(lines: &[&str]) -> (bool, Vec<Exposure>) {
    let running = lines.first().is_some_and(|l| l.trim() == "running");
    let mut out = Vec::new();
    let mut host: Option<String> = None;
    for l in lines.iter().skip(1) {
        let t = l.trim().trim_start_matches('-').trim();
        if let Some(h) = t.strip_prefix("hostname:") {
            host = Some(h.trim().trim_matches(['"', '\'']).to_owned());
        } else if let Some(svc) = t.strip_prefix("service:") {
            let svc = svc.trim().trim_matches(['"', '\'']).to_owned();
            if let Some(h) = host.take() {
                let port = svc
                    .split("://")
                    .nth(1)
                    .and_then(|r| r.split('/').next())
                    .and_then(|hp| hp.rsplit_once(':'))
                    .and_then(|(_, p)| p.parse().ok());
                out.push(Exposure {
                    kind: ExposureKind::Tunnel,
                    port,
                    public: h,
                    target: svc,
                    detail: "Cloudflare Tunnel: public through Cloudflare".into(),
                });
            }
        }
    }
    (running, out)
}

pub fn parse_discovery(out: &str) -> Discovery {
    let mut d = Discovery {
        ifaces: Vec::new(),
        gateway: None,
        devices: Vec::new(),
        exposures: Vec::new(),
        swept: None,
        notes: Vec::new(),
    };
    let mut devs: BTreeMap<String, Device> = BTreeMap::new();
    for (name, body) in sections(out) {
        let head = name.split_whitespace().next().unwrap_or("");
        match head {
            "addr" => d.ifaces = parse_addrs(&body),
            "route" => d.gateway = parse_gateway(&body),
            "sweep" => {
                d.swept = name.split_whitespace().nth(1).map(str::to_owned);
                for l in &body {
                    let ip = l.trim();
                    if ip.parse::<IpAddr>().is_ok() {
                        add(&mut devs, ip, Seen::Ping);
                    }
                }
            }
            "neigh" => {
                for (ip, mac) in parse_neigh(&body) {
                    let e = add(&mut devs, &ip, Seen::Neighbour);
                    if e.mac.is_none() {
                        e.mac = mac;
                    }
                }
            }
            "mdns" => {
                if body.first().is_some_and(|l| l.trim() == "missing") {
                    d.notes.push(
                        "No mDNS names: avahi-browse isn't installed (on Linux: apt install avahi-utils).".into(),
                    );
                }
                for (ip, n, svc) in parse_mdns(&body) {
                    let e = add(&mut devs, &ip, Seen::Mdns);
                    if !e.names.contains(&n) {
                        e.names.push(n);
                    }
                    if !e.services.contains(&svc) {
                        e.services.push(svc);
                    }
                }
            }
            "tailscale" => {
                if body.first().is_some_and(|l| l.trim() == "missing") {
                    continue;
                }
                for (ip, n, os, online) in parse_tailscale(&body.join("\n")) {
                    let e = add(&mut devs, &ip, Seen::Tailscale);
                    if !e.names.contains(&n) {
                        e.names.insert(0, n);
                    }
                    e.os = os;
                    e.online = online;
                }
            }
            "funnel" => d.exposures.extend(parse_funnel(&body)),
            "upnp" => {
                if body.first().is_some_and(|l| l.trim() == "missing") {
                    d.notes.push(
                        "upnpc isn't installed: router port forwards (UPnP) weren't read (apt install miniupnpc)."
                            .into(),
                    );
                    continue;
                }
                let (ext, maps) = parse_upnp(&body);
                let public = ext.unwrap_or_else(|| "router".into());
                for (proto, eport, iip, iport, desc) in maps {
                    d.exposures.push(Exposure {
                        kind: ExposureKind::Upnp,
                        port: Some(iport),
                        public: format!("{public}:{eport}/{}", proto.to_ascii_lowercase()),
                        target: format!("{iip}:{iport}"),
                        detail: if desc.is_empty() {
                            "UPnP port forward on the router".into()
                        } else {
                            format!("UPnP port forward on the router ({desc})")
                        },
                    });
                }
            }
            "cloudflared" => {
                let (_, ex) = parse_cloudflared(&body);
                d.exposures.extend(ex);
            }
            _ => {}
        }
    }
    if let Some(g) = &d.gateway {
        add(&mut devs, g, Seen::Gateway);
    }
    for i in &d.ifaces {
        if i.kind == AddrKind::Public {
            d.exposures.push(Exposure {
                kind: ExposureKind::PublicAddress,
                port: None,
                public: i.addr.clone(),
                target: i.name.clone(),
                detail: if i.addr.contains(':') {
                    "Global IPv6 address: ports on every interface are reachable unless a firewall blocks them".into()
                } else {
                    "Public IPv4 address on this interface: ports on every interface face the internet unless a firewall blocks them".into()
                },
            });
        }
    }
    // The host's own addresses aren't other devices.
    let own: Vec<&str> = d.ifaces.iter().map(|i| i.addr.as_str()).collect();
    d.devices = devs.into_values().filter(|x| !own.contains(&x.ip.as_str())).collect();
    d.devices.sort_by_key(|x| ip_sort_key(&x.ip));
    d
}

fn add<'a>(devs: &'a mut BTreeMap<String, Device>, ip: &str, seen: Seen) -> &'a mut Device {
    let e = devs.entry(ip.to_owned()).or_insert_with(|| Device {
        ip: ip.to_owned(),
        mac: None,
        names: Vec::new(),
        services: Vec::new(),
        seen: Vec::new(),
        os: None,
        online: None,
    });
    if !e.seen.contains(&seen) {
        e.seen.push(seen);
    }
    e
}

fn ip_sort_key(ip: &str) -> (u8, u128) {
    match ip.parse::<IpAddr>() {
        Ok(IpAddr::V4(v)) => (0, u128::from(u32::from(v))),
        Ok(IpAddr::V6(v)) => (1, u128::from(v)),
        Err(_) => (2, 0),
    }
}

/// Runs discovery on `conn`, optionally sweeping its LAN subnet.
pub async fn discover(conn: &Connection, sweep: bool) -> Result<Discovery, String> {
    discover_with(sweep, |script| async move {
        let (_, out, _) = conn.exec(&script).await.map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(&out).into_owned())
    })
    .await
}

/// Runs discovery on this computer (macOS, Linux and BSDs; `sh` with the same script).
#[cfg(unix)]
pub async fn discover_local(sweep: bool) -> Result<Discovery, String> {
    discover_with(sweep, |script| async move {
        let run = tokio::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(&script)
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true)
            .output();
        let out = tokio::time::timeout(std::time::Duration::from_secs(90), run)
            .await
            .map_err(|_| "discovery took too long".to_string())?
            .map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    })
    .await
}

#[cfg(not(unix))]
pub async fn discover_local(_sweep: bool) -> Result<Discovery, String> {
    Err("Scanning from this computer works on macOS and Linux. On Windows, scan from one of your hosts.".into())
}

async fn discover_with<F, Fut>(sweep: bool, exec: F) -> Result<Discovery, String>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Result<String, String>>,
{
    let range = if sweep {
        let out = exec(discover_command(None)).await?;
        let first = parse_discovery(&out);
        let gw = first.gateway.as_deref().and_then(|g| g.parse::<Ipv4Addr>().ok());
        // The interface that holds the gateway, else the first private /24.
        first
            .ifaces
            .iter()
            .filter(|i| i.kind == AddrKind::Private)
            .filter_map(|i| sweep_range(i).map(|r| (i, r)))
            .find(|(_, (net, n))| gw.is_some_and(|g| (u32::from(g).wrapping_sub(u32::from(*net))) <= *n))
            .or_else(|| first.ifaces.iter().filter_map(|i| sweep_range(i).map(|r| (i, r))).next())
            .map(|(_, r)| r)
    } else {
        None
    };
    let out = exec(discover_command(range)).await?;
    let mut d = parse_discovery(&out);
    if sweep && range.is_none() {
        d.notes.push("No private /24 (or smaller) subnet to sweep on this host.".into());
    }
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        let k = |s: &str| addr_kind(&s.parse().unwrap());
        assert_eq!(k("192.168.1.5"), AddrKind::Private);
        assert_eq!(k("100.101.2.3"), AddrKind::Tailnet);
        assert_eq!(k("203.0.113.7"), AddrKind::Public);
        assert_eq!(k("2001:db8::1"), AddrKind::Public);
        assert_eq!(k("fe80::1"), AddrKind::Private);
        assert_eq!(k("fd7a:115c:a1e0::5"), AddrKind::Tailnet);
        assert_eq!(k("127.0.0.1"), AddrKind::Loopback);
    }

    /// The same script runs on this computer (macOS on CI, Linux here).
    #[cfg(unix)]
    #[tokio::test]
    async fn runs_locally() {
        let d = discover_local(false).await.expect("local discovery");
        assert!(d.swept.is_none());
        assert!(d.ifaces.iter().all(|i| i.kind != AddrKind::Loopback));
    }

    #[test]
    fn sweep_only_small_private() {
        let i =
            |a: &str, p| Iface { name: "eth0".into(), addr: a.into(), prefix: p, kind: addr_kind(&a.parse().unwrap()) };
        assert_eq!(sweep_range(&i("192.168.1.5", 24)), Some(("192.168.1.0".parse().unwrap(), 254)));
        assert_eq!(sweep_range(&i("10.0.0.9", 28)), Some(("10.0.0.0".parse().unwrap(), 14)));
        assert_eq!(sweep_range(&i("10.0.0.9", 16)), None);
        assert_eq!(sweep_range(&i("203.0.113.7", 24)), None);
    }

    #[test]
    fn parses_a_linux_discovery() {
        let out = "@@NGNET@@ addr\n\
1: lo    inet 127.0.0.1/8 scope host lo\n\
2: eth0    inet 192.168.1.5/24 brd 192.168.1.255 scope global eth0\n\
2: eth0    inet6 2001:db8::5/64 scope global\n\
3: tailscale0    inet 100.101.2.3/32 scope global tailscale0\n\
@@NGNET@@ route\n\
default via 192.168.1.1 dev eth0 proto dhcp\n\
@@NGNET@@ sweep 192.168.1.0/24\n\
192.168.1.1\n192.168.1.30\n\
@@NGNET@@ neigh\n\
192.168.1.1 dev eth0 lladdr AA:BB:CC:00:00:01 REACHABLE\n\
192.168.1.44 dev eth0  FAILED\n\
192.168.1.30 dev eth0 lladdr aa:bb:cc:00:00:30 STALE\n\
fe80::1 dev eth0 lladdr aa:bb:cc:00:00:01 router STALE\n\
@@NGNET@@ mdns\n\
=;eth0;IPv4;Living\\032Room\\032TV;_googlecast._tcp;local;tv.local;192.168.1.30;8009;\"id=1\"\n\
@@NGNET@@ tailscale\n\
{\"Peer\":{\"k\":{\"HostName\":\"laptop\",\"DNSName\":\"laptop.tail.ts.net.\",\"OS\":\"macOS\",\"TailscaleIPs\":[\"100.64.0.9\",\"fd7a:115c:a1e0::9\"],\"Online\":true}}}\n\
@@NGNET@@ funnel\n\
https://box.tail1234.ts.net (Funnel on)\n\
|-- / proxy http://127.0.0.1:8080\n\
@@NGNET@@ upnp\n\
ExternalIPAddress = 203.0.113.7\n\
 0 TCP  8443->192.168.1.5:443  'nas web' '' 0\n\
@@NGNET@@ cloudflared\n\
running\n\
  - hostname: app.example.com\n\
    service: http://localhost:3000\n\
@@NGNET@@ end\n";
        let d = parse_discovery(out);
        assert_eq!(d.gateway.as_deref(), Some("192.168.1.1"));
        assert_eq!(d.ifaces.len(), 4);
        assert_eq!(d.swept.as_deref(), Some("192.168.1.0/24"));
        let ips: Vec<&str> = d.devices.iter().map(|x| x.ip.as_str()).collect();
        assert_eq!(ips, vec!["100.64.0.9", "192.168.1.1", "192.168.1.30"]);
        let gw = &d.devices[1];
        assert!(gw.seen.contains(&Seen::Gateway) && gw.seen.contains(&Seen::Neighbour));
        assert_eq!(gw.mac.as_deref(), Some("aa:bb:cc:00:00:01"));
        let tv = &d.devices[2];
        assert_eq!(tv.names, vec!["Living Room TV"]);
        assert_eq!(tv.services, vec!["_googlecast._tcp"]);
        assert_eq!(d.devices[0].names, vec!["laptop"]);
        assert_eq!(d.devices[0].online, Some(true));
        let kinds: Vec<ExposureKind> = d.exposures.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![ExposureKind::Funnel, ExposureKind::Upnp, ExposureKind::Tunnel, ExposureKind::PublicAddress]
        );
        assert_eq!(d.exposures[0].port, Some(8080));
        assert_eq!(d.exposures[1].public, "203.0.113.7:8443/tcp");
        assert_eq!(d.exposures[1].target, "192.168.1.5:443");
        assert_eq!(d.exposures[2].port, Some(3000));
        assert!(d.notes.is_empty());
    }

    #[test]
    fn parses_bsd_tools() {
        let addrs =
            ["en0: flags=8863<UP,BROADCAST> mtu 1500", "\tinet 192.168.1.8 netmask 0xffffff00 broadcast 192.168.1.255"];
        let a = parse_addrs(&addrs);
        assert_eq!(
            a,
            vec![Iface { name: "en0".into(), addr: "192.168.1.8".into(), prefix: 24, kind: AddrKind::Private }]
        );
        assert_eq!(
            parse_gateway(&["default            192.168.1.1        UGScg                 en0"]).as_deref(),
            Some("192.168.1.1")
        );
        let n = parse_neigh(&[
            "? (192.168.1.1) at aa:bb:cc:0:0:1 on en0 ifscope [ethernet]",
            "? (192.168.1.9) at (incomplete) on en0",
        ]);
        assert_eq!(n, vec![("192.168.1.1".into(), Some("aa:bb:cc:0:0:1".into()))]);
    }

    #[test]
    fn missing_tools_are_notes() {
        let d = parse_discovery("@@NGNET@@ mdns\nmissing\n@@NGNET@@ upnp\nmissing\n@@NGNET@@ tailscale\nmissing\n");
        assert_eq!(d.notes.len(), 2);
    }

    #[test]
    fn script_runs_under_sh() {
        let s = discover_command(Some(("127.0.0.0".parse().unwrap(), 2)));
        let o = std::process::Command::new("sh").arg("-c").arg(&s).output().unwrap();
        assert!(o.status.success() || o.status.code().is_some());
        let out = String::from_utf8_lossy(&o.stdout);
        assert!(out.contains("@@NGNET@@ end"), "{out}");
        let _ = parse_discovery(&out);
    }
}
