//! Network tools (D-142): ping, path, DNS, port check, HTTP/TLS, neighbour
//! table and connections, run from whichever machine the user picks (a host
//! over SSH or this computer) with that machine's own tools. Nothing here
//! talks to an outside service (rule 7): each tool only reaches the target
//! the user typed.
//!
//! Every input is validated and shell-quoted before it goes into a script,
//! and scripts only read: no tool changes anything on the machine it runs on.

use serde::{Deserialize, Serialize};

use crate::files::path::shell_quote;
use crate::netmap::{mac_vendor, parse_neigh};
use crate::ssh::client::Connection;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Tool {
    Ping,
    Path,
    Dns,
    Ports,
    Http,
    Neighbours,
    Connections,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ToolRequest {
    pub tool: Tool,
    /// Host name or address; a URL for `http`; unused for the tables.
    pub target: String,
    /// Ping count, DNS record type or the port list (`22,80,8000-8010`).
    pub option: String,
    /// DNS server to ask (blank: the machine's own resolver).
    pub server: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Tone {
    Info,
    Ok,
    Warn,
    Bad,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Fact {
    pub label: String,
    pub value: String,
    pub tone: Tone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ToolTable {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    /// What ran, in words ("ping -c 4 192.168.1.1").
    pub ran: String,
    /// The tool's own output (capped).
    pub output: String,
    pub facts: Vec<Fact>,
    pub table: Option<ToolTable>,
    /// A missing tool or another reason the result is thin.
    pub note: Option<String>,
}

const MAX_OUTPUT: usize = 64 * 1024;
const MAX_PORTS: usize = 64;
pub const DNS_TYPES: &[&str] = &["A", "AAAA", "CNAME", "MX", "TXT", "NS", "SOA", "SRV", "PTR", "CAA"];

fn bad(field: &str, why: &str) -> String {
    format!("{field}: {why}")
}

/// A host name or an address: letters, digits, `.-_:`, at most 253, not
/// starting with `-` (so it can't be read as an option).
pub fn valid_host(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && !s.starts_with('-')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':'))
}

/// `22, 80 443,8000-8010` → sorted, unique ports (at most 64).
pub fn parse_ports(s: &str) -> Result<Vec<u16>, String> {
    let mut out = Vec::new();
    for part in s.split(|c: char| c == ',' || c.is_whitespace()).filter(|p| !p.is_empty()) {
        let (a, b) = part.split_once('-').unwrap_or((part, part));
        let a: u16 = a.trim().parse().map_err(|_| bad("ports", &format!("{part} isn't a port")))?;
        let b: u16 = b.trim().parse().map_err(|_| bad("ports", &format!("{part} isn't a port")))?;
        if a == 0 || b < a {
            return Err(bad("ports", &format!("{part} isn't a port range")));
        }
        for p in a..=b {
            out.push(p);
            if out.len() > MAX_PORTS * 4 {
                break;
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    if out.is_empty() {
        return Err(bad("ports", "list one or more ports"));
    }
    if out.len() > MAX_PORTS {
        return Err(bad("ports", &format!("at most {MAX_PORTS} ports at a time")));
    }
    Ok(out)
}

/// The host and port of an `http(s)://` URL, validated.
pub fn parse_url(u: &str) -> Result<(bool, String, u16), String> {
    let u = u.trim();
    if u.len() > 2048 || u.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(bad("url", "one http:// or https:// address, no spaces"));
    }
    let (tls, rest) = if let Some(r) = u.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = u.strip_prefix("http://") {
        (false, r)
    } else {
        return Err(bad("url", "starts with http:// or https://"));
    };
    let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
    if auth.contains('@') {
        return Err(bad("url", "no user name or password in the address"));
    }
    let (host, port) = if let Some(r) = auth.strip_prefix('[') {
        let (h, p) = r.split_once(']').ok_or_else(|| bad("url", "unclosed ["))?;
        (h.to_owned(), p.strip_prefix(':').map(str::to_owned))
    } else {
        match auth.rsplit_once(':') {
            Some((h, p)) => (h.to_owned(), Some(p.to_owned())),
            None => (auth.to_owned(), None),
        }
    };
    if !valid_host(&host) {
        return Err(bad("url", "the address has no valid host name"));
    }
    let port = match port {
        Some(p) => p.parse().map_err(|_| bad("url", "the port isn't a number"))?,
        None if tls => 443,
        None => 80,
    };
    Ok((tls, host, port))
}

/// How long a tool may run.
pub fn timeout_secs(t: Tool) -> u64 {
    match t {
        Tool::Path => 90,
        Tool::Ports => 150,
        Tool::Ping => 30,
        _ => 30,
    }
}

const NOTE: &str = "@@NGNOTE@@";
const PORT: &str = "@@NGP@@";
const WEB: &str = "@@NGW@@";
const CERT: &str = "@@NGCERT@@";

/// The `sh` script for a request, and a short description of what runs.
pub fn script(r: &ToolRequest) -> Result<(String, String), String> {
    let target = r.target.trim();
    let need_host = || -> Result<String, String> {
        if valid_host(target) {
            Ok(shell_quote(target))
        } else {
            Err(bad("target", "a host name or address"))
        }
    };
    Ok(match r.tool {
        Tool::Ping => {
            let t = need_host()?;
            let n: u8 = if r.option.trim().is_empty() {
                4
            } else {
                r.option.trim().parse().map_err(|_| bad("count", "1 to 10"))?
            };
            if !(1..=10).contains(&n) {
                return Err(bad("count", "1 to 10"));
            }
            let six = if target.contains(':') {
                "if command -v ping6 >/dev/null 2>&1 && ! ping -6 -c 1 ::1 >/dev/null 2>&1; then P=ping6; fi; "
            } else {
                ""
            };
            (
                format!("P=ping; {six}if command -v $P >/dev/null 2>&1; then $P -c {n} {t} 2>&1; else echo '{NOTE} ping is not installed here'; fi"),
                format!("ping -c {n} {target}"),
            )
        }
        Tool::Path => {
            let t = need_host()?;
            (
                format!(
                    "if command -v tracepath >/dev/null 2>&1; then tracepath -n -m 20 {t} 2>&1; \
                     elif command -v traceroute >/dev/null 2>&1; then traceroute -n -m 20 -w 2 -q 1 {t} 2>&1; \
                     elif command -v mtr >/dev/null 2>&1; then mtr -n -r -c 1 -m 20 {t} 2>&1; \
                     else echo '{NOTE} neither tracepath nor traceroute is installed here'; fi"
                ),
                format!("tracepath / traceroute {target}"),
            )
        }
        Tool::Dns => {
            let t = need_host()?;
            let ty = r.option.trim().to_ascii_uppercase();
            let ty = if ty.is_empty() { "A".to_owned() } else { ty };
            if !DNS_TYPES.contains(&ty.as_str()) {
                return Err(bad("type", &format!("one of {}", DNS_TYPES.join(", "))));
            }
            let server = r.server.trim();
            if !server.is_empty() && !valid_host(server) {
                return Err(bad("server", "a host name or address"));
            }
            let s = if server.is_empty() { String::new() } else { shell_quote(server) };
            let at = if server.is_empty() { String::new() } else { format!("@{s} ") };
            let reverse = ty == "PTR" && target.parse::<std::net::IpAddr>().is_ok();
            let dig = if reverse {
                format!("dig +noall +answer +comments {at}-x {t}")
            } else {
                format!("dig +noall +answer +comments {at}{t} {ty}")
            };
            let host = if reverse { format!("host {t} {s}") } else { format!("host -t {ty} {t} {s}") };
            (
                format!(
                    "if command -v dig >/dev/null 2>&1; then {dig} 2>&1; \
                     elif command -v host >/dev/null 2>&1; then {host} 2>&1; \
                     elif command -v nslookup >/dev/null 2>&1; then nslookup -type={ty} {t} {s} 2>&1; \
                     elif command -v getent >/dev/null 2>&1; then echo '{NOTE} no dig, host or nslookup here: addresses only, from getent'; getent ahosts {t} 2>&1; \
                     else echo '{NOTE} no DNS tool (dig, host, nslookup) here'; fi"
                ),
                format!("{ty} {target}{}", if server.is_empty() { String::new() } else { format!(" @{server}") }),
            )
        }
        Tool::Ports => {
            let t = need_host()?;
            let ports = parse_ports(&r.option)?;
            let list = ports.iter().map(u16::to_string).collect::<Vec<_>>().join(" ");
            (
                format!(
                    "T={t}; \
                     if command -v nc >/dev/null 2>&1; then M=nc; elif command -v python3 >/dev/null 2>&1; then M=py; \
                     else echo '{NOTE} needs nc or python3 on this machine'; exit 0; fi; \
                     for p in {list}; do \
                       if [ $M = nc ]; then nc -z -w 2 \"$T\" $p >/dev/null 2>&1 && r=open || r=closed; \
                       else python3 -c 'import socket,sys; socket.create_connection((sys.argv[1], int(sys.argv[2])), 2)' \"$T\" $p >/dev/null 2>&1 && r=open || r=closed; fi; \
                       echo \"{PORT} $p $r\"; \
                     done"
                ),
                format!("TCP connect to {target} on {}", compact_ports(&ports)),
            )
        }
        Tool::Http => {
            let (tls, host, port) = parse_url(target)?;
            let u = shell_quote(target);
            let hp = if host.contains(':') { format!("[{host}]:{port}") } else { format!("{host}:{port}") };
            let cert = if tls {
                format!(
                    "; if command -v openssl >/dev/null 2>&1; then echo '{CERT}'; \
                     echo | openssl s_client -connect {hp} -servername {h} 2>/dev/null | openssl x509 -noout -subject -issuer -enddate 2>&1; \
                     else echo '{NOTE} openssl is not installed here: no certificate details'; fi",
                    hp = shell_quote(&hp),
                    h = shell_quote(&host),
                )
            } else {
                String::new()
            };
            (
                format!(
                    "if command -v curl >/dev/null 2>&1; then curl -sS -k -o /dev/null -D - -m 10 \
                     -w '{WEB} %{{http_code}} %{{time_total}} %{{ssl_verify_result}} %{{remote_ip}}\\n' {u} 2>&1; \
                     else echo '{NOTE} curl is not installed here'; fi{cert}"
                ),
                format!("HEAD-style GET {target}"),
            )
        }
        Tool::Neighbours => (
            "if command -v ip >/dev/null 2>&1; then ip neigh 2>&1; else arp -an 2>&1; fi".into(),
            "ip neigh / arp -an".into(),
        ),
        Tool::Connections => (
            "if command -v ss >/dev/null 2>&1; then ss -tunH 2>&1 | head -n 400; \
             else netstat -an 2>&1 | grep -Ei 'established|time_wait|close_wait|syn_sent' | head -n 400; fi"
                .into(),
            "ss -tun / netstat -an".into(),
        ),
    })
}

/// What a port usually is (a short, well-known list).
pub fn port_name(p: u16) -> &'static str {
    match p {
        21 => "FTP",
        22 => "SSH",
        23 => "Telnet",
        25 => "SMTP",
        53 => "DNS",
        80 => "HTTP",
        110 => "POP3",
        111 => "RPC (NFS)",
        139 => "NetBIOS",
        143 => "IMAP",
        161 => "SNMP",
        443 => "HTTPS",
        445 => "SMB",
        548 => "AFP",
        631 => "IPP printing",
        873 => "rsync",
        993 => "IMAPS",
        1883 => "MQTT",
        2049 => "NFS",
        2375 => "Docker API (plain)",
        2376 => "Docker API (TLS)",
        3000 => "Web app",
        3306 => "MySQL",
        3389 => "RDP",
        5000 | 5001 => "Web UI (Synology and others)",
        5432 => "PostgreSQL",
        5900..=5909 => "VNC",
        6379 => "Redis",
        8006 => "Proxmox",
        8080 | 8000 | 8888 => "Web (alt)",
        8443 => "HTTPS (alt)",
        9000 => "Portainer / web",
        9090 => "Cockpit / Prometheus",
        9100 => "Printer (raw)",
        27017 => "MongoDB",
        32400 => "Plex",
        51820 => "WireGuard (UDP)",
        _ => "",
    }
}

fn compact_ports(ports: &[u16]) -> String {
    if ports.len() <= 8 {
        ports.iter().map(u16::to_string).collect::<Vec<_>>().join(", ")
    } else {
        format!("{} ports", ports.len())
    }
}

fn fact(label: &str, value: impl Into<String>, tone: Tone) -> Fact {
    Fact { label: label.into(), value: value.into(), tone }
}

/// Turns a tool's output into facts and a table. `now` is unix seconds (for certificate expiry).
pub fn parse(r: &ToolRequest, ran: String, raw: &str, now: i64) -> ToolResult {
    let mut raw = raw.to_owned();
    if raw.len() > MAX_OUTPUT {
        let mut cut = MAX_OUTPUT;
        while !raw.is_char_boundary(cut) {
            cut -= 1;
        }
        raw.truncate(cut);
        raw.push_str("\n…");
    }
    let note = raw.lines().find_map(|l| l.strip_prefix(NOTE)).map(|s| s.trim().to_owned());
    let mut facts = Vec::new();
    let mut table = None;
    match r.tool {
        Tool::Ping => ping_facts(&raw, &mut facts),
        Tool::Path => {
            let hops: Vec<&str> = raw
                .lines()
                .filter(|l| l.trim_start().split(|c: char| !c.is_ascii_digit()).next().is_some_and(|n| !n.is_empty()))
                .collect();
            let n = hops
                .iter()
                .filter_map(|l| l.trim_start().split(|c: char| !c.is_ascii_digit()).next()?.parse::<u32>().ok())
                .max();
            if let Some(n) = n {
                facts.push(fact("Hops", n.to_string(), Tone::Info));
            }
            let silent = hops.iter().filter(|l| l.contains("no reply") || l.trim_end().ends_with('*')).count();
            if silent > 0 {
                facts.push(fact(
                    "No answer",
                    format!("{silent} hop{}", if silent == 1 { "" } else { "s" }),
                    Tone::Info,
                ));
            }
        }
        Tool::Dns => {
            if let Some(s) = raw.split("status: ").nth(1).and_then(|s| s.split(',').next()) {
                facts.push(fact("Status", s.trim(), if s.trim() == "NOERROR" { Tone::Ok } else { Tone::Bad }));
            }
            let answers: Vec<Vec<String>> = raw
                .lines()
                .filter(|l| !l.starts_with(';') && !l.starts_with(NOTE) && !l.trim().is_empty())
                .filter_map(|l| {
                    let w: Vec<&str> = l.split_whitespace().collect();
                    // dig: name ttl class type data…
                    (w.len() >= 5 && w[2] == "IN").then(|| {
                        vec![w[0].trim_end_matches('.').to_owned(), w[3].to_owned(), w[4..].join(" "), w[1].to_owned()]
                    })
                })
                .collect();
            if !answers.is_empty() {
                facts.push(fact("Answers", answers.len().to_string(), Tone::Ok));
                table = Some(ToolTable {
                    columns: vec!["Name".into(), "Type".into(), "Value".into(), "TTL".into()],
                    rows: answers,
                });
            }
        }
        Tool::Ports => {
            let mut rows = Vec::new();
            let (mut open, mut closed) = (Vec::new(), 0);
            for l in raw.lines() {
                let Some(rest) = l.strip_prefix(PORT) else { continue };
                let mut w = rest.split_whitespace();
                let (Some(p), Some(s)) = (w.next(), w.next()) else { continue };
                if s == "open" {
                    open.push(p.to_owned());
                } else {
                    closed += 1;
                }
                rows.push(vec![p.to_owned(), s.to_owned(), port_name(p.parse().unwrap_or(0)).to_owned()]);
            }
            if !rows.is_empty() {
                facts.push(fact(
                    "Open",
                    if open.is_empty() { "none".into() } else { open.join(", ") },
                    if open.is_empty() { Tone::Warn } else { Tone::Ok },
                ));
                facts.push(fact("Closed or filtered", closed.to_string(), Tone::Info));
                table = Some(ToolTable { columns: vec!["Port".into(), "State".into(), "Usually".into()], rows });
            }
            raw = raw.lines().filter(|l| !l.starts_with(PORT)).collect::<Vec<_>>().join("\n");
        }
        Tool::Http => http_facts(&raw, now, &mut facts),
        Tool::Neighbours => {
            let lines: Vec<&str> = raw.lines().collect();
            let rows: Vec<Vec<String>> = parse_neigh(&lines)
                .into_iter()
                .map(|(ip, mac)| {
                    let v = mac.as_deref().and_then(mac_vendor).unwrap_or_default();
                    vec![ip, mac.unwrap_or_default(), v]
                })
                .collect();
            facts.push(fact("Entries", rows.len().to_string(), Tone::Info));
            table = Some(ToolTable { columns: vec!["Address".into(), "MAC".into(), "Maker".into()], rows });
        }
        Tool::Connections => {
            let rows: Vec<Vec<String>> = raw.lines().filter_map(connection_row).collect();
            let mut peers: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
            for r in &rows {
                *peers.entry(strip_port(&r[2])).or_default() += 1;
            }
            let mut top: Vec<(String, usize)> = peers.into_iter().collect();
            top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            facts.push(fact("Connections", rows.len().to_string(), Tone::Info));
            if !top.is_empty() {
                let t: Vec<String> = top.iter().take(5).map(|(p, n)| format!("{p} ({n})")).collect();
                facts.push(fact("Most to", t.join(", "), Tone::Info));
            }
            table = Some(ToolTable {
                columns: vec!["Proto".into(), "Local".into(), "Remote".into(), "State".into()],
                rows,
            });
        }
    }
    let output = raw
        .lines()
        .filter(|l| !l.starts_with(NOTE) && !l.starts_with(WEB) && *l != CERT)
        .collect::<Vec<_>>()
        .join("\n");
    ToolResult { ran, output, facts, table, note }
}

fn ping_facts(raw: &str, facts: &mut Vec<Fact>) {
    let loss = raw.lines().find(|l| l.contains("packet loss")).and_then(|l| {
        let i = l.find("% packet loss")?;
        let start = l[..i].rfind(|c: char| !(c.is_ascii_digit() || c == '.')).map_or(0, |s| s + 1);
        l[start..i].parse::<f64>().ok()
    });
    if let Some(loss) = loss {
        let tone = if loss >= 100.0 {
            Tone::Bad
        } else if loss > 0.0 {
            Tone::Warn
        } else {
            Tone::Ok
        };
        facts.push(fact("Answers", if loss >= 100.0 { "no".into() } else { "yes".to_string() }, tone));
        facts.push(fact("Loss", format!("{loss}%"), tone));
    }
    // Linux "rtt min/avg/max/mdev = a/b/c/d ms", BSD "round-trip min/avg/max/stddev = …".
    if let Some(v) = raw.lines().find(|l| l.contains("min/avg/max")).and_then(|l| l.split('=').nth(1)) {
        let n: Vec<&str> = v.trim().trim_end_matches("ms").trim().split('/').collect();
        if n.len() >= 3 {
            facts.push(fact("Round trip", format!("{} ms avg ({}–{})", n[1], n[0], n[2]), Tone::Info));
        }
    }
}

fn http_facts(raw: &str, now: i64, facts: &mut Vec<Fact>) {
    let header = |name: &str| {
        raw.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case(name).then(|| v.trim().to_owned())
        })
    };
    if let Some(w) = raw.lines().rev().find_map(|l| l.strip_prefix(WEB)) {
        let p: Vec<&str> = w.split_whitespace().collect();
        if let Some(code) = p.first() {
            let tone = match code.chars().next() {
                Some('2' | '3') => Tone::Ok,
                Some('4') => Tone::Warn,
                _ => Tone::Bad,
            };
            facts.push(fact(
                "Status",
                if *code == "000" { "no answer".to_string() } else { (*code).to_string() },
                if *code == "000" { Tone::Bad } else { tone },
            ));
        }
        if let Some(t) = p.get(1).and_then(|t| t.parse::<f64>().ok()) {
            facts.push(fact("Time", format!("{:.0} ms", t * 1000.0), Tone::Info));
        }
        if let Some(ip) = p.get(3).filter(|s| !s.is_empty()) {
            facts.push(fact("Address", *ip, Tone::Info));
        }
        if raw.contains(CERT) {
            match p.get(2) {
                Some(&"0") => facts.push(fact("Certificate", "trusted by this machine", Tone::Ok)),
                Some(c) => {
                    facts.push(fact("Certificate", format!("not trusted by this machine (check {c})"), Tone::Bad))
                }
                None => {}
            }
        }
    }
    if let Some(s) = header("server") {
        facts.push(fact("Server", s, Tone::Info));
    }
    if let Some(l) = header("location") {
        facts.push(fact("Redirects to", l, Tone::Info));
    }
    if let Some(cert) = raw.split(CERT).nth(1) {
        let get = |k: &str| {
            cert.lines().find_map(|l| l.trim().strip_prefix(k)).map(|v| v.trim_start_matches('=').trim().to_owned())
        };
        let subject = get("subject");
        let issuer = get("issuer");
        if let Some(s) = &subject {
            facts.push(fact("Issued to", s.clone(), Tone::Info));
        }
        if let Some(i) = &issuer {
            let selfsigned = subject.as_ref() == Some(i);
            facts.push(fact(
                "Issued by",
                if selfsigned { format!("{i} (self-signed)") } else { i.clone() },
                if selfsigned { Tone::Warn } else { Tone::Info },
            ));
        }
        if let Some(end) = get("notAfter").and_then(|d| crate::docker::proxy::parse_openssl_date(&d)) {
            #[allow(clippy::cast_possible_truncation)]
            let days = ((end as i64) - now).div_euclid(86_400);
            let tone = if days < 0 {
                Tone::Bad
            } else if days < 14 {
                Tone::Warn
            } else {
                Tone::Ok
            };
            let v = if days < 0 { format!("expired {} days ago", -days) } else { format!("in {days} days") };
            facts.push(fact("Expires", v, tone));
        }
    }
}

fn strip_port(a: &str) -> String {
    if let Some(r) = a.strip_prefix('[') {
        return r.split(']').next().unwrap_or(r).to_owned();
    }
    match a.rsplit_once([':', '.']) {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit() || c == '*') && (h.contains('.') || h.contains(':')) => {
            h.to_owned()
        }
        _ => a.to_owned(),
    }
}

/// `ss -tunH`: `tcp ESTAB 0 0 local peer`; netstat: `tcp4 0 0 local peer ESTABLISHED`.
fn connection_row(l: &str) -> Option<Vec<String>> {
    let w: Vec<&str> = l.split_whitespace().collect();
    if w.len() >= 6 && (w[0] == "tcp" || w[0] == "udp") && w[2].parse::<u64>().is_ok() {
        return Some(vec![w[0].into(), w[4].into(), w[5].into(), w[1].into()]);
    }
    if w.len() >= 6 && w[0].starts_with("tcp") && w[1].parse::<u64>().is_ok() {
        return Some(vec![w[0].into(), w[3].into(), w[4].into(), w[5].into()]);
    }
    None
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

/// Runs a tool on a host over SSH.
pub async fn run(conn: &Connection, r: &ToolRequest) -> Result<ToolResult, String> {
    let (sh, ran) = script(r)?;
    let (_, out, err) = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs(r.tool)), conn.exec(&sh))
        .await
        .map_err(|_| "the tool took too long".to_string())?
        .map_err(|e| e.to_string())?;
    let mut text = String::from_utf8_lossy(&out).into_owned();
    if text.trim().is_empty() {
        text = String::from_utf8_lossy(&err).into_owned();
    }
    Ok(parse(r, ran, &text, unix_now()))
}

/// Runs a tool on this computer (macOS, Linux, BSDs).
#[cfg(unix)]
pub async fn run_local(r: &ToolRequest) -> Result<ToolResult, String> {
    let (sh, ran) = script(r)?;
    let run = tokio::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(&sh)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs(r.tool)), run)
        .await
        .map_err(|_| "the tool took too long".to_string())?
        .map_err(|e| e.to_string())?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    if text.trim().is_empty() {
        text = String::from_utf8_lossy(&out.stderr).into_owned();
    }
    Ok(parse(r, ran, &text, unix_now()))
}

#[cfg(not(unix))]
pub async fn run_local(_r: &ToolRequest) -> Result<ToolResult, String> {
    Err("The tools run on macOS and Linux. On Windows, run them from one of your hosts.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(tool: Tool, target: &str, option: &str) -> ToolRequest {
        ToolRequest { tool, target: target.into(), option: option.into(), server: String::new() }
    }

    #[test]
    fn validates_inputs() {
        assert!(valid_host("nas.lan") && valid_host("192.168.1.1") && valid_host("fe80::1"));
        assert!(!valid_host("-oProxyCommand=x") && !valid_host("a b") && !valid_host("a;b") && !valid_host("$(id)"));
        assert_eq!(parse_ports("22, 80 8000-8002,22").unwrap(), [22, 80, 8000, 8001, 8002]);
        assert!(
            parse_ports("1-1000").is_err()
                && parse_ports("0").is_err()
                && parse_ports("x").is_err()
                && parse_ports("").is_err()
        );
        assert_eq!(parse_url("https://nas.lan:5001/x").unwrap(), (true, "nas.lan".into(), 5001));
        assert_eq!(parse_url("http://[fe80::1]/").unwrap(), (false, "fe80::1".into(), 80));
        assert!(
            parse_url("https://u:p@x.example").is_err()
                && parse_url("ftp://x").is_err()
                && parse_url("https://x y").is_err()
        );
        assert!(script(&req(Tool::Ping, "a'b", "")).is_err());
        assert!(script(&req(Tool::Ping, "x", "50")).is_err());
        assert!(script(&req(Tool::Dns, "x.example", "ANY")).is_err());
        let (s, _) = script(&req(Tool::Http, "https://x.example/'$(id)'", "")).unwrap();
        assert!(s.contains(r"'https://x.example/'\''$(id)'\'''"));
        let (s, ran) = script(&req(Tool::Dns, "192.168.1.1", "ptr")).unwrap();
        assert!(s.contains("-x '192.168.1.1'") && ran.starts_with("PTR"));
    }

    #[test]
    fn reads_ping_and_ports() {
        let linux = "4 packets transmitted, 4 received, 0% packet loss, time 3004ms\nrtt min/avg/max/mdev = 0.412/0.530/0.701/0.112 ms\n";
        let r = parse(&req(Tool::Ping, "x", ""), "ping".into(), linux, 0);
        assert_eq!(r.facts[0].value, "yes");
        assert_eq!(r.facts[2].value, "0.530 ms avg (0.412–0.701)");
        let mac = "4 packets transmitted, 0 packets received, 100.0% packet loss\n";
        let r = parse(&req(Tool::Ping, "x", ""), "ping".into(), mac, 0);
        assert_eq!((r.facts[0].value.as_str(), r.facts[0].tone), ("no", Tone::Bad));
        let r = parse(&req(Tool::Ports, "x", "22,80"), "".into(), "@@NGP@@ 22 open\n@@NGP@@ 80 closed\n", 0);
        assert_eq!(r.facts[0].value, "22");
        assert_eq!(r.table.unwrap().rows[0], ["22", "open", "SSH"]);
        assert_eq!(r.output, "");
    }

    #[test]
    fn reads_dns_http_and_tables() {
        let dig = ";; ->>HEADER<<- opcode: QUERY, status: NOERROR, id: 1\nexample.com.\t300\tIN\tA\t93.184.215.14\n";
        let r = parse(&req(Tool::Dns, "example.com", "A"), "".into(), dig, 0);
        assert_eq!(r.facts[0].value, "NOERROR");
        assert_eq!(r.table.unwrap().rows[0], ["example.com", "A", "93.184.215.14", "300"]);

        let http = "HTTP/1.1 302 Found\r\nServer: nginx\r\nLocation: /login\r\n\r\n@@NGW@@ 302 0.051 18 192.168.1.20\n@@NGCERT@@\nsubject=CN = nas.lan\nissuer=CN = nas.lan\nnotAfter=Jan 10 00:00:00 2027 GMT\n";
        let r = parse(&req(Tool::Http, "https://nas.lan", ""), "".into(), http, 1_794_800_000);
        let get = |l: &str| r.facts.iter().find(|f| f.label == l).unwrap().clone();
        assert_eq!(get("Status").value, "302");
        assert_eq!(get("Certificate").tone, Tone::Bad);
        assert_eq!(get("Issued by").tone, Tone::Warn);
        assert_eq!(get("Expires").tone, Tone::Ok);
        assert_eq!(get("Redirects to").value, "/login");
        assert!(!r.output.contains("@@"));

        let r = parse(
            &req(Tool::Neighbours, "", ""),
            "".into(),
            "192.168.1.20 dev eth0 lladdr 00:11:32:aa:bb:cc REACHABLE\n",
            0,
        );
        assert_eq!(r.table.unwrap().rows[0][0], "192.168.1.20");
        let ss = "tcp ESTAB 0 0 192.168.1.40:22 192.168.1.5:53422\ntcp ESTAB 0 0 192.168.1.40:22 192.168.1.5:53423\n";
        let r = parse(&req(Tool::Connections, "", ""), "".into(), ss, 0);
        assert_eq!(r.facts[1].value, "192.168.1.5 (2)");
        let r = parse(&req(Tool::Ping, "x", ""), "".into(), "@@NGNOTE@@ ping is not installed here\n", 0);
        assert_eq!(r.note.as_deref(), Some("ping is not installed here"));
    }
}
