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
            metrics_enabled: true,
            metrics_interval_secs: 2,
            metrics_background_secs: 60,
            metrics_retention_days: 7,
            services_checks: true,
            services_interval_secs: 60,
            update_check: true,
            update_skip_version: None,
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
    }

    #[test]
    fn tolerates_partial_and_corrupt_rows() {
        let store = Store::open_in_memory().unwrap();
        store.set_setting(KEY, &serde_json::json!({ "theme": "plain" })).unwrap();
        let s = AppSettings::load(&store).unwrap();
        assert_eq!(s.theme, "plain");
        assert_eq!(s.ui_scale, 100);

        store.set_setting(KEY, &serde_json::json!("garbage")).unwrap();
        assert_eq!(AppSettings::load(&store).unwrap(), AppSettings::default());
    }
}
