//! App-wide UI settings persisted in the `settings` table under one key.

use serde::{Deserialize, Serialize};

use crate::store::Store;
use crate::{Error, Result};

const KEY: &str = "app";

pub const THEMES: &[&str] = &["nightgrid", "ice", "bloodline", "plain"];
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
    /// Theme id: `nightgrid`, `ice`, `bloodline` or `plain`.
    pub theme: String,
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
}

pub const STATUS_BINDS: &[&str] = &["lan", "tailscale", "all"];

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
    /// Error bursts and new error patterns in container logs and the journal
    /// (read every 10 minutes on connected hosts). Off by default.
    pub log_anomaly: bool,
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
            log_anomaly: false,
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
            handheld: false,
            controller_nav: true,
            helper_enabled: false,
        }
    }
}

impl AppSettings {
    pub fn validate(&self) -> Result<()> {
        if !THEMES.contains(&self.theme.as_str()) {
            return Err(Error::Invalid { field: "theme", reason: format!("unknown theme {:?}", self.theme) });
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
        if !STATUS_BINDS.contains(&self.status_page.bind.as_str()) {
            return Err(Error::Invalid {
                field: "statusPage.bind",
                reason: format!("unknown bind {:?}", self.status_page.bind),
            });
        }
        let a = &self.alerts;
        range("alerts.diskPercent", u16::from(a.disk_percent), 50, 99)?;
        range("alerts.certDays", a.cert_days, 1, 90)?;
        range("alerts.quietStart", a.quiet_start, 0, 24 * 60 - 1)?;
        range("alerts.quietEnd", a.quiet_end, 0, 24 * 60 - 1)?;
        if !WEBHOOK_KINDS.contains(&a.webhook.as_str()) {
            return Err(Error::Invalid { field: "alerts.webhook", reason: format!("unknown webhook {:?}", a.webhook) });
        }
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
        Ok(s.filter(|s| s.validate().is_ok()).unwrap_or_default())
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
