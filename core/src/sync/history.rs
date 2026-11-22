//! Rolling snapshots of the synced profile (D-209).
//!
//! Before changes from another device land here, the synced items are
//! saved, and the last few snapshots are kept (3 by default). Rolling back
//! to one puts the items back as a *new edit* ([`merge::rollback_to`]), so
//! the next sync undoes the bad change on the other devices too. A snapshot
//! taken before a whole profile arrived (push, pull, import) also keeps the
//! profile bundle: those reset item ids, so rolling back across one
//! restores the bundle instead (on this device only).
//!
//! Snapshots are state of this computer: never synced or exported. They
//! hold no secrets.

use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::merge::{self, Item, Merged};
use crate::backup::{self, Bundle, ImportMode};
use crate::secrets::SecretStore;
use crate::store::Store;
use crate::{Error, Result};

/// Snapshots kept unless the user picks another number.
pub const DEFAULT_KEEP: u8 = 3;
pub const MAX_KEEP: u8 = 20;
/// Changes arriving within this many seconds of the last snapshot of the
/// same kind share it (one snapshot per burst of syncing, not per item).
pub const COALESCE_SECS: i64 = 600;
/// A snapshot bigger than this (compressed) isn't kept.
const MAX_BLOB: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SnapshotReason {
    /// Before automatic changes from a device.
    Changes,
    /// Before a device pushed its whole profile here.
    Push,
    /// Before this device pulled a profile.
    Pull,
    /// Before a profile was imported while pairing.
    Import,
    /// Before a rollback (so it can be undone).
    Rollback,
    /// Taken by hand.
    Manual,
}

impl SnapshotReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Changes => "changes",
            Self::Push => "push",
            Self::Pull => "pull",
            Self::Import => "import",
            Self::Rollback => "rollback",
            Self::Manual => "manual",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "changes" => Self::Changes,
            "push" => Self::Push,
            "pull" => Self::Pull,
            "import" => Self::Import,
            "rollback" => Self::Rollback,
            _ => Self::Manual,
        }
    }

    /// A whole profile is about to arrive: keep the bundle too.
    fn whole(self) -> bool {
        matches!(self, Self::Push | Self::Pull | Self::Import)
    }
}

/// A snapshot as listed (without its contents).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SyncSnapshot {
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub id: i64,
    /// Unix seconds.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub taken_at: i64,
    pub reason: SnapshotReason,
    /// The device whose changes came next.
    pub peer: Option<String>,
    /// Items per table in the snapshot.
    pub counts: BTreeMap<String, u32>,
    /// Rolling back undoes the change on synced devices too (false: this
    /// device only, because a whole profile arrived since).
    pub spreads: bool,
    /// It can be rolled back to at all.
    pub available: bool,
}

/// What a rollback did.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RolledBack {
    pub merged: Merged,
    /// The whole profile was restored: restart to load it.
    pub restart: bool,
    /// Devices with automatic sync get it too.
    pub spreads: bool,
}

fn gz(data: &[u8]) -> Result<Vec<u8>> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(data).map_err(|e| Error::Invalid { field: "snapshot", reason: e.to_string() })?;
    e.finish().map_err(|e| Error::Invalid { field: "snapshot", reason: e.to_string() })
}

fn ungz(data: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(data)
        .take(256 * 1024 * 1024)
        .read_to_end(&mut out)
        .map_err(|e| Error::Invalid { field: "snapshot", reason: e.to_string() })?;
    Ok(out)
}

/// Takes a snapshot of the synced profile and drops the oldest beyond
/// `keep`. Unless `force`, it's skipped (returns `None`) when the last one
/// has the same reason and device, is in the same generation and is less
/// than [`COALESCE_SECS`] old.
pub fn take(
    store: &mut Store,
    reason: SnapshotReason,
    peer: Option<&str>,
    keep: u8,
    force: bool,
    app_version: &str,
    now_ms: i64,
) -> Result<Option<i64>> {
    let now = now_ms / 1000;
    let gen = merge::generation(store)?;
    if !force {
        let last: Option<(i64, String, Option<String>, i64)> = store
            .conn()
            .query_row("SELECT taken_at, reason, peer, gen FROM sync_snapshots ORDER BY id DESC LIMIT 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .optional()?;
        if let Some((at, r, p, g)) = last {
            if r == reason.as_str() && p.as_deref() == peer && g == gen && now - at < COALESCE_SECS {
                return Ok(None);
            }
        }
    }
    let items: Vec<Item> = merge::scan(store, now_ms)?.items.into_iter().filter(|i| !i.deleted).collect();
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for i in &items {
        *counts.entry(i.tbl.clone()).or_default() += 1;
    }
    let blob = gz(&serde_json::to_vec(&items)?)?;
    let bundle = if reason.whole() {
        let mut b = backup::snapshot(store, app_version, now)?;
        super::strip_local_settings(&mut b);
        Some(gz(&serde_json::to_vec(&b)?)?)
    } else {
        None
    };
    if blob.len() + bundle.as_ref().map_or(0, Vec::len) > MAX_BLOB {
        tracing::warn!("sync: the profile is too big to snapshot");
        return Ok(None);
    }
    let keep = i64::from(keep.clamp(1, MAX_KEEP));
    let id = store.tx(|tx| {
        tx.execute(
            "INSERT INTO sync_snapshots (taken_at, reason, peer, gen, counts, items, bundle) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![now, reason.as_str(), peer, gen, serde_json::to_string(&counts)?, blob, bundle],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "DELETE FROM sync_snapshots WHERE id NOT IN (SELECT id FROM sync_snapshots ORDER BY id DESC LIMIT ?1)",
            [keep],
        )?;
        Ok(id)
    })?;
    Ok(Some(id))
}

/// Drops the oldest snapshots beyond `keep` (after the setting changed).
pub fn prune(store: &Store, keep: u8) -> Result<()> {
    store.conn().execute(
        "DELETE FROM sync_snapshots WHERE id NOT IN (SELECT id FROM sync_snapshots ORDER BY id DESC LIMIT ?1)",
        [i64::from(keep.clamp(1, MAX_KEEP))],
    )?;
    Ok(())
}

/// Snapshots, newest first.
pub fn list(store: &Store) -> Result<Vec<SyncSnapshot>> {
    let gen = merge::generation(store)?;
    let mut st = store.conn().prepare(
        "SELECT id, taken_at, reason, peer, gen, counts, bundle IS NOT NULL FROM sync_snapshots ORDER BY id DESC",
    )?;
    let rows = st.query_map([], |r| {
        let g: i64 = r.get(4)?;
        let has_bundle: bool = r.get(6)?;
        let counts: String = r.get(5)?;
        Ok(SyncSnapshot {
            id: r.get(0)?,
            taken_at: r.get(1)?,
            reason: SnapshotReason::parse(&r.get::<_, String>(2)?),
            peer: r.get(3)?,
            counts: serde_json::from_str(&counts).unwrap_or_default(),
            spreads: g == gen,
            available: g == gen || has_bundle,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Rolls back to snapshot `id`. A snapshot of the current state is taken
/// first, so the rollback itself can be undone.
pub fn rollback(
    store: &mut Store,
    keychain: &dyn SecretStore,
    id: i64,
    keep: u8,
    app_version: &str,
    now_ms: i64,
) -> Result<RolledBack> {
    let row: Option<(i64, Vec<u8>, Option<Vec<u8>>)> = store
        .conn()
        .query_row("SELECT gen, items, bundle FROM sync_snapshots WHERE id = ?1", [id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()?;
    let (gen, items, bundle) = row.ok_or(Error::NotFound("snapshot"))?;
    let same = gen == merge::generation(store)?;
    if !same && bundle.is_none() {
        return Err(Error::Invalid {
            field: "snapshot",
            reason: "a whole profile arrived after it, so it can't be rolled back to".into(),
        });
    }
    // Keep at least the target and the new one.
    take(store, SnapshotReason::Rollback, None, keep.max(2), true, app_version, now_ms)?;
    if same {
        let items: Vec<Item> = serde_json::from_slice(&ungz(&items)?)?;
        let merged = merge::rollback_to(store, &items, now_ms)?;
        return Ok(RolledBack { merged, restart: false, spreads: true });
    }
    let bundle: Bundle = serde_json::from_slice(&ungz(&bundle.expect("checked"))?)?;
    super::apply_bundle(store, keychain, bundle, ImportMode::Replace, &[])?;
    merge::reset(store)?;
    Ok(RolledBack { merged: Merged::default(), restart: true, spreads: false })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemoryStore;

    fn host_names(s: &Store) -> Vec<String> {
        let mut st = s.conn().prepare("SELECT name FROM hosts ORDER BY name").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().collect::<rusqlite::Result<Vec<String>>>().unwrap()
    }

    fn add_host(s: &Store, alias: &str) {
        s.conn().execute("INSERT INTO hosts (name, hostname) VALUES (?1, ?1)", [alias]).unwrap();
    }

    #[test]
    fn rolls_back_items_and_spreads_as_a_new_edit() {
        let mut s = Store::open_in_memory().unwrap();
        add_host(&s, "alpha");
        add_host(&s, "beta");
        merge::scan(&mut s, 1_000).unwrap();
        let id = take(&mut s, SnapshotReason::Changes, Some("Desk"), 3, false, "t", 2_000).unwrap().unwrap();
        // Coalesced: the same burst shares the snapshot.
        assert!(take(&mut s, SnapshotReason::Changes, Some("Desk"), 3, false, "t", 3_000).unwrap().is_none());

        // A "bad change" arrives: beta renamed, alpha deleted, gamma added.
        s.conn().execute("UPDATE hosts SET name = 'broken' WHERE name = 'beta'", []).unwrap();
        s.conn().execute("DELETE FROM hosts WHERE name = 'alpha'", []).unwrap();
        add_host(&s, "gamma");
        merge::scan(&mut s, 4_000).unwrap();
        assert_eq!(host_names(&s), vec!["broken", "gamma"]);

        let kc = MemoryStore::new();
        let r = rollback(&mut s, &kc, id, 3, "t", 5_000).unwrap();
        assert!(r.spreads && !r.restart);
        assert_eq!(host_names(&s), vec!["alpha", "beta"]);
        // The rollback is the newest edit, so it wins on the other devices.
        let items = merge::scan(&mut s, 6_000).unwrap().items;
        let restored: Vec<&Item> = items.iter().filter(|i| i.tbl == "hosts" && !i.deleted).collect();
        assert!(restored.iter().all(|i| i.modified == 5_001 || i.modified < 2_000));
        assert!(items.iter().any(|i| i.tbl == "hosts" && i.deleted && i.modified == 5_001));

        // The state before the rollback was kept, so it can be undone.
        let l = list(&s).unwrap();
        assert_eq!(l[0].reason, SnapshotReason::Rollback);
        assert!(l.iter().all(|x| x.available && x.spreads));
        assert_eq!(l.iter().find(|x| x.id == id).unwrap().counts.get("hosts"), Some(&2));
    }

    #[test]
    fn keeps_only_the_newest_and_detects_changes() {
        let mut s = Store::open_in_memory().unwrap();
        add_host(&s, "a");
        for i in 0..5 {
            take(&mut s, SnapshotReason::Manual, None, 3, true, "t", 10_000 + i).unwrap();
        }
        assert_eq!(list(&s).unwrap().len(), 3);
        prune(&s, 1).unwrap();
        assert_eq!(list(&s).unwrap().len(), 1);

        let items = merge::scan(&mut s, 20_000).unwrap().items;
        assert!(!merge::would_change(&mut s, &items).unwrap());
        let mut changed = items.clone();
        let h = changed.iter_mut().find(|i| i.tbl == "hosts").unwrap();
        h.hash = "different".into();
        h.modified = 30_000;
        assert!(merge::would_change(&mut s, &changed).unwrap());
    }

    #[test]
    fn whole_profile_snapshots_survive_a_reset() {
        let mut s = Store::open_in_memory().unwrap();
        add_host(&s, "keep");
        merge::scan(&mut s, 1_000).unwrap();
        let before_changes = take(&mut s, SnapshotReason::Changes, None, 5, true, "t", 1_500).unwrap().unwrap();
        let id = take(&mut s, SnapshotReason::Pull, Some("Desk"), 5, true, "t", 2_000).unwrap().unwrap();
        // A pulled profile replaces everything and resets item ids.
        s.conn().execute("DELETE FROM hosts", []).unwrap();
        add_host(&s, "pulled");
        merge::reset(&mut s).unwrap();
        let l = list(&s).unwrap();
        let p = l.iter().find(|x| x.id == id).unwrap();
        assert!(p.available && !p.spreads);
        assert!(!l.iter().find(|x| x.id == before_changes).unwrap().available);
        let kc = MemoryStore::new();
        assert!(rollback(&mut s, &kc, before_changes, 5, "t", 3_000).is_err());
        let r = rollback(&mut s, &kc, id, 5, "t", 3_000).unwrap();
        assert!(r.restart && !r.spreads);
        assert_eq!(host_names(&s), vec!["keep"]);
    }
}
