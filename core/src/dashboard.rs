//! Overview dashboards (M10o): named layouts of widgets on a 12-column grid,
//! kept in the app settings so they travel in profile exports.
//!
//! The UI owns placement (moving, resizing, compacting); this module only
//! checks that what gets saved is well-formed, so a broken layout can never
//! make the whole settings row fail to load.

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Grid columns.
pub const COLUMNS: u8 = 12;
/// Tallest widget, in rows.
pub const MAX_ROWS: u16 = 16;
/// Rows a layout may extend to.
pub const MAX_Y: u16 = 400;
pub const MAX_DASHBOARDS: usize = 12;
pub const MAX_WIDGETS: usize = 48;
pub const MAX_PRESETS: usize = 24;
pub const MAX_NAME: usize = 40;

/// Widget kinds the UI knows how to draw. Unknown kinds in a saved layout
/// (from a newer version) are refused on save but drawn as a placeholder.
pub const WIDGET_KINDS: &[&str] =
    &["nodes", "alerts", "services", "updates", "disks", "backups", "tunnels", "notes", "desktops", "status", "gpus"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Widget {
    /// Stable within its dashboard (lets the UI keep state while moving).
    pub id: String,
    pub kind: String,
    pub x: u8,
    pub y: u16,
    pub w: u8,
    pub h: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub name: String,
    pub widgets: Vec<Widget>,
}

fn bad(field: &'static str, reason: impl Into<String>) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

fn check_name(field: &'static str, name: &str) -> Result<()> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > MAX_NAME || n.chars().any(char::is_control) {
        return Err(bad(field, format!("a name of 1 to {MAX_NAME} characters")));
    }
    Ok(())
}

/// Checks one dashboard: a name, known widget kinds, unique ids and widgets
/// inside the grid that don't overlap.
pub fn validate(d: &Dashboard, field: &'static str) -> Result<()> {
    check_name(field, &d.name)?;
    if d.widgets.len() > MAX_WIDGETS {
        return Err(bad(field, format!("at most {MAX_WIDGETS} widgets")));
    }
    for (i, w) in d.widgets.iter().enumerate() {
        if w.id.is_empty() || w.id.len() > 24 || !w.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(bad(field, "widget ids are 1 to 24 letters, digits or dashes"));
        }
        if !WIDGET_KINDS.contains(&w.kind.as_str()) {
            return Err(bad(field, format!("unknown widget {:?}", w.kind)));
        }
        if w.w == 0 || w.h == 0 || w.x + w.w > COLUMNS || w.h > MAX_ROWS || w.y + w.h > MAX_Y {
            return Err(bad(field, format!("widget {} is outside the grid", w.id)));
        }
        for o in &d.widgets[..i] {
            if o.id == w.id {
                return Err(bad(field, format!("duplicate widget id {}", w.id)));
            }
            let apart = w.x + w.w <= o.x || o.x + o.w <= w.x || w.y + w.h <= o.y || o.y + o.h <= w.y;
            if !apart {
                return Err(bad(field, format!("widgets {} and {} overlap", o.id, w.id)));
            }
        }
    }
    Ok(())
}

pub fn validate_all(dashboards: &[Dashboard], presets: &[Dashboard]) -> Result<()> {
    if dashboards.len() > MAX_DASHBOARDS {
        return Err(bad("dashboards", format!("at most {MAX_DASHBOARDS} dashboards")));
    }
    if presets.len() > MAX_PRESETS {
        return Err(bad("dashboardPresets", format!("at most {MAX_PRESETS} presets")));
    }
    for d in dashboards {
        validate(d, "dashboards")?;
    }
    for p in presets {
        validate(p, "dashboardPresets")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(id: &str, kind: &str, x: u8, y: u16, ww: u8, h: u16) -> Widget {
        Widget { id: id.into(), kind: kind.into(), x, y, w: ww, h }
    }

    #[test]
    fn accepts_a_tidy_layout() {
        let d = Dashboard {
            name: "Home".into(),
            widgets: vec![w("a", "nodes", 0, 0, 12, 4), w("b", "alerts", 0, 4, 6, 3), w("c", "status", 6, 4, 6, 3)],
        };
        validate_all(std::slice::from_ref(&d), std::slice::from_ref(&d)).unwrap();
        validate_all(&[], &[]).unwrap();
    }

    #[test]
    fn refuses_broken_layouts() {
        let ok = |ws: Vec<Widget>| validate(&Dashboard { name: "x".into(), widgets: ws }, "dashboards");
        assert!(ok(vec![w("a", "nodes", 6, 0, 7, 2)]).is_err(), "past column 12");
        assert!(ok(vec![w("a", "nodes", 0, 0, 0, 2)]).is_err(), "zero width");
        assert!(ok(vec![w("a", "nodes", 0, 0, 2, 17)]).is_err(), "too tall");
        assert!(ok(vec![w("a", "clock", 0, 0, 2, 2)]).is_err(), "unknown kind");
        assert!(ok(vec![w("a", "nodes", 0, 0, 4, 2), w("a", "alerts", 4, 0, 4, 2)]).is_err(), "duplicate id");
        assert!(ok(vec![w("a", "nodes", 0, 0, 4, 2), w("b", "alerts", 3, 1, 4, 2)]).is_err(), "overlap");
        assert!(ok(vec![w("a b", "nodes", 0, 0, 4, 2)]).is_err(), "bad id");
        assert!(validate(&Dashboard { name: " ".into(), widgets: vec![] }, "dashboards").is_err());
        assert!(validate(&Dashboard { name: "x".repeat(41), widgets: vec![] }, "dashboards").is_err());
        let many: Vec<Dashboard> = (0..13).map(|i| Dashboard { name: format!("d{i}"), widgets: vec![] }).collect();
        assert!(validate_all(&many, &[]).is_err());
    }
}
