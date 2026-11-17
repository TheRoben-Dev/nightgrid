//! Reverse proxy routes (M10g): reads what Caddy, Traefik and Nginx Proxy
//! Manager on a host route where, maps each domain to the container behind
//! it and checks certificate expiry. Read-only.
//!
//! - **Traefik**: router rules from container labels (`traefik.http.routers.*`).
//! - **Caddy**: the Caddyfile (in a `caddy` container or `/etc/caddy` on the
//!   host), plus caddy-docker-proxy labels.
//! - **Nginx Proxy Manager**: its generated `proxy_host/*.conf` files.
//!
//! Files inside containers are read through the Docker API's exec (`cat`),
//! so no docker CLI or file permissions on the host are needed. Certificates
//! are read by `openssl s_client` on the host against its own port 443.

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use bollard::exec::StartExecResults;
use bollard::models::ExecConfig;
use bollard::query_parameters::ListContainersOptions;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use super::{ContainerView, DockerError, DockerHost, DockerResult};
use crate::files::path::shell_quote;

/// Domains whose certificate is checked per scan.
pub const MAX_CERT_CHECKS: usize = 40;
const EXEC_LIMIT: usize = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ProxyKind {
    Caddy,
    Traefik,
    Npm,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CertInfo {
    /// Unix seconds.
    pub not_after: Option<f64>,
    pub issuer: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ProxyRoute {
    pub proxy: ProxyKind,
    /// The proxy's container name, or "host" for a Caddy installed on the host.
    pub proxy_name: String,
    pub domains: Vec<String>,
    /// Where it forwards, as written in the config ("app:8080", "http://10.0.0.5:3000").
    pub upstream: String,
    /// The container that upstream resolves to on this host, if any.
    pub container: Option<String>,
    pub tls: bool,
    /// Certificate of the first domain (TLS routes only).
    pub cert: Option<CertInfo>,
    /// Router, file or label it came from.
    pub origin: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ProxyFound {
    pub kind: ProxyKind,
    pub name: String,
    pub routes: u32,
    /// Why its routes couldn't be read.
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ProxyReport {
    pub proxies: Vec<ProxyFound>,
    pub routes: Vec<ProxyRoute>,
    /// The host's clock (unix seconds) when certificates were read.
    pub now: f64,
}

/// A route before container matching and certificates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawRoute {
    pub domains: Vec<String>,
    pub upstream: String,
    pub tls: bool,
    pub origin: String,
}

// ---- Caddy ----------------------------------------------------------------

/// Site blocks of a Caddyfile with their `reverse_proxy` upstreams.
pub fn parse_caddyfile(text: &str) -> Vec<RawRoute> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut site: Option<(Vec<String>, Vec<String>)> = None;
    let mut in_proxy_block: Option<usize> = None;
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if line == "}" {
            depth = depth.saturating_sub(1);
            if in_proxy_block == Some(depth) {
                in_proxy_block = None;
            }
            if depth == 0 {
                if let Some((domains, ups)) = site.take() {
                    push_caddy_site(&mut out, domains, ups);
                }
            }
            continue;
        }
        let opens = line == "{" || line.ends_with(" {");
        let head = line.trim_end_matches('{').trim();
        if depth == 0 {
            // Global options `{`, snippets `(name) {` and named routes are not sites.
            if opens && !head.is_empty() && !head.starts_with('(') && !head.starts_with('&') {
                site = Some((caddy_addresses(head), Vec::new()));
            } else if !opens && !head.is_empty() && !head.starts_with('(') && !head.starts_with("import") {
                // A one-site Caddyfile may skip the braces: the first line is the address.
                if site.is_none() && out.is_empty() && looks_like_address(head) {
                    site = Some((caddy_addresses(head), Vec::new()));
                    depth = 1;
                    // No closing brace will come: flush at the end.
                }
            }
        } else if let Some((_, ups)) = site.as_mut() {
            let mut words = head.split_whitespace();
            match words.next() {
                Some("reverse_proxy") => {
                    ups.extend(words.filter(|w| !is_matcher(w)).map(str::to_owned));
                    if opens {
                        in_proxy_block = Some(depth);
                    }
                }
                Some("to") if in_proxy_block.is_some() => ups.extend(words.map(str::to_owned)),
                _ => {}
            }
        }
        if opens {
            depth += 1;
        }
    }
    if let Some((domains, ups)) = site.take() {
        push_caddy_site(&mut out, domains, ups);
    }
    out
}

fn push_caddy_site(out: &mut Vec<RawRoute>, addresses: Vec<String>, ups: Vec<String>) {
    if ups.is_empty() || addresses.is_empty() {
        return;
    }
    let tls = addresses.iter().any(|a| caddy_tls(a));
    let domains: Vec<String> = addresses.iter().map(|a| bare_host(a)).collect();
    let mut seen = Vec::new();
    for u in ups {
        if seen.contains(&u) {
            continue;
        }
        seen.push(u.clone());
        out.push(RawRoute { domains: domains.clone(), upstream: u, tls, origin: "Caddyfile".into() });
    }
}

pub(crate) fn strip_comment(line: &str) -> &str {
    if line.trim_start().starts_with('#') {
        return "";
    }
    match line.find(" #") {
        Some(i) => &line[..i],
        None => line,
    }
}

pub(crate) fn is_matcher(w: &str) -> bool {
    w.starts_with('/') || w.starts_with('@') || w == "*"
}

fn looks_like_address(s: &str) -> bool {
    let first = s.split([',', ' ']).next().unwrap_or("");
    first.contains('.') || first.starts_with(':') || first.starts_with("http") || first == "localhost"
}

fn caddy_addresses(head: &str) -> Vec<String> {
    head.split([',', ' ', '\t']).map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect()
}

/// Caddy serves HTTPS automatically for real host names unless told `http://`.
fn caddy_tls(addr: &str) -> bool {
    if addr.starts_with("http://") || addr.ends_with(":80") {
        return false;
    }
    let host = bare_host(addr);
    addr.starts_with("https://") || (host.contains('.') && !is_ip(&host) && !host.is_empty())
}

/// `https://a.example.com:443/path` → `a.example.com`.
pub fn bare_host(addr: &str) -> String {
    let s = addr.split("://").nth(1).unwrap_or(addr);
    let s = s.split('/').next().unwrap_or(s);
    if s.starts_with('[') {
        return s.split(']').next().unwrap_or(s).trim_start_matches('[').to_owned();
    }
    match s.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => {
            if h.is_empty() {
                s.to_owned()
            } else {
                h.to_owned()
            }
        }
        _ => s.to_owned(),
    }
}

fn is_ip(s: &str) -> bool {
    s.parse::<std::net::IpAddr>().is_ok()
}

// ---- Traefik and caddy-docker-proxy labels ---------------------------------

/// Host names in a Traefik rule: ``Host(`a`) || Host(`b`, `c`)``.
pub fn traefik_hosts(rule: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = rule;
    while let Some(i) = rest.find("Host(") {
        // HostRegexp( / HostSNI( also contain "Host": only take plain Host(.
        let before = &rest[..i];
        let plain = !before.ends_with(|c: char| c.is_ascii_alphanumeric());
        rest = &rest[i + 5..];
        let Some(end) = rest.find(')') else { break };
        if plain {
            out.extend(
                rest[..end]
                    .split(',')
                    .map(|h| h.trim().trim_matches(['`', '"', '\'']).to_owned())
                    .filter(|h| !h.is_empty()),
            );
        }
        rest = &rest[end..];
    }
    out
}

/// Routes declared in one container's labels.
pub fn label_routes(container: &str, labels: &BTreeMap<String, String>) -> Vec<(ProxyKind, RawRoute)> {
    let mut out = Vec::new();
    // Traefik
    if labels.get("traefik.enable").map(String::as_str) != Some("false") {
        let services: Vec<(&str, &str)> = labels
            .iter()
            .filter_map(|(k, v)| {
                let svc = k.strip_prefix("traefik.http.services.")?.strip_suffix(".loadbalancer.server.port")?;
                Some((svc, v.as_str()))
            })
            .collect();
        for (k, rule) in labels {
            let Some(router) = k.strip_prefix("traefik.http.routers.").and_then(|r| r.strip_suffix(".rule")) else {
                continue;
            };
            let domains = traefik_hosts(rule);
            if domains.is_empty() {
                continue;
            }
            let get = |suffix: &str| labels.get(&format!("traefik.http.routers.{router}.{suffix}"));
            let port = get("service")
                .and_then(|s| services.iter().find(|(n, _)| n == s))
                .or(services.first())
                .map(|(_, p)| *p);
            let tls = get("tls").is_some_and(|v| v == "true")
                || get("tls.certresolver").is_some()
                || get("entrypoints").is_some_and(|e| e.contains("websecure") || e.contains("https"));
            out.push((
                ProxyKind::Traefik,
                RawRoute {
                    domains,
                    upstream: match port {
                        Some(p) => format!("{container}:{p}"),
                        None => container.to_owned(),
                    },
                    tls,
                    origin: format!("router {router}"),
                },
            ));
        }
    }
    // caddy-docker-proxy: caddy=a.example.com, caddy.reverse_proxy={{upstreams 80}}
    for (k, v) in labels {
        let Some(prefix) = k.strip_suffix(".reverse_proxy").or((k == "caddy.reverse_proxy").then_some("caddy")) else {
            continue;
        };
        let Some(domains) = labels.get(prefix) else { continue };
        if !prefix.starts_with("caddy") {
            continue;
        }
        let port = v.split("upstreams").nth(1).map(|r| r.trim_matches(|c: char| !c.is_ascii_digit()).to_owned());
        let upstream = match port {
            Some(p) if !p.is_empty() => format!("{container}:{p}"),
            _ if v.contains("upstreams") => container.to_owned(),
            _ => v.clone(),
        };
        let addrs = caddy_addresses(domains);
        out.push((
            ProxyKind::Caddy,
            RawRoute {
                tls: addrs.iter().any(|a| caddy_tls(a)),
                domains: addrs.iter().map(|a| bare_host(a)).collect(),
                upstream,
                origin: format!("label {k}"),
            },
        ));
    }
    out
}

// ---- Nginx Proxy Manager ---------------------------------------------------

/// NPM's `proxy_host/*.conf` files, separated by `@@F <path>` lines.
pub fn parse_npm(text: &str) -> Vec<RawRoute> {
    let mut out = Vec::new();
    for chunk in text.split("@@F ").filter(|c| !c.trim().is_empty()) {
        let (file, body) = chunk.split_once('\n').unwrap_or((chunk, ""));
        let mut domains = Vec::new();
        let mut scheme = "http".to_owned();
        let mut server = None;
        let mut port = None;
        let mut tls = false;
        for line in body.lines().map(str::trim) {
            let line = line.trim_end_matches(';');
            let w: Vec<&str> = line.split_whitespace().collect();
            match w.as_slice() {
                ["server_name", names @ ..] if domains.is_empty() => {
                    domains = names.iter().map(|s| (*s).to_owned()).collect();
                }
                ["set", "$forward_scheme", v] => scheme = v.trim_matches('"').to_owned(),
                ["set", "$server", v] => server = Some(v.trim_matches('"').to_owned()),
                ["set", "$port", v] => port = Some(v.trim_matches('"').to_owned()),
                ["listen", rest @ ..] if rest.contains(&"ssl") => tls = true,
                _ => {}
            }
        }
        let Some(server) = server else { continue };
        if domains.is_empty() {
            continue;
        }
        let upstream = match port {
            Some(p) => format!("{scheme}://{server}:{p}"),
            None => format!("{scheme}://{server}"),
        };
        let name = file.trim().rsplit('/').next().unwrap_or(file).to_owned();
        out.push(RawRoute { domains, upstream, tls, origin: name });
    }
    out
}

// ---- matching and certificates --------------------------------------------

/// The container an upstream points at: by container or compose service name,
/// or by a port published on the host for `localhost` / host addresses.
pub fn match_container(upstream: &str, containers: &[ContainerView]) -> Option<String> {
    let rest = upstream.split("://").nth(1).unwrap_or(upstream);
    let rest = rest.split('/').next().unwrap_or(rest);
    let (host, port) = match rest.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => (h, p.parse::<u16>().ok()),
        _ => (rest, None),
    };
    let host = host.trim_matches(['[', ']']);
    if let Some(c) = containers
        .iter()
        .find(|c| c.name == host || c.service.as_deref() == Some(host) || (host.len() >= 12 && c.id.starts_with(host)))
    {
        return Some(c.name.clone());
    }
    let local =
        matches!(host, "localhost" | "127.0.0.1" | "::1" | "host.docker.internal" | "172.17.0.1") || is_ip(host);
    if local {
        let port = port?;
        return containers
            .iter()
            .find(|c| c.state == "running" && c.ports.iter().any(|p| p.public == Some(port)))
            .map(|c| c.name.clone());
    }
    None
}

/// Host names that can safely go into the cert script.
fn cert_domain(d: &str) -> bool {
    !d.is_empty()
        && d.len() <= 253
        && d.contains('.')
        && !is_ip(d)
        && d.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
}

pub fn cert_command(domains: &[String]) -> String {
    let list: Vec<String> =
        domains.iter().filter(|d| cert_domain(d)).take(MAX_CERT_CHECKS).map(|d| shell_quote(d)).collect();
    format!(
        "exec 2>/dev/null; echo \"now $(date +%s)\"; command -v openssl >/dev/null || {{ echo noopenssl; exit 0; }}\n\
         for d in {}; do e=$(echo | timeout 5 openssl s_client -connect 127.0.0.1:443 -servername \"$d\" | \
         openssl x509 -noout -enddate -issuer | tr '\\n' '|'); printf 'cert\\t%s\\t%s\\n' \"$d\" \"$e\"; done\n",
        if list.is_empty() { "''".to_owned() } else { list.join(" ") }
    )
}

pub fn parse_certs(out: &str) -> (Option<f64>, HashMap<String, CertInfo>) {
    let mut now = None;
    let mut map = HashMap::new();
    let mut missing = false;
    for line in out.lines() {
        if let Some(n) = line.strip_prefix("now ") {
            now = n.trim().parse().ok();
        } else if line.trim() == "noopenssl" {
            missing = true;
        } else if let Some(rest) = line.strip_prefix("cert\t") {
            let (domain, info) = rest.split_once('\t').unwrap_or((rest, ""));
            let mut c = CertInfo { not_after: None, issuer: None, error: None };
            for part in info.split('|') {
                if let Some(d) = part.strip_prefix("notAfter=") {
                    c.not_after = parse_openssl_date(d);
                } else if let Some(i) = part.strip_prefix("issuer=") {
                    c.issuer = Some(issuer_name(i));
                }
            }
            if c.not_after.is_none() {
                c.error = Some("No certificate answered on port 443 of this host for this name.".into());
            }
            if !domain.is_empty() {
                map.insert(domain.to_owned(), c);
            }
        }
    }
    if missing {
        map.insert(
            String::new(),
            CertInfo { not_after: None, issuer: None, error: Some("openssl isn't installed on this host.".into()) },
        );
    }
    (now, map)
}

/// `C = US, O = Let's Encrypt, CN = R11` → `Let's Encrypt R11`.
fn issuer_name(s: &str) -> String {
    let get = |k: &str| {
        s.split([',', '/'])
            .map(str::trim)
            .find_map(|p| p.strip_prefix(k).map(|v| v.trim_start_matches([' ', '=']).trim().to_owned()))
    };
    match (get("O"), get("CN")) {
        (Some(o), Some(cn)) if o != cn => format!("{o} {cn}"),
        (Some(o), _) => o,
        (None, Some(cn)) => cn,
        _ => s.trim().to_owned(),
    }
}

/// `Feb  1 12:00:00 2027 GMT` → unix seconds.
pub fn parse_openssl_date(s: &str) -> Option<f64> {
    let w: Vec<&str> = s.split_whitespace().collect();
    let [mon, day, time, year, ..] = w.as_slice() else { return None };
    let m = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
        .iter()
        .position(|x| x == mon)? as i64
        + 1;
    let d: i64 = day.parse().ok()?;
    let y: i64 = year.parse().ok()?;
    let hms: Vec<i64> = time.split(':').filter_map(|p| p.parse().ok()).collect();
    let [hh, mm, ss] = hms.as_slice() else { return None };
    // Days from civil (Howard Hinnant).
    let (y2, m2) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * m2 + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some((days * 86_400 + hh * 3600 + mm * 60 + ss) as f64)
}

// ---- DockerHost --------------------------------------------------------------

impl DockerHost {
    /// Runs `cmd` in a running container and returns its output (capped).
    pub(crate) async fn exec_read(&self, id: &str, cmd: &[&str]) -> DockerResult<String> {
        let cfg = ExecConfig {
            cmd: Some(cmd.iter().map(|s| (*s).to_owned()).collect()),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };
        let e = self.docker.create_exec(id, cfg).await.map_err(|e| DockerError::from_bollard(e, Some(id)))?;
        let started = self.docker.start_exec(&e.id, None).await.map_err(|e| DockerError::from_bollard(e, Some(id)))?;
        let StartExecResults::Attached { mut output, .. } = started else { return Ok(String::new()) };
        let mut buf = Vec::new();
        let read = async {
            while let Some(Ok(chunk)) = output.next().await {
                buf.extend_from_slice(&chunk.into_bytes());
                if buf.len() > EXEC_LIMIT {
                    break;
                }
            }
        };
        let _ = tokio::time::timeout(Duration::from_secs(10), read).await;
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    /// Every route the host's proxies declare, matched to containers, with
    /// certificate expiry for TLS routes.
    pub async fn proxy_routes(&self) -> DockerResult<ProxyReport> {
        let opts = ListContainersOptions { all: false, ..Default::default() };
        let raw = self.docker.list_containers(Some(opts)).await.map_err(|e| DockerError::from_bollard(e, None))?;
        let containers = self.containers().await?;
        let mut proxies: Vec<ProxyFound> = Vec::new();
        let mut routes: Vec<ProxyRoute> = Vec::new();
        let mut add =
            |proxies: &mut Vec<ProxyFound>, kind: ProxyKind, name: &str, raws: Vec<RawRoute>, err: Option<String>| {
                let n = raws.len() as u32;
                match proxies.iter_mut().find(|p| p.kind == kind && p.name == name) {
                    Some(p) => {
                        p.routes += n;
                        p.error = p.error.take().or(err);
                    }
                    None => proxies.push(ProxyFound { kind, name: name.to_owned(), routes: n, error: err }),
                }
                for r in raws {
                    routes.push(ProxyRoute {
                        proxy: kind,
                        proxy_name: name.to_owned(),
                        container: match_container(&r.upstream, &containers),
                        domains: r.domains,
                        upstream: r.upstream,
                        tls: r.tls,
                        cert: None,
                        origin: r.origin,
                    });
                }
            };
        let name_of = |c: &bollard::models::ContainerSummary| {
            c.names.as_ref().and_then(|n| n.first()).map(|n| n.trim_start_matches('/').to_owned()).unwrap_or_default()
        };
        // Which proxy serves label routes: the first container of that kind.
        let image_has = |c: &bollard::models::ContainerSummary, s: &str| c.image.as_deref().unwrap_or("").contains(s);
        let traefik = raw.iter().find(|c| image_has(c, "traefik")).map(name_of);
        let caddy_labels = raw.iter().find(|c| image_has(c, "caddy-docker-proxy")).map(name_of);
        for c in &raw {
            let name = name_of(c);
            let Some(id) = c.id.as_deref() else { continue };
            let labels: BTreeMap<String, String> = c.labels.clone().unwrap_or_default().into_iter().collect();
            for (kind, r) in label_routes(&name, &labels) {
                let owner = match kind {
                    ProxyKind::Traefik => traefik.clone().unwrap_or_else(|| "traefik".into()),
                    _ => caddy_labels.clone().unwrap_or_else(|| "caddy".into()),
                };
                add(&mut proxies, kind, &owner, vec![r], None);
            }
            if image_has(c, "nginx-proxy-manager") {
                let script = "for f in /data/nginx/proxy_host/*.conf; do [ -r \"$f\" ] && { echo \"@@F $f\"; cat \"$f\"; }; done";
                match self.exec_read(id, &["sh", "-c", script]).await {
                    Ok(out) => add(&mut proxies, ProxyKind::Npm, &name, parse_npm(&out), None),
                    Err(e) => add(&mut proxies, ProxyKind::Npm, &name, vec![], Some(e.to_string())),
                }
            } else if image_has(c, "caddy") && !image_has(c, "caddy-docker-proxy") {
                match self.exec_read(id, &["cat", "/etc/caddy/Caddyfile"]).await {
                    Ok(out) if out.contains("No such file") => add(
                        &mut proxies,
                        ProxyKind::Caddy,
                        &name,
                        vec![],
                        Some("No /etc/caddy/Caddyfile in the container (JSON config isn't read).".into()),
                    ),
                    Ok(out) => add(&mut proxies, ProxyKind::Caddy, &name, parse_caddyfile(&out), None),
                    Err(e) => add(&mut proxies, ProxyKind::Caddy, &name, vec![], Some(e.to_string())),
                }
            } else if image_has(c, "traefik") && !proxies.iter().any(|p| p.kind == ProxyKind::Traefik && p.name == name)
            {
                add(&mut proxies, ProxyKind::Traefik, &name, vec![], None);
            }
        }
        // Routes Nightgrid keeps in Traefik's file provider (M10v).
        for c in raw.iter().filter(|c| image_has(c, "traefik")) {
            let name = name_of(c);
            if let Ok(t) = self.traefik_target(&name).await {
                if !t.routes.is_empty() {
                    let raws = t
                        .routes
                        .into_iter()
                        .map(|r| RawRoute {
                            domains: r.domains,
                            upstream: r.upstream,
                            tls: r.tls,
                            origin: format!("{} {}", super::proxy_edit::TRAEFIK_FILE, r.name),
                        })
                        .collect();
                    add(&mut proxies, ProxyKind::Traefik, &name, raws, None);
                }
            }
        }
        // Caddy installed on the host itself.
        if let Ok((0, out, _)) = self.conn.exec("cat /etc/caddy/Caddyfile 2>/dev/null").await {
            let text = String::from_utf8_lossy(&out);
            if !text.trim().is_empty() {
                add(&mut proxies, ProxyKind::Caddy, "host", parse_caddyfile(&text), None);
            }
        }
        // Certificates of the first domain of each TLS route.
        let mut domains: Vec<String> =
            routes.iter().filter(|r| r.tls).filter_map(|r| r.domains.first().cloned()).collect();
        domains.sort();
        domains.dedup();
        let mut now = 0.0;
        if !domains.is_empty() {
            if let Ok((_, out, _)) = self.conn.exec(&cert_command(&domains)).await {
                let (n, certs) = parse_certs(&String::from_utf8_lossy(&out));
                now = n.unwrap_or(0.0);
                let missing = certs.get("").cloned();
                for r in routes.iter_mut().filter(|r| r.tls) {
                    let d = r.domains.first().cloned().unwrap_or_default();
                    r.cert = certs.get(&d).cloned().or_else(|| missing.clone());
                }
            }
        }
        routes.sort_by(|a, b| a.domains.first().cmp(&b.domains.first()));
        Ok(ProxyReport { proxies, routes, now })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docker::PortView;

    #[test]
    fn caddyfile_sites() {
        let text = r#"
{
    email admin@example.com
}

(common) {
    encode gzip
}

# the wiki
wiki.example.com, www.wiki.example.com {
    import common
    reverse_proxy wiki:3000
}

http://media.lan {
    handle /api/* {
        reverse_proxy @api jellyfin:8096 # api
    }
    reverse_proxy jellyfin:8096
}

:8080 {
    reverse_proxy {
        to 127.0.0.1:9000 127.0.0.1:9001
        lb_policy first
    }
}

static.example.com {
    file_server
}
"#;
        let r = parse_caddyfile(text);
        assert_eq!(r.len(), 4, "{r:#?}");
        assert_eq!(r[0].domains, vec!["wiki.example.com", "www.wiki.example.com"]);
        assert_eq!(r[0].upstream, "wiki:3000");
        assert!(r[0].tls);
        assert_eq!(r[1].domains, vec!["media.lan"]);
        assert_eq!(r[1].upstream, "jellyfin:8096");
        assert!(!r[1].tls);
        assert_eq!(r[2].domains, vec![":8080"]);
        assert_eq!(r[2].upstream, "127.0.0.1:9000");
        assert_eq!(r[3].upstream, "127.0.0.1:9001");
        assert!(!r[2].tls);
    }

    #[test]
    fn caddyfile_without_braces() {
        let r = parse_caddyfile("app.example.com\n\nreverse_proxy localhost:8000\n");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].domains, vec!["app.example.com"]);
        assert_eq!(r[0].upstream, "localhost:8000");
    }

    #[test]
    fn traefik_rules_and_labels() {
        assert_eq!(
            traefik_hosts("Host(`a.example.com`) || Host(`b.example.com`, `c.example.com`)"),
            vec!["a.example.com", "b.example.com", "c.example.com"]
        );
        assert_eq!(traefik_hosts("HostRegexp(`{sub:[a-z]+}.example.com`) && PathPrefix(`/x`)"), Vec::<String>::new());
        let labels: BTreeMap<String, String> = [
            ("traefik.enable", "true"),
            ("traefik.http.routers.app.rule", "Host(`app.example.com`)"),
            ("traefik.http.routers.app.entrypoints", "websecure"),
            ("traefik.http.routers.app.tls.certresolver", "le"),
            ("traefik.http.services.app.loadbalancer.server.port", "8080"),
            ("traefik.http.routers.app-http.rule", "Host(`app.example.com`)"),
            ("traefik.http.routers.app-http.entrypoints", "web"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
        let r = label_routes("app", &labels);
        assert_eq!(r.len(), 2);
        let (kind, a) = r.iter().find(|(_, x)| x.origin == "router app").unwrap();
        assert_eq!(*kind, ProxyKind::Traefik);
        assert_eq!(a.upstream, "app:8080");
        assert!(a.tls);
        assert!(!r.iter().find(|(_, x)| x.origin == "router app-http").unwrap().1.tls);
    }

    #[test]
    fn caddy_docker_proxy_labels() {
        let labels: BTreeMap<String, String> =
            [("caddy", "photos.example.com"), ("caddy.reverse_proxy", "{{upstreams 2342}}")]
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect();
        let r = label_routes("photoprism", &labels);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, ProxyKind::Caddy);
        assert_eq!(r[0].1.upstream, "photoprism:2342");
        assert!(r[0].1.tls);
    }

    #[test]
    fn npm_confs() {
        let text = "@@F /data/nginx/proxy_host/3.conf\n\
            # ------------------------------------------------------------\n# cloud.example.com\n\
            server {\n  set $forward_scheme http;\n  set $server         \"nextcloud\";\n  set $port           80;\n\
            listen 80;\nlisten [::]:80;\nlisten 443 ssl;\n  server_name cloud.example.com;\n  ssl_certificate /etc/letsencrypt/live/npm-3/fullchain.pem;\n}\n\
            @@F /data/nginx/proxy_host/4.conf\nserver {\n  set $forward_scheme https;\n  set $server \"192.168.1.20\";\n  set $port 8443;\n  listen 80;\n  server_name nas.example.com nas2.example.com;\n}\n";
        let r = parse_npm(text);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].domains, vec!["cloud.example.com"]);
        assert_eq!(r[0].upstream, "http://nextcloud:80");
        assert!(r[0].tls);
        assert_eq!(r[0].origin, "3.conf");
        assert_eq!(r[1].upstream, "https://192.168.1.20:8443");
        assert!(!r[1].tls);
    }

    fn cv(name: &str, service: Option<&str>, public: Option<u16>) -> ContainerView {
        ContainerView {
            id: format!("{name}0123456789abcdef"),
            name: name.into(),
            image: "x".into(),
            state: "running".into(),
            status: "Up".into(),
            health: None,
            created: 0,
            ports: public
                .map(|p| {
                    vec![PortView { private: 80, public: Some(p), ip: Some("0.0.0.0".into()), proto: "tcp".into() }]
                })
                .unwrap_or_default(),
            project: None,
            service: service.map(str::to_owned),
            compose_dir: None,
            compose_files: vec![],
            web: vec![],
        }
    }

    #[test]
    fn matches_containers() {
        let cs = [cv("wiki-app-1", Some("wiki"), None), cv("grafana", None, Some(3000))];
        assert_eq!(match_container("wiki:3000", &cs).as_deref(), Some("wiki-app-1"));
        assert_eq!(match_container("http://localhost:3000", &cs).as_deref(), Some("grafana"));
        assert_eq!(match_container("http://192.168.1.5:3000", &cs).as_deref(), Some("grafana"));
        assert_eq!(match_container("other.lan:80", &cs), None);
    }

    #[test]
    fn certs_and_dates() {
        assert_eq!(parse_openssl_date("Jan  1 00:00:00 1970 GMT"), Some(0.0));
        assert_eq!(parse_openssl_date("Feb  1 12:00:00 2027 GMT"), Some(1_801_483_200.0));
        let out = "now 1800000000\ncert\ta.example.com\tnotAfter=Feb  1 12:00:00 2027 GMT|issuer=C = US, O = Let's Encrypt, CN = R11|\ncert\tb.example.com\t\n";
        let (now, c) = parse_certs(out);
        assert_eq!(now, Some(1_800_000_000.0));
        assert_eq!(c["a.example.com"].issuer.as_deref(), Some("Let's Encrypt R11"));
        assert!(c["b.example.com"].error.is_some());
        let cmd = cert_command(&["a.example.com".into(), "$(reboot)".into(), "10.0.0.1".into()]);
        assert!(cmd.contains("'a.example.com'") && !cmd.contains("reboot") && !cmd.contains("10.0.0.1"));
    }

    #[test]
    fn cert_script_runs_under_sh() {
        let out = std::process::Command::new("sh").arg("-c").arg(cert_command(&[])).output().unwrap();
        assert!(out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout).starts_with("now "));
    }

    #[test]
    fn bare_hosts() {
        assert_eq!(bare_host("https://a.example.com:443/x"), "a.example.com");
        assert_eq!(bare_host(":8080"), ":8080");
        assert_eq!(bare_host("[::1]:80"), "::1");
    }
}
