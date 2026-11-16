//! The network map as the user arranged it (D-141): where each node sits,
//! which router, access point or switch a device connects through when the
//! user set it by hand, and which nodes are expected. Part of profile
//! exports. The UI computes the diff against a scan.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::store::Store;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LayoutEntry {
    /// A device's MAC or `ip:<address>`, `host:<name>`, or `gw`, `ts`,
    /// `net`, `local`.
    pub key: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    /// The node it connects through, set by hand (same key space).
    pub parent: Option<String>,
    /// Part of the saved "expected" picture.
    pub expected: bool,
}

/// The most nodes kept.
pub const MAX_ENTRIES: usize = 2000;
/// Coordinates are map units; anything wider is a bug, not a layout.
const MAX_COORD: f64 = 1.0e6;

fn valid_key(k: &str) -> bool {
    !k.is_empty() && k.len() <= 120 && !k.chars().any(char::is_control)
}

fn clean(e: &LayoutEntry) -> Result<LayoutEntry> {
    let key = e.key.trim().to_owned();
    if !valid_key(&key) {
        return Err(Error::Invalid { field: "key", reason: "1–120 printable characters".into() });
    }
    let coord = |v: Option<f64>| match v {
        Some(x) if !x.is_finite() || x.abs() > MAX_COORD => {
            Err(Error::Invalid { field: "x", reason: "a finite map coordinate".into() })
        }
        v => Ok(v),
    };
    let parent = e.parent.as_deref().map(str::trim).filter(|p| !p.is_empty()).map(str::to_owned);
    if parent.as_deref().is_some_and(|p| !valid_key(p) || p == key) {
        return Err(Error::Invalid { field: "parent", reason: "another node's key".into() });
    }
    Ok(LayoutEntry { key, x: coord(e.x)?, y: coord(e.y)?, parent, expected: e.expected })
}

impl Store {
    pub fn netmap_layout(&self) -> Result<Vec<LayoutEntry>> {
        let mut st = self.conn().prepare("SELECT key, x, y, parent, expected FROM netmap_layout ORDER BY key")?;
        let rows = st.query_map([], |r| {
            Ok(LayoutEntry {
                key: r.get(0)?,
                x: r.get(1)?,
                y: r.get(2)?,
                parent: r.get(3)?,
                expected: r.get::<_, i64>(4)? != 0,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Saves entries (replacing ones with the same key). With `replace`, the
    /// saved layout becomes exactly `entries` (a reset, or "save as
    /// expected").
    pub fn netmap_layout_save(&mut self, entries: &[LayoutEntry], replace: bool) -> Result<()> {
        if entries.len() > MAX_ENTRIES {
            return Err(Error::Invalid { field: "entries", reason: format!("at most {MAX_ENTRIES} nodes") });
        }
        let clean: Vec<LayoutEntry> = entries.iter().map(clean).collect::<Result<_>>()?;
        let tx = self.conn_mut().transaction()?;
        if replace {
            tx.execute("DELETE FROM netmap_layout", [])?;
        }
        for e in &clean {
            tx.execute(
                "INSERT INTO netmap_layout (key, x, y, parent, expected) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(key) DO UPDATE SET x = ?2, y = ?3, parent = ?4, expected = ?5, updated_at = unixepoch()",
                params![e.key, e.x, e.y, e.parent, i64::from(e.expected)],
            )?;
        }
        let n: i64 = tx.query_row("SELECT count(*) FROM netmap_layout", [], |r| r.get(0))?;
        if n as usize > MAX_ENTRIES {
            return Err(Error::Invalid { field: "entries", reason: format!("at most {MAX_ENTRIES} nodes") });
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(key: &str, x: f64, parent: Option<&str>) -> LayoutEntry {
        LayoutEntry { key: key.into(), x: Some(x), y: Some(1.0), parent: parent.map(Into::into), expected: true }
    }

    #[test]
    fn saves_merges_and_replaces() {
        let mut s = Store::open_in_memory().unwrap();
        s.netmap_layout_save(&[e("gw", 0.0, None), e("aa:bb:cc:00:00:30", 10.0, Some("ap:1"))], false).unwrap();
        s.netmap_layout_save(&[e("gw", 5.0, None)], false).unwrap();
        let l = s.netmap_layout().unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l.iter().find(|x| x.key == "gw").unwrap().x, Some(5.0));
        s.netmap_layout_save(&[e("host:nas", 1.0, Some("gw"))], true).unwrap();
        assert_eq!(s.netmap_layout().unwrap().len(), 1);
        assert!(s.netmap_layout_save(&[e("gw", f64::NAN, None)], false).is_err());
        assert!(s.netmap_layout_save(&[e("gw", 1.0, Some("gw"))], false).is_err());
    }
}
