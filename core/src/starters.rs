//! Starter blueprints and runbooks (D-177): ready-made setups for common
//! homelab jobs, kept as data files in `starters/` at the repository root.
//!
//! A starter blueprint is a blueprint (D-161) written by hand instead of read
//! from a host: compose stacks with placeholders, firewall ports, Nightgrid
//! timers and steps done by hand. Placeholders are `${NAME}` variables that
//! the user fills in (or leaves to be generated) and that go into the `.env`
//! vault, so a starter itself never holds a secret. [`build`] turns a starter
//! plus the user's values into an ordinary [`Blueprint`], which is then
//! planned and applied with the same code as a blueprint file: the target is
//! read first, each piece becomes a step, and only ticked steps run.
//!
//! A starter runbook is a Markdown note with a small YAML header naming the
//! alert kinds it is for; adding one creates a runbook note (D-111).
//!
//! The files are parsed from YAML here (not in the UI) so the same checks
//! apply to the bundled set and to any set shipped later as a data pack.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use yaml_rust2::{Yaml, YamlLoader};

use crate::alerts::AlertKind;
use crate::blueprint::{Blueprint, BpEnv, BpFile, BpFirewall, BpManual, BpStack, BpZone};
use crate::docker::compose::valid_project;
use crate::envvault;
use crate::system::jobs::TimerJob;
use crate::{Error, Result};

/// Version of the starter file format; a pack with a newer one is ignored.
pub const FORMAT: u32 = 1;
/// A value typed into a starter's prompt.
pub const MAX_VALUE: usize = 1024;
/// A generated secret's length (letters and digits).
pub const GENERATED_LEN: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StarterVar {
    /// The `${NAME}` used in the compose file.
    pub name: String,
    /// The question asked, in plain words.
    pub label: String,
    #[serde(default)]
    pub default: String,
    #[serde(default)]
    pub help: String,
    /// Masked in the form and kept as a secret in the vault.
    #[serde(default)]
    pub secret: bool,
    /// Left empty, a random value is made.
    #[serde(default)]
    pub generate: bool,
    /// An absolute folder on the host.
    #[serde(default)]
    pub path: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StarterFile {
    /// Relative to the stack's folder.
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StarterPort {
    /// `8096/tcp`.
    pub port: String,
    /// What answers there.
    pub what: String,
    /// Bound to 127.0.0.1 only: open it through Nightgrid's web UI window.
    #[serde(default)]
    pub local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StarterStack {
    /// Compose project name, also its folder's name.
    pub name: String,
    /// `compose.yaml`.
    pub compose: String,
    /// Other files the stack needs (a Caddyfile, a config file).
    #[serde(default)]
    pub files: Vec<StarterFile>,
    #[serde(default)]
    pub env: Vec<StarterVar>,
    #[serde(default)]
    pub ports: Vec<StarterPort>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct StarterFirewall {
    /// Ports to allow (`8096/tcp`).
    pub allow: Vec<String>,
    /// Addresses or networks allowed to reach everything (`192.168.1.0/24`).
    pub allow_from: Vec<String>,
    /// Ports to allow with ufw's rate limit (SSH).
    pub limit: Vec<String>,
    /// Suggest denying other incoming traffic (always done by hand).
    pub deny_incoming: bool,
    /// Suggest turning the firewall on (always done by hand).
    pub enable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StarterManual {
    pub title: String,
    pub detail: String,
}

/// Which shelf of the gallery a starter is on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum StarterKind {
    /// A whole server's job: several stacks, firewall, checklist.
    #[default]
    Setup,
    /// One app as a compose stack.
    App,
    /// Scheduled jobs (Nightgrid timers).
    Jobs,
    /// A firewall profile.
    Firewall,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StarterBlueprint {
    pub id: String,
    #[serde(default)]
    pub kind: StarterKind,
    pub title: String,
    /// One line for the card.
    pub summary: String,
    /// What it is for, in plain words.
    pub purpose: String,
    /// A Library topic id (`media`, `network`, …).
    pub category: String,
    /// Library articles (`apps/jellyfin`), the first being the main guide.
    #[serde(default)]
    pub library: Vec<String>,
    #[serde(default)]
    pub stacks: Vec<StarterStack>,
    #[serde(default)]
    pub firewall: Option<StarterFirewall>,
    #[serde(default)]
    pub timers: Vec<TimerJob>,
    #[serde(default)]
    pub manual: Vec<StarterManual>,
    /// Prompts used as `{{NAME}}` in timers and firewall entries (a disk,
    /// a unit, a network). Values are single words: no spaces or shell
    /// characters, since they end up on a command line.
    #[serde(default)]
    pub vars: Vec<StarterVar>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StarterRunbook {
    pub id: String,
    pub title: String,
    pub summary: String,
    /// Empty when Nightgrid raises no alert for it yet.
    #[serde(default)]
    pub alert_kinds: Vec<AlertKind>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub library: Vec<String>,
    /// Markdown (the part after the header).
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StarterCatalog {
    pub format: u32,
    pub blueprints: Vec<StarterBlueprint>,
    pub runbooks: Vec<StarterRunbook>,
    /// App settings ready to add (alert rules, tiles, dashboards, log searches).
    pub presets: Vec<crate::starter_presets::StarterPreset>,
}

// ---- the bundled set -----------------------------------------------------------

macro_rules! bundled {
    ($dir:literal, $ext:literal: $($id:literal),* $(,)?) => {
        &[$(($id, include_str!(concat!("../../starters/", $dir, "/", $id, $ext)))),*]
    };
}

/// File name (without extension) and text of each bundled starter blueprint.
pub const BLUEPRINT_FILES: &[(&str, &str)] = bundled!("blueprints", ".yaml":
    "media-server",
    "reverse-proxy",
    "dns-filter",
    "file-sync",
    "monitoring-basics",
    "home-automation",
    "game-server",
    "hardened-docker-host",
    "backup-target",
    "app-actual",
    "app-audiobookshelf",
    "app-forgejo",
    "app-freshrss",
    "app-homepage",
    "app-immich",
    "app-mealie",
    "app-navidrome",
    "app-nextcloud",
    "app-paperless",
    "app-uptime-kuma",
    "app-vaultwarden",
    "fw-docker-host",
    "fw-lan-only",
    "fw-ssh-only",
    "fw-web-server",
    "job-apt-upgrade",
    "job-backup-kickoff",
    "job-docker-prune",
    "job-fstrim",
    "job-smart-test",
    "job-zfs-scrub",
);

/// File name (without extension) and text of each bundled starter runbook.
pub const RUNBOOK_FILES: &[(&str, &str)] = bundled!("runbooks", ".md":
    "disk-full",
    "container-restart-loop",
    "failed-systemd-unit",
    "cert-expiring",
    "host-unreachable",
    "high-memory",
    "failed-backup",
);

fn bad(field: &'static str, reason: impl Into<String>) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

/// A YAML document as JSON, so serde can read it.
fn to_json(y: &Yaml) -> std::result::Result<serde_json::Value, String> {
    use serde_json::Value;
    Ok(match y {
        Yaml::Null | Yaml::BadValue => Value::Null,
        Yaml::Boolean(b) => Value::Bool(*b),
        Yaml::Integer(i) => Value::from(*i),
        Yaml::Real(r) => r.parse::<f64>().map(Value::from).unwrap_or_else(|_| Value::String(r.clone())),
        Yaml::String(s) => Value::String(s.clone()),
        Yaml::Array(a) => Value::Array(a.iter().map(to_json).collect::<std::result::Result<_, _>>()?),
        Yaml::Hash(h) => {
            let mut m = serde_json::Map::new();
            for (k, v) in h {
                let key = match k {
                    Yaml::String(s) => s.clone(),
                    Yaml::Integer(i) => i.to_string(),
                    _ => return Err("keys must be text".into()),
                };
                m.insert(key, to_json(v)?);
            }
            Value::Object(m)
        }
        Yaml::Alias(_) => return Err("aliases aren't supported".into()),
    })
}

pub(crate) fn parse_yaml<T: serde::de::DeserializeOwned>(text: &str) -> std::result::Result<T, String> {
    let docs = YamlLoader::load_from_str(text).map_err(|e| e.to_string())?;
    let doc = docs.first().ok_or("empty file")?;
    serde_json::from_value(to_json(doc)?).map_err(|e| e.to_string())
}

/// Parses one starter blueprint file and checks it.
pub fn parse_blueprint(text: &str) -> Result<StarterBlueprint> {
    let s: StarterBlueprint = parse_yaml(text).map_err(|e| bad("starter", e))?;
    check_blueprint(&s)?;
    Ok(s)
}

/// Parses one starter runbook: a YAML header between `---` lines, then Markdown.
pub fn parse_runbook(text: &str) -> Result<StarterRunbook> {
    let text = text.replace("\r\n", "\n");
    let rest = text.strip_prefix("---\n").ok_or_else(|| bad("runbook", "starts with a --- header"))?;
    let (head, body) = rest.split_once("\n---\n").ok_or_else(|| bad("runbook", "the header has no closing ---"))?;
    let mut r: StarterRunbook = parse_yaml(head).map_err(|e| bad("runbook", e))?;
    r.body = body.trim().to_owned();
    check_runbook(&r)?;
    Ok(r)
}

/// The starters that ship with the app. A file that fails its checks is
/// left out (the bundled set is tested, so this only guards a future pack).
pub fn bundled() -> StarterCatalog {
    StarterCatalog {
        format: FORMAT,
        blueprints: BLUEPRINT_FILES.iter().filter_map(|(_, t)| parse_blueprint(t).ok()).collect(),
        runbooks: RUNBOOK_FILES.iter().filter_map(|(_, t)| parse_runbook(t).ok()).collect(),
        presets: crate::starter_presets::bundled(),
    }
}

// ---- checks -----------------------------------------------------------------------

pub(crate) fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !s.starts_with('-')
}

/// `apps/jellyfin`: a Library article id.
pub fn valid_article(s: &str) -> bool {
    s.split_once('/').is_some_and(|(shelf, slug)| valid_id(shelf) && valid_id(slug))
}

/// `8096/tcp`, `60000-61000/udp`.
pub fn valid_port(s: &str) -> bool {
    let Some((p, proto)) = s.split_once('/') else { return false };
    matches!(proto, "tcp" | "udp")
        && !p.is_empty()
        && p.split('-').count() <= 2
        && p.split('-').all(|n| n.parse::<u16>().is_ok_and(|v| v > 0))
}

/// A path inside a stack folder: relative, no `..`, plain characters.
fn valid_rel_path(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 200
        && !p.starts_with('/')
        && p.split('/').all(|seg| !seg.is_empty() && seg != "." && seg != "..")
        && p.chars().all(|c| c.is_ascii_alphanumeric() || "._-/".contains(c))
}

/// `${NAME}`, `${NAME:-x}`, `${NAME?err}` and `$NAME` references in compose text.
pub fn compose_vars(text: &str) -> BTreeSet<String> {
    let b = text.as_bytes();
    let mut out = BTreeSet::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'$' {
            if b.get(i + 1) == Some(&b'$') {
                i += 2;
                continue;
            }
            let braced = b.get(i + 1) == Some(&b'{');
            let start = if braced { i + 2 } else { i + 1 };
            let mut j = start;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            if j > start && (b[start].is_ascii_alphabetic() || b[start] == b'_') {
                out.insert(text[start..j].to_owned());
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    out
}

fn check_blueprint(s: &StarterBlueprint) -> Result<()> {
    if !valid_id(&s.id) {
        return Err(bad("id", "lowercase letters, digits and -"));
    }
    if s.title.trim().is_empty() || s.summary.trim().is_empty() || s.purpose.trim().is_empty() {
        return Err(bad("starter", format!("{}: title, summary and purpose are needed", s.id)));
    }
    if let Some(a) = s.library.iter().find(|a| !valid_article(a)) {
        return Err(bad("library", format!("{}: {a} isn't an article id", s.id)));
    }
    let mut names = BTreeSet::new();
    for st in &s.stacks {
        if !valid_project(&st.name) || !names.insert(st.name.clone()) {
            return Err(bad("stack", format!("{}: stack name {:?}", s.id, st.name)));
        }
        let docs =
            YamlLoader::load_from_str(&st.compose).map_err(|e| bad("compose", format!("{}/{}: {e}", s.id, st.name)))?;
        if !docs.first().is_some_and(|d| matches!(d["services"], Yaml::Hash(_))) {
            return Err(bad("compose", format!("{}/{}: no services", s.id, st.name)));
        }
        let mut vars = BTreeSet::new();
        for v in &st.env {
            if !envvault::valid_name(&v.name) || !vars.insert(v.name.clone()) {
                return Err(bad("env", format!("{}/{}: variable {:?}", s.id, st.name, v.name)));
            }
            if v.label.trim().is_empty() {
                return Err(bad("env", format!("{}: {} has no label", s.id, v.name)));
            }
            if v.default.contains(|c: char| c.is_control()) {
                return Err(bad("env", format!("{}: {} default", s.id, v.name)));
            }
        }
        if let Some(f) =
            st.files.iter().find(|f| !valid_rel_path(&f.path) || f.path == "compose.yaml" || f.path == ".env")
        {
            return Err(bad("files", format!("{}/{}: file path {:?}", s.id, st.name, f.path)));
        }
        if let Some(p) = st.ports.iter().find(|p| !valid_port(&p.port)) {
            return Err(bad("ports", format!("{}: port {:?}", s.id, p.port)));
        }
    }
    let mut names = BTreeSet::new();
    for v in &s.vars {
        if !envvault::valid_name(&v.name) || !names.insert(v.name.clone()) || v.label.trim().is_empty() || v.secret {
            return Err(bad("vars", format!("{}: variable {:?} (named, labelled, never secret)", s.id, v.name)));
        }
        if !v.default.is_empty() && !valid_word(&v.default) {
            return Err(bad("vars", format!("{}: {} default must be one plain word", s.id, v.name)));
        }
    }
    let used: BTreeSet<String> = templated(s).iter().flat_map(|t| template_vars(t)).collect();
    if used != names {
        return Err(bad("vars", format!("{}: {{{{VARS}}}} used {used:?} but declared {names:?}", s.id)));
    }
    // Check the shape with each variable's default (or a stand-in).
    let sample: Vec<StarterValue> =
        s.vars.iter().map(|v| StarterValue { key: format!("vars/{}", v.name), value: sample_value(v) }).collect();
    let (timers, fw) = fill(s, &sample)?;
    if let Some(fw) = &fw {
        check_firewall(fw).map_err(|e| bad("firewall", format!("{}: {e}", s.id)))?;
    }
    for t in &timers {
        t.normalized()?;
    }
    Ok(())
}

fn check_firewall(fw: &StarterFirewall) -> std::result::Result<(), String> {
    if let Some(p) = fw.allow.iter().chain(&fw.limit).find(|p| !valid_port(p)) {
        return Err(format!("{p} isn't a port such as 8080/tcp"));
    }
    if let Some(a) = fw.allow_from.iter().find(|a| !valid_source(a)) {
        return Err(format!("{a} isn't an address or network such as 192.168.1.0/24"));
    }
    Ok(())
}

fn sample_value(v: &StarterVar) -> String {
    if !v.default.is_empty() {
        v.default.clone()
    } else if v.path {
        "/srv/x".into()
    } else {
        "x".into()
    }
}

/// A value that can go into a command line or firewall rule as it is.
pub fn valid_word(s: &str) -> bool {
    !s.is_empty() && s.len() <= 200 && s.chars().all(|c| c.is_ascii_alphanumeric() || "/._:@+-".contains(c))
}

/// An IPv4/IPv6 address or network (`192.168.1.0/24`).
pub fn valid_source(s: &str) -> bool {
    let (ip, bits) = s.split_once('/').map_or((s, None), |(a, b)| (a, Some(b)));
    let Ok(addr) = ip.parse::<std::net::IpAddr>() else { return false };
    let max = if addr.is_ipv4() { 32 } else { 128 };
    bits.map_or(true, |b| b.parse::<u8>().is_ok_and(|n| n <= max))
}

/// The text fields of a starter that may hold `{{NAME}}`.
fn templated(s: &StarterBlueprint) -> Vec<&String> {
    let mut v: Vec<&String> = Vec::new();
    for t in &s.timers {
        v.extend([&t.name, &t.command, &t.calendar]);
    }
    if let Some(f) = &s.firewall {
        v.extend(f.allow.iter().chain(&f.limit).chain(&f.allow_from));
    }
    v
}

/// `{{NAME}}` placeholders in a text.
pub fn template_vars(t: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = t;
    while let Some(i) = rest.find("{{") {
        let after = &rest[i + 2..];
        let Some(j) = after.find("}}") else { break };
        out.insert(after[..j].trim().to_owned());
        rest = &after[j + 2..];
    }
    out
}

fn substitute(t: &str, vals: &[(String, String)]) -> String {
    let mut out = t.to_owned();
    for (n, v) in vals {
        out = out.replace(&format!("{{{{{n}}}}}"), v);
    }
    out
}

type Filled = (Vec<TimerJob>, Option<StarterFirewall>);

/// The starter's timers and firewall with its `{{vars}}` filled in.
fn fill(s: &StarterBlueprint, values: &[StarterValue]) -> Result<Filled> {
    let mut vals = Vec::new();
    for v in &s.vars {
        let key = format!("vars/{}", v.name);
        let typed = values.iter().find(|x| x.key == key).map(|x| x.value.as_str());
        let value = resolve_value(v, typed)?;
        if !valid_word(&value) {
            return Err(bad("value", format!("“{}” must be one word: letters, digits and / . _ : @ + -", v.label)));
        }
        vals.push((v.name.clone(), value));
    }
    let timers = s
        .timers
        .iter()
        .map(|t| TimerJob {
            name: substitute(&t.name, &vals),
            command: substitute(&t.command, &vals),
            calendar: substitute(&t.calendar, &vals),
            run_as: t.run_as.clone(),
            persistent: t.persistent,
        })
        .collect();
    let fw = s.firewall.as_ref().map(|f| StarterFirewall {
        allow: f.allow.iter().map(|x| substitute(x, &vals)).collect(),
        allow_from: f.allow_from.iter().map(|x| substitute(x, &vals)).collect(),
        limit: f.limit.iter().map(|x| substitute(x, &vals)).collect(),
        deny_incoming: f.deny_incoming,
        enable: f.enable,
    });
    Ok((timers, fw))
}

fn check_runbook(r: &StarterRunbook) -> Result<()> {
    if !valid_id(&r.id) {
        return Err(bad("id", "lowercase letters, digits and -"));
    }
    if r.title.trim().is_empty() || r.summary.trim().is_empty() || r.body.is_empty() {
        return Err(bad("runbook", format!("{}: title, summary and text are needed", r.id)));
    }
    if let Some(a) = r.library.iter().find(|a| !valid_article(a)) {
        return Err(bad("library", format!("{}: {a} isn't an article id", r.id)));
    }
    // It becomes a note, so it has to pass the note's checks.
    crate::notes::normalize_note(&crate::notes::NoteInput {
        title: r.title.clone(),
        body: r.body.clone(),
        tags: r.tags.clone(),
        runbook: true,
        alert_kinds: r.alert_kinds.clone(),
        ..Default::default()
    })?;
    Ok(())
}

// ---- building a blueprint ----------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StarterValue {
    /// `stack/NAME`.
    pub key: String,
    pub value: String,
}

/// A folder for stacks: absolute, plain, no `..`.
pub fn valid_parent(p: &str) -> bool {
    p.starts_with('/')
        && p.len() <= 512
        && !p.split('/').any(|s| s == "..")
        && !p.chars().any(|c| c.is_control() || "'\"`$\\".contains(c))
}

/// A random value for a generated secret: letters and digits only, so it
/// needs no quoting in a `.env` file or a URL.
pub fn generate_secret() -> String {
    const ABC: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    let mut out = String::with_capacity(GENERATED_LEN);
    let mut buf = [0u8; 64];
    while out.len() < GENERATED_LEN {
        getrandom::fill(&mut buf).expect("system randomness");
        // Rejection sampling keeps every character equally likely.
        for b in buf {
            let limit = 256 - 256 % ABC.len();
            if (b as usize) < limit && out.len() < GENERATED_LEN {
                out.push(ABC[b as usize % ABC.len()] as char);
            }
        }
    }
    out
}

/// The value for one variable: what was typed, else the default, else a
/// generated one when allowed. Checked for what a `.env` line can hold.
pub fn resolve_value(v: &StarterVar, typed: Option<&str>) -> Result<String> {
    let typed = typed.map(str::trim).unwrap_or("");
    let value = if !typed.is_empty() {
        typed.to_owned()
    } else if !v.default.is_empty() {
        v.default.clone()
    } else if v.generate {
        generate_secret()
    } else {
        return Err(bad("value", format!("fill in “{}”", v.label)));
    };
    if value.len() > MAX_VALUE || value.chars().any(char::is_control) {
        return Err(bad("value", format!("“{}” must be one line, up to {MAX_VALUE} characters", v.label)));
    }
    if v.path && !valid_parent(&value) {
        return Err(bad("value", format!("“{}” must be an absolute folder such as /srv/data", v.label)));
    }
    Ok(value)
}

fn rule_port(rule: &str) -> &str {
    rule.rsplit(' ').next().unwrap_or(rule)
}

/// Turns a starter into a blueprint for stacks under `parent` on the host.
/// `values` hold what the user typed, keyed `stack/NAME`.
pub fn build(s: &StarterBlueprint, values: &[StarterValue], parent: &str, now: i64) -> Result<Blueprint> {
    let parent = parent.trim();
    let parent = if parent.len() > 1 { parent.trim_end_matches('/') } else { parent };
    if !s.stacks.is_empty() && !valid_parent(parent) {
        return Err(bad("folder", "use an absolute folder such as /opt/stacks"));
    }
    let mut bp = Blueprint::new(&format!("starter: {}", s.title), now);
    for st in &s.stacks {
        let dir = format!("{}/{}", if parent == "/" { "" } else { parent }, st.name);
        let mut files = vec![BpFile { path: format!("{dir}/compose.yaml"), text: st.compose.clone() }];
        files.extend(st.files.iter().map(|f| BpFile { path: format!("{dir}/{}", f.path), text: f.text.clone() }));
        let mut env = Vec::new();
        for v in &st.env {
            let key = format!("{}/{}", st.name, v.name);
            let typed = values.iter().find(|x| x.key == key).map(|x| x.value.as_str());
            env.push(BpEnv { name: v.name.clone(), value: resolve_value(v, typed)?, secret: v.secret });
        }
        bp.stacks.push(BpStack { name: st.name.clone(), dir, files, env });
    }
    let (timers, fw) = fill(s, values)?;
    if let Some(fw) = &fw {
        check_firewall(fw).map_err(|e| bad("value", e))?;
        let rules: Vec<String> = fw
            .allow_from
            .iter()
            .map(|a| format!("allow from {a}"))
            .chain(fw.allow.iter().map(|p| format!("allow {p}")))
            .chain(fw.limit.iter().map(|p| format!("limit {p}")))
            .collect();
        bp.firewall = Some(BpFirewall {
            backend: ANY_FIREWALL.into(),
            active: fw.enable,
            ufw_rules: rules,
            default_in: fw.deny_incoming.then(|| "deny".to_owned()),
            zones: Vec::new(),
        });
    }
    bp.timers = timers.iter().map(TimerJob::normalized).collect::<Result<_>>()?;
    bp.manual = s.manual.iter().map(|m| BpManual { title: m.title.clone(), detail: m.detail.clone() }).collect();
    Ok(bp)
}

/// A starter's firewall is written for whichever firewall the target uses.
pub const ANY_FIREWALL: &str = "any";

/// Fits a starter's firewall to the target's: firewalld gets the ports in
/// its default zone, anything else keeps the ufw rules (and so becomes a
/// step done by hand where ufw isn't used).
pub fn fit_firewall(bp: &mut Blueprint, target: Option<&BpFirewall>, default_zone: Option<&str>) {
    let Some(f) = bp.firewall.as_mut().filter(|f| f.backend == ANY_FIREWALL) else { return };
    if target.is_some_and(|t| t.backend == "firewalld") {
        let mut ports: Vec<String> = Vec::new();
        let mut sources: Vec<String> = Vec::new();
        for r in &f.ufw_rules {
            if let Some(src) = r.strip_prefix("allow from ") {
                sources.push(src.to_owned());
            } else if !ports.contains(&rule_port(r).to_owned()) {
                ports.push(rule_port(r).to_owned());
            }
        }
        // Trusting a network is a zone change: done by hand, behind the rollback.
        let manual: Vec<BpManual> = sources
            .into_iter()
            .map(|src| BpManual {
                title: format!("Trust {src}"),
                detail: format!(
                    "firewalld: add {src} as a source of the trusted zone in System → FIREWALL \
                     (firewall-cmd --permanent --zone=trusted --add-source={src})"
                ),
            })
            .collect();
        f.backend = "firewalld".into();
        f.zones = vec![BpZone { name: default_zone.unwrap_or("public").to_owned(), services: Vec::new(), ports }];
        f.ufw_rules.clear();
        f.default_in = None;
        bp.manual.extend(manual);
    } else {
        f.backend = "ufw".into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprint::{plan, BpStepKind, BpStepState, BpTarget};

    fn repo(p: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(p)
    }

    fn files_in(dir: &str, ext: &str) -> BTreeSet<String> {
        std::fs::read_dir(repo(dir))
            .unwrap()
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter_map(|n| n.strip_suffix(ext).map(str::to_owned))
            .collect()
    }

    #[test]
    fn every_file_is_bundled_and_parses() {
        let listed: BTreeSet<String> = BLUEPRINT_FILES.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(listed, files_in("starters/blueprints", ".yaml"), "list each file in BLUEPRINT_FILES");
        let listed: BTreeSet<String> = RUNBOOK_FILES.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(listed, files_in("starters/runbooks", ".md"), "list each file in RUNBOOK_FILES");
        for (name, text) in BLUEPRINT_FILES {
            let s = parse_blueprint(text).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(&s.id, name, "id matches the file name");
        }
        for (name, text) in RUNBOOK_FILES {
            let r = parse_runbook(text).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(&r.id, name, "id matches the file name");
        }
        let c = bundled();
        assert_eq!(c.blueprints.len(), BLUEPRINT_FILES.len());
        assert_eq!(c.runbooks.len(), RUNBOOK_FILES.len());
    }

    #[test]
    fn library_links_point_at_real_articles() {
        let c = bundled();
        let mut ids: Vec<(String, String)> = Vec::new();
        for s in &c.blueprints {
            assert!(!s.library.is_empty(), "{} links a Library guide", s.id);
            ids.extend(s.library.iter().map(|a| (s.id.clone(), a.clone())));
        }
        for r in &c.runbooks {
            assert!(!r.library.is_empty(), "{} links a Library guide", r.id);
            ids.extend(r.library.iter().map(|a| (r.id.clone(), a.clone())));
            // kb: links inside the text too.
            for part in r.body.split("(kb:").skip(1) {
                let id = part.split(')').next().unwrap();
                ids.push((r.id.clone(), id.to_owned()));
            }
        }
        for (who, a) in ids {
            assert!(repo(&format!("docs/kb/{a}.md")).exists(), "{who}: no Library article {a}");
        }
    }

    #[test]
    fn variables_match_the_compose_files_and_hold_no_secrets() {
        for s in bundled().blueprints {
            for st in &s.stacks {
                let used = compose_vars(&st.compose);
                let declared: BTreeSet<String> = st.env.iter().map(|v| v.name.clone()).collect();
                assert_eq!(used, declared, "{}/{}: ${{VARS}} in compose vs env prompts", s.id, st.name);
                for v in &st.env {
                    // The vault decides "secret" from the name; keep them in step.
                    assert_eq!(v.secret, envvault::looks_secret(&v.name), "{}: {} secret flag", s.id, v.name);
                    if v.secret {
                        assert!(v.default.is_empty(), "{}: {} must not ship a default secret", s.id, v.name);
                    }
                }
                for f in std::iter::once(&st.compose).chain(st.files.iter().map(|f| &f.text)) {
                    assert!(crate::redact::secret_hint(f).is_none(), "{}: a file looks like it holds a secret", s.id);
                }
                for p in &st.ports {
                    let public = !p.local;
                    let allowed = s.firewall.as_ref().is_some_and(|f| f.allow.contains(&p.port));
                    assert_eq!(public, allowed, "{}: port {} vs firewall allow list", s.id, p.port);
                }
            }
        }
    }

    #[test]
    fn compose_vars_reads_the_forms_compose_knows() {
        let v = compose_vars("a: ${TZ}\nb: ${PORT:-80}\nc: $$NOT\nd: $HOME_DIR/x\ne: ${X?missing}");
        assert_eq!(v.into_iter().collect::<Vec<_>>(), ["HOME_DIR", "PORT", "TZ", "X"]);
    }

    fn sample() -> StarterBlueprint {
        parse_blueprint(
            r#"
id: demo
title: Demo
summary: one line
purpose: what for
category: start
library: [apps/jellyfin]
stacks:
  - name: demo
    compose: |
      services:
        app:
          image: example/app
          environment:
            TZ: ${TZ}
            DB_PASSWORD: ${DB_PASSWORD}
          volumes:
            - ${DATA_DIR}:/data
    files:
      - path: conf/app.conf
        text: "x = 1\n"
    env:
      - { name: TZ, label: Time zone, default: Etc/UTC }
      - { name: DATA_DIR, label: Data folder, path: true }
      - { name: DB_PASSWORD, label: Database password, secret: true, generate: true }
    ports:
      - { port: 8080/tcp, what: the app }
firewall:
  allow: [8080/tcp]
  limit: [22/tcp]
  denyIncoming: true
  enable: true
timers:
  - { name: prune, command: docker image prune -af, calendar: "Sun *-*-* 04:00:00", runAs: null, persistent: true }
manual:
  - { title: Point DNS here, detail: at your router }
"#,
        )
        .unwrap()
    }

    #[test]
    fn build_fills_values_and_generates_secrets() {
        let s = sample();
        let missing = build(&s, &[], "/opt/stacks", 0).unwrap_err();
        assert!(missing.to_string().contains("Data folder"), "{missing}");
        let rel = [StarterValue { key: "demo/DATA_DIR".into(), value: "srv".into() }];
        assert!(build(&s, &rel, "/opt/stacks", 0).is_err(), "a path must be absolute");
        assert!(build(&s, &[StarterValue { key: "demo/DATA_DIR".into(), value: "/srv".into() }], "stacks", 0).is_err());

        let vals = [
            StarterValue { key: "demo/DATA_DIR".into(), value: " /srv/demo ".into() },
            StarterValue { key: "demo/TZ".into(), value: "Europe/Stockholm".into() },
        ];
        let bp = build(&s, &vals, "/opt/stacks/", 7).unwrap();
        let st = &bp.stacks[0];
        assert_eq!(st.dir, "/opt/stacks/demo");
        let paths: Vec<&str> = st.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["/opt/stacks/demo/compose.yaml", "/opt/stacks/demo/conf/app.conf"]);
        let get = |n: &str| st.env.iter().find(|e| e.name == n).unwrap().value.clone();
        assert_eq!(get("TZ"), "Europe/Stockholm");
        assert_eq!(get("DATA_DIR"), "/srv/demo");
        let pw = get("DB_PASSWORD");
        assert_eq!(pw.len(), GENERATED_LEN);
        assert!(pw.chars().all(|c| c.is_ascii_alphanumeric()));
        assert_ne!(pw, build(&s, &vals, "/opt/stacks", 7).unwrap().stacks[0].env[2].value, "generated afresh");
        assert_eq!(bp.timers.len(), 1);
        assert_eq!(bp.manual.len(), 1);
        let line = [StarterValue { key: "demo/TZ".into(), value: "a\nb".into() }, vals[0].clone()];
        assert!(build(&s, &line, "/opt/stacks", 0).is_err(), "one line only");
    }

    #[test]
    fn firewall_fits_the_target_and_never_denies_on_its_own() {
        let s = sample();
        let vals = [StarterValue { key: "demo/DATA_DIR".into(), value: "/srv".into() }];
        let ufw = BpFirewall {
            backend: "ufw".into(),
            active: false,
            ufw_rules: vec!["allow 22/tcp".into()],
            default_in: Some("allow".into()),
            zones: vec![],
        };
        let mut bp = build(&s, &vals, "/opt/stacks", 0).unwrap();
        fit_firewall(&mut bp, Some(&ufw), None);
        let t = BpTarget { docker: true, firewall: Some(ufw), ..Default::default() };
        let steps = plan(&bp, &t);
        let fw: Vec<_> = steps.iter().filter(|s| s.kind == BpStepKind::Firewall).collect();
        assert!(fw.iter().any(|s| s.id == "fw:allow 8080/tcp" && s.state == BpStepState::New));
        assert!(fw.iter().any(|s| s.id == "fw:limit 22/tcp" && s.state == BpStepState::New));
        assert!(fw.iter().any(|s| s.id == "fw:default" && s.state == BpStepState::Manual));
        assert!(fw.iter().any(|s| s.id == "fw:enable" && s.state == BpStepState::Manual));
        let manual = steps.iter().find(|s| s.kind == BpStepKind::Manual).unwrap();
        assert_eq!((manual.state, manual.default_on), (BpStepState::Manual, false));

        let fwd = BpFirewall {
            backend: "firewalld".into(),
            active: true,
            ufw_rules: vec![],
            default_in: None,
            zones: vec![BpZone { name: "home".into(), services: vec!["ssh".into()], ports: vec![] }],
        };
        let mut bp = build(&s, &vals, "/opt/stacks", 0).unwrap();
        fit_firewall(&mut bp, Some(&fwd), Some("home"));
        let f = bp.firewall.as_ref().unwrap();
        assert_eq!(f.backend, "firewalld");
        assert_eq!(f.zones[0].name, "home");
        assert_eq!(f.zones[0].ports, ["8080/tcp", "22/tcp"]);
        let steps = plan(&bp, &BpTarget { docker: true, firewall: Some(fwd), ..Default::default() });
        assert!(steps.iter().any(|s| s.id == "fw:zone:home" && s.state == BpStepState::New));

        let mut bp = build(&s, &vals, "/opt/stacks", 0).unwrap();
        fit_firewall(&mut bp, None, None);
        let steps = plan(&bp, &BpTarget { docker: true, ..Default::default() });
        assert!(steps.iter().any(|s| s.id == "fw:backend" && s.state == BpStepState::Manual));
    }

    #[test]
    fn vars_fill_timers_and_firewall_and_stay_one_word() {
        let s = parse_blueprint(
            r#"
id: v
kind: jobs
title: V
summary: s
purpose: p
category: storage
library: [howto/backups]
vars:
  - { name: DISK, label: Disk, default: /dev/sda }
  - { name: LAN, label: Network, default: 192.168.0.0/16 }
timers:
  - { name: "smart-{{DISK}}", command: "smartctl -t short {{DISK}}", calendar: "Sun *-*-* 02:00:00", persistent: true }
firewall:
  allowFrom: ["{{LAN}}"]
"#,
        )
        .unwrap();
        assert_eq!(s.kind, StarterKind::Jobs);
        let bp = build(&s, &[], "/unused", 0).unwrap();
        assert_eq!(bp.timers[0].command, "smartctl -t short /dev/sda");
        let fw = bp.firewall.as_ref().unwrap();
        assert_eq!(fw.ufw_rules, ["allow from 192.168.0.0/16"]);
        assert!(crate::blueprint::ufw_add_command(&fw.ufw_rules[0]).is_some(), "an allow-from rule can be added");
        let typed = |k: &str, v: &str| vec![StarterValue { key: format!("vars/{k}"), value: v.into() }];
        let disk = build(&s, &typed("DISK", "/dev/disk/by-id/ata-X_1"), "/", 0).unwrap();
        assert_eq!(disk.timers[0].command, "smartctl -t short /dev/disk/by-id/ata-X_1");
        assert!(build(&s, &typed("DISK", "/dev/sda; rm -rf /"), "/", 0).is_err(), "no shell through a var");
        assert!(build(&s, &typed("LAN", "300.1.1.1/8"), "/", 0).is_err(), "not a network");
        assert!(build(&s, &typed("LAN", "10.0.0.0/33"), "/", 0).is_err());

        // firewalld can't take a source rule from here: it becomes a step by hand.
        let mut bp = build(&s, &[], "/", 0).unwrap();
        let fwd = BpFirewall {
            backend: "firewalld".into(),
            active: true,
            ufw_rules: vec![],
            default_in: None,
            zones: vec![],
        };
        fit_firewall(&mut bp, Some(&fwd), Some("public"));
        assert!(bp.firewall.as_ref().unwrap().zones[0].ports.is_empty());
        assert!(bp.manual.iter().any(|m| m.title == "Trust 192.168.0.0/16"));

        let undeclared = "id: u\ntitle: U\nsummary: s\npurpose: p\ncategory: start\ntimers:\n  - { name: x, command: \"echo {{NOPE}}\", calendar: daily, persistent: false }\n";
        assert!(parse_blueprint(undeclared).is_err(), "every {{VAR}} needs a prompt");
    }

    #[test]
    fn bad_starters_are_refused() {
        let base = "id: x\ntitle: X\nsummary: s\npurpose: p\ncategory: start\n";
        assert!(parse_blueprint(&format!("{base}surprise: 1\n")).is_err(), "unknown keys");
        assert!(parse_blueprint(&format!("{base}library: [nope]\n")).is_err());
        let traversal = format!(
            "{base}stacks:\n  - name: a\n    compose: \"services: {{a: {{image: x}}}}\"\n    files:\n      - {{ path: ../etc/passwd, text: x }}\n"
        );
        assert!(parse_blueprint(&traversal).is_err(), "files stay in the stack folder");
        let rb = "---\nid: r\ntitle: R\nsummary: s\nalertKinds: [diskFull]\n---\nDB_PASSWORD=hunter2hunter2\n";
        assert!(parse_runbook(rb).is_err(), "a runbook can't carry a secret");
        let ok = parse_runbook("---\nid: r\ntitle: R\nsummary: s\nalertKinds: [diskFull]\n---\n\n# Hi\n").unwrap();
        assert_eq!(ok.body, "# Hi");
        assert_eq!(ok.alert_kinds, [AlertKind::DiskFull]);
    }
}
