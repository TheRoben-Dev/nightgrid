//! Lab blueprints (M12b): a host's setup as one encrypted file, and a plan
//! to rebuild it on another (or a fresh) machine.
//!
//! A blueprint holds what makes a host *that* host and can be put back
//! without guessing: compose stacks (their compose files and `.env`, plus
//! the `.env` vault's values for them), crontabs, Nightgrid's systemd
//! timers, the firewall's rules, and reverse-proxy routes (a Caddyfile,
//! Nightgrid's Traefik routes). It is age-encrypted with a passphrase like a
//! profile, and never written anywhere unencrypted.
//!
//! Rebuilding is a plan, not a script: the target host is read first, each
//! piece becomes a step marked new, already there, different or by hand,
//! and only the steps the user ticks are applied, each through the same
//! code the editors use. Nothing is ever deleted on the target. Firewall
//! steps only *add* allow rules (they can't lock anyone out); deny rules,
//! default policies and turning a firewall on stay manual, where System →
//! FIREWALL applies them behind its rollback.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::docker::proxy_edit::TraefikRoute;
use crate::files::path::shell_quote;
use crate::system::jobs::TimerJob;
use crate::{Error, Result};

pub const FORMAT: &str = "nightgrid-blueprint";
pub const VERSION: u32 = 1;
/// A blueprint file is refused above this size.
pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
/// One stack file is left out above this size.
pub const MAX_STACK_FILE: usize = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BpFile {
    /// Absolute path on the source host.
    pub path: String,
    pub text: String,
}

impl Drop for BpFile {
    fn drop(&mut self) {
        // `.env` files hold secrets.
        self.text.zeroize();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BpEnv {
    pub name: String,
    pub value: String,
    pub secret: bool,
}

impl Drop for BpEnv {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BpStack {
    pub name: String,
    pub dir: String,
    /// Compose files (and `.env` when the vault has nothing for the stack).
    pub files: Vec<BpFile>,
    /// The `.env` vault's values for the stack, in order.
    pub env: Vec<BpEnv>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BpCrontab {
    /// `None`: the login user's.
    pub user: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BpZone {
    pub name: String,
    pub services: Vec<String>,
    /// `8080/tcp`, `60000-61000/udp`.
    pub ports: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BpFirewall {
    /// `ufw` or `firewalld`.
    pub backend: String,
    pub active: bool,
    /// ufw rule specs as `ufw show added` prints them.
    pub ufw_rules: Vec<String>,
    pub default_in: Option<String>,
    pub zones: Vec<BpZone>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BpCaddy {
    /// `host` or the Caddy container's name.
    pub proxy_name: String,
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BpTraefik {
    pub proxy_name: String,
    pub routes: Vec<TraefikRoute>,
}

/// A step only a person can do (from a starter, D-177): shown in the plan,
/// never applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BpManual {
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blueprint {
    pub format: String,
    pub version: u32,
    /// Unix seconds.
    pub made_at: i64,
    /// The source host's name in Nightgrid.
    pub host: String,
    /// What the host said it runs (`/etc/os-release` PRETTY_NAME).
    pub os: String,
    pub stacks: Vec<BpStack>,
    pub crontabs: Vec<BpCrontab>,
    pub timers: Vec<TimerJob>,
    pub firewall: Option<BpFirewall>,
    pub caddy: Vec<BpCaddy>,
    pub traefik: Vec<BpTraefik>,
    /// What couldn't be read when it was made.
    pub notes: Vec<String>,
    /// Steps done by hand (starters only; files made from a host have none).
    #[serde(default)]
    pub manual: Vec<BpManual>,
}

impl Blueprint {
    pub fn new(host: &str, made_at: i64) -> Self {
        Blueprint {
            format: FORMAT.into(),
            version: VERSION,
            made_at,
            host: host.to_owned(),
            os: String::new(),
            stacks: Vec::new(),
            crontabs: Vec::new(),
            timers: Vec::new(),
            firewall: None,
            caddy: Vec::new(),
            traefik: Vec::new(),
            notes: Vec::new(),
            manual: Vec::new(),
        }
    }
}

/// What to put in a blueprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct BpInclude {
    /// Stack names, when not every stack.
    pub stacks: Vec<String>,
    pub all_stacks: bool,
    /// The `.env` vault's values for the stacks.
    pub env: bool,
    pub cron: bool,
    pub timers: bool,
    pub firewall: bool,
    pub proxy: bool,
}

impl Default for BpInclude {
    fn default() -> Self {
        BpInclude {
            stacks: Vec::new(),
            all_stacks: true,
            env: true,
            cron: true,
            timers: true,
            firewall: true,
            proxy: true,
        }
    }
}

impl BpInclude {
    pub fn wants_stack(&self, name: &str) -> bool {
        self.all_stacks || self.stacks.iter().any(|s| s == name)
    }
}

// ---- encryption -------------------------------------------------------------

fn bad(reason: impl Into<String>) -> Error {
    Error::Invalid { field: "blueprint", reason: reason.into() }
}

/// Serializes and encrypts with an age passphrase.
pub fn encrypt(bp: &Blueprint, pass: &str) -> Result<Vec<u8>> {
    encrypt_with(bp, pass, None)
}

fn encrypt_with(bp: &Blueprint, pass: &str, work_factor: Option<u8>) -> Result<Vec<u8>> {
    if pass.chars().count() < crate::backup::MIN_PASSPHRASE {
        return Err(Error::Invalid {
            field: "passphrase",
            reason: format!("use at least {} characters", crate::backup::MIN_PASSPHRASE),
        });
    }
    let json = Zeroizing::new(serde_json::to_vec(bp)?);
    let mut r = age::scrypt::Recipient::new(age::secrecy::SecretString::from(pass.to_owned()));
    if let Some(n) = work_factor {
        r.set_work_factor(n);
    }
    age::encrypt(&r, &json).map_err(|e| bad(format!("couldn't encrypt: {e}")))
}

/// Decrypts and parses a blueprint file.
pub fn decrypt(bytes: &[u8], pass: &str) -> Result<Blueprint> {
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(bad("the file is too big to be a Nightgrid blueprint"));
    }
    if !bytes.starts_with(b"age-encryption.org/v1") {
        return Err(bad("not a Nightgrid blueprint (not an age file)"));
    }
    let id = age::scrypt::Identity::new(age::secrecy::SecretString::from(pass.to_owned()));
    let json = Zeroizing::new(age::decrypt(&id, bytes).map_err(|e| match e {
        age::DecryptError::DecryptionFailed | age::DecryptError::NoMatchingKeys => {
            bad("wrong passphrase, or the file isn't a blueprint")
        }
        age::DecryptError::ExcessiveWork { .. } => bad("the file asks for too much work to decrypt"),
        other => bad(format!("couldn't decrypt: {other}")),
    })?);
    let bp: Blueprint = serde_json::from_slice(&json).map_err(|_| bad("not a Nightgrid blueprint"))?;
    if bp.format != FORMAT {
        return Err(bad("not a Nightgrid blueprint (a profile or backup?)"));
    }
    if bp.version > VERSION {
        return Err(bad("made by a newer Nightgrid; update this one first"));
    }
    Ok(bp)
}

// ---- summary (no secret values) -----------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StackSummary {
    pub name: String,
    pub dir: String,
    pub files: Vec<String>,
    /// Vault variable names (values are never shown here).
    pub env: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CronSummary {
    /// "" for the login user.
    pub user: String,
    pub lines: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BlueprintSummary {
    pub host: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub made_at: i64,
    pub os: String,
    pub stacks: Vec<StackSummary>,
    pub crontabs: Vec<CronSummary>,
    pub timers: Vec<String>,
    /// `ufw`/`firewalld` and how many rules and zone entries it has.
    pub firewall: Option<String>,
    pub firewall_rules: u32,
    /// "Caddy (host): /etc/caddy/Caddyfile", "Traefik (traefik): 4 routes".
    pub proxies: Vec<String>,
    pub notes: Vec<String>,
    /// Titles of the steps done by hand.
    pub manual: Vec<String>,
}

/// Cron lines that are jobs or settings (not comments or blanks).
fn cron_lines(text: &str) -> Vec<&str> {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect()
}

impl Blueprint {
    pub fn summary(&self) -> BlueprintSummary {
        BlueprintSummary {
            host: self.host.clone(),
            made_at: self.made_at,
            os: self.os.clone(),
            stacks: self
                .stacks
                .iter()
                .map(|s| StackSummary {
                    name: s.name.clone(),
                    dir: s.dir.clone(),
                    files: s.files.iter().map(|f| f.path.clone()).collect(),
                    env: s.env.iter().map(|e| e.name.clone()).collect(),
                })
                .collect(),
            crontabs: self
                .crontabs
                .iter()
                .map(|c| CronSummary {
                    user: c.user.clone().unwrap_or_default(),
                    lines: cron_lines(&c.text).len() as u32,
                })
                .collect(),
            timers: self.timers.iter().map(|t| t.name.clone()).collect(),
            firewall: self.firewall.as_ref().map(|f| f.backend.clone()),
            firewall_rules: self.firewall.as_ref().map_or(0, |f| {
                (f.ufw_rules.len() + f.zones.iter().map(|z| z.services.len() + z.ports.len()).sum::<usize>()) as u32
            }),
            proxies: self
                .caddy
                .iter()
                .map(|c| format!("Caddy ({}): {}", c.proxy_name, c.path))
                .chain(self.traefik.iter().map(|t| format!("Traefik ({}): {} routes", t.proxy_name, t.routes.len())))
                .collect(),
            notes: self.notes.clone(),
            manual: self.manual.iter().map(|m| m.title.clone()).collect(),
        }
    }
}

// ---- planning a rebuild -----------------------------------------------------------

/// What the target host has now, read before planning.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BpTarget {
    /// Paths that exist (from the blueprint's stack files and dirs).
    pub existing: BTreeSet<String>,
    /// Stack names Docker knows on the target.
    pub stacks: BTreeSet<String>,
    /// Docker is installed and answers.
    pub docker: bool,
    /// Crontab text per user (`None`: login user); missing = couldn't read.
    pub crontabs: Vec<(Option<String>, String)>,
    /// Nightgrid timer names installed.
    pub timers: BTreeSet<String>,
    /// The firewall there (rules and zones as the blueprint would hold them).
    pub firewall: Option<BpFirewall>,
    /// Caddy targets there: proxy name → text.
    pub caddy: Vec<(String, String)>,
    /// Traefik targets there: proxy name → route names.
    pub traefik: Vec<(String, Vec<String>)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum BpStepState {
    /// Not on the target yet.
    New,
    /// Already there and the same.
    Same,
    /// There, but different: applying overwrites (off by default).
    Differs,
    /// Can't be applied from here; the detail says how.
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum BpStepKind {
    /// Write a stack's files (and its vault values).
    StackFiles,
    /// `docker compose up -d` for a stack.
    StackStart,
    Cron,
    Timer,
    Firewall,
    Caddy,
    Traefik,
    /// Done by hand (a starter's checklist item).
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BpStep {
    /// Stable within one plan: `stack:<name>`, `start:<name>`, `cron:<user>`, …
    pub id: String,
    pub kind: BpStepKind,
    pub title: String,
    pub detail: String,
    pub state: BpStepState,
    /// Ticked when the plan opens.
    pub default_on: bool,
}

fn step(id: String, kind: BpStepKind, title: String, detail: String, state: BpStepState) -> BpStep {
    let default_on = state == BpStepState::New;
    BpStep { id, kind, title, detail, state, default_on }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// The cron lines of `src` that `dst` doesn't have, in order.
pub fn missing_cron_lines(src: &str, dst: &str) -> Vec<String> {
    let have: BTreeSet<&str> = cron_lines(dst).into_iter().collect();
    cron_lines(src).into_iter().filter(|l| !have.contains(l)).map(str::to_owned).collect()
}

/// `dst` with the missing lines of `src` added: settings such as `MAILTO=`
/// go first so they apply to every job, jobs at the end.
pub fn merge_crontab(src: &str, dst: &str) -> String {
    let missing = missing_cron_lines(src, dst);
    if missing.is_empty() {
        return dst.to_owned();
    }
    let is_setting = |l: &str| {
        let head = l.split('=').next().unwrap_or("");
        l.contains('=') && !head.is_empty() && !head.contains(char::is_whitespace)
    };
    let (settings, jobs): (Vec<&String>, Vec<&String>) = missing.iter().partition(|l| is_setting(l));
    let mut out = String::new();
    for s in &settings {
        out.push_str(s);
        out.push('\n');
    }
    out.push_str(dst.trim_end_matches('\n'));
    if !dst.trim().is_empty() {
        out.push('\n');
    }
    if !jobs.is_empty() {
        out.push_str("# added by a Nightgrid blueprint\n");
        for j in jobs {
            out.push_str(j);
            out.push('\n');
        }
    }
    out
}

fn ufw_words(spec: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote = false;
    for c in spec.chars() {
        match c {
            '\'' => quote = !quote,
            c if c.is_whitespace() && !quote => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The command that adds one ufw rule from its spec, for `allow` and
/// `limit` rules only (adding those can't lock anyone out). `None` for
/// anything else or anything with unexpected characters.
pub fn ufw_add_command(spec: &str) -> Option<String> {
    let words = ufw_words(spec);
    let cut = words.iter().position(|w| w == "comment").unwrap_or(words.len());
    let (rule, comment) = words.split_at(cut);
    if !matches!(rule.first().map(String::as_str), Some("allow" | "limit")) {
        return None;
    }
    if rule.iter().any(|w| !w.chars().all(|c| c.is_ascii_alphanumeric() || "./:,_-".contains(c))) {
        return None;
    }
    let mut cmd = format!("ufw {}", rule.iter().map(|w| shell_quote(w)).collect::<Vec<_>>().join(" "));
    if let Some(c) = comment.get(1) {
        if c.len() <= 60 && c.chars().all(|ch| ch.is_ascii_alphanumeric() || " ._-".contains(ch)) {
            cmd.push_str(&format!(" comment {}", shell_quote(c)));
        }
    }
    Some(cmd)
}

fn fw_name_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
}

fn fw_port_ok(s: &str) -> bool {
    let (p, proto) = s.split_once('/').unwrap_or((s, ""));
    matches!(proto, "tcp" | "udp" | "sctp" | "dccp")
        && !p.is_empty()
        && p.split('-').all(|n| n.parse::<u16>().is_ok_and(|v| v > 0))
}

/// Adds firewalld services and ports to a zone, permanently, then reloads.
pub fn firewalld_add_command(zone: &str, services: &[String], ports: &[String]) -> Option<String> {
    if !fw_name_ok(zone) || services.iter().any(|s| !fw_name_ok(s)) || ports.iter().any(|p| !fw_port_ok(p)) {
        return None;
    }
    if services.is_empty() && ports.is_empty() {
        return None;
    }
    let mut cmd = format!("firewall-cmd -q --permanent --zone={zone}");
    for s in services {
        cmd.push_str(&format!(" --add-service={s}"));
    }
    for p in ports {
        cmd.push_str(&format!(" --add-port={p}"));
    }
    cmd.push_str(" && firewall-cmd -q --reload");
    Some(cmd)
}

/// What a zone of `bp` adds on `t`: (services, ports) not there yet.
pub fn zone_missing(z: &BpZone, t: Option<&BpZone>) -> (Vec<String>, Vec<String>) {
    let services = z.services.iter().filter(|s| t.map_or(true, |x| !x.services.contains(s))).cloned().collect();
    let ports = z.ports.iter().filter(|p| t.map_or(true, |x| !x.ports.contains(p))).cloned().collect();
    (services, ports)
}

/// A shell script that prints `@@have <path>` for each path that exists.
pub fn exists_script(paths: &[String]) -> String {
    let mut s = String::new();
    for p in paths {
        let q = shell_quote(p);
        s.push_str(&format!("[ -e {q} ] && printf '@@have %s\\n' {q}\n"));
    }
    s.push_str("echo @@end\n");
    s
}

pub fn parse_exists(out: &str) -> BTreeSet<String> {
    out.lines().filter_map(|l| l.strip_prefix("@@have ")).map(str::to_owned).collect()
}

pub fn env_path(s: &BpStack) -> String {
    format!("{}/.env", s.dir.trim_end_matches('/'))
}

/// Every path a blueprint's stacks would write, with their folders.
pub fn stack_paths(bp: &Blueprint) -> Vec<String> {
    let mut v = Vec::new();
    for s in &bp.stacks {
        v.push(s.dir.clone());
        v.extend(s.files.iter().map(|f| f.path.clone()));
        if !s.env.is_empty() {
            v.push(env_path(s));
        }
    }
    v
}

/// Creates the folders a stack's files go in.
pub fn mkdir_script(s: &BpStack) -> String {
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    dirs.insert(s.dir.trim_end_matches('/').to_owned());
    for f in &s.files {
        if let Some((d, _)) = f.path.rsplit_once('/') {
            if !d.is_empty() {
                dirs.insert(d.to_owned());
            }
        }
    }
    format!("mkdir -p {} && echo @@ok", dirs.iter().map(|d| shell_quote(d)).collect::<Vec<_>>().join(" "))
}

/// The steps to rebuild `bp` on `t`.
pub fn plan(bp: &Blueprint, t: &BpTarget) -> Vec<BpStep> {
    let mut out = Vec::new();
    for s in &bp.stacks {
        let mut paths: Vec<String> = s.files.iter().map(|f| f.path.clone()).collect();
        if !s.env.is_empty() && !paths.contains(&env_path(s)) {
            paths.push(env_path(s));
        }
        let there: Vec<&String> = paths.iter().filter(|p| t.existing.contains(*p)).collect();
        let state = if there.is_empty() { BpStepState::New } else { BpStepState::Differs };
        let mut detail = format!("{} file{} in {}", paths.len(), plural(paths.len()), s.dir);
        if !s.env.is_empty() {
            detail.push_str(&format!("; {} .env value{} into the vault", s.env.len(), plural(s.env.len())));
        }
        if !there.is_empty() {
            let names: Vec<&str> = there.iter().map(|p| p.as_str()).collect();
            detail.push_str(&format!("; overwrites {}", names.join(", ")));
        }
        out.push(step(
            format!("stack:{}", s.name),
            BpStepKind::StackFiles,
            format!("Stack {}: write files", s.name),
            detail,
            state,
        ));
        let (state, detail) = if !t.docker {
            (
                BpStepState::Manual,
                "Docker doesn't answer on the target: install it, then start the stack from Docker → STACKS",
            )
        } else if t.stacks.contains(&s.name) {
            (BpStepState::Same, "a stack with this name already runs there")
        } else {
            (BpStepState::New, "docker compose up -d (pulls the images)")
        };
        out.push(step(
            format!("start:{}", s.name),
            BpStepKind::StackStart,
            format!("Stack {}: start", s.name),
            detail.into(),
            state,
        ));
    }
    for c in &bp.crontabs {
        let who = c.user.clone().unwrap_or_default();
        let label = if who.is_empty() { "the login user's crontab".to_owned() } else { format!("{who}'s crontab") };
        let cur = t.crontabs.iter().find(|(u, _)| u.as_deref().unwrap_or("") == who).map(|(_, x)| x.as_str());
        let (state, detail) = match cur {
            None => (BpStepState::Manual, "couldn't read it on the target".to_owned()),
            Some(dst) => {
                let missing = missing_cron_lines(&c.text, dst);
                if missing.is_empty() {
                    (BpStepState::Same, "every line is there".to_owned())
                } else {
                    (
                        BpStepState::New,
                        format!("adds {} line{}; existing lines stay", missing.len(), plural(missing.len())),
                    )
                }
            }
        };
        out.push(step(format!("cron:{who}"), BpStepKind::Cron, format!("Crontab: {label}"), detail, state));
    }
    for j in &bp.timers {
        let state = if t.timers.contains(&j.name) { BpStepState::Same } else { BpStepState::New };
        let detail = format!("{} · {}", j.calendar, j.command.chars().take(60).collect::<String>());
        out.push(step(format!("timer:{}", j.name), BpStepKind::Timer, format!("Timer {}", j.name), detail, state));
    }
    if let Some(f) = &bp.firewall {
        plan_firewall(f, t.firewall.as_ref(), &mut out);
    }
    for c in &bp.caddy {
        let id = format!("caddy:{}", c.proxy_name);
        let title = format!("Caddyfile ({})", c.proxy_name);
        let Some((_, text)) = t.caddy.iter().find(|(n, _)| *n == c.proxy_name) else {
            let how = if c.proxy_name == "host" { "install Caddy" } else { "start the Caddy stack" };
            let detail = format!("no Caddy there yet: {how} first, then plan again");
            out.push(step(id, BpStepKind::Caddy, title, detail, BpStepState::Manual));
            continue;
        };
        let (state, detail) = if text.trim() == c.text.trim() {
            (BpStepState::Same, "the same Caddyfile".to_owned())
        } else if text.trim().is_empty() {
            (BpStepState::New, format!("writes {}", c.path))
        } else {
            (
                BpStepState::Differs,
                format!("replaces {} (checked with caddy validate, put back if Caddy can't load it)", c.path),
            )
        };
        out.push(step(id, BpStepKind::Caddy, title, detail, state));
    }
    for tr in &bp.traefik {
        let id = format!("traefik:{}", tr.proxy_name);
        let title = format!("Traefik routes ({})", tr.proxy_name);
        let Some((_, have)) = t.traefik.iter().find(|(n, _)| *n == tr.proxy_name) else {
            let detail = "no Traefik with this name there yet: start it first, then plan again".to_owned();
            out.push(step(id, BpStepKind::Traefik, title, detail, BpStepState::Manual));
            continue;
        };
        let missing = tr.routes.iter().filter(|r| !have.contains(&r.name)).count();
        let (state, detail) = if missing == 0 {
            (BpStepState::Same, "every route is there".to_owned())
        } else {
            (BpStepState::New, format!("adds {missing} route{}; existing routes stay", plural(missing)))
        };
        out.push(step(id, BpStepKind::Traefik, title, detail, state));
    }
    for (i, m) in bp.manual.iter().enumerate() {
        out.push(step(
            format!("manual:{i}"),
            BpStepKind::Manual,
            m.title.clone(),
            m.detail.clone(),
            BpStepState::Manual,
        ));
    }
    out
}

fn plan_firewall(f: &BpFirewall, target: Option<&BpFirewall>, out: &mut Vec<BpStep>) {
    let Some(tf) = target.filter(|x| x.backend == f.backend) else {
        out.push(step(
            "fw:backend".into(),
            BpStepKind::Firewall,
            format!("Firewall ({})", f.backend),
            format!("the target doesn't use {}: add the rules by hand in System → FIREWALL", f.backend),
            BpStepState::Manual,
        ));
        return;
    };
    let have: BTreeSet<&str> = tf.ufw_rules.iter().map(String::as_str).collect();
    for spec in &f.ufw_rules {
        let id = format!("fw:{spec}");
        let title = format!("ufw {spec}");
        let (state, detail) = if have.contains(spec.as_str()) {
            (BpStepState::Same, "already there")
        } else if ufw_add_command(spec).is_some() {
            (BpStepState::New, "adds an allow rule")
        } else {
            (BpStepState::Manual, "deny and reject rules are added by hand in System → FIREWALL, behind its rollback")
        };
        out.push(step(id, BpStepKind::Firewall, title, detail.into(), state));
    }
    for z in &f.zones {
        let (services, ports) = zone_missing(z, tf.zones.iter().find(|x| x.name == z.name));
        let id = format!("fw:zone:{}", z.name);
        let title = format!("firewalld zone {}", z.name);
        if services.is_empty() && ports.is_empty() {
            out.push(step(id, BpStepKind::Firewall, title, "already there".into(), BpStepState::Same));
        } else {
            let what = services.iter().chain(ports.iter()).cloned().collect::<Vec<_>>().join(", ");
            out.push(step(id, BpStepKind::Firewall, title, format!("adds {what}"), BpStepState::New));
        }
    }
    if f.default_in.is_some() && f.default_in != tf.default_in {
        out.push(step(
            "fw:default".into(),
            BpStepKind::Firewall,
            format!("Default incoming: {}", f.default_in.clone().unwrap_or_default()),
            "set it in System → FIREWALL, behind its rollback".into(),
            BpStepState::Manual,
        ));
    }
    if f.active && !tf.active {
        out.push(step(
            "fw:enable".into(),
            BpStepKind::Firewall,
            format!("Turn {} on", f.backend),
            "turn it on in System → FIREWALL once the rules are in, behind its rollback".into(),
            BpStepState::Manual,
        ));
    }
}

/// The target's Traefik routes plus the blueprint's missing ones.
pub fn traefik_after(have: &[TraefikRoute], want: &[TraefikRoute]) -> Vec<TraefikRoute> {
    let mut v = have.to_vec();
    for r in want {
        if !v.iter().any(|x| x.name == r.name) {
            v.push(r.clone());
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Blueprint {
        let mut bp = Blueprint::new("server01", 1_800_000_000);
        bp.os = "Debian GNU/Linux 12".into();
        bp.stacks.push(BpStack {
            name: "media".into(),
            dir: "/srv/media".into(),
            files: vec![BpFile { path: "/srv/media/compose.yaml".into(), text: "services: {}\n".into() }],
            env: vec![BpEnv { name: "DB_PASSWORD".into(), value: "correct horse".into(), secret: true }],
        });
        bp.crontabs.push(BpCrontab { user: None, text: "MAILTO=me\n0 3 * * * /usr/local/bin/backup\n# note\n".into() });
        bp.timers.push(TimerJob {
            name: "scrub".into(),
            command: "zpool scrub tank".into(),
            calendar: "Sun 03:00".into(),
            run_as: None,
            persistent: true,
        });
        bp.firewall = Some(BpFirewall {
            backend: "ufw".into(),
            active: true,
            ufw_rules: vec![
                "allow 22/tcp".into(),
                "allow from 192.168.1.0/24 to any port 445 proto tcp comment 'smb'".into(),
                "deny 23".into(),
            ],
            default_in: Some("deny".into()),
            zones: vec![],
        });
        bp.caddy.push(BpCaddy {
            proxy_name: "host".into(),
            path: "/etc/caddy/Caddyfile".into(),
            text: "a.example.lan {\n}\n".into(),
        });
        bp
    }

    #[test]
    fn encrypts_and_reads_back() {
        let bp = sample();
        let bytes = encrypt_with(&bp, "a long passphrase", Some(2)).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("correct horse"));
        assert_eq!(decrypt(&bytes, "a long passphrase").unwrap(), bp);
        assert!(decrypt(&bytes, "the wrong passphrase").is_err());
        assert!(encrypt(&bp, "short").is_err());
        assert!(decrypt(b"{}", "a long passphrase").is_err());
    }

    #[test]
    fn summary_has_no_values() {
        let s = sample().summary();
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("DB_PASSWORD"));
        assert!(!json.contains("correct horse"));
        assert_eq!(s.crontabs, vec![CronSummary { user: String::new(), lines: 2 }]);
        assert_eq!((s.firewall.as_deref(), s.firewall_rules), (Some("ufw"), 3));
    }

    #[test]
    fn plans_against_a_fresh_host() {
        let bp = sample();
        let t = BpTarget {
            docker: true,
            crontabs: vec![(None, String::new())],
            firewall: Some(BpFirewall {
                backend: "ufw".into(),
                active: false,
                ufw_rules: vec!["allow 22/tcp".into()],
                default_in: Some("deny".into()),
                zones: vec![],
            }),
            caddy: vec![("host".into(), String::new())],
            ..Default::default()
        };
        let p = plan(&bp, &t);
        let by = |id: &str| p.iter().find(|s| s.id == id).unwrap_or_else(|| panic!("{id} in {p:#?}"));
        assert_eq!(by("stack:media").state, BpStepState::New);
        assert!(by("stack:media").detail.contains("1 .env value"));
        assert_eq!(by("start:media").state, BpStepState::New);
        assert_eq!(by("cron:").state, BpStepState::New);
        assert_eq!(by("timer:scrub").state, BpStepState::New);
        assert_eq!(by("fw:allow 22/tcp").state, BpStepState::Same);
        assert_eq!(by("fw:deny 23").state, BpStepState::Manual);
        assert!(!by("fw:deny 23").default_on);
        assert_eq!(by("fw:enable").state, BpStepState::Manual);
        assert!(p.iter().all(|s| s.id != "fw:default"));
        assert_eq!(by("caddy:host").state, BpStepState::New);
    }

    #[test]
    fn plans_against_a_host_that_has_things() {
        let bp = sample();
        let mut t = BpTarget { docker: false, ..Default::default() };
        t.existing.insert("/srv/media/compose.yaml".into());
        t.timers.insert("scrub".into());
        t.caddy.push(("host".into(), "other {\n}\n".into()));
        let p = plan(&bp, &t);
        let by = |id: &str| p.iter().find(|s| s.id == id).unwrap();
        assert_eq!(by("stack:media").state, BpStepState::Differs);
        assert!(!by("stack:media").default_on);
        assert_eq!(by("start:media").state, BpStepState::Manual);
        assert_eq!(by("cron:").state, BpStepState::Manual);
        assert_eq!(by("timer:scrub").state, BpStepState::Same);
        assert_eq!(by("fw:backend").state, BpStepState::Manual);
        assert_eq!(by("caddy:host").state, BpStepState::Differs);
    }

    #[test]
    fn merges_crontabs_without_losing_lines() {
        let src = "MAILTO=me\n0 3 * * * /usr/local/bin/backup\n";
        let dst = "# mine\n*/5 * * * * /bin/true\n";
        let m = merge_crontab(src, dst);
        assert_eq!(
            m,
            "MAILTO=me\n# mine\n*/5 * * * * /bin/true\n# added by a Nightgrid blueprint\n0 3 * * * /usr/local/bin/backup\n"
        );
        assert!(missing_cron_lines(src, &m).is_empty());
        assert_eq!(merge_crontab(src, &m), m);
        assert_eq!(merge_crontab("0 1 * * * x\n", ""), "# added by a Nightgrid blueprint\n0 1 * * * x\n");
    }

    #[test]
    fn firewall_commands_only_add_allows() {
        assert_eq!(ufw_add_command("allow 22/tcp").unwrap(), "ufw 'allow' '22/tcp'");
        assert_eq!(
            ufw_add_command("allow from 192.168.1.0/24 to any port 445 proto tcp comment 'smb share'").unwrap(),
            "ufw 'allow' 'from' '192.168.1.0/24' 'to' 'any' 'port' '445' 'proto' 'tcp' comment 'smb share'"
        );
        assert!(ufw_add_command("deny 23").is_none());
        assert!(ufw_add_command("allow 22; rm -rf /").is_none());
        assert_eq!(
            firewalld_add_command("public", &["http".into()], &["8080/tcp".into()]).unwrap(),
            "firewall-cmd -q --permanent --zone=public --add-service=http --add-port=8080/tcp && firewall-cmd -q --reload"
        );
        assert!(firewalld_add_command("public", &["$(x)".into()], &[]).is_none());
        assert!(firewalld_add_command("public", &[], &["99999/tcp".into()]).is_none());
        let z = BpZone { name: "public".into(), services: vec!["http".into(), "ssh".into()], ports: vec![] };
        let t = BpZone { name: "public".into(), services: vec!["ssh".into()], ports: vec![] };
        assert_eq!(zone_missing(&z, Some(&t)), (vec!["http".to_owned()], vec![]));
    }

    #[test]
    fn scripts_quote_paths() {
        let s = exists_script(&["/srv/a b".into()]);
        assert!(s.contains("[ -e '/srv/a b' ]"));
        assert_eq!(parse_exists("@@have /srv/a b\n@@end\n").into_iter().collect::<Vec<_>>(), vec!["/srv/a b"]);
        let bp = sample();
        assert_eq!(mkdir_script(&bp.stacks[0]), "mkdir -p '/srv/media' && echo @@ok");
        assert_eq!(stack_paths(&bp), vec!["/srv/media", "/srv/media/compose.yaml", "/srv/media/.env"]);
    }

    #[test]
    fn traefik_routes_merge_by_name() {
        let r = |n: &str| TraefikRoute {
            name: n.into(),
            domains: vec![format!("{n}.example.lan")],
            upstream: "http://app:80".into(),
            entrypoints: vec![],
            tls: false,
            cert_resolver: None,
        };
        let v = traefik_after(&[r("a")], &[r("a"), r("b")]);
        assert_eq!(v.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    }
}
