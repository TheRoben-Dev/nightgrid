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

use std::collections::{BTreeMap, HashMap};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::LazyLock;

use serde::Serialize;

use crate::netident::{guess_kind, parse_leases, parse_names, parse_ssdp};
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
    /// Answered a TCP connect during the sweep.
    Probe,
    /// The scanned host mounts a network share from it (NFS, SMB).
    Mount,
    /// The scanned host has an open TCP connection to it.
    Connection,
    /// Answered a UPnP (SSDP) search.
    Ssdp,
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
    /// Maker from the MAC address (bundled IEEE list), or "Random MAC" for a
    /// locally administered address (phones' private Wi-Fi addresses, VMs).
    pub vendor: Option<String>,
    /// Other addresses of the same device (same MAC), e.g. its IPv6 address.
    pub also: Vec<String>,
    /// What it probably is (a `device_labels::KINDS` id), guessed from its
    /// maker, names, announced services and open ports. "other" when unsure.
    pub kind: String,
    /// Model from its UPnP description ("Sonos One", "HP LaserJet …").
    pub model: Option<String>,
    /// Found in a routed subnet (not on the scanning host's own network):
    /// no MAC, vendor or local names can be seen across a router.
    pub routed: bool,
    /// Inventory (filled by the app): first time any scan saw it (unix s).
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub first_seen: Option<i64>,
    /// First seen by this scan.
    pub is_new: bool,
    /// Marked as known and expected.
    pub trusted: bool,
    /// Tailscale peers: `direct` (peer-to-peer) or `relay <region>` (through
    /// a Tailscale relay; also shown for idle peers, since a direct path is
    /// only set up when there's traffic).
    pub link: Option<String>,
}

/// A switch or other device the scanning host's port is plugged into, from LLDP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LldpNeighbour {
    /// The scanning host's interface.
    pub iface: String,
    pub name: Option<String>,
    pub mac: Option<String>,
    pub mgmt_ip: Option<String>,
    /// The neighbour's port ("gi1/0/5", "Port 5").
    pub port: Option<String>,
    pub descr: Option<String>,
}

/// The routers between the scanner and a routed range (first hop first).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RangePath {
    pub range: String,
    pub hops: Vec<String>,
}

/// A device the inventory knows that this scan didn't see, in a subnet this
/// scan covered (filled by the app).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MissingDevice {
    pub key: String,
    pub ip: String,
    pub name: Option<String>,
    pub vendor: Option<String>,
    pub kind: String,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub last_seen: i64,
    pub trusted: bool,
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
    /// Every subnet swept: the host's own and any routed ranges asked for.
    pub ranges: Vec<String>,
    /// Inventory devices in the swept subnets that didn't answer this time.
    pub missing: Vec<MissingDevice>,
    /// What the scanning host is plugged into (LLDP), when it can tell.
    pub lldp: Vec<LldpNeighbour>,
    /// How each swept range is reached (traceroute).
    pub paths: Vec<RangePath>,
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
/// Ports tried, in order, on addresses of a sweep (SSH, web, SMB, NFS, rpcbind).
pub const PROBE_PORTS: &str = "22 445 80 443 2049 111 139";

pub fn discover_command(sweeps: &[(Ipv4Addr, u32)]) -> String {
    // `t N cmd`: `timeout` where it exists (macOS and BSDs don't ship it).
    // macOS nc's -w doesn't bound the connect: -G does (an unanswered SYN
    // otherwise waits ~75 s per port).
    let mut s = String::from(
        "export LC_ALL=C\nt() { if command -v timeout >/dev/null 2>&1; then timeout \"$@\"; else shift; \"$@\"; fi; }\n\
         case $(uname 2>/dev/null) in Darwin|*BSD) pw='-t 1' ;; *) pw='-W 1' ;; esac\n\
         case $(uname 2>/dev/null) in Darwin) ncw='-G 1 -w 1' ;; *) ncw='-w 1' ;; esac\n\
         NGT=$(mktemp 2>/dev/null || echo \"/tmp/ngnet.$$\"); : > \"$NGT\"\n",
    );
    let sec = |s: &mut String, name: &str, body: &str| {
        s.push_str(&format!("echo '{MARK} {name}'\n{body}\n"));
    };
    sec(&mut s, "addr", "ip -o addr show 2>/dev/null || ifconfig 2>/dev/null");
    sec(
        &mut s,
        "route",
        "{ ip -4 route show default 2>/dev/null; ip -4 route show table all 2>/dev/null | grep '^default'; ip -6 route show default 2>/dev/null; } | grep . \
         || netstat -rn 2>/dev/null | awk '$1==\"default\"||$1==\"0.0.0.0\"'",
    );
    for &(net, n) in sweeps {
        let o = net.octets();
        let base = u32::from(net);
        let last = Ipv4Addr::from(base + n);
        // Parallel single pings, 1 s each; answers fill the neighbour table too.
        sec(
            &mut s,
            &format!("sweep {net}/{}", 32 - (n + 1).next_power_of_two().trailing_zeros()),
            &format!(
                "if command -v fping >/dev/null 2>&1; then fping -a -q -r 0 -t 300 -g {}.{}.{}.{} {} 2>/dev/null | tee -a \"$NGT\"; \
                 else i=1; while [ $i -le {n} ]; do ip=\"{}.{}.{}.$(( {} + i ))\"; (ping -c 1 $pw \"$ip\" >/dev/null 2>&1 && echo \"$ip\" && echo \"$ip\" >> \"$NGT\") & \
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
        // Devices that drop ping still answer (or refuse) a TCP connect, and
        // the attempt puts them in the neighbour table either way.
        sec(
            &mut s,
            "probe",
            &format!(
                "pr() {{ if command -v nc >/dev/null 2>&1; then nc -z $ncw \"$1\" \"$2\" 2>/dev/null; \
                 elif command -v bash >/dev/null 2>&1; then t 2 bash -c \"</dev/tcp/$1/$2\" 2>/dev/null; else return 1; fi; }}; \
                 i=1; while [ $i -le {n} ]; do ip=\"{}.{}.{}.$(( {} + i ))\"; \
                 (for p in {PROBE_PORTS}; do if pr \"$ip\" $p; then echo \"$ip $p\"; echo \"$ip\" >> \"$NGT\"; break; fi; done) & \
                 i=$((i+1)); if [ $((i % 64)) -eq 0 ]; then wait; fi; done; wait",
                o[0],
                o[1],
                o[2],
                o[3]
            ),
        );
    }
    // How each swept range is reached: the routers on the way to its first address.
    for &(net, n) in sweeps {
        let first = Ipv4Addr::from(u32::from(net) + 1);
        sec(
            &mut s,
            &format!("path {net}/{}", 32 - (n + 1).next_power_of_two().trailing_zeros()),
            &format!(
                "if command -v traceroute >/dev/null 2>&1; then t 15 traceroute -n -m 6 -q 1 -w 1 {first} 2>/dev/null | awk 'NR>1 {{print $2}}'; \
                 elif command -v tracepath >/dev/null 2>&1; then t 15 tracepath -n -m 6 {first} 2>/dev/null | awk '$1 ~ /^[0-9]+:$/ {{print $2}}' | uniq; fi"
            ),
        );
    }
    sec(
        &mut s,
        "lldp",
        "if command -v lldpctl >/dev/null 2>&1; then t 5 lldpctl -f keyvalue 2>/dev/null | head -n 300; \
         elif command -v networkctl >/dev/null 2>&1; then t 5 networkctl lldp --no-legend 2>/dev/null | head -n 50 | sed 's/^/nctl /'; \
         else echo missing; fi",
    );
    sec(&mut s, "neigh", "ip neigh show 2>/dev/null || arp -an 2>/dev/null");
    s.push_str(IDENT_SCRIPT);
    // Network shares this host mounts, and LAN peers it talks to right now.
    sec(
        &mut s,
        "mounts",
        "awk '$3 ~ /^(nfs|nfs4|cifs|smb3|smbfs)$/ {print $1, $2, $3}' /proc/mounts 2>/dev/null; \
         mount 2>/dev/null | awk '/ (nfs|smbfs|cifs)/ && $2==\"on\" {print $1, $3, \"mnt\"}' | head -n 50",
    );
    sec(
        &mut s,
        "conns",
        "if command -v ss >/dev/null 2>&1; then ss -tnH state established 2>/dev/null | awk '{print $NF}' | head -n 500; \
         else netstat -tn 2>/dev/null | awk '$NF ~ /ESTAB/ {print $5}' | head -n 500; fi",
    );
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
    sec(&mut s, "end", "rm -f \"$NGT\" \"$NGT.ips\"");
    s
}

/// Names for what the scan found: reverse DNS and NetBIOS for every address
/// seen (neighbours and sweep answers), the host's own DHCP leases, and a
/// UPnP search with descriptions read from the devices themselves.
const IDENT_SCRIPT: &str = r#"{ ip neigh show 2>/dev/null | awk '{print $1}'; arp -an 2>/dev/null | sed -n 's/.*(\([0-9.]*\)).*/\1/p'; cat "$NGT" 2>/dev/null; } \
 | grep -E '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$' | sort -u | head -n 600 > "$NGT.ips"
echo '@@NGNET@@ rdns'
i=0; while read -r ip; do
 ( if command -v getent >/dev/null 2>&1; then n=$(t 2 getent hosts "$ip" | awk '{print $2; exit}');
   elif command -v host >/dev/null 2>&1; then n=$(t 2 host -W 1 "$ip" 2>/dev/null | awk '/pointer/ {print $NF; exit}'); fi
   [ -n "$n" ] && echo "$ip $n" ) &
 i=$((i+1)); if [ $((i % 64)) -eq 0 ]; then wait; fi
done < "$NGT.ips"; wait
echo '@@NGNET@@ leases'
for f in /var/lib/misc/dnsmasq.leases /tmp/dnsmasq.leases /etc/pihole/dhcp.leases /var/lib/dnsmasq/dnsmasq.leases /tmp/dhcp.leases; do
 [ -r "$f" ] && sed 's/^/dnsmasq /' "$f" | head -n 1000; done
for f in /var/lib/dhcp/dhcpd.leases /var/lib/dhcpd/dhcpd.leases; do
 [ -r "$f" ] && tr -d '";' < "$f" | awk '/^lease /{ip=$2} /hardware ethernet/{mac=$3} /client-hostname/{h=$2} /^}/{if(ip!=""&&h!="")print "isc", ip, mac, h; ip="";mac="";h=""}' | tail -n 1000; done
echo '@@NGNET@@ netbios'
if command -v nmblookup >/dev/null 2>&1; then
 i=0; while read -r ip; do
  ( t 3 nmblookup -A "$ip" 2>/dev/null | awk -v ip="$ip" '/<00>/ && !/GROUP/ {print ip, $1; exit}' ) &
  i=$((i+1)); if [ $((i % 64)) -eq 0 ]; then wait; fi
 done < "$NGT.ips"; wait
else echo missing; fi
echo '@@NGNET@@ ssdp'
if command -v python3 >/dev/null 2>&1; then t 15 python3 - <<'NGPY' 2>/dev/null
import socket, time, re, urllib.request
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.settimeout(0.4)
s.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_TTL, 2)
m = b'M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: "ssdp:discover"\r\nMX: 2\r\nST: ssdp:all\r\n\r\n'
try:
    s.sendto(m, ("239.255.255.250", 1900))
except Exception:
    raise SystemExit
seen = {}
end = time.time() + 3
while time.time() < end:
    try:
        d, a = s.recvfrom(8192)
    except Exception:
        continue
    h = {}
    for l in d.decode("latin1").split("\r\n")[1:]:
        if ":" in l:
            k, v = l.split(":", 1)
            h[k.strip().lower()] = v.strip()
    e = seen.setdefault(a[0], {"server": "", "loc": ""})
    e["server"] = e["server"] or h.get("server", "")
    loc = h.get("location", "")
    if loc.startswith("http://") and not e["loc"]:
        e["loc"] = loc
for ip, e in list(seen.items())[:60]:
    f = mk = mo = dt = ""
    if e["loc"]:
        try:
            x = urllib.request.urlopen(e["loc"], timeout=1.5).read(65536).decode("utf-8", "replace")
            def g(t):
                r = re.search("<" + t + ">([^<]*)</" + t + ">", x)
                return r.group(1).strip() if r else ""
            f, mk, mo, dt = g("friendlyName"), g("manufacturer"), g("modelName"), g("deviceType")
        except Exception:
            pass
    print("\t".join(v.replace("\t", " ").replace("\n", " ") for v in [ip, e["server"], f, mk, mo, dt]))
NGPY
else echo missing; fi
"#;

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
    // Loopback says nothing about the network (and every computer has it).
    out.retain(|i| i.kind != AddrKind::Loopback);
    out
}

/// `default via 192.168.1.1 dev eth0` or `default 192.168.1.1 UGScg en0`.
/// IPv4 gateways win over IPv6 ones (often link-local `fe80::…`).
pub fn parse_gateway(lines: &[&str]) -> Option<String> {
    let all: Vec<IpAddr> = lines
        .iter()
        .filter_map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            let i = w.iter().position(|x| *x == "via").map(|i| i + 1).unwrap_or(1);
            w.get(i).and_then(|a| a.split('%').next()?.parse::<IpAddr>().ok())
        })
        .collect();
    all.iter().find(|a| a.is_ipv4()).or(all.first()).map(IpAddr::to_string)
}

/// Docker, Podman and Kubernetes bridges: their neighbours are containers,
/// which the Docker view already lists.
pub fn is_container_bridge(name: &str) -> bool {
    name == "docker0"
        || name == "podman0"
        || name == "cni0"
        || name == "flannel.1"
        || name.starts_with("br-")
        || name.starts_with("veth")
        || name.starts_with("cali")
}

pub fn in_v4_subnet(ip: &str, net: &str, prefix: u8) -> bool {
    match (ip.parse::<Ipv4Addr>(), net.parse::<Ipv4Addr>()) {
        (Ok(a), Ok(n)) if prefix <= 32 => {
            let mask = if prefix == 0 { 0 } else { u32::MAX << (32 - u32::from(prefix)) };
            u32::from(a) & mask == u32::from(n) & mask
        }
        _ => false,
    }
}

static OUI: LazyLock<HashMap<u32, &'static str>> = LazyLock::new(|| {
    include_str!("../data/oui.tsv")
        .lines()
        .filter_map(|l| {
            let (k, v) = l.split_once('\t')?;
            Some((u32::from_str_radix(k, 16).ok()?, v))
        })
        .collect()
});

/// The maker of a network card from its MAC address (offline, bundled list).
pub fn mac_vendor(mac: &str) -> Option<String> {
    let hex: String = mac.chars().filter(char::is_ascii_hexdigit).collect();
    if hex.len() != 12 {
        return None;
    }
    let first = u8::from_str_radix(&hex[..2], 16).ok()?;
    if first & 0x02 != 0 {
        return Some("Random MAC".into());
    }
    OUI.get(&u32::from_str_radix(&hex[..6], 16).ok()?).map(|v| (*v).to_owned())
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

/// `192.168.1.5:/export /mnt/media nfs` or `//nas/share /mnt/x cifs` (and
/// macOS `mount` lines) → (server, mount point, "NFS" | "SMB"). Names that
/// aren't addresses are kept as they are.
pub fn parse_mounts(lines: &[&str]) -> Vec<(String, String, &'static str)> {
    let mut out: Vec<(String, String, &'static str)> = Vec::new();
    for l in lines {
        let w: Vec<&str> = l.split_whitespace().collect();
        let (Some(src), Some(path)) = (w.first(), w.get(1)) else { continue };
        let (server, fs) = if let Some(rest) = src.strip_prefix("//") {
            let host = rest.split('/').next().unwrap_or("");
            (host.rsplit('@').next().unwrap_or(host), "SMB")
        } else if let Some((host, _)) = src.split_once(":/") {
            (host.trim_matches(['[', ']']), "NFS")
        } else {
            continue;
        };
        if server.is_empty() || out.iter().any(|(s, p, _)| s == server && p == path) {
            continue;
        }
        out.push((server.to_owned(), (*path).to_owned(), fs));
    }
    out
}

/// Peer addresses of established TCP connections (`ss`: `10.0.0.5:2049`,
/// `[fd00::5]:22`; `netstat`: `10.0.0.5.2049` on BSD).
pub fn parse_conns(lines: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in lines {
        let a = l.trim();
        let a = a.strip_prefix("::ffff:").unwrap_or(a);
        let host = if let Some(r) = a.strip_prefix('[') {
            r.split(']').next().unwrap_or("")
        } else if a.matches(':').count() == 1 {
            a.split(':').next().unwrap_or("")
        } else if a.matches('.').count() == 4 {
            a.rsplit_once('.').map_or("", |(h, _)| h)
        } else {
            a
        };
        let host = host.strip_prefix("::ffff:").unwrap_or(host);
        if host.parse::<IpAddr>().is_ok() && !out.iter().any(|x| x == host) {
            out.push(host.to_owned());
        }
    }
    out
}

/// Link-local IPv6 neighbours (`fe80::…`) and their MACs, to tell which
/// device an IPv6 gateway is.
pub fn parse_link_local(lines: &[&str]) -> HashMap<String, String> {
    lines
        .iter()
        .filter_map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            let ip: IpAddr = w.first()?.split('%').next()?.parse().ok()?;
            let IpAddr::V6(v) = ip else { return None };
            if v.segments()[0] & 0xffc0 != 0xfe80 {
                return None;
            }
            let mac = w.iter().position(|x| *x == "lladdr").and_then(|i| w.get(i + 1))?;
            Some((v.to_string(), mac.to_ascii_lowercase()))
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

/// One peer from `tailscale status --json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsPeer {
    pub ip: String,
    pub name: String,
    pub os: Option<String>,
    pub online: Option<bool>,
    /// `direct` or `relay <region>` (see `Device::link`).
    pub link: Option<String>,
}

/// Peers from `tailscale status --json`.
pub fn parse_tailscale(json: &str) -> Vec<TsPeer> {
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
            let online = p.get("Online").and_then(serde_json::Value::as_bool);
            let cur = p.get("CurAddr").and_then(|x| x.as_str()).unwrap_or("");
            let relay = p.get("Relay").and_then(|x| x.as_str()).unwrap_or("");
            let link = if online == Some(false) {
                None
            } else if !cur.is_empty() {
                Some("direct".to_owned())
            } else if !relay.is_empty() {
                Some(format!("relay {relay}"))
            } else {
                None
            };
            Some(TsPeer {
                ip: ip.to_owned(),
                name: name.to_owned(),
                os: p.get("OS").and_then(|x| x.as_str()).filter(|s| !s.is_empty()).map(str::to_owned),
                online,
                link,
            })
        })
        .collect()
}

/// `lldpctl -f keyvalue` (`lldp.eth0.chassis.name=sw1`, …) or systemd's
/// `networkctl lldp` table (prefixed `nctl`: link, chassis id, system name,
/// caps, port id, port description).
pub fn parse_lldp(lines: &[&str]) -> Vec<LldpNeighbour> {
    let mut by: BTreeMap<String, LldpNeighbour> = BTreeMap::new();
    let blank = |iface: &str| LldpNeighbour {
        iface: iface.to_owned(),
        name: None,
        mac: None,
        mgmt_ip: None,
        port: None,
        descr: None,
    };
    let keep = |v: &str| {
        let v = v.trim();
        (!v.is_empty() && v.len() <= 120 && !v.contains(char::is_control)).then(|| v.to_owned())
    };
    for l in lines {
        if let Some(rest) = l.strip_prefix("nctl ") {
            let w: Vec<&str> = rest.split_whitespace().collect();
            if w.len() >= 5 {
                let mut n = blank(w[0]);
                n.mac = (w[1].len() == 17 && w[1].matches(':').count() == 5).then(|| w[1].to_ascii_lowercase());
                n.name = keep(w[2]).filter(|s| s != "n/a");
                n.port = keep(w[4]).filter(|s| s != "n/a");
                n.descr = keep(&w[5..].join(" ")).filter(|s| s != "n/a");
                by.insert(w[0].to_owned(), n);
            }
            continue;
        }
        let Some((k, v)) = l.split_once('=') else { continue };
        let parts: Vec<&str> = k.split('.').collect();
        if parts.len() < 4 || parts[0] != "lldp" {
            continue;
        }
        let e = by.entry(parts[1].to_owned()).or_insert_with(|| blank(parts[1]));
        match &parts[2..] {
            ["chassis", "name"] => e.name = keep(v),
            ["chassis", "mac"] => e.mac = keep(v).map(|m| m.to_ascii_lowercase()),
            ["chassis", "mgmt-ip"] if e.mgmt_ip.is_none() && v.trim().parse::<Ipv4Addr>().is_ok() => {
                e.mgmt_ip = keep(v)
            }
            ["chassis", "descr"] => e.descr = keep(v).map(|d| d.chars().take(80).collect()),
            ["port", "ifname"] => e.port = keep(v),
            ["port", "descr"] if e.port.is_none() => e.port = keep(v),
            _ => {}
        }
    }
    by.into_values().filter(|n| n.name.is_some() || n.mac.is_some()).collect()
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
        ranges: Vec::new(),
        missing: Vec::new(),
        lldp: Vec::new(),
        paths: Vec::new(),
        notes: Vec::new(),
    };
    let mut ssdp_missing = false;
    let mut devs: BTreeMap<String, Device> = BTreeMap::new();
    let mut link_local: HashMap<String, String> = HashMap::new();
    let mut conns: Vec<String> = Vec::new();
    for (name, body) in sections(out) {
        let head = name.split_whitespace().next().unwrap_or("");
        match head {
            "addr" => d.ifaces = parse_addrs(&body),
            "path" => {
                if let Some(r) = name.split_whitespace().nth(1) {
                    let hops: Vec<String> = body
                        .iter()
                        .map(|l| l.trim())
                        .filter(|l| l.parse::<Ipv4Addr>().is_ok())
                        .map(str::to_owned)
                        .collect();
                    d.paths.push(RangePath { range: r.to_owned(), hops });
                }
            }
            "lldp" => d.lldp = parse_lldp(&body),
            "route" => d.gateway = parse_gateway(&body),
            "sweep" => {
                if let Some(r) = name.split_whitespace().nth(1) {
                    if d.swept.is_none() {
                        d.swept = Some(r.to_owned());
                    }
                    d.ranges.push(r.to_owned());
                }
                for l in &body {
                    let ip = l.trim();
                    if ip.parse::<IpAddr>().is_ok() {
                        add(&mut devs, ip, Seen::Ping);
                    }
                }
            }
            "probe" => {
                for l in &body {
                    let mut w = l.split_whitespace();
                    if let (Some(ip), Some(port)) = (w.next(), w.next()) {
                        if ip.parse::<IpAddr>().is_ok() {
                            let e = add(&mut devs, ip, Seen::Probe);
                            let svc = format!("tcp/{port}");
                            if !e.services.contains(&svc) {
                                e.services.push(svc);
                            }
                        }
                    }
                }
            }
            "mounts" => {
                for (server, path, fs) in parse_mounts(&body) {
                    let e = add(&mut devs, &server, Seen::Mount);
                    let svc = format!("{fs} share → {path}");
                    if !e.services.contains(&svc) {
                        e.services.push(svc);
                    }
                }
            }
            "conns" => conns = parse_conns(&body),
            "neigh" => {
                link_local = parse_link_local(&body);
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
                for p in parse_tailscale(&body.join("\n")) {
                    let e = add(&mut devs, &p.ip, Seen::Tailscale);
                    if !e.names.contains(&p.name) {
                        e.names.insert(0, p.name);
                    }
                    e.os = p.os;
                    e.online = p.online;
                    e.link = p.link;
                }
            }
            "rdns" | "netbios" => {
                if body.first().is_some_and(|l| l.trim() == "missing") {
                    continue;
                }
                for (ip, n) in parse_names(&body) {
                    if let Some(e) = devs.get_mut(&ip) {
                        if !e.names.iter().any(|x| x.eq_ignore_ascii_case(&n)) {
                            e.names.push(n);
                        }
                    }
                }
            }
            "leases" => {
                for (ip, mac, n) in parse_leases(&body) {
                    // Leases name devices the scan saw; a lease alone isn't a sighting.
                    if let Some(e) = devs.get_mut(&ip) {
                        if e.mac.is_none() {
                            e.mac = mac;
                        }
                        if !e.names.iter().any(|x| x.eq_ignore_ascii_case(&n)) {
                            e.names.push(n);
                        }
                    }
                }
            }
            "ssdp" => {
                if body.first().is_some_and(|l| l.trim() == "missing") {
                    ssdp_missing = true;
                    continue;
                }
                for x in parse_ssdp(&body) {
                    let e = add(&mut devs, &x.ip, Seen::Ssdp);
                    if !x.friendly.is_empty() && !e.names.iter().any(|n| n.eq_ignore_ascii_case(&x.friendly)) {
                        e.names.push(x.friendly.clone());
                    }
                    let model = [x.maker.as_str(), x.model.as_str()]
                        .iter()
                        .filter(|s| !s.is_empty())
                        .copied()
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !model.is_empty() {
                        e.model = Some(model);
                    }
                    if !x.device_type.is_empty() {
                        let svc = format!("upnp:{}", x.device_type);
                        if !e.services.contains(&svc) {
                            e.services.push(svc);
                        }
                    }
                    if !x.server.is_empty() && e.os.is_none() {
                        e.os = Some(x.server.clone());
                    }
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
    // Open connections count only for peers on the host's own LAN subnets
    // (not the internet, not container bridges, not the tailnet).
    let lans: Vec<(String, u8)> = d
        .ifaces
        .iter()
        .filter(|i| i.kind == AddrKind::Private && !i.addr.contains(':') && !is_container_bridge(&i.name))
        .map(|i| (i.addr.clone(), i.prefix))
        .collect();
    for ip in conns {
        if lans.iter().any(|(net, p)| in_v4_subnet(&ip, net, *p)) {
            add(&mut devs, &ip, Seen::Connection);
        }
    }
    // An IPv6 link-local gateway: name it by the device with the same MAC.
    if let Some(g) = d.gateway.clone().filter(|g| g.starts_with("fe80")) {
        let mac = link_local.get(&g).cloned();
        let same = |v4: bool| {
            devs.values()
                .filter(|x| x.mac.is_some() && x.mac == mac && x.ip.contains('.') == v4)
                .map(|x| x.ip.clone())
                .next()
        };
        if let Some(ip) = same(true).or_else(|| same(false)) {
            d.gateway = Some(ip);
        } else if let Some(m) = mac {
            add(&mut devs, &g, Seen::Gateway).mac = Some(m);
        }
    }
    if let Some(g) = &d.gateway {
        add(&mut devs, g, Seen::Gateway);
    }
    // Containers on Docker/Podman bridges aren't LAN devices.
    let bridges: Vec<(String, u8)> = d
        .ifaces
        .iter()
        .filter(|i| is_container_bridge(&i.name) && !i.addr.contains(':'))
        .map(|i| (i.addr.clone(), i.prefix))
        .collect();
    let before = devs.len();
    devs.retain(|ip, _| !bridges.iter().any(|(net, p)| in_v4_subnet(ip, net, *p)));
    let hidden = before - devs.len();
    if hidden > 0 {
        d.notes.push(format!(
            "Left out {hidden} container address{} on Docker bridges; containers are listed in the Docker view.",
            if hidden == 1 { "" } else { "es" }
        ));
    }
    // One device per MAC: an IPv6 address joins the IPv4 entry of the same card.
    let v6_with_mac: Vec<String> =
        devs.values().filter(|x| x.ip.contains(':') && x.mac.is_some()).map(|x| x.ip.clone()).collect();
    for ip6 in v6_with_mac {
        let mac = devs[&ip6].mac.clone();
        let target = devs.values().find(|x| !x.ip.contains(':') && x.mac == mac).map(|x| x.ip.clone());
        if let Some(t) = target {
            let gone = devs.remove(&ip6).expect("present");
            let e = devs.get_mut(&t).expect("present");
            e.also.push(gone.ip);
            for n in gone.names {
                if !e.names.contains(&n) {
                    e.names.push(n);
                }
            }
            for sv in gone.seen {
                if !e.seen.contains(&sv) {
                    e.seen.push(sv);
                }
            }
            if d.gateway.as_deref() == Some(ip6.as_str()) {
                d.gateway = Some(t.clone());
                if !e.seen.contains(&Seen::Gateway) {
                    e.seen.push(Seen::Gateway);
                }
            }
        }
    }
    if ssdp_missing {
        d.notes.push("No UPnP names or models: python3 isn't installed on the scanning host.".into());
    }
    // Ranges that aren't on one of the host's own subnets were reached
    // through a router.
    let own_nets: Vec<(String, u8)> = d
        .ifaces
        .iter()
        .filter(|i| !i.addr.contains(':') && i.kind != AddrKind::Loopback)
        .map(|i| (i.addr.clone(), i.prefix))
        .collect();
    for x in devs.values_mut() {
        x.vendor = x.mac.as_deref().and_then(mac_vendor);
        x.routed = x.ip.contains('.')
            && x.mac.is_none()
            && x.seen.iter().all(|s| matches!(s, Seen::Ping | Seen::Probe | Seen::Ssdp))
            && !own_nets.iter().any(|(net, p)| in_v4_subnet(&x.ip, net, *p));
        x.names.retain(|n| !n.is_empty());
        x.kind = guess_kind(x, d.gateway.as_deref()).into();
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
        vendor: None,
        also: Vec::new(),
        kind: "other".into(),
        model: None,
        routed: false,
        first_seen: None,
        is_new: false,
        trusted: false,
        link: None,
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

/// Most routed ranges one scan sweeps.
pub const MAX_RANGES: usize = 4;

/// A routed range the user asked to sweep: an IPv4 CIDR of /24 to /30 (a
/// bigger range takes too long over one SSH session).
pub fn parse_range(r: &str) -> Result<(Ipv4Addr, u32), String> {
    let r = r.trim();
    let (ip, p) = r.split_once('/').ok_or_else(|| format!("{r}: write a range like 10.20.0.0/24"))?;
    let ip: Ipv4Addr = ip.trim().parse().map_err(|_| format!("{r}: not an IPv4 address"))?;
    let p: u8 = p.trim().parse().map_err(|_| format!("{r}: the part after / is the prefix length"))?;
    if !(24..=30).contains(&p) {
        return Err(format!("{r}: ranges from /24 (254 addresses) to /30 can be swept"));
    }
    if addr_kind(&IpAddr::V4(ip)) == AddrKind::Public {
        return Err(format!("{r}: only private ranges are swept (10.x, 172.16–31.x, 192.168.x, 100.64.x)"));
    }
    let mask = u32::MAX << (32 - u32::from(p));
    Ok((Ipv4Addr::from(u32::from(ip) & mask), (!mask).saturating_sub(1)))
}

/// Runs discovery on `conn`, optionally sweeping its LAN subnet and any
/// routed `ranges` it can reach.
pub async fn discover(conn: &Connection, sweep: bool, ranges: &[String]) -> Result<Discovery, String> {
    discover_with(sweep, ranges, |script| async move {
        let (_, out, _) = conn.exec(&script).await.map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(&out).into_owned())
    })
    .await
}

/// Runs discovery on this computer (macOS, Linux and BSDs; `sh` with the same script).
#[cfg(unix)]
pub async fn discover_local(sweep: bool, ranges: &[String]) -> Result<Discovery, String> {
    discover_with(sweep, ranges, |script| async move {
        let run = tokio::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(&script)
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true)
            .output();
        let out = tokio::time::timeout(std::time::Duration::from_secs(150), run)
            .await
            .map_err(|_| "discovery took too long".to_string())?
            .map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    })
    .await
}

#[cfg(not(unix))]
pub async fn discover_local(_sweep: bool, _ranges: &[String]) -> Result<Discovery, String> {
    Err("Scanning from this computer works on macOS and Linux. On Windows, scan from one of your hosts.".into())
}

async fn discover_with<F, Fut>(sweep: bool, ranges: &[String], exec: F) -> Result<Discovery, String>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Result<String, String>>,
{
    if ranges.len() > MAX_RANGES {
        return Err(format!("At most {MAX_RANGES} ranges per scan."));
    }
    let mut sweeps: Vec<(Ipv4Addr, u32)> = Vec::new();
    for r in ranges {
        let x = parse_range(r)?;
        if !sweeps.contains(&x) {
            sweeps.push(x);
        }
    }
    let mut own_missing = false;
    if sweep {
        let out = exec(discover_command(&[])).await?;
        let first = parse_discovery(&out);
        let gw = first.gateway.as_deref().and_then(|g| g.parse::<Ipv4Addr>().ok());
        // The interface that holds the gateway, else the first private /24.
        let own = first
            .ifaces
            .iter()
            .filter(|i| i.kind == AddrKind::Private)
            .filter_map(|i| sweep_range(i).map(|r| (i, r)))
            .find(|(_, (net, n))| gw.is_some_and(|g| (u32::from(g).wrapping_sub(u32::from(*net))) <= *n))
            .or_else(|| first.ifaces.iter().filter_map(|i| sweep_range(i).map(|r| (i, r))).next())
            .map(|(_, r)| r);
        match own {
            Some(r) if !sweeps.contains(&r) => sweeps.insert(0, r),
            Some(_) => {}
            None => own_missing = true,
        }
    }
    let out = exec(discover_command(&sweeps)).await?;
    let mut d = parse_discovery(&out);
    if own_missing {
        d.notes.push("No private /24 (or smaller) subnet to sweep on this host.".into());
    }
    if d.devices.iter().any(|x| x.routed) {
        d.notes.push(
            "Devices in routed ranges show no maker or local names: MAC addresses, mDNS and NetBIOS don't cross a router. Scan from a host on that network for the full picture."
                .into(),
        );
    }
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_peers_left_out_ipv6_merged_gateway_from_link_local() {
        let m = MARK;
        let out = format!(
            "{m} addr\n2: eth0    inet 192.168.1.10/24 brd 192.168.1.255 scope global eth0\n\
             3: docker0    inet 172.17.0.1/16 brd 172.17.255.255 scope global docker0\n\
             4: br-1a2b3c    inet 172.18.0.1/16 scope global br-1a2b3c\n\
             {m} route\ndefault via fe80::1 dev eth0 proto ra metric 1024\n\
             {m} neigh\n172.17.0.2 dev docker0 lladdr a2:a3:7b:00:00:01 REACHABLE\n\
             172.18.0.2 dev br-1a2b3c lladdr 26:54:c0:00:00:02 STALE\n\
             192.168.1.1 dev eth0 lladdr b8:27:eb:00:00:01 REACHABLE\n\
             192.168.1.40 dev eth0 lladdr 3c:22:fb:00:00:02 STALE\n\
             fd00:1:2::1 dev eth0 lladdr b8:27:eb:00:00:01 router STALE\n\
             fe80::1 dev eth0 lladdr b8:27:eb:00:00:01 router REACHABLE\n\
             {m} end\n"
        );
        let d = parse_discovery(&out);
        let ips: Vec<&str> = d.devices.iter().map(|x| x.ip.as_str()).collect();
        assert_eq!(ips, ["192.168.1.1", "192.168.1.40"]);
        assert_eq!(d.gateway.as_deref(), Some("192.168.1.1"));
        let gw = &d.devices[0];
        assert!(gw.seen.contains(&Seen::Gateway));
        assert_eq!(gw.also, ["fd00:1:2::1"]);
        assert_eq!(gw.vendor.as_deref(), Some("Raspberry Pi Foundation"));
        assert_eq!(d.devices[1].vendor.as_deref(), Some("Apple"));
        assert!(d.notes.iter().any(|n| n.contains("2 container addresses")));
    }

    #[test]
    fn mounts_conns_and_probes_find_quiet_devices() {
        let m = MARK;
        let out = format!(
            "{m} addr\n2: eth0    inet 192.168.1.10/24 brd 192.168.1.255 scope global eth0\n\
             3: tailscale0    inet 100.64.0.5/32 scope global tailscale0\n\
             {m} route\ndefault via 192.168.1.1 dev eth0\n\
             {m} probe\n192.168.1.77 445\n\
             {m} mounts\n192.168.1.50:/export/media /mnt/media nfs4\n//192.168.1.51/share /mnt/smb cifs\n\
             {m} conns\n192.168.1.60:22\n140.82.112.3:443\n[fd7a:115c:a1e0::1]:22\n100.64.0.9:443\n\
             {m} neigh\n{m} end\n"
        );
        let d = parse_discovery(&out);
        let ips: Vec<&str> = d.devices.iter().map(|x| x.ip.as_str()).collect();
        assert_eq!(ips, ["192.168.1.1", "192.168.1.50", "192.168.1.51", "192.168.1.60", "192.168.1.77"]);
        let nfs = &d.devices[1];
        assert_eq!(nfs.seen, [Seen::Mount]);
        assert_eq!(nfs.services, ["NFS share → /mnt/media"]);
        assert_eq!(d.devices[2].services, ["SMB share → /mnt/smb"]);
        assert_eq!(d.devices[3].seen, [Seen::Connection]);
        assert_eq!(d.devices[4].services, ["tcp/445"]);
        assert_eq!(parse_conns(&["10.0.0.5.2049", "::ffff:10.0.0.6:22"]), ["10.0.0.5", "10.0.0.6"]);
        assert!(discover_command(&[("192.168.1.0".parse().unwrap(), 254)]).contains("probe"));
    }

    #[test]
    fn vendors() {
        assert_eq!(mac_vendor("b8:27:eb:12:34:56").as_deref(), Some("Raspberry Pi Foundation"));
        assert_eq!(mac_vendor("02:42:ac:11:00:02").as_deref(), Some("Random MAC"));
        assert_eq!(mac_vendor("nope"), None);
        assert_eq!(
            parse_gateway(&["default via fe80::1 dev eth0", "default via 10.0.0.1 dev eth0"]).as_deref(),
            Some("10.0.0.1")
        );
    }

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
        let d = discover_local(false, &[]).await.expect("local discovery");
        assert!(d.swept.is_none());
        assert!(d.ifaces.iter().all(|i| i.kind != AddrKind::Loopback));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn sweeps_a_routed_range_locally() {
        let d = discover_local(false, &["127.0.0.0/30".into()]).await.expect("local discovery");
        assert_eq!(d.ranges, ["127.0.0.0/30"]);
        assert!(discover_local(false, &["8.8.8.0/24".into()]).await.is_err());
    }

    #[test]
    fn lldp_paths_and_tailscale_links() {
        let n = parse_lldp(&[
            "lldp.eth0.via=LLDP",
            "lldp.eth0.chassis.mac=AA:BB:CC:00:00:02",
            "lldp.eth0.chassis.name=office-switch",
            "lldp.eth0.chassis.mgmt-ip=192.168.1.2",
            "lldp.eth0.chassis.mgmt-ip=fe80::1",
            "lldp.eth0.port.ifname=gi1/0/5",
            "lldp.eth0.port.descr=uplink to lab",
        ]);
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].name.as_deref(), Some("office-switch"));
        assert_eq!(n[0].mac.as_deref(), Some("aa:bb:cc:00:00:02"));
        assert_eq!(n[0].mgmt_ip.as_deref(), Some("192.168.1.2"));
        assert_eq!(n[0].port.as_deref(), Some("gi1/0/5"));
        let n = parse_lldp(&["nctl enp3s0 aa:bb:cc:00:00:03 core-sw ..b........ 5 Port 5"]);
        assert_eq!(
            (n[0].name.as_deref(), n[0].port.as_deref(), n[0].descr.as_deref()),
            (Some("core-sw"), Some("5"), Some("Port 5"))
        );

        let d = parse_discovery(
            "@@NGNET@@ path 10.20.0.0/24\n192.168.1.1\n10.0.0.2\n*\n10.20.0.1\n@@NGNET@@ lldp\nmissing\n@@NGNET@@ tailscale\n\
             {\"Peer\":{\"a\":{\"TailscaleIPs\":[\"100.64.0.2\"],\"HostName\":\"laptop\",\"Online\":true,\"CurAddr\":\"203.0.113.9:41641\",\"Relay\":\"fra\"},\
             \"b\":{\"TailscaleIPs\":[\"100.64.0.3\"],\"HostName\":\"phone\",\"Online\":true,\"CurAddr\":\"\",\"Relay\":\"ams\"}}}\n@@NGNET@@ end\n",
        );
        assert_eq!(
            d.paths,
            [RangePath {
                range: "10.20.0.0/24".into(),
                hops: vec!["192.168.1.1".into(), "10.0.0.2".into(), "10.20.0.1".into()]
            }]
        );
        assert!(d.lldp.is_empty());
        let link = |ip: &str| d.devices.iter().find(|x| x.ip == ip).and_then(|x| x.link.clone());
        assert_eq!(link("100.64.0.2").as_deref(), Some("direct"));
        assert_eq!(link("100.64.0.3").as_deref(), Some("relay ams"));
        assert!(discover_command(&[(Ipv4Addr::new(10, 20, 0, 0), 254)]).contains("path 10.20.0.0/24"));
    }

    #[test]
    fn ranges_and_identity_sections() {
        assert_eq!(parse_range("10.20.0.7/24"), Ok(("10.20.0.0".parse().unwrap(), 254)));
        assert!(parse_range("10.20.0.0/16").unwrap_err().contains("/24"));
        assert!(parse_range("203.0.113.0/24").unwrap_err().contains("private"));
        let m = MARK;
        let out = format!(
            "{m} addr\n2: eth0    inet 192.168.1.10/24 brd 192.168.1.255 scope global eth0\n\
             {m} route\ndefault via 192.168.1.1 dev eth0\n\
             {m} sweep 10.20.0.0/24\n10.20.0.5\n\
             {m} neigh\n192.168.1.1 dev eth0 lladdr b8:27:eb:00:00:01 REACHABLE\n192.168.1.30 dev eth0 lladdr aa:bb:cc:00:00:30 REACHABLE\n\
             {m} rdns\n192.168.1.30 lamp-kitchen.lan.\n10.20.0.5 printer.site2.example\n\
             {m} leases\ndnsmasq 1 aa:bb:cc:00:00:30 192.168.1.30 kitchen-lamp *\n\
             {m} netbios\nmissing\n\
             {m} ssdp\n192.168.1.1\tLinux UPnP/1.0\tHome Router\tExample\tR7000\turn:schemas-upnp-org:device:InternetGatewayDevice:1\n\
             {m} end\n"
        );
        let d = parse_discovery(&out);
        let lamp = d.devices.iter().find(|x| x.ip == "192.168.1.30").unwrap();
        assert_eq!(lamp.names, ["lamp-kitchen.lan", "kitchen-lamp"]);
        assert_eq!(lamp.kind, "iot");
        let gw = d.devices.iter().find(|x| x.ip == "192.168.1.1").unwrap();
        assert_eq!(gw.kind, "router");
        assert_eq!(gw.model.as_deref(), Some("Example R7000"));
        assert!(gw.names.contains(&"Home Router".to_string()));
        let far = d.devices.iter().find(|x| x.ip == "10.20.0.5").unwrap();
        assert!(far.routed);
        assert_eq!(far.kind, "printer");
        assert!(!lamp.routed);
        assert_eq!(d.ranges, ["10.20.0.0/24"]);
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
        assert_eq!(d.ifaces.len(), 3); // lo is left out
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
        let addrs = [
            "lo0: flags=8049<UP,LOOPBACK,RUNNING> mtu 16384",
            "\tinet 127.0.0.1 netmask 0xff000000",
            "en0: flags=8863<UP,BROADCAST> mtu 1500",
            "\tinet 192.168.1.8 netmask 0xffffff00 broadcast 192.168.1.255",
        ];
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
        let s = discover_command(&[("127.0.0.0".parse().unwrap(), 2)]);
        let o = std::process::Command::new("sh").arg("-c").arg(&s).output().unwrap();
        assert!(o.status.success() || o.status.code().is_some());
        let out = String::from_utf8_lossy(&o.stdout);
        assert!(out.contains("@@NGNET@@ end"), "{out}");
        let _ = parse_discovery(&out);
    }
}
