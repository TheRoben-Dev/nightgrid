//! App-wide UI settings persisted in the `settings` table under one key.

use serde::{Deserialize, Serialize};

use crate::store::Store;
use crate::{Error, Result};

const KEY: &str = "app";

pub const THEMES: &[&str] = &["nightgrid", "ice", "bloodline", "plain"];
/// UI scale bounds in percent.
pub const UI_SCALE_MIN: u16 = 90;
pub const UI_SCALE_MAX: u16 = 150;

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
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { theme: "nightgrid".into(), reduce_effects: false, scanlines: true, ui_scale: 100, rail_collapsed: false }
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
