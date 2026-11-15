//! Labels for devices on the network map that aren't hosts: a name, a kind
//! (phone, TV, printer …), tags and a note, so a scan still says what each
//! address is. Keyed by MAC address, or by IP when the scan saw no MAC.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::store::Store;
use crate::{Error, Result};

/// Device kinds the map has an icon for.
pub const KINDS: &[&str] = &[
    "other", "phone", "tablet", "laptop", "desktop", "tv", "media", "speaker", "console", "printer", "camera", "iot",
    "nas", "router", "ap", "switch", "server", "vm",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DeviceLabel {
    /// Lower-case MAC (`aa:bb:cc:dd:ee:ff`), or `ip:<address>`.
    pub key: String,
    pub name: String,
    pub kind: String,
    pub tags: Vec<String>,
    pub note: String,
}

/// The key for a device: its MAC when known, else its address.
pub fn key_for(mac: Option<&str>, ip: &str) -> String {
    match mac.map(str::trim).filter(|m| !m.is_empty()) {
        Some(m) => m.to_ascii_lowercase().replace('-', ":"),
        None => format!("ip:{}", ip.trim()),
    }
}

fn valid_key(k: &str) -> bool {
    let mac = k.len() == 17
        && k.split(':').count() == 6
        && k.split(':').all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()));
    let ip = k.strip_prefix("ip:").is_some_and(|a| {
        !a.is_empty() && a.len() <= 45 && a.chars().all(|c| c.is_ascii_hexdigit() || c == '.' || c == ':')
    });
    mac || ip
}

fn clean(l: &DeviceLabel) -> Result<DeviceLabel> {
    let key = l.key.trim().to_ascii_lowercase();
    if !valid_key(&key) {
        return Err(Error::Invalid { field: "key", reason: "a MAC address or ip:<address>".into() });
    }
    let name = l.name.trim().to_owned();
    if name.chars().count() > 80 {
        return Err(Error::Invalid { field: "name", reason: "at most 80 characters".into() });
    }
    if !KINDS.contains(&l.kind.as_str()) {
        return Err(Error::Invalid { field: "kind", reason: format!("unknown kind {:?}", l.kind) });
    }
    let mut tags: Vec<String> = Vec::new();
    for t in &l.tags {
        let t = t.trim().trim_start_matches('#').to_lowercase();
        if t.is_empty() || tags.contains(&t) {
            continue;
        }
        if t.chars().count() > 32 || t.chars().any(char::is_whitespace) {
            return Err(Error::Invalid {
                field: "tags",
                reason: "tags are single words of at most 32 characters".into(),
            });
        }
        tags.push(t);
    }
    if tags.len() > 16 {
        return Err(Error::Invalid { field: "tags", reason: "at most 16 tags".into() });
    }
    let note = l.note.trim().to_owned();
    if note.chars().count() > 2000 {
        return Err(Error::Invalid { field: "note", reason: "at most 2000 characters".into() });
    }
    Ok(DeviceLabel { key, name, kind: l.kind.clone(), tags, note })
}

impl Store {
    pub fn list_device_labels(&self) -> Result<Vec<DeviceLabel>> {
        let mut stmt = self.conn().prepare("SELECT key, name, kind, tags, note FROM device_labels ORDER BY key")?;
        let rows = stmt.query_map([], |r| {
            let tags: String = r.get(3)?;
            Ok(DeviceLabel {
                key: r.get(0)?,
                name: r.get(1)?,
                kind: r.get(2)?,
                tags: serde_json::from_str(&tags).unwrap_or_default(),
                note: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Saves a label (replacing the one with the same key). A label with no
    /// name, tags or note and kind "other" is deleted instead.
    pub fn save_device_label(&mut self, label: &DeviceLabel) -> Result<Option<DeviceLabel>> {
        let l = clean(label)?;
        if l.name.is_empty() && l.tags.is_empty() && l.note.is_empty() && l.kind == "other" {
            self.delete_device_label(&l.key)?;
            return Ok(None);
        }
        let tags =
            serde_json::to_string(&l.tags).map_err(|e| Error::Invalid { field: "tags", reason: e.to_string() })?;
        self.conn().execute(
            "INSERT INTO device_labels (key, name, kind, tags, note) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(key) DO UPDATE SET name = ?2, kind = ?3, tags = ?4, note = ?5, updated_at = unixepoch()",
            params![l.key, l.name, l.kind, tags, l.note],
        )?;
        Ok(Some(l))
    }

    pub fn delete_device_label(&mut self, key: &str) -> Result<()> {
        self.conn().execute("DELETE FROM device_labels WHERE key = ?1", [key.trim().to_ascii_lowercase()])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(key: &str) -> DeviceLabel {
        DeviceLabel {
            key: key.into(),
            name: "Living room TV".into(),
            kind: "tv".into(),
            tags: vec![],
            note: String::new(),
        }
    }

    #[test]
    fn keys() {
        assert_eq!(key_for(Some("AA-BB-CC-DD-EE-0F"), "10.0.0.2"), "aa:bb:cc:dd:ee:0f");
        assert_eq!(key_for(None, "10.0.0.2"), "ip:10.0.0.2");
        assert_eq!(key_for(Some(" "), "10.0.0.2"), "ip:10.0.0.2");
        assert!(valid_key("aa:bb:cc:dd:ee:0f"));
        assert!(valid_key("ip:fd7a::1"));
        assert!(!valid_key("ip:"));
        assert!(!valid_key("aa:bb"));
        assert!(!valid_key("ip:10.0.0.2; drop"));
    }

    #[test]
    fn save_list_update_clear() {
        let mut s = Store::open_in_memory().unwrap();
        let mut l = label("AA:BB:CC:DD:EE:0F");
        l.tags = vec!["#Media".into(), "media".into(), "upstairs".into()];
        let saved = s.save_device_label(&l).unwrap().unwrap();
        assert_eq!(saved.key, "aa:bb:cc:dd:ee:0f");
        assert_eq!(saved.tags, vec!["media", "upstairs"]);
        l.name = "Bedroom TV".into();
        s.save_device_label(&l).unwrap();
        let all = s.list_device_labels().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "Bedroom TV");

        // Emptying every field removes the label.
        let empty = DeviceLabel { name: String::new(), kind: "other".into(), tags: vec![], ..l };
        assert_eq!(s.save_device_label(&empty).unwrap(), None);
        assert!(s.list_device_labels().unwrap().is_empty());
    }

    #[test]
    fn rejects_bad_input() {
        let mut s = Store::open_in_memory().unwrap();
        assert!(s.save_device_label(&DeviceLabel { kind: "toaster".into(), ..label("ip:10.0.0.9") }).is_err());
        assert!(s.save_device_label(&DeviceLabel { tags: vec!["two words".into()], ..label("ip:10.0.0.9") }).is_err());
        assert!(s.save_device_label(&label("not-a-key")).is_err());
    }
}
