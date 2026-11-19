//! Starter presets: ready-made app settings in the starter gallery (alert
//! rules, command tiles, dashboards and saved log searches). Unlike starter
//! blueprints they change nothing on a host: the UI shows what would change
//! and, once confirmed, saves them through the same calls as the Settings,
//! Overview and Fleet → LOGS editors.
//!
//! Files live in `starters/presets/*.yaml` and are checked here with the
//! same validators those editors use, so a preset can never save something
//! the editors would refuse.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::cmdwidget::{self, CmdWidget};
use crate::dashboard::{self, Dashboard};
use crate::docker::proxy_edit::{valid_domain, valid_upstream, TraefikRoute};
use crate::logsearch::{self, LogSearchQuery};
use crate::proxmox::create::{valid_guest_name, OSTYPES};
use crate::settings::{AlertSettings, AppSettings};
use crate::starters::{parse_yaml, valid_article, valid_id};
use crate::{Error, Result};

/// Which shelf of the gallery a preset is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PresetKind {
    /// Alert switches and thresholds.
    Alerts,
    /// Command tiles for the Overview.
    Tiles,
    /// An Overview dashboard (may bring its own tiles).
    Dashboard,
    /// Saved log searches.
    Searches,
    /// Reverse proxy routes for Caddy or Traefik.
    Routes,
    /// A Proxmox VM or container shape for the create dialog.
    Guests,
    /// Restore drills for every backup job.
    Drills,
    /// Security posture and network watch settings.
    Posture,
}

/// An alert setting's new value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(untagged)]
pub enum AlertValue {
    On(bool),
    Number(u32),
}

/// A command tile without its host (picked when it is added).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresetTile {
    pub name: String,
    pub command: String,
    #[serde(default = "text")]
    pub parse: String,
    #[serde(default)]
    pub pattern: String,
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub warn: Option<f64>,
    #[serde(default)]
    pub crit: Option<f64>,
    #[serde(default)]
    pub below: bool,
    #[serde(default = "five_minutes")]
    pub interval_secs: u32,
    /// What the tile shows, in plain words.
    #[serde(default)]
    pub what: String,
}

fn text() -> String {
    "text".into()
}
fn five_minutes() -> u32 {
    300
}

impl PresetTile {
    /// The tile as a command widget on `host_id` (id 0 = new).
    pub fn widget(&self, host_id: crate::hosts::HostId) -> CmdWidget {
        CmdWidget {
            id: 0,
            host_id,
            name: self.name.clone(),
            command: self.command.clone(),
            parse: self.parse.clone(),
            pattern: self.pattern.clone(),
            unit: self.unit.clone(),
            warn: self.warn,
            crit: self.crit,
            below: self.below,
            interval_secs: self.interval_secs,
        }
    }
}

/// A saved log search over every host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresetSearch {
    pub name: String,
    pub query: LogSearchQuery,
    #[serde(default)]
    pub alert: bool,
    /// What it finds, in plain words.
    #[serde(default)]
    pub what: String,
}

/// One proxy route: `<sub>.<your domain>` to an upstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresetRoute {
    /// Route name (Traefik) and the app's name.
    pub name: String,
    /// Subdomain, put in front of the domain the user types.
    pub sub: String,
    /// `app:8080`: a container name on the proxy's network, or host:port.
    pub upstream: String,
    #[serde(default)]
    pub what: String,
}

/// A guest shape for Proxmox → Create.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresetGuest {
    /// `vm` or `ct`.
    pub kind: String,
    pub name: String,
    pub cores: u32,
    pub memory_mb: u32,
    pub disk_gb: u32,
    #[serde(default)]
    pub swap_mb: u32,
    /// VMs: `l26`, `win11`, `win10`, `other`.
    #[serde(default)]
    pub ostype: String,
    #[serde(default)]
    pub uefi: bool,
    /// Containers: the first template whose name starts with this is picked.
    #[serde(default)]
    pub template: String,
    #[serde(default = "yes")]
    pub unprivileged: bool,
    #[serde(default)]
    pub nesting: bool,
}

fn yes() -> bool {
    true
}

/// Drills added for each backup job that has none yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresetDrills {
    /// 0 (by hand), 7, 14, 30 or 90.
    pub every_days: u32,
}

/// Drill schedules the Drills view offers.
pub const DRILL_EVERY: &[u32] = &[0, 7, 14, 30, 90];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StarterPreset {
    pub id: String,
    pub kind: PresetKind,
    pub title: String,
    pub summary: String,
    pub purpose: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub library: Vec<String>,
    /// Alert settings to change, by their Settings name (`diskPercent`).
    #[serde(default)]
    pub alerts: BTreeMap<String, AlertValue>,
    #[serde(default)]
    pub tiles: Vec<PresetTile>,
    /// Widgets may be `tile:<n>`: the preset's n-th tile, once added.
    #[serde(default)]
    pub dashboard: Option<Dashboard>,
    #[serde(default)]
    pub searches: Vec<PresetSearch>,
    #[serde(default)]
    pub routes: Vec<PresetRoute>,
    #[serde(default)]
    pub guest: Option<PresetGuest>,
    #[serde(default)]
    pub drills: Option<PresetDrills>,
    /// Security view settings to change (`postureDaily`).
    #[serde(default)]
    pub security: BTreeMap<String, AlertValue>,
    /// Network watch settings to change (`alertRisks`).
    #[serde(default)]
    pub net_watch: BTreeMap<String, AlertValue>,
}

/// Alert settings a preset may not touch: whether alerts run at all and
/// where they go are the user's own choices.
pub const ALERT_KEYS_KEPT: &[&str] = &["enabled", "native", "webhook", "webhookResolved", "logMutes"];

macro_rules! bundled {
    ($($id:literal),* $(,)?) => {
        &[$(($id, include_str!(concat!("../../starters/presets/", $id, ".yaml")))),*]
    };
}

/// The bundled presets, by file name (without `.yaml`).
pub const PRESET_FILES: &[(&str, &str)] = bundled!(
    "alerts-quiet-nights",
    "alerts-strict-storage",
    "alerts-calm-logs",
    "alerts-everything",
    "tiles-host-basics",
    "tiles-docker",
    "tiles-storage",
    "tiles-network",
    "dash-media-box",
    "dash-nas-watch",
    "dash-network-ops",
    "search-ssh-logins",
    "search-oom-kills",
    "search-disk-errors",
    "search-container-errors",
    "search-failed-units",
    "routes-media",
    "routes-documents",
    "routes-home-tools",
    "routes-monitoring",
    "guest-docker-ct",
    "guest-small-ct",
    "guest-linux-vm",
    "guest-homeassistant-vm",
    "guest-windows-vm",
    "drills-monthly",
    "drills-weekly",
    "posture-watchful",
    "posture-relaxed",
);

fn bad(field: &'static str, reason: impl Into<String>) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

/// Parses one preset file and checks it.
pub fn parse_preset(text: &str) -> Result<StarterPreset> {
    let p: StarterPreset = parse_yaml(text).map_err(|e| bad("preset", e))?;
    check(&p)?;
    Ok(p)
}

pub fn bundled() -> Vec<StarterPreset> {
    PRESET_FILES.iter().filter_map(|(_, t)| parse_preset(t).ok()).collect()
}

/// `patch` applied to one settings section. Unknown keys, kept keys and
/// values of the wrong type are refused.
fn patch_section<T: Serialize + serde::de::DeserializeOwned>(
    field: &'static str,
    base: &T,
    patch: &BTreeMap<String, AlertValue>,
    kept: &[&str],
) -> Result<T> {
    let mut v = serde_json::to_value(base).map_err(|e| bad(field, e.to_string()))?;
    let obj = v.as_object_mut().ok_or_else(|| bad(field, "not an object"))?;
    for (k, val) in patch {
        if kept.contains(&k.as_str()) {
            return Err(bad(field, format!("{k} is left to you")));
        }
        let cur = obj.get_mut(k).ok_or_else(|| bad(field, format!("no setting {k}")))?;
        *cur = match (val, &*cur) {
            (AlertValue::On(b), serde_json::Value::Bool(_)) => serde_json::Value::Bool(*b),
            (AlertValue::Number(n), serde_json::Value::Number(_)) => serde_json::Value::from(*n),
            _ => return Err(bad(field, format!("{k} has the wrong kind of value"))),
        };
    }
    serde_json::from_value(v).map_err(|e| bad(field, e.to_string()))
}

/// `alerts` applied to `base`; ranges are checked by the settings themselves.
pub fn apply_alerts(base: &AlertSettings, patch: &BTreeMap<String, AlertValue>) -> Result<AlertSettings> {
    let out = patch_section("alerts", base, patch, ALERT_KEYS_KEPT)?;
    let settings = AppSettings { alerts: out.clone(), ..AppSettings::default() };
    settings.validate()?;
    Ok(out)
}

/// A preset's settings changes applied to `base` (alerts, security, network watch).
pub fn apply_settings(base: &AppSettings, p: &StarterPreset) -> Result<AppSettings> {
    let mut s = base.clone();
    s.alerts = patch_section("alerts", &s.alerts, &p.alerts, ALERT_KEYS_KEPT)?;
    s.security = patch_section("security", &s.security, &p.security, &[])?;
    // Which machine scans is the user's choice.
    s.net_watch = patch_section("netWatch", &s.net_watch, &p.net_watch, &["scanner"])?;
    s.validate()?;
    Ok(s)
}

/// The dashboard with `tile:<n>` widgets pointing at command widget ids.
pub fn resolve_dashboard(d: &Dashboard, tile_ids: &[i32]) -> Result<Dashboard> {
    let mut d = d.clone();
    for w in &mut d.widgets {
        if let Some(n) = w.kind.strip_prefix("tile:") {
            let i: usize = n.parse().map_err(|_| bad("dashboard", format!("{} isn't tile:<n>", w.kind)))?;
            let id = tile_ids.get(i).ok_or_else(|| bad("dashboard", format!("{} has no tile", w.kind)))?;
            w.kind = format!("cmd:{id}");
        }
    }
    dashboard::validate(&d, "dashboard")?;
    Ok(d)
}

fn check(p: &StarterPreset) -> Result<()> {
    let who = &p.id;
    if !valid_id(who) {
        return Err(bad("id", "lowercase letters, digits and -"));
    }
    if p.title.trim().is_empty() || p.summary.trim().is_empty() || p.purpose.trim().is_empty() {
        return Err(bad("preset", format!("{who}: title, summary and purpose are needed")));
    }
    if let Some(a) = p.library.iter().find(|a| !valid_article(a)) {
        return Err(bad("library", format!("{who}: {a} isn't an article id")));
    }
    let settings = !p.security.is_empty() || !p.net_watch.is_empty();
    let has = (!p.alerts.is_empty(), !p.tiles.is_empty(), p.dashboard.is_some(), !p.searches.is_empty());
    let other = (!p.routes.is_empty(), p.guest.is_some(), p.drills.is_some(), settings);
    let none = (false, false, false, false);
    let fits = match p.kind {
        PresetKind::Alerts => has == (true, false, false, false) && other == none,
        PresetKind::Tiles => has == (false, true, false, false) && other == none,
        PresetKind::Dashboard => has.2 && !has.0 && !has.3 && other == none,
        PresetKind::Searches => has == (false, false, false, true) && other == none,
        PresetKind::Routes => has == (false, false, false, false) && other == (true, false, false, false),
        PresetKind::Guests => has == (false, false, false, false) && other == (false, true, false, false),
        PresetKind::Drills => has == (false, false, false, false) && other == (false, false, true, false),
        // Posture may also tune alerts (certificates, say).
        PresetKind::Posture => !has.1 && !has.2 && !has.3 && other == (false, false, false, true),
    };
    if !fits {
        return Err(bad("preset", format!("{who}: holds things that don't match its kind")));
    }
    apply_settings(&AppSettings::default(), p).map_err(|e| bad("settings", format!("{who}: {e}")))?;
    for r in &p.routes {
        let domain = format!("{}.example.com", r.sub);
        let route = TraefikRoute {
            name: r.name.clone(),
            domains: vec![domain.clone()],
            upstream: r.upstream.clone(),
            entrypoints: vec![],
            tls: true,
            cert_resolver: None,
        };
        if !valid_domain(&domain) || !valid_upstream(&r.upstream) || route.normalized().is_err() {
            return Err(bad("routes", format!("{who}: {} needs a plain subdomain, name and upstream", r.name)));
        }
    }
    if let Some(g) = &p.guest {
        let ok = match g.kind.as_str() {
            "vm" => OSTYPES.contains(&g.ostype.as_str()) && g.memory_mb >= 128,
            "ct" => !g.template.trim().is_empty() && g.memory_mb >= 64,
            _ => false,
        };
        if !ok || !valid_guest_name(&g.name) || !(1..=128).contains(&g.cores) || g.disk_gb == 0 {
            return Err(bad(
                "guest",
                format!("{who}: a vm or ct with a name, cores, memory, disk (and an OS or template)"),
            ));
        }
    }
    if let Some(d) = &p.drills {
        if !DRILL_EVERY.contains(&d.every_days) {
            return Err(bad("drills", format!("{who}: everyDays is one of {DRILL_EVERY:?}")));
        }
    }
    for t in &p.tiles {
        cmdwidget::validate(&t.widget(0)).map_err(|e| bad("tiles", format!("{who}: {}: {e}", t.name)))?;
    }
    if let Some(d) = &p.dashboard {
        let ids: Vec<i32> = (0..p.tiles.len() as i32).collect();
        resolve_dashboard(d, &ids).map_err(|e| bad("dashboard", format!("{who}: {e}")))?;
    }
    for s in &p.searches {
        let name = s.name.trim();
        if name.is_empty() || name.chars().count() > 60 {
            return Err(bad("searches", format!("{who}: names are 1-60 characters")));
        }
        if !s.query.hosts.is_empty() {
            return Err(bad("searches", format!("{who}: {name}: searches cover every host")));
        }
        logsearch::validate(&s.query).map_err(|e| bad("searches", format!("{who}: {name}: {e}")))?;
        if s.alert && s.query.text.trim().is_empty() {
            return Err(bad("searches", format!("{who}: {name}: an alert needs text to look for")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn repo(p: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(p)
    }

    #[test]
    fn every_preset_is_bundled_and_parses() {
        let on_disk: BTreeSet<String> = std::fs::read_dir(repo("starters/presets"))
            .unwrap()
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter_map(|n| n.strip_suffix(".yaml").map(str::to_owned))
            .collect();
        let listed: BTreeSet<String> = PRESET_FILES.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(listed, on_disk, "list each file in PRESET_FILES");
        for (name, text) in PRESET_FILES {
            let p = parse_preset(text).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(&p.id, name, "id matches the file name");
            for a in &p.library {
                assert!(repo(&format!("docs/kb/{a}.md")).exists(), "{name}: no Library article {a}");
            }
        }
        assert_eq!(bundled().len(), PRESET_FILES.len());
    }

    #[test]
    fn alert_patches_are_typed_and_ranged() {
        let base = AlertSettings::default();
        let set = |k: &str, v: AlertValue| apply_alerts(&base, &BTreeMap::from([(k.to_owned(), v)]));
        assert_eq!(set("diskPercent", AlertValue::Number(85)).unwrap().disk_percent, 85);
        assert!(set("logAnomaly", AlertValue::On(true)).unwrap().log_anomaly);
        assert!(set("diskPercent", AlertValue::Number(120)).is_err(), "out of range");
        assert!(set("diskPercent", AlertValue::On(true)).is_err(), "wrong type");
        assert!(set("nope", AlertValue::On(true)).is_err(), "unknown");
        assert!(set("enabled", AlertValue::On(false)).is_err(), "left to the user");
    }

    #[test]
    fn dashboards_point_tiles_at_saved_widgets() {
        let p = parse_preset(PRESET_FILES.iter().find(|(n, _)| *n == "dash-nas-watch").unwrap().1).unwrap();
        let ids: Vec<i32> = (0..p.tiles.len() as i32).map(|i| 100 + i).collect();
        let d = resolve_dashboard(p.dashboard.as_ref().unwrap(), &ids).unwrap();
        assert!(d.widgets.iter().any(|w| w.kind == "cmd:100"));
        assert!(d.widgets.iter().all(|w| !w.kind.starts_with("tile:")));
        assert!(resolve_dashboard(p.dashboard.as_ref().unwrap(), &[]).is_err());
    }

    #[test]
    fn posture_changes_settings_but_not_the_scanner() {
        let p = parse_preset(PRESET_FILES.iter().find(|(n, _)| *n == "posture-watchful").unwrap().1).unwrap();
        let s = apply_settings(&AppSettings::default(), &p).unwrap();
        assert!(s.net_watch.enabled && s.security.posture_daily);
        assert_eq!(s.alerts.cert_days, 30);
        let mut q = p.clone();
        q.net_watch.insert("scanner".into(), AlertValue::Number(3));
        assert!(apply_settings(&AppSettings::default(), &q).is_err());
    }

    #[test]
    fn tiles_only_read() {
        let bad = "kind: tiles\nid: x\ntitle: X\nsummary: s\npurpose: p\ntiles:\n  - name: wipe\n    command: rm -rf /tmp/x\n";
        assert!(parse_preset(bad).is_err());
    }
}
