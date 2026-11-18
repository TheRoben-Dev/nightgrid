//! App-wide UI settings persisted in the `settings` table under one key.

use serde::{Deserialize, Serialize};

use crate::store::Store;
use crate::{Error, Result};

const KEY: &str = "app";

/// Theme ids. `plain` is the old fourth theme, read as `custom`.
pub const THEMES: &[&str] = &["nightgrid", "ice", "bloodline", "custom", "plain"];
/// Colour tokens the custom theme may set.
pub const THEME_TOKENS: &[&str] =
    &["bg-0", "bg-1", "bg-2", "line", "text-0", "text-1", "accent", "cyan", "magenta", "danger", "ok", "warn", "mint"];
/// UI scale bounds in percent.
pub const UI_SCALE_MIN: u16 = 90;
pub const UI_SCALE_MAX: u16 = 150;
pub const TERM_FONT_MIN: u16 = 9;
pub const TERM_FONT_MAX: u16 = 28;
pub const TERM_SCROLLBACK_MAX: u32 = 200_000;

/// User-facing app settings. Unknown/missing fields fall back to defaults so
/// older databases keep loading as fields are added.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    /// Theme id: `nightgrid`, `ice`, `bloodline` or `custom`.
    pub theme: String,
    /// The custom theme's colours, by token (`bg-0`, `accent`, …) as `#RRGGBB`.
    /// Tokens left out use the custom theme's defaults.
    pub custom_theme: std::collections::BTreeMap<String, String>,
    /// Disables glitch text, scanlines, glow and animations.
    pub reduce_effects: bool,
    /// Low-opacity scanline overlay (ignored when effects are reduced).
    pub scanlines: bool,
    /// UI scale in percent, 90–150.
    pub ui_scale: u16,
    /// Collapse the left rail to icons.
    pub rail_collapsed: bool,
    /// Handheld (Ally, Steam Deck) layout: larger touch targets.
    pub handheld: bool,
    /// Move focus and act with a game controller when one is connected.
    pub controller_nav: bool,
    /// The DRIFT helper (local model). Off until the user turns it on.
    pub helper_enabled: bool,
    /// The DRIFT model in use (an id from `helper::model::MODELS`).
    pub helper_model: String,
    /// Terminal font size in px.
    pub term_font_size: u16,
    /// Terminal scrollback lines.
    pub term_scrollback: u32,
    /// Copy terminal selections to the clipboard automatically.
    pub term_copy_on_select: bool,
    /// Right-click pastes (otherwise it opens the normal context menu).
    pub term_right_click_paste: bool,
    pub term_cursor_blink: bool,
    /// SSH terminals: `nano FILE` and `$EDITOR` open the file in the app's
    /// editor (session-only shell hook, see `edit_hook`).
    pub term_edit_in_app: bool,
    /// Show the keys of the program in front of a terminal (htop, vim, less …).
    pub term_key_hints: bool,
    /// Paint terminals on a solid background instead of letting the app's
    /// backdrop show through.
    pub term_solid_bg: bool,
    /// Built-in Tailscale client (M10j): off until the user turns it on.
    pub tailnet_enabled: bool,
    /// Use subnet routes other tailnet nodes advertise.
    pub tailnet_accept_routes: bool,
    /// This node's name in the tailnet.
    pub tailnet_hostname: String,
    /// A self-hosted control server (Headscale); empty = Tailscale's.
    pub tailnet_control_url: String,
    /// Search the shell history (bash, zsh, fish) of hosts from the terminal.
    /// Off by default; lines that look like they hold a secret are dropped and
    /// nothing is stored.
    pub history_search: bool,
    /// Collect host metrics (CPU, memory, disks, network, temps) over SSH.
    pub metrics_enabled: bool,
    /// Seconds between samples for hosts on screen.
    pub metrics_interval_secs: u16,
    /// Seconds between samples for hosts not on screen.
    pub metrics_background_secs: u16,
    /// Days of 1-minute history to keep (15-minute rollups are kept 90 days).
    pub metrics_retention_days: u16,
    /// Run service health checks in the background.
    pub services_checks: bool,
    /// Seconds between service health checks.
    pub services_interval_secs: u16,
    /// Check GitHub Releases for a newer Nightgrid on launch.
    pub update_check: bool,
    /// A version the user chose to skip (no launch prompt for it).
    pub update_skip_version: Option<String>,
    /// Alert rules and delivery.
    pub alerts: AlertSettings,
    /// Show a tray / menu bar icon with host status, tunnels and alerts.
    pub tray_enabled: bool,
    /// Closing the window hides it to the tray instead of quitting.
    pub close_to_tray: bool,
    /// Folder (e.g. a Syncthing or git folder) for quick encrypted backups.
    /// Machine-specific: kept as is when a backup is imported.
    pub backup_folder: Option<String>,
    /// Remind to export a profile after this many days without one; 0 = off.
    pub profile_reminder_days: u16,
    /// When a profile was last exported from this machine (unix seconds).
    /// Machine-specific: kept as is when a backup is imported.
    pub last_profile_export: Option<f64>,
    /// No reminder before this time (unix seconds).
    pub profile_reminder_snooze: Option<f64>,
    /// Read-only phone status page served by the app. Machine-specific.
    pub status_page: StatusPageSettings,
    /// Electricity price for Fleet → POWER.
    pub power: PowerSettings,
    /// The MCP server (M12g). Machine-specific, like the status page.
    pub mcp: McpSettings,
    /// Where notices go besides the inbox (1.1). The ntfy URL and token
    /// live in the keychain.
    pub notify: crate::notify::NotifySettings,
    /// Background network scans with alerts (D-145).
    pub net_watch: NetWatchSettings,
    /// Overview dashboards (M10o). Empty means the built-in default.
    pub dashboards: Vec<crate::dashboard::Dashboard>,
    /// Index of the dashboard shown.
    pub dashboard_active: u8,
    /// Layouts saved as named presets.
    pub dashboard_presets: Vec<crate::dashboard::Dashboard>,
    /// DRIFT has offered its first-run tour (M10q).
    pub tour_offered: bool,
    /// Features turned off in Settings → Features (`docker`, `fleet.logs` …).
    /// Turning a parent off turns its children off too. Off hides the
    /// feature and stops its background work; its data is kept. Unknown ids
    /// (from a newer version's profile) are kept as they are.
    pub disabled_features: Vec<String>,
}

/// Feature ids the backend knows about (the UI has the full list with labels).
pub const FEATURES: &[&str] = &[
    "terminal",
    "files",
    "docker",
    "services",
    "services.expiry",
    "tunnels",
    "system",
    "fleet",
    "fleet.updates",
    "fleet.disks",
    "fleet.backups",
    "fleet.ups",
    "fleet.uptime",
    "fleet.logs",
    "fleet.access",
    "fleet.speed",
    "fleet.journal",
    "fleet.blueprints",
    "fleet.power",
    "fleet.node",
    "fleet.catalog",
    "fleet.drills",
    "fleet.capacity",
    "fleet.replay",
    "security",
    "security.posture",
    "security.secrets",
    "desktops",
    "proxmox",
    "proxmox.sandbox",
    "network",
    "nettools",
    "automation",
    "notes",
    "drift",
];

pub const STATUS_BINDS: &[&str] = &["lan", "tailscale", "all"];

/// Power & cost (M12d): the electricity price for cost estimates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct PowerSettings {
    /// Price of one kWh; 0 shows energy only.
    pub price_per_kwh: f64,
    /// Shown with prices: `€`, `$`, `kr`, `£` … (up to 8 characters).
    pub currency: String,
}

/// The MCP server (M12g, D-166): off by default, only on 127.0.0.1, with a
/// token kept in the app's data folder. Read-only unless `allow_writes`,
/// and even then each change waits for approval in the app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct McpSettings {
    pub enabled: bool,
    pub port: u16,
    pub allow_writes: bool,
}

impl Default for McpSettings {
    fn default() -> Self {
        Self { enabled: false, port: crate::mcp::DEFAULT_PORT, allow_writes: false }
    }
}

/// The phone status page (off by default; the token lives in the keychain).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct StatusPageSettings {
    pub enabled: bool,
    /// `lan` (this machine's LAN address), `tailscale` (its 100.x address)
    /// or `all` (every interface).
    pub bind: String,
    pub port: u16,
}

impl Default for StatusPageSettings {
    fn default() -> Self {
        Self { enabled: false, bind: "lan".into(), port: 8787 }
    }
}

/// The network watch: a quiet discovery every so often from one machine,
/// with alerts for new devices, expected devices that are gone and risky
/// exposure. Off by default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct NetWatchSettings {
    pub enabled: bool,
    /// The host to scan from; `None` = this computer.
    pub scanner: Option<crate::hosts::HostId>,
    /// Minutes between scans, 10–1440.
    pub minutes: u16,
    /// Also ping-sweep the scanner's subnet (finds quiet devices; ~30 s).
    pub sweep: bool,
    pub alert_new: bool,
    pub alert_missing: bool,
    pub alert_risks: bool,
}

impl Default for NetWatchSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            scanner: None,
            minutes: 60,
            sweep: false,
            alert_new: true,
            alert_missing: true,
            alert_risks: true,
        }
    }
}

pub const WEBHOOK_KINDS: &[&str] = &["none", "ntfy", "discord", "json"];

/// Which alerts fire and where they go. Webhook URLs live in the keychain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct AlertSettings {
    pub enabled: bool,
    pub host_offline: bool,
    pub container_down: bool,
    pub disk_full: bool,
    /// Percent used that counts as full, 50–99.
    pub disk_percent: u8,
    pub service_down: bool,
    pub cert_expiring: bool,
    /// Alert when a certificate expires within this many days, 1–90.
    pub cert_days: u16,
    /// Background SMART reads (every 6 h, only on hosts where sudo needs no
    /// prompt: root, NOPASSWD or a remembered password).
    pub smart_warning: bool,
    /// Failed or stale backup jobs (checked every 30 minutes on connected hosts).
    pub backup_stale: bool,
    /// Mounts forecast to be full within `forecast_days` at their current
    /// growth (from usage recorded while monitoring).
    pub disk_forecast: bool,
    /// 1–90.
    pub forecast_days: u16,
    /// Degraded ZFS pools, RAID arrays and btrfs filesystems (read every 30
    /// minutes on connected hosts, no root needed).
    pub pool_degraded: bool,
    /// Error bursts and new error patterns in container logs and the journal
    /// (read every 10 minutes on connected hosts). Off by default.
    pub log_anomaly: bool,
    /// A new error pattern alerts once it has happened this many times
    /// within `log_repeat_minutes` (1–50); fewer is a note. FATAL/panic, and
    /// a container that is unhealthy or restarting, alert at once.
    pub log_repeat_count: u16,
    /// 10–240.
    pub log_repeat_minutes: u16,
    /// Minutes after a container, or a reverse proxy on its host, starts
    /// during which its log errors are notes, not alerts (0–60).
    pub log_grace_minutes: u16,
    /// HTTP 404 / NOT_FOUND lines never alert on their own; only a spike
    /// well above the container's usual rate does.
    pub log_not_found_noise: bool,
    /// Patterns that never alert, per container or journal source.
    pub log_mutes: Vec<crate::logwatch::LogMute>,
    /// Fold alerts explained by an outage upstream (a host offline, a
    /// service down that others depend on) under it, and don't notify for
    /// them separately (M10s).
    pub fold_dependent: bool,
    /// Native desktop notifications.
    pub native: bool,
    pub quiet_hours: bool,
    /// Minutes after local midnight.
    pub quiet_start: u16,
    pub quiet_end: u16,
    /// `none`, `ntfy`, `discord` or `json`.
    pub webhook: String,
    /// Also send "resolved" messages to the webhook.
    pub webhook_resolved: bool,
}

impl Default for AlertSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            host_offline: true,
            container_down: true,
            disk_full: true,
            disk_percent: 90,
            service_down: true,
            cert_expiring: true,
            cert_days: 14,
            smart_warning: true,
            backup_stale: true,
            disk_forecast: true,
            forecast_days: 14,
            pool_degraded: true,
            log_anomaly: false,
            log_repeat_count: 3,
            log_repeat_minutes: 10,
            log_grace_minutes: 5,
            log_not_found_noise: true,
            log_mutes: Vec::new(),
            fold_dependent: true,
            native: true,
            quiet_hours: false,
            quiet_start: 22 * 60,
            quiet_end: 7 * 60,
            webhook: "none".into(),
            webhook_resolved: true,
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "nightgrid".into(),
            custom_theme: Default::default(),
            reduce_effects: false,
            scanlines: true,
            ui_scale: 100,
            rail_collapsed: false,
            term_font_size: 13,
            term_scrollback: 10_000,
            term_copy_on_select: true,
            term_right_click_paste: true,
            term_cursor_blink: true,
            term_edit_in_app: false,
            term_key_hints: true,
            term_solid_bg: false,
            tailnet_enabled: false,
            tailnet_accept_routes: true,
            tailnet_hostname: "nightgrid".into(),
            tailnet_control_url: String::new(),
            history_search: false,
            metrics_enabled: true,
            metrics_interval_secs: 2,
            metrics_background_secs: 60,
            metrics_retention_days: 7,
            services_checks: true,
            services_interval_secs: 60,
            update_check: true,
            update_skip_version: None,
            alerts: AlertSettings::default(),
            tray_enabled: true,
            close_to_tray: false,
            backup_folder: None,
            profile_reminder_days: 7,
            last_profile_export: None,
            profile_reminder_snooze: None,
            status_page: StatusPageSettings::default(),
            notify: crate::notify::NotifySettings::default(),
            power: PowerSettings::default(),
            mcp: McpSettings::default(),
            net_watch: NetWatchSettings::default(),
            handheld: false,
            controller_nav: true,
            helper_enabled: false,
            helper_model: crate::helper::model::DEFAULT_MODEL.id.into(),
            dashboards: Vec::new(),
            dashboard_active: 0,
            tour_offered: false,
            dashboard_presets: Vec::new(),
            disabled_features: Vec::new(),
        }
    }
}

impl AppSettings {
    /// Whether a feature is on: neither it nor a parent (`fleet` for
    /// `fleet.logs`) is turned off.
    pub fn feature(&self, id: &str) -> bool {
        !self
            .disabled_features
            .iter()
            .any(|d| id == d || id.strip_prefix(d.as_str()).is_some_and(|rest| rest.starts_with('.')))
    }

    /// The settings background work should run with: rules and checks that
    /// belong to a turned-off feature are off.
    pub fn effective(&self) -> Self {
        let mut s = self.clone();
        if s.disabled_features.is_empty() {
            return s;
        }
        let a = &mut s.alerts;
        if !self.feature("docker") {
            a.container_down = false;
        }
        if !self.feature("fleet.disks") {
            a.smart_warning = false;
            a.pool_degraded = false;
            a.disk_forecast = false;
        }
        if !self.feature("fleet.backups") {
            a.backup_stale = false;
        }
        if !self.feature("fleet.logs") {
            a.log_anomaly = false;
        }
        if !self.feature("services") {
            s.services_checks = false;
        }
        if !self.feature("drift") {
            s.helper_enabled = false;
        }
        s
    }

    pub fn validate(&self) -> Result<()> {
        let a = &self.alerts;
        if !(1..=50).contains(&a.log_repeat_count)
            || !(10..=240).contains(&a.log_repeat_minutes)
            || a.log_grace_minutes > 60
        {
            return Err(Error::Invalid {
                field: "alerts.logRepeat",
                reason: "repeat count 1–50, window 10–240 minutes, grace 0–60 minutes".into(),
            });
        }
        if a.log_mutes.len() > 200 {
            return Err(Error::Invalid { field: "alerts.logMutes", reason: "at most 200".into() });
        }
        if let Some(p) = a.log_mutes.iter().find_map(|m| m.problem()) {
            return Err(Error::Invalid { field: "alerts.logMutes", reason: p });
        }
        if self.disabled_features.len() > 64
            || self.disabled_features.iter().any(|f| {
                f.is_empty()
                    || f.len() > 40
                    || !f.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
            })
        {
            return Err(Error::Invalid {
                field: "disabledFeatures",
                reason: "at most 64 ids of lowercase letters, digits, dots and dashes".into(),
            });
        }
        if crate::helper::model::model_by_id(&self.helper_model).is_none() {
            return Err(Error::Invalid { field: "helper_model", reason: "not one of the offered models".into() });
        }
        let h = &self.tailnet_hostname;
        if h.is_empty()
            || h.len() > 63
            || !h.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            || h.starts_with('-')
        {
            return Err(Error::Invalid {
                field: "tailnet_hostname",
                reason: "letters, digits and dashes, at most 63".into(),
            });
        }
        let u = self.tailnet_control_url.trim();
        if !u.is_empty() && (!u.starts_with("https://") || u.len() > 300 || u.contains(char::is_whitespace)) {
            return Err(Error::Invalid {
                field: "tailnet_control_url",
                reason: "an https:// address, or empty".into(),
            });
        }
        if !THEMES.contains(&self.theme.as_str()) {
            return Err(Error::Invalid { field: "theme", reason: format!("unknown theme {:?}", self.theme) });
        }
        for (k, v) in &self.custom_theme {
            let hex = v.len() == 7 && v.starts_with('#') && v[1..].chars().all(|c| c.is_ascii_hexdigit());
            if !THEME_TOKENS.contains(&k.as_str()) || !hex {
                return Err(Error::Invalid {
                    field: "customTheme",
                    reason: format!("{k}: expected a known token and #RRGGBB"),
                });
            }
        }
        if !(UI_SCALE_MIN..=UI_SCALE_MAX).contains(&self.ui_scale) {
            return Err(Error::Invalid {
                field: "uiScale",
                reason: format!("must be between {UI_SCALE_MIN} and {UI_SCALE_MAX} percent"),
            });
        }
        if !(TERM_FONT_MIN..=TERM_FONT_MAX).contains(&self.term_font_size) {
            return Err(Error::Invalid {
                field: "termFontSize",
                reason: format!("must be between {TERM_FONT_MIN} and {TERM_FONT_MAX}"),
            });
        }
        let range = |field: &'static str, v: u16, lo: u16, hi: u16| {
            if (lo..=hi).contains(&v) {
                Ok(())
            } else {
                Err(Error::Invalid { field, reason: format!("must be between {lo} and {hi}") })
            }
        };
        range("metricsIntervalSecs", self.metrics_interval_secs, 1, 60)?;
        range("metricsBackgroundSecs", self.metrics_background_secs, 10, 600)?;
        range("metricsRetentionDays", self.metrics_retention_days, 1, 90)?;
        range("servicesIntervalSecs", self.services_interval_secs, 15, 3600)?;
        range("profileReminderDays", self.profile_reminder_days, 0, 90)?;
        range("statusPage.port", self.status_page.port, 1024, 65535)?;
        range("mcp.port", self.mcp.port, 1024, 65535)?;
        if !self.power.price_per_kwh.is_finite() || !(0.0..=100.0).contains(&self.power.price_per_kwh) {
            return Err(Error::Invalid { field: "power.pricePerKwh", reason: "a price from 0 to 100 per kWh".into() });
        }
        if self.power.currency.chars().count() > 8 || self.power.currency.chars().any(char::is_control) {
            return Err(Error::Invalid { field: "power.currency", reason: "up to 8 characters".into() });
        }
        range("netWatch.minutes", self.net_watch.minutes, 10, 1440)?;
        if !STATUS_BINDS.contains(&self.status_page.bind.as_str()) {
            return Err(Error::Invalid {
                field: "statusPage.bind",
                reason: format!("unknown bind {:?}", self.status_page.bind),
            });
        }
        let a = &self.alerts;
        range("alerts.diskPercent", u16::from(a.disk_percent), 50, 99)?;
        range("alerts.certDays", a.cert_days, 1, 90)?;
        range("alerts.forecastDays", a.forecast_days, 1, 90)?;
        range("alerts.quietStart", a.quiet_start, 0, 24 * 60 - 1)?;
        range("alerts.quietEnd", a.quiet_end, 0, 24 * 60 - 1)?;
        if !WEBHOOK_KINDS.contains(&a.webhook.as_str()) {
            return Err(Error::Invalid { field: "alerts.webhook", reason: format!("unknown webhook {:?}", a.webhook) });
        }
        self.notify.validate()?;
        crate::dashboard::validate_all(&self.dashboards, &self.dashboard_presets)?;
        if self.term_scrollback > TERM_SCROLLBACK_MAX {
            return Err(Error::Invalid { field: "termScrollback", reason: format!("at most {TERM_SCROLLBACK_MAX}") });
        }
        Ok(())
    }

    /// Loads settings, falling back to defaults when unset or invalid.
    pub fn load(store: &Store) -> Result<Self> {
        let s: Option<Self> = store.get_setting(KEY).unwrap_or_else(|e| {
            tracing::warn!("ignoring unreadable settings: {e}");
            None
        });
        let Some(mut s) = s else { return Ok(Self::default()) };
        if s.validate().is_err() && crate::dashboard::validate_all(&s.dashboards, &s.dashboard_presets).is_err() {
            // A layout from a newer version shouldn't cost every other setting.
            tracing::warn!("ignoring unreadable dashboards");
            s.dashboards.clear();
            s.dashboard_presets.clear();
        }
        Ok(if s.validate().is_ok() { s } else { Self::default() })
    }

    pub fn save(&self, store: &Store) -> Result<()> {
        self.validate()?;
        store.set_setting(KEY, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bad_dashboard_only_drops_the_dashboards() {
        let store = Store::open_in_memory().unwrap();
        let mut s = AppSettings { theme: "ice".into(), ..AppSettings::default() };
        s.dashboards = vec![crate::dashboard::Dashboard {
            name: "Home".into(),
            widgets: vec![crate::dashboard::Widget {
                id: "a".into(),
                kind: "from-the-future".into(),
                x: 0,
                y: 0,
                w: 4,
                h: 2,
            }],
        }];
        assert!(s.save(&store).is_err());
        store.set_setting(KEY, &s).unwrap();
        let back = AppSettings::load(&store).unwrap();
        assert_eq!(back.theme, "ice");
        assert!(back.dashboards.is_empty());
    }

    #[test]
    fn features_and_effective() {
        let mut s = AppSettings::default();
        assert!(s.feature("fleet.logs") && s.effective() == s);
        s.disabled_features = vec!["fleet".into(), "docker".into()];
        assert!(!s.feature("fleet"));
        assert!(!s.feature("fleet.logs"));
        assert!(!s.feature("docker"));
        assert!(s.feature("dockerx"), "a prefix is not a parent");
        assert!(s.feature("services"));
        let e = s.effective();
        assert!(!e.alerts.container_down && !e.alerts.smart_warning && !e.alerts.backup_stale && !e.alerts.log_anomaly);
        assert!(e.services_checks && e.alerts.service_down);
        assert_eq!(e.disabled_features, s.disabled_features);
        s.disabled_features = vec!["services".into(), "drift".into()];
        s.helper_enabled = true;
        let e = s.effective();
        assert!(!e.services_checks && !e.helper_enabled && e.alerts.container_down);
        assert!(s.validate().is_ok());
        s.disabled_features = vec!["Bad Id".into()];
        assert!(s.validate().is_err());
        // The UI's list (ui/src/lib/features.ts) has the same ids.
        let ui = include_str!("../../ui/src/lib/features.ts");
        for f in FEATURES {
            assert!(ui.contains(&format!("id: \"{f}\"")), "{f} missing in features.ts");
        }
        assert_eq!(ui.matches("  { id: \"").count(), FEATURES.len(), "features.ts has ids the backend lacks");
    }

    #[test]
    fn defaults_when_empty() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(AppSettings::load(&store).unwrap(), AppSettings::default());
    }

    #[test]
    fn save_and_load() {
        let store = Store::open_in_memory().unwrap();
        let s = AppSettings { theme: "ice".into(), ui_scale: 125, ..Default::default() };
        s.save(&store).unwrap();
        assert_eq!(AppSettings::load(&store).unwrap(), s);
    }

    #[test]
    fn rejects_invalid() {
        let store = Store::open_in_memory().unwrap();
        let bad_theme = AppSettings { theme: "neon".into(), ..Default::default() };
        assert!(bad_theme.save(&store).is_err());
        for ui_scale in [0, 89, 151, 300] {
            assert!(AppSettings { ui_scale, ..Default::default() }.save(&store).is_err());
        }
        for alerts in [
            AlertSettings { disk_percent: 100, ..Default::default() },
            AlertSettings { cert_days: 0, ..Default::default() },
            AlertSettings { quiet_end: 1440, ..Default::default() },
            AlertSettings { webhook: "slack".into(), ..Default::default() },
        ] {
            assert!(AppSettings { alerts, ..Default::default() }.save(&store).is_err());
        }
    }

    #[test]
    fn custom_theme_tokens() {
        let store = Store::open_in_memory().unwrap();
        let mut ok = AppSettings { theme: "custom".into(), ..Default::default() };
        ok.custom_theme.insert("accent".into(), "#E8D44D".into());
        ok.save(&store).unwrap();
        assert_eq!(AppSettings::load(&store).unwrap().custom_theme["accent"], "#E8D44D");
        for (k, v) in [("accent", "red"), ("accent", "#E8D44"), ("font", "#000000"), ("bg-0", "url(x)")] {
            let mut bad = AppSettings::default();
            bad.custom_theme.insert(k.into(), v.into());
            assert!(bad.save(&store).is_err(), "{k}={v}");
        }
    }

    #[test]
    fn tolerates_partial_and_corrupt_rows() {
        let store = Store::open_in_memory().unwrap();
        store.set_setting(KEY, &serde_json::json!({ "theme": "plain" })).unwrap();
        let s = AppSettings::load(&store).unwrap();
        assert_eq!(s.theme, "plain");
        assert_eq!(s.ui_scale, 100);

        store.set_setting(KEY, &serde_json::json!({ "alerts": { "diskPercent": 80 } })).unwrap();
        let s = AppSettings::load(&store).unwrap();
        assert_eq!((s.alerts.disk_percent, s.alerts.cert_days), (80, 14));

        store.set_setting(KEY, &serde_json::json!("garbage")).unwrap();
        assert_eq!(AppSettings::load(&store).unwrap(), AppSettings::default());
    }
}
