//! Reverse proxy route editor (M10v): changes Caddy sites in a Caddyfile and
//! Traefik routes in a file Nightgrid owns.
//!
//! - **Caddy**: the Caddyfile is edited as text (add a site, change a simple
//!   site's upstream, remove a site, or replace the whole file), checked with
//!   `caddy validate` where Caddy runs, written in place (a copy of the old
//!   one is kept next to it) and loaded with `caddy reload`. A reload that
//!   fails puts the old file back.
//! - **Traefik**: routes go to `nightgrid.yml` in the directory of Traefik's
//!   file provider, which Traefik watches; routes from labels or other files
//!   stay read-only.

use std::time::Duration;

use base64::Engine as _;
use bollard::exec::StartExecResults;
use bollard::models::ExecConfig;
use bollard::query_parameters::{InspectContainerOptions, ListContainersOptions};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use yaml_rust2::{Yaml, YamlLoader};

use super::proxy::{is_matcher, strip_comment};
use super::{DockerError, DockerHost, DockerResult};
use crate::files::path::shell_quote;

/// Where Caddy keeps its Caddyfile in the official image and in packages.
pub const CADDYFILE: &str = "/etc/caddy/Caddyfile";
/// The Traefik file Nightgrid writes in the file provider's directory.
pub const TRAEFIK_FILE: &str = "nightgrid.yml";
const MAX_TEXT: usize = 256 * 1024;
const EXEC_LIMIT: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct EditError(pub String);

fn err(s: impl Into<String>) -> EditError {
    EditError(s.into())
}

/// A short SHA-256 of a file's text, to refuse writing over a change made
/// elsewhere since it was read.
pub fn hash(text: &str) -> String {
    let d = Sha256::digest(text.as_bytes());
    d.iter().take(12).map(|b| format!("{b:02x}")).collect()
}

// ---- validation ----------------------------------------------------------------

/// A site address: `example.com`, `*.example.com`, `:8080`, `http://a.lan`,
/// `localhost:8443`.
pub fn valid_address(a: &str) -> bool {
    let rest = a.strip_prefix("https://").or_else(|| a.strip_prefix("http://")).unwrap_or(a);
    let (host, port) = match rest.rsplit_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (rest, None),
    };
    let port_ok = port.map_or(true, |p| !p.is_empty() && p.len() <= 5 && p.chars().all(|c| c.is_ascii_digit()));
    let host_ok = (host.is_empty() && port.is_some())
        || (host.len() <= 253
            && host.split('.').enumerate().all(|(i, l)| {
                (i == 0 && l == "*")
                    || (!l.is_empty()
                        && l.len() <= 63
                        && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                        && !l.starts_with('-'))
            }));
    !a.is_empty() && port_ok && host_ok
}

/// A plain host name for Traefik's `Host()` (no scheme, port or wildcard).
pub fn valid_domain(d: &str) -> bool {
    valid_address(d) && !d.contains(':') && !d.contains('*')
}

/// An upstream: `app:8080`, `10.0.0.5:3000`, `http://app:8080`, `https://nas.lan:5001`.
pub fn valid_upstream(u: &str) -> bool {
    let rest = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://")).unwrap_or(u);
    !rest.is_empty()
        && u.len() <= 200
        && rest.chars().all(|c| c.is_ascii_alphanumeric() || ".-_:[]/".contains(c))
        && !rest.starts_with(['/', ':', '-'])
}

fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 40 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn valid_path(p: &str) -> bool {
    p.starts_with('/')
        && p.len() <= 200
        && !p.contains("..")
        && p.chars().all(|c| c.is_ascii_alphanumeric() || "/._-".contains(c))
}

// ---- Caddy -----------------------------------------------------------------------

/// A top-level site block of a Caddyfile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CaddySite {
    pub addresses: Vec<String>,
    /// 0-based first and last line of the block.
    pub first_line: u32,
    pub last_line: u32,
    pub upstreams: Vec<String>,
    /// The site has exactly one one-line `reverse_proxy` without matchers,
    /// so its upstream can be changed in place.
    pub simple: bool,
}

/// The site blocks of a Caddyfile, with line ranges. A Caddyfile without
/// braces (one site) has none: it can only be edited as text.
pub fn caddy_sites(text: &str) -> Vec<CaddySite> {
    caddy_sites_inner(text).into_iter().map(|(s, _)| s).collect()
}

/// A site block being read: addresses, first line, upstreams, lines of
/// one-line `reverse_proxy` at depth 1, and whether any was complex.
type Open = (Vec<String>, usize, Vec<String>, Vec<usize>, bool);

/// Sites plus the line of their simple `reverse_proxy`, if any.
fn caddy_sites_inner(text: &str) -> Vec<(CaddySite, Option<usize>)> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut cur: Option<Open> = None;
    for (i, raw) in text.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if line == "}" {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                if let Some((addresses, first, upstreams, lines, complex)) = cur.take() {
                    let simple = !complex && lines.len() == 1;
                    let pl = if simple { lines.first().copied() } else { None };
                    out.push((
                        CaddySite { addresses, first_line: first as u32, last_line: i as u32, upstreams, simple },
                        pl,
                    ));
                }
            }
            continue;
        }
        let opens = line == "{" || line.ends_with(" {");
        let head = line.trim_end_matches('{').trim();
        if depth == 0 {
            if opens && !head.is_empty() && !head.starts_with('(') && !head.starts_with('&') {
                let addresses = head.split([',', ' ', '\t']).filter(|s| !s.is_empty()).map(str::to_owned).collect();
                cur = Some((addresses, i, Vec::new(), Vec::new(), false));
            }
        } else if let Some((_, _, ups, lines, complex)) = cur.as_mut() {
            let mut words = head.split_whitespace();
            if words.next() == Some("reverse_proxy") {
                let args: Vec<&str> = words.collect();
                ups.extend(args.iter().filter(|w| !is_matcher(w)).map(|w| (*w).to_owned()));
                if depth == 1 && !opens && !args.is_empty() && !args.iter().any(|w| is_matcher(w)) {
                    lines.push(i);
                } else {
                    *complex = true;
                }
            }
        }
        if opens {
            depth += 1;
        }
    }
    out
}

/// One change to a Caddyfile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum CaddyEdit {
    /// `domains { reverse_proxy upstream }` at the end.
    AddSite { domains: Vec<String>, upstream: String },
    /// Changes the upstream of a simple site, found by its first address.
    SetUpstream { site: String, upstream: String },
    /// Deletes a site block, found by its first address.
    RemoveSite { site: String },
    /// The whole file, as typed.
    Replace { text: String },
}

impl CaddyEdit {
    /// A summary for logs and the confirmation (no file text).
    pub fn describe(&self) -> String {
        match self {
            Self::AddSite { domains, upstream } => format!("add site {} → {upstream}", domains.join(", ")),
            Self::SetUpstream { site, upstream } => format!("send {site} to {upstream}"),
            Self::RemoveSite { site } => format!("remove site {site}"),
            Self::Replace { .. } => "edit the Caddyfile".into(),
        }
    }
}

fn find_site(sites: &[(CaddySite, Option<usize>)], site: &str) -> Result<usize, EditError> {
    sites
        .iter()
        .position(|(s, _)| s.addresses.first().map(String::as_str) == Some(site))
        .ok_or_else(|| err(format!("no site {site} in the Caddyfile")))
}

/// Applies `edit` to a Caddyfile's text.
pub fn apply_caddy(text: &str, edit: &CaddyEdit) -> Result<String, EditError> {
    let sites = caddy_sites_inner(text);
    let lines: Vec<&str> = text.lines().collect();
    let join = |v: Vec<String>| {
        let mut s = v.join("\n");
        s.push('\n');
        s
    };
    match edit {
        CaddyEdit::AddSite { domains, upstream } => {
            if domains.is_empty() {
                return Err(err("give at least one domain"));
            }
            if let Some(d) = domains.iter().find(|d| !valid_address(d)) {
                return Err(err(format!("{d:?} isn't a site address")));
            }
            if !valid_upstream(upstream) {
                return Err(err(format!("{upstream:?} isn't an upstream (host:port or a URL)")));
            }
            if !text.trim().is_empty() && sites.is_empty() && !text.contains('{') {
                return Err(err("this Caddyfile has one site without braces: edit it as text"));
            }
            if let Some(d) = domains.iter().find(|d| sites.iter().any(|(s, _)| s.addresses.contains(d))) {
                return Err(err(format!("{d} already has a site")));
            }
            let mut out = text.trim_end().to_owned();
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&format!("{} {{\n\treverse_proxy {upstream}\n}}\n", domains.join(", ")));
            Ok(out)
        }
        CaddyEdit::SetUpstream { site, upstream } => {
            if !valid_upstream(upstream) {
                return Err(err(format!("{upstream:?} isn't an upstream (host:port or a URL)")));
            }
            let i = find_site(&sites, site)?;
            let Some(l) = sites[i].1 else {
                return Err(err(format!("{site} has more than one reverse_proxy or a block: edit it as text")));
            };
            let indent: String = lines[l].chars().take_while(|c| c.is_whitespace()).collect();
            let mut v: Vec<String> = lines.iter().map(|s| (*s).to_owned()).collect();
            v[l] = format!("{indent}reverse_proxy {upstream}");
            Ok(join(v))
        }
        CaddyEdit::RemoveSite { site } => {
            let i = find_site(&sites, site)?;
            let (a, b) = (sites[i].0.first_line as usize, sites[i].0.last_line as usize);
            let mut v: Vec<String> = Vec::new();
            for (n, l) in lines.iter().enumerate() {
                if n < a || n > b {
                    v.push((*l).to_owned());
                }
            }
            // Drop the blank line the block leaves behind, if it doubles up.
            if a > 0 && a < v.len() && v[a - 1].trim().is_empty() && v[a].trim().is_empty() {
                v.remove(a);
            }
            while v.last().is_some_and(|l| l.trim().is_empty()) {
                v.pop();
            }
            Ok(if v.is_empty() { String::new() } else { join(v) })
        }
        CaddyEdit::Replace { text } => {
            if text.len() > MAX_TEXT || text.contains('\0') {
                return Err(err("the Caddyfile is too large or not text"));
            }
            Ok(text.clone())
        }
    }
}

/// Script (root on the host, or inside the Caddy container) that writes
/// `text` to `path`: back up, `caddy validate`, write in place (keeps a bind
/// mount of the single file working), `caddy reload`, restore if that fails.
/// Prints `@@ok`, `@@invalid` or `@@reloadfailed` followed by Caddy's words.
pub fn caddy_write_script(path: &str, text: &str, on_host: bool) -> Result<String, EditError> {
    if !valid_path(path) {
        return Err(err("invalid Caddyfile path"));
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    let fallback =
        if on_host { " || { command -v systemctl >/dev/null 2>&1 && systemctl reload caddy 2>&1; }" } else { "" };
    Ok(format!(
        "F={f}; D=$(dirname \"$F\"); B=\"$D/.nightgrid-backups\"; T=\"$D/.Caddyfile.nightgrid-new\"; K=\"$B/Caddyfile.$(date +%s)\"; \
cd \"$D\" || exit 2; mkdir -p \"$B\" && {{ [ ! -f \"$F\" ] || cp -p \"$F\" \"$K\"; }} || {{ echo 'could not back up the Caddyfile' >&2; exit 2; }}; \
(cd \"$B\" && ls -1t | tail -n +11 | while IFS= read -r f; do rm -f -- \"$f\"; done); \
printf '%s' '{b64}' | base64 -d > \"$T\" || {{ echo 'could not write a temporary file' >&2; exit 2; }}; \
if ! out=$(caddy validate --config \"$T\" --adapter caddyfile 2>&1); then rm -f \"$T\"; echo @@invalid; printf '%s\\n' \"$out\" | grep -v '^{{' | tail -n 12; exit 0; fi; \
cat \"$T\" > \"$F\"; rm -f \"$T\"; \
if out=$( {{ caddy reload --config \"$F\" --adapter caddyfile 2>&1{fallback}; }} ); then echo @@ok; else \
[ -f \"$K\" ] && cat \"$K\" > \"$F\" && caddy reload --config \"$F\" --adapter caddyfile >/dev/null 2>&1; \
echo @@reloadfailed; printf '%s\\n' \"$out\" | tail -n 12; fi",
        f = shell_quote(path),
    ))
}

/// The outcome of a Caddyfile write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SaveOutcome {
    pub ok: bool,
    /// Caddy's or the shell's words when it failed.
    pub message: String,
}

pub fn parse_write(out: &str, stderr: &str) -> SaveOutcome {
    let mut lines = out.lines();
    let first = lines.by_ref().find(|l| l.starts_with("@@")).unwrap_or("");
    let rest: Vec<&str> = lines.collect();
    let rest = rest.join("\n").trim().to_owned();
    match first {
        "@@ok" => SaveOutcome { ok: true, message: String::new() },
        "@@invalid" => {
            SaveOutcome { ok: false, message: format!("Caddy refused the new Caddyfile; nothing changed. {rest}") }
        }
        "@@reloadfailed" => {
            SaveOutcome { ok: false, message: format!("Caddy couldn't load it, so the old Caddyfile is back. {rest}") }
        }
        _ => SaveOutcome {
            ok: false,
            message: format!("couldn't write the Caddyfile: {}", stderr.trim().lines().last().unwrap_or("no output")),
        },
    }
}

// ---- Traefik ------------------------------------------------------------------------

/// A route Nightgrid keeps in Traefik's file provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TraefikRoute {
    pub name: String,
    pub domains: Vec<String>,
    /// `http://app:8080`; a bare `app:8080` gets `http://`.
    pub upstream: String,
    /// Empty = every entry point.
    pub entrypoints: Vec<String>,
    pub tls: bool,
    pub cert_resolver: Option<String>,
}

impl TraefikRoute {
    pub fn normalized(mut self) -> Result<Self, EditError> {
        self.name = self.name.trim().to_ascii_lowercase().replace(' ', "-");
        if !valid_name(&self.name) {
            return Err(err("a route name is letters, digits, - and _ (up to 40)"));
        }
        self.domains = self.domains.iter().map(|d| d.trim().to_ascii_lowercase()).filter(|d| !d.is_empty()).collect();
        if self.domains.is_empty() {
            return Err(err(format!("{}: give at least one domain", self.name)));
        }
        if let Some(d) = self.domains.iter().find(|d| !valid_domain(d)) {
            return Err(err(format!("{d:?} isn't a domain name")));
        }
        self.upstream = self.upstream.trim().to_owned();
        if !valid_upstream(&self.upstream) {
            return Err(err(format!("{:?} isn't an upstream (host:port or a URL)", self.upstream)));
        }
        if !self.upstream.contains("://") {
            self.upstream = format!("http://{}", self.upstream);
        }
        if let Some(e) = self.entrypoints.iter().find(|e| !valid_name(e)) {
            return Err(err(format!("{e:?} isn't an entry point name")));
        }
        self.cert_resolver = self.cert_resolver.map(|r| r.trim().to_owned()).filter(|r| !r.is_empty());
        if let Some(r) = self.cert_resolver.as_deref().filter(|r| !valid_name(r)) {
            return Err(err(format!("{r:?} isn't a certificate resolver name")));
        }
        if self.cert_resolver.is_some() {
            self.tls = true;
        }
        Ok(self)
    }
}

const SPEC_PREFIX: &str = "# nightgrid ";

/// `nightgrid.yml` for these routes; the routes as JSON on the first line
/// so the file reads back exactly.
pub fn traefik_yaml(routes: &[TraefikRoute]) -> Result<String, EditError> {
    let mut names = Vec::new();
    for r in routes {
        if names.contains(&&r.name) {
            return Err(err(format!("two routes are named {}", r.name)));
        }
        names.push(&r.name);
    }
    let spec = serde_json::to_string(routes).map_err(|e| err(e.to_string()))?;
    let mut y = format!(
        "{SPEC_PREFIX}{spec}\n# Written by Nightgrid (Docker → Routes). Changes made here are replaced the next time it saves.\nhttp:\n  routers:\n"
    );
    for r in routes {
        let rule = r.domains.iter().map(|d| format!("Host(`{d}`)")).collect::<Vec<_>>().join(" || ");
        y.push_str(&format!("    ng-{n}:\n      rule: \"{rule}\"\n      service: ng-{n}\n", n = r.name));
        if !r.entrypoints.is_empty() {
            y.push_str("      entryPoints:\n");
            for e in &r.entrypoints {
                y.push_str(&format!("        - {e}\n"));
            }
        }
        match (&r.cert_resolver, r.tls) {
            (Some(c), _) => y.push_str(&format!("      tls:\n        certResolver: {c}\n")),
            (None, true) => y.push_str("      tls: {}\n"),
            _ => {}
        }
    }
    y.push_str("  services:\n");
    for r in routes {
        y.push_str(&format!(
            "    ng-{n}:\n      loadBalancer:\n        servers:\n          - url: \"{u}\"\n",
            n = r.name,
            u = r.upstream
        ));
    }
    Ok(y)
}

/// Routes from a `nightgrid.yml` Nightgrid wrote; `None` if it isn't one.
pub fn parse_traefik_file(text: &str) -> Option<Vec<TraefikRoute>> {
    if text.trim().is_empty() {
        return Some(Vec::new());
    }
    let first = text.lines().next()?;
    serde_json::from_str(first.strip_prefix(SPEC_PREFIX)?).ok()
}

/// What Traefik's static configuration says, as far as the editor cares.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TraefikStatic {
    /// `providers.file.directory`.
    pub directory: Option<String>,
    /// `providers.file.filename` (a single file: not edited).
    pub filename: Option<String>,
    pub entrypoints: Vec<String>,
    pub resolvers: Vec<String>,
}

/// Reads command-line flags (`--providers.file.directory=/x`, case-insensitive
/// like Traefik) and a static YAML file.
pub fn traefik_static(args: &[String], yaml: Option<&str>) -> TraefikStatic {
    let mut s = TraefikStatic::default();
    let push = |v: &mut Vec<String>, name: &str| {
        if !name.is_empty() && !v.iter().any(|x| x == name) {
            v.push(name.to_owned());
        }
    };
    let mut i = 0;
    while i < args.len() {
        let a = args[i].trim_start_matches('-');
        let (key, val) = match a.split_once('=') {
            Some((k, v)) => (k.to_owned(), Some(v.to_owned())),
            None => (a.to_owned(), args.get(i + 1).filter(|n| !n.starts_with('-')).cloned()),
        };
        let lower = key.to_ascii_lowercase();
        if lower == "providers.file.directory" {
            s.directory = val.clone();
        } else if lower == "providers.file.filename" {
            s.filename = val.clone();
        } else if let Some(rest) = lower.strip_prefix("entrypoints.") {
            let n = rest.len();
            let name = &key[key.len() - n..];
            push(&mut s.entrypoints, name.split('.').next().unwrap_or(""));
        } else if let Some(rest) = lower.strip_prefix("certificatesresolvers.") {
            let n = rest.len();
            let name = &key[key.len() - n..];
            push(&mut s.resolvers, name.split('.').next().unwrap_or(""));
        }
        i += 1;
    }
    if let Some(doc) = yaml.and_then(|y| YamlLoader::load_from_str(y).ok()).and_then(|d| d.into_iter().next()) {
        let get = |y: &Yaml, k: &str| -> Option<Yaml> {
            y.as_hash()?
                .iter()
                .find(|(kk, _)| kk.as_str().is_some_and(|x| x.eq_ignore_ascii_case(k)))
                .map(|(_, v)| v.clone())
        };
        if let Some(file) = get(&doc, "providers").and_then(|p| get(&p, "file")) {
            if s.directory.is_none() {
                s.directory = get(&file, "directory").and_then(|d| d.as_str().map(str::to_owned));
            }
            if s.filename.is_none() {
                s.filename = get(&file, "filename").and_then(|d| d.as_str().map(str::to_owned));
            }
        }
        for (key, v) in [("entryPoints", &mut s.entrypoints), ("certificatesResolvers", &mut s.resolvers)] {
            if let Some(Yaml::Hash(h)) = get(&doc, key) {
                for k in h.keys().filter_map(|k| k.as_str()) {
                    push(v, k);
                }
            }
        }
    }
    s
}

/// Container-side script that writes (or, for no routes, removes) the file.
pub fn traefik_write_script(directory: &str, text: Option<&str>) -> Result<String, EditError> {
    if !valid_path(directory) {
        return Err(err(format!("{directory:?} isn't a directory Nightgrid writes to")));
    }
    let d = shell_quote(directory);
    Ok(match text {
        Some(t) => {
            let b64 = base64::engine::general_purpose::STANDARD.encode(t.as_bytes());
            format!(
                "D={d}; mkdir -p \"$D\" && printf '%s' '{b64}' | base64 -d > \"$D/.{TRAEFIK_FILE}.tmp\" && mv -f \"$D/.{TRAEFIK_FILE}.tmp\" \"$D/{TRAEFIK_FILE}\" && echo @@ok"
            )
        }
        None => format!("rm -f {d}/{TRAEFIK_FILE} && echo @@ok"),
    })
}

// ---- reading what can be edited -------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CaddyTarget {
    /// The proxy's container name, or "host".
    pub proxy_name: String,
    pub path: String,
    pub text: String,
    pub hash: String,
    pub sites: Vec<CaddySite>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TraefikTarget {
    pub proxy_name: String,
    pub config: TraefikStatic,
    /// Routes in Nightgrid's file (empty when there is none yet).
    pub routes: Vec<TraefikRoute>,
    pub hash: String,
    /// Why routes can't be added here, if they can't.
    pub blocked: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct EditTargets {
    pub caddy: Vec<CaddyTarget>,
    pub traefik: Vec<TraefikTarget>,
}

fn caddy_target(proxy_name: &str, text: String) -> CaddyTarget {
    CaddyTarget {
        proxy_name: proxy_name.to_owned(),
        path: CADDYFILE.into(),
        hash: hash(&text),
        sites: caddy_sites(&text),
        text,
    }
}

impl DockerHost {
    /// Runs a shell script in a running container; (exit code, stdout+stderr).
    pub async fn exec_script(&self, id: &str, script: &str) -> DockerResult<(i64, String)> {
        let cfg = ExecConfig {
            cmd: Some(vec!["sh".into(), "-c".into(), script.to_owned()]),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };
        let e = self.docker.create_exec(id, cfg).await.map_err(|e| DockerError::from_bollard(e, Some(id)))?;
        let started = self.docker.start_exec(&e.id, None).await.map_err(|e| DockerError::from_bollard(e, Some(id)))?;
        let mut buf = Vec::new();
        if let StartExecResults::Attached { mut output, .. } = started {
            let read = async {
                while let Some(Ok(chunk)) = output.next().await {
                    buf.extend_from_slice(&chunk.into_bytes());
                    if buf.len() > EXEC_LIMIT {
                        break;
                    }
                }
            };
            let _ = tokio::time::timeout(Duration::from_secs(60), read).await;
        }
        let code = self.docker.inspect_exec(&e.id).await.ok().and_then(|i| i.exit_code).unwrap_or(-1);
        Ok((code, String::from_utf8_lossy(&buf).into_owned()))
    }

    async fn container_named(&self, name: &str) -> DockerResult<(String, String)> {
        let opts = ListContainersOptions { all: false, ..Default::default() };
        let raw = self.docker.list_containers(Some(opts)).await.map_err(|e| DockerError::from_bollard(e, None))?;
        raw.into_iter()
            .find(|c| c.names.as_ref().is_some_and(|n| n.iter().any(|n| n.trim_start_matches('/') == name)))
            .and_then(|c| Some((c.id?, c.image.unwrap_or_default())))
            .ok_or_else(|| DockerError::Other { message: format!("no running container named {name}") })
    }

    /// The Caddyfile of the Caddy container `name`.
    pub async fn caddy_in_container(&self, name: &str) -> DockerResult<CaddyTarget> {
        let (id, _) = self.container_named(name).await?;
        let (code, text) = self.exec_script(&id, &format!("cat {CADDYFILE}")).await?;
        if code != 0 {
            return Err(DockerError::Other { message: format!("no {CADDYFILE} in {name} (JSON config isn't edited)") });
        }
        Ok(caddy_target(name, text))
    }

    /// The Traefik container `name`'s file provider and Nightgrid's routes.
    pub async fn traefik_target(&self, name: &str) -> DockerResult<TraefikTarget> {
        let (id, _) = self.container_named(name).await?;
        let r = self
            .docker
            .inspect_container(&id, None::<InspectContainerOptions>)
            .await
            .map_err(|e| DockerError::from_bollard(e, Some(&id)))?;
        let mut args: Vec<String> = r.config.as_ref().and_then(|c| c.cmd.clone()).unwrap_or_default();
        args.extend(r.args.clone().unwrap_or_default());
        args.dedup();
        let (_, yaml) = self
            .exec_script(&id, "for f in /etc/traefik/traefik.yml /etc/traefik/traefik.yaml; do [ -f \"$f\" ] && { cat \"$f\"; break; }; done")
            .await?;
        let config = traefik_static(&args, Some(&yaml).filter(|y| !y.trim().is_empty()).map(String::as_str));
        let mut t =
            TraefikTarget { proxy_name: name.to_owned(), config, routes: Vec::new(), hash: hash(""), blocked: None };
        match t.config.directory.clone() {
            Some(d) if valid_path(&d) => {
                let (_, text) = self.exec_script(&id, &format!("cat {}/{TRAEFIK_FILE} 2>/dev/null", shell_quote(&d))).await?;
                t.hash = hash(&text);
                match parse_traefik_file(&text) {
                    Some(r) => t.routes = r,
                    None => t.blocked = Some(format!("{d}/{TRAEFIK_FILE} exists but wasn't written by Nightgrid; rename it to add routes here.")),
                }
            }
            Some(d) => t.blocked = Some(format!("the file provider directory {d:?} has characters Nightgrid doesn't write to")),
            None if t.config.filename.is_some() => {
                t.blocked = Some("Traefik reads one dynamic file (providers.file.filename); Nightgrid adds routes only through a directory (providers.file.directory).".into())
            }
            None => {
                t.blocked = Some("Traefik has no file provider. Add --providers.file.directory=/etc/traefik/dynamic (and a volume for it) to its command, then refresh.".into())
            }
        }
        Ok(t)
    }

    /// Every Caddyfile and Traefik instance on the host that can be edited.
    pub async fn edit_targets(&self) -> DockerResult<EditTargets> {
        let opts = ListContainersOptions { all: false, ..Default::default() };
        let raw = self.docker.list_containers(Some(opts)).await.map_err(|e| DockerError::from_bollard(e, None))?;
        let mut out = EditTargets::default();
        let mut names: Vec<(String, bool)> = Vec::new();
        for c in &raw {
            let image = c.image.as_deref().unwrap_or("");
            let Some(name) = c.names.as_ref().and_then(|n| n.first()).map(|n| n.trim_start_matches('/').to_owned())
            else {
                continue;
            };
            if image.contains("caddy") && !image.contains("caddy-docker-proxy") {
                names.push((name, true));
            } else if image.contains("traefik") {
                names.push((name, false));
            }
        }
        for (name, caddy) in names {
            if caddy {
                if let Ok(t) = self.caddy_in_container(&name).await {
                    out.caddy.push(t);
                }
            } else if let Ok(t) = self.traefik_target(&name).await {
                out.traefik.push(t);
            }
        }
        if let Ok(t) = self.caddy_on_host().await {
            out.caddy.push(t);
        }
        Ok(out)
    }

    /// `/etc/caddy/Caddyfile` of a Caddy installed on the host (readable
    /// without root in the packages).
    pub async fn caddy_on_host(&self) -> DockerResult<CaddyTarget> {
        match self.conn.exec(&format!("cat {CADDYFILE} 2>/dev/null")).await {
            Ok((0, o, _)) if !o.is_empty() => Ok(caddy_target("host", String::from_utf8_lossy(&o).into_owned())),
            _ => Err(DockerError::Other { message: format!("no readable {CADDYFILE} on the host") }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CF: &str = "{\n\temail admin@example.com\n}\n\n(common) {\n\tencode gzip\n}\n\napp.example.com {\n\timport common\n\treverse_proxy app:8080\n}\n\n# media\nmedia.example.com, tv.example.com {\n\treverse_proxy /api/* api:9000\n\treverse_proxy jellyfin:8096\n}\n\nnas.example.com {\n\treverse_proxy {\n\t\tto 192.168.1.20:5000\n\t}\n}\n";

    #[test]
    fn finds_sites() {
        let s = caddy_sites(CF);
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].addresses, vec!["app.example.com"]);
        assert!(s[0].simple);
        assert_eq!(s[0].upstreams, vec!["app:8080"]);
        assert_eq!(s[1].addresses, vec!["media.example.com", "tv.example.com"]);
        assert!(!s[1].simple);
        assert!(!s[2].simple);
        assert_eq!(CF.lines().nth(s[2].last_line as usize), Some("}"));
    }

    #[test]
    fn edits_caddyfile() {
        let t =
            apply_caddy(CF, &CaddyEdit::AddSite { domains: vec!["new.example.com".into()], upstream: "web:80".into() })
                .unwrap();
        assert!(t.ends_with("}\n\nnew.example.com {\n\treverse_proxy web:80\n}\n"));
        assert_eq!(caddy_sites(&t).len(), 4);
        assert!(apply_caddy(
            CF,
            &CaddyEdit::AddSite { domains: vec!["app.example.com".into()], upstream: "x:1".into() }
        )
        .is_err());
        assert!(apply_caddy(CF, &CaddyEdit::AddSite { domains: vec!["a b".into()], upstream: "x:1".into() }).is_err());
        assert!(apply_caddy(
            CF,
            &CaddyEdit::AddSite { domains: vec!["ok.example.com".into()], upstream: "x:1; rm".into() }
        )
        .is_err());

        let t =
            apply_caddy(CF, &CaddyEdit::SetUpstream { site: "app.example.com".into(), upstream: "app2:9090".into() })
                .unwrap();
        assert!(t.contains("\treverse_proxy app2:9090\n"));
        assert!(!t.contains("app:8080"));
        assert!(apply_caddy(CF, &CaddyEdit::SetUpstream { site: "media.example.com".into(), upstream: "x:1".into() })
            .is_err());

        let t = apply_caddy(CF, &CaddyEdit::RemoveSite { site: "media.example.com".into() }).unwrap();
        assert!(!t.contains("jellyfin"));
        assert!(t.contains("# media\n\nnas.example.com {"));
        assert_eq!(caddy_sites(&t).len(), 2);
        let t = apply_caddy(&t, &CaddyEdit::RemoveSite { site: "nas.example.com".into() }).unwrap();
        assert!(t.ends_with("# media\n"));

        let one = "example.com\nreverse_proxy app:80\n";
        assert!(caddy_sites(one).is_empty());
        assert!(apply_caddy(
            one,
            &CaddyEdit::AddSite { domains: vec!["b.example.com".into()], upstream: "b:1".into() }
        )
        .is_err());
        assert_eq!(
            apply_caddy("", &CaddyEdit::AddSite { domains: vec![":8080".into()], upstream: "b:1".into() }).unwrap(),
            ":8080 {\n\treverse_proxy b:1\n}\n"
        );
    }

    #[test]
    fn validates() {
        for ok in
            ["example.com", "*.example.com", ":8080", "http://a.lan", "localhost:8443", "https://x.example.com:443"]
        {
            assert!(valid_address(ok), "{ok}");
        }
        for bad in ["", "a b", "ex;ample.com", "-a.com", "a..com", "a.com:99999x", "$(x)"] {
            assert!(!valid_address(bad), "{bad}");
        }
        for ok in ["app:8080", "http://10.0.0.5:3000", "https://nas.lan:5001", "[::1]:80", "unix//run/x.sock"] {
            assert!(valid_upstream(ok), "{ok}");
        }
        for bad in ["", "a b", "x'y", "/etc", "a;b", "$(id)"] {
            assert!(!valid_upstream(bad), "{bad}");
        }
    }

    #[test]
    fn write_script() {
        let s = caddy_write_script("/etc/caddy/Caddyfile", "a {\n}\n", true).unwrap();
        assert!(s.contains("caddy validate --config \"$T\" --adapter caddyfile"));
        assert!(s.contains("systemctl reload caddy"));
        assert!(!caddy_write_script("/etc/caddy/Caddyfile", "", false).unwrap().contains("systemctl"));
        assert!(caddy_write_script("/etc/../x", "", true).is_err());
        assert_eq!(parse_write("@@ok\n", ""), SaveOutcome { ok: true, message: String::new() });
        let o = parse_write("@@invalid\nError: adapting config: unrecognized directive: revers_proxy\n", "");
        assert!(!o.ok && o.message.contains("unrecognized directive"));
        assert!(parse_write("@@reloadfailed\nconnection refused\n", "").message.contains("old Caddyfile is back"));
        assert!(parse_write("", "Permission denied\n").message.contains("Permission denied"));
    }

    #[test]
    fn traefik_file_round_trip() {
        let r = TraefikRoute {
            name: "Photo App".into(),
            domains: vec!["Photos.Example.com".into()],
            upstream: "immich:2283".into(),
            entrypoints: vec!["websecure".into()],
            tls: false,
            cert_resolver: Some("le".into()),
        }
        .normalized()
        .unwrap();
        assert_eq!(r.name, "photo-app");
        assert_eq!(r.upstream, "http://immich:2283");
        assert!(r.tls);
        let plain = TraefikRoute {
            name: "wiki".into(),
            domains: vec!["wiki.lan".into(), "w.lan".into()],
            upstream: "http://wiki:3000".into(),
            entrypoints: vec![],
            tls: false,
            cert_resolver: None,
        };
        let y = traefik_yaml(&[r.clone(), plain.clone()]).unwrap();
        assert!(y.contains("rule: \"Host(`photos.example.com`)\""));
        assert!(y.contains("rule: \"Host(`wiki.lan`) || Host(`w.lan`)\""));
        assert!(y.contains("certResolver: le"));
        assert!(y.contains("- url: \"http://immich:2283\""));
        let doc = &YamlLoader::load_from_str(&y).unwrap()[0];
        assert_eq!(doc["http"]["routers"]["ng-wiki"]["service"].as_str(), Some("ng-wiki"));
        assert_eq!(
            doc["http"]["services"]["ng-photo-app"]["loadBalancer"]["servers"][0]["url"].as_str(),
            Some("http://immich:2283")
        );
        assert_eq!(parse_traefik_file(&y), Some(vec![r.clone(), plain]));
        assert_eq!(parse_traefik_file(""), Some(vec![]));
        assert_eq!(parse_traefik_file("http:\n  routers: {}\n"), None);
        assert!(traefik_yaml(&[r.clone(), r]).is_err());
        assert!(TraefikRoute {
            name: "x".into(),
            domains: vec!["*.lan".into()],
            upstream: "a:1".into(),
            entrypoints: vec![],
            tls: false,
            cert_resolver: None
        }
        .normalized()
        .is_err());
        assert!(traefik_write_script("/etc/traefik/dynamic", Some("x")).unwrap().contains("mv -f"));
        assert!(traefik_write_script("/etc/traefik/dynamic", None).unwrap().starts_with("rm -f"));
        assert!(traefik_write_script("rel/dir", None).is_err());
    }

    #[test]
    fn reads_traefik_static() {
        let args: Vec<String> = [
            "--api.dashboard=true",
            "--entryPoints.web.address=:80",
            "--entrypoints.websecure.address=:443",
            "--providers.docker=true",
            "--providers.file.directory",
            "/etc/traefik/dynamic",
            "--certificatesresolvers.le.acme.email=admin@example.com",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        let s = traefik_static(&args, None);
        assert_eq!(s.directory.as_deref(), Some("/etc/traefik/dynamic"));
        assert_eq!(s.entrypoints, vec!["web", "websecure"]);
        assert_eq!(s.resolvers, vec!["le"]);
        let y = "entryPoints:\n  http:\n    address: ':80'\n  https:\n    address: ':443'\nproviders:\n  file:\n    filename: /etc/traefik/dyn.yml\ncertificatesResolvers:\n  cf:\n    acme: {}\n";
        let s = traefik_static(&[], Some(y));
        assert_eq!(s.directory, None);
        assert_eq!(s.filename.as_deref(), Some("/etc/traefik/dyn.yml"));
        assert_eq!(s.entrypoints, vec!["http", "https"]);
        assert_eq!(s.resolvers, vec!["cf"]);
    }
}
