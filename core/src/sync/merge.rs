//! Automatic sync: item-level merge, last edit wins (D-205).
//!
//! Every synced thing is an *item*: a row of a config table (the same
//! tables a profile export holds), a host tag, or one top-level app
//! setting. Each item has a uid shared by every device, the hash of its
//! content and when it last changed here (unix ms), kept in `sync_items`.
//!
//! [`scan`] compares the database with `sync_items` and stamps what
//! changed, appeared or disappeared since the last scan. On the wire rows
//! point at each other by uid, not by local id, so ids can differ per
//! device. [`merge`] takes another device's items: for each uid the newer
//! edit wins (ties go to the larger hash, so both sides pick the same
//! one), deletions included. An item that only exists on the other side
//! but matches a local row by its natural key (the same host alias, the
//! same tag name …) is the same item: both sides keep the smaller uid.
//!
//! Columns that change on their own (timestamps, last check results, a
//! host's detected OS) don't count as edits and are never overwritten.

use std::collections::{HashMap, HashSet};

use rusqlite::types::Value as SqlValue;
use rusqlite::{params, params_from_iter, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::Digest as _;

use crate::backup::{columns, find_existing, to_json, to_sql, Missing, Table, TABLES};
use crate::store::Store;
use crate::Result;

use super::LOCAL_SETTINGS;

/// The edit time the very first scan gives existing rows.
pub const BOOTSTRAP: i64 = 1;
/// Days a deleted item is remembered.
pub const TOMBSTONE_DAYS: i64 = 90;
const DAY_MS: i64 = 86_400_000;

/// Columns that aren't edits (see the module docs).
fn volatile(table: &str, col: &str) -> bool {
    matches!(col, "created_at" | "updated_at" | "checked_at" | "check_error" | "tested_at" | "test_ok" | "test_note")
        || (table == "hosts" && col == "os")
}

/// One synced item as it travels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub tbl: String,
    pub uid: String,
    /// Unix ms of the last edit.
    pub modified: i64,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub hash: String,
    /// The content: columns with references as uids (absent when deleted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<Map<String, Value>>,
}

#[derive(Debug, Clone)]
struct Known {
    local_id: Option<i64>,
    local_key: Option<String>,
    hash: String,
    modified: i64,
    deleted: bool,
}

/// Hash of an item's content, without the volatile columns.
pub fn hash_row(tbl: &str, row: &Map<String, Value>) -> String {
    let m: Map<String, Value> =
        row.iter().filter(|(k, _)| !volatile(tbl, k)).map(|(k, v)| (k.clone(), v.clone())).collect();
    let h = sha2::Sha256::digest(serde_json::to_vec(&m).unwrap_or_default());
    h[..16].iter().map(|b| format!("{b:02x}")).collect()
}

fn new_uid() -> String {
    use ring::rand::SecureRandom as _;
    let mut b = [0u8; 12];
    ring::rand::SystemRandom::new().fill(&mut b).expect("system random");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn table(name: &str) -> Option<&'static Table> {
    TABLES.iter().find(|t| t.name == name)
}

fn order(name: &str) -> usize {
    TABLES.iter().position(|t| t.name == name).unwrap_or(usize::MAX)
}

fn load_known(tx: &Transaction<'_>) -> Result<HashMap<(String, String), Known>> {
    let mut st = tx.prepare("SELECT tbl, uid, local_id, local_key, hash, modified, deleted FROM sync_items")?;
    let rows = st.query_map([], |r| {
        Ok((
            (r.get::<_, String>(0)?, r.get::<_, String>(1)?),
            Known {
                local_id: r.get(2)?,
                local_key: r.get(3)?,
                hash: r.get(4)?,
                modified: r.get(5)?,
                deleted: r.get::<_, i64>(6)? != 0,
            },
        ))
    })?;
    Ok(rows.collect::<rusqlite::Result<HashMap<_, _>>>()?)
}

fn save_known(tx: &Transaction<'_>, tbl: &str, uid: &str, k: &Known) -> Result<()> {
    // A stale item can still hold the local id or key: let go of it first.
    if let Some(id) = k.local_id {
        tx.execute(
            "UPDATE sync_items SET local_id = NULL WHERE tbl = ?1 AND local_id = ?2 AND uid <> ?3",
            params![tbl, id, uid],
        )?;
    }
    if let Some(key) = &k.local_key {
        tx.execute(
            "UPDATE sync_items SET local_key = NULL WHERE tbl = ?1 AND local_key = ?2 AND uid <> ?3",
            params![tbl, key, uid],
        )?;
    }
    tx.execute(
        "INSERT INTO sync_items (tbl, uid, local_id, local_key, hash, modified, deleted) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(tbl, uid) DO UPDATE SET local_id = excluded.local_id, local_key = excluded.local_key,
             hash = excluded.hash, modified = excluded.modified, deleted = excluded.deleted",
        params![tbl, uid, k.local_id, k.local_key, k.hash, k.modified, i64::from(k.deleted)],
    )?;
    Ok(())
}

fn app_settings(tx: &Transaction<'_>) -> Result<Map<String, Value>> {
    let raw: Option<String> =
        tx.query_row("SELECT value FROM settings WHERE key = 'app'", [], |r| r.get(0)).optional()?;
    Ok(raw
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default())
}

struct Current {
    tbl: &'static str,
    uid: String,
    local_id: Option<i64>,
    local_key: Option<String>,
    row: Map<String, Value>,
}

/// What a scan found.
#[derive(Debug)]
pub struct Scan {
    /// Every item, deleted ones included.
    pub items: Vec<Item>,
    /// Items stamped as changed by this scan.
    pub changed: usize,
}

/// Stamps what changed since the last scan and returns every item.
pub fn scan(store: &mut Store, now_ms: i64) -> Result<Scan> {
    store.tx(|tx| scan_tx(tx, now_ms))
}

fn scan_tx(tx: &Transaction<'_>, now: i64) -> Result<Scan> {
    let mut known = load_known(tx)?;
    // The first scan can't tell old rows from new edits: it stamps them as
    // old, so real edits on another device win over them.
    let stamp = if known.is_empty() { BOOTSTRAP } else { now };
    // Local id → uid per table, for rows and the references to them.
    let mut uid_of: HashMap<(&'static str, i64), String> = HashMap::new();
    for ((tbl, uid), k) in &known {
        if let (Some(t), Some(id), false) = (table(tbl), k.local_id, k.deleted) {
            uid_of.insert((t.name, id), uid.clone());
        }
    }
    for t in TABLES.iter().filter(|t| t.id) {
        let mut st = tx.prepare(&format!("SELECT id FROM {} ORDER BY id", t.name))?;
        let ids = st.query_map([], |r| r.get::<_, i64>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        for id in ids {
            uid_of.entry((t.name, id)).or_insert_with(new_uid);
        }
    }

    let mut current: Vec<Current> = Vec::new();
    for t in TABLES {
        if t.name == "settings" {
            for (k, v) in app_settings(tx)? {
                if LOCAL_SETTINGS.contains(&k.as_str()) {
                    continue;
                }
                let uid = format!("settings:{k}");
                let mut row = Map::new();
                row.insert("value".into(), v);
                current.push(Current { tbl: t.name, uid: uid.clone(), local_id: None, local_key: Some(uid), row });
            }
            continue;
        }
        let cols = columns(tx, t.name)?;
        let mut st = tx.prepare(&format!("SELECT * FROM {} ORDER BY rowid", t.name))?;
        let mut rows = st.query([])?;
        'rows: while let Some(r) = rows.next()? {
            let mut m = Map::new();
            for (i, c) in cols.iter().enumerate() {
                m.insert(c.clone(), to_json(r.get::<_, SqlValue>(i)?)?);
            }
            let id = if t.id { m.remove("id").and_then(|v| v.as_i64()) } else { None };
            let mut key_parts = Vec::new();
            let mut uid_parts = Vec::new();
            for (col, target, _) in t.fks {
                let local = m.get(*col).and_then(Value::as_i64);
                let r = local.and_then(|v| table(target).and_then(|tt| uid_of.get(&(tt.name, v)).cloned()));
                if !t.id {
                    // A keyless row is its references.
                    let (Some(l), Some(u)) = (local, r.clone()) else { continue 'rows };
                    key_parts.push(l.to_string());
                    uid_parts.push(u);
                }
                m.insert((*col).to_owned(), r.map_or(Value::Null, Value::String));
            }
            if t.id {
                let Some(id) = id else { continue };
                let Some(uid) = uid_of.get(&(t.name, id)).cloned() else { continue };
                current.push(Current { tbl: t.name, uid, local_id: Some(id), local_key: None, row: m });
            } else {
                current.push(Current {
                    tbl: t.name,
                    uid: format!("{}:{}", t.name, uid_parts.join(":")),
                    local_id: None,
                    local_key: Some(key_parts.join(":")),
                    row: m,
                });
            }
        }
    }

    let mut changed = 0;
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut items = Vec::with_capacity(current.len());
    for c in current {
        let h = hash_row(c.tbl, &c.row);
        let key = (c.tbl.to_owned(), c.uid.clone());
        let next = match known.get(&key) {
            Some(k) if !k.deleted && k.hash == h => {
                if k.local_id != c.local_id || k.local_key != c.local_key {
                    Some(Known { local_id: c.local_id, local_key: c.local_key.clone(), ..k.clone() })
                } else {
                    None
                }
            }
            _ => {
                changed += 1;
                Some(Known {
                    local_id: c.local_id,
                    local_key: c.local_key.clone(),
                    hash: h.clone(),
                    modified: stamp,
                    deleted: false,
                })
            }
        };
        if let Some(n) = next {
            save_known(tx, c.tbl, &c.uid, &n)?;
            known.insert(key.clone(), n);
        }
        let modified = known.get(&key).map_or(stamp, |k| k.modified);
        items.push(Item { tbl: c.tbl.into(), uid: c.uid, modified, deleted: false, hash: h, row: Some(c.row) });
        seen.insert(key);
    }
    for ((tbl, uid), k) in &known {
        if seen.contains(&(tbl.clone(), uid.clone())) {
            continue;
        }
        let k = if k.deleted {
            k.clone()
        } else {
            changed += 1;
            let n = Known { local_id: None, local_key: None, hash: String::new(), modified: now, deleted: true };
            save_known(tx, tbl, uid, &n)?;
            n
        };
        if k.modified >= now - TOMBSTONE_DAYS * DAY_MS {
            items.push(Item {
                tbl: tbl.clone(),
                uid: uid.clone(),
                modified: k.modified,
                deleted: true,
                hash: String::new(),
                row: None,
            });
        }
    }
    tx.execute("DELETE FROM sync_items WHERE deleted = 1 AND modified < ?1", [now - TOMBSTONE_DAYS * DAY_MS])?;
    Ok(Scan { items, changed })
}

/// What a merge changed here.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Merged {
    /// Items added or updated from the other side.
    pub applied: u32,
    pub deleted: u32,
    /// Items that couldn't be applied (a missing parent, a clash).
    pub skipped: u32,
    /// App settings changed.
    pub settings: bool,
    /// Tables that changed, for the UI to reload.
    pub tables: Vec<String>,
}

impl Merged {
    pub fn any(&self) -> bool {
        self.applied + self.deleted > 0
    }

    fn touch(&mut self, t: &str) {
        if !self.tables.iter().any(|x| x == t) {
            self.tables.push(t.to_owned());
        }
    }
}

fn wins(remote_mod: i64, remote_hash: &str, local: &Known) -> bool {
    remote_mod > local.modified || (remote_mod == local.modified && remote_hash > local.hash.as_str())
}

/// Takes another device's items (see the module docs). Local edits are
/// stamped first, in the same transaction, so an edit made a moment ago
/// isn't lost to an older one from the other side.
pub fn merge(store: &mut Store, remote: &[Item], now_ms: i64) -> Result<Merged> {
    store.tx(|tx| {
        scan_tx(tx, now_ms)?;
        merge_tx(tx, remote)
    })
}

/// Forgets every item, so the next scan starts over (after a profile was
/// pushed or pulled by hand, local ids no longer match).
pub fn reset(store: &mut Store) -> Result<()> {
    store.tx(|tx| {
        tx.execute("DELETE FROM sync_items", [])?;
        // Item ids from before don't match any more (see `generation`).
        tx.execute(
            "INSERT INTO sync_state (key, value) VALUES ('generation', 1)
             ON CONFLICT(key) DO UPDATE SET value = value + 1",
            [],
        )?;
        Ok(())
    })
}

/// Bumped by every [`reset`]: item ids saved under one generation can be
/// merged back (a rollback) only within it.
pub fn generation(store: &Store) -> Result<i64> {
    Ok(store
        .conn()
        .query_row("SELECT value FROM sync_state WHERE key = 'generation'", [], |r| r.get(0))
        .optional()?
        .unwrap_or(0))
}

/// Whether merging `remote` would change anything here (without stamping
/// or writing): an item that's new, differs and wins, or deletes a live one.
pub fn would_change(store: &mut Store, remote: &[Item]) -> Result<bool> {
    store.tx(|tx| {
        let known = load_known(tx)?;
        Ok(remote.iter().any(|it| match known.get(&(it.tbl.clone(), it.uid.clone())) {
            None => !it.deleted,
            Some(k) if it.deleted => !k.deleted && wins(it.modified, "", k),
            Some(k) => (k.deleted || k.hash != it.hash) && wins(it.modified, &it.hash, k),
        }))
    })
}

/// Puts the synced items back to `snapshot` (live items saved earlier in
/// this generation) as a new edit at `now_ms`: items that changed or went
/// away come back, items made since are deleted. Because it's an edit, the
/// next sync carries it to the other devices too.
pub fn rollback_to(store: &mut Store, snapshot: &[Item], now_ms: i64) -> Result<Merged> {
    store.tx(|tx| {
        scan_tx(tx, now_ms)?;
        // One ms later than anything the scan just stamped.
        let at = now_ms + 1;
        let keep: HashSet<(&str, &str)> = snapshot.iter().map(|i| (i.tbl.as_str(), i.uid.as_str())).collect();
        let mut items: Vec<Item> =
            snapshot.iter().filter(|i| !i.deleted).map(|i| Item { modified: at, ..i.clone() }).collect();
        for ((tbl, uid), k) in load_known(tx)? {
            if !k.deleted && !keep.contains(&(tbl.as_str(), uid.as_str())) {
                items.push(Item { tbl, uid, modified: at, deleted: true, hash: String::new(), row: None });
            }
        }
        merge_tx(tx, &items)
    })
}

/// References from uid to local id; `Err(())` when the row must be skipped.
fn localize(
    t: &Table,
    row: &Map<String, Value>,
    ids: &HashMap<(String, String), i64>,
    unresolved: &mut bool,
) -> std::result::Result<Map<String, Value>, ()> {
    let mut m = row.clone();
    m.remove("id");
    for (col, target, missing) in t.fks {
        let local =
            m.get(*col).and_then(Value::as_str).and_then(|u| ids.get(&((*target).to_owned(), u.to_owned())).copied());
        let was_set = m.get(*col).is_some_and(|v| !v.is_null());
        match (local, missing) {
            (Some(id), _) => {
                m.insert((*col).to_owned(), Value::from(id));
            }
            (None, Missing::Skip) if !t.id || was_set => return Err(()),
            (None, _) => {
                if was_set {
                    *unresolved = true;
                }
                m.insert((*col).to_owned(), Value::Null);
            }
        }
    }
    Ok(m)
}

fn write_row(
    tx: &Transaction<'_>,
    t: &Table,
    cols: &[String],
    row: &Map<String, Value>,
    existing: Option<i64>,
) -> Result<Option<i64>> {
    match existing {
        Some(id) => {
            let set: Vec<&String> =
                cols.iter().filter(|c| *c != "id" && !volatile(t.name, c) && row.contains_key(c.as_str())).collect();
            if !set.is_empty() {
                let assign = set.iter().map(|c| format!("{c} = ?")).collect::<Vec<_>>().join(", ");
                let mut vals = set.iter().map(|c| to_sql(&row[c.as_str()])).collect::<Result<Vec<_>>>()?;
                vals.push(SqlValue::Integer(id));
                tx.execute(&format!("UPDATE {} SET {assign} WHERE id = ?", t.name), params_from_iter(vals))?;
            }
            Ok(Some(id))
        }
        None => {
            let keep: Vec<&String> = cols.iter().filter(|c| *c != "id" && row.contains_key(c.as_str())).collect();
            if keep.is_empty() {
                return Ok(None);
            }
            let names = keep.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", ");
            let marks = vec!["?"; keep.len()].join(", ");
            let vals = keep.iter().map(|c| to_sql(&row[c.as_str()])).collect::<Result<Vec<_>>>()?;
            let verb = if t.id { "INSERT" } else { "INSERT OR IGNORE" };
            tx.execute(&format!("{verb} INTO {} ({names}) VALUES ({marks})", t.name), params_from_iter(vals))?;
            Ok(t.id.then(|| tx.last_insert_rowid()))
        }
    }
}

fn merge_tx(tx: &Transaction<'_>, remote: &[Item]) -> Result<Merged> {
    let mut known = load_known(tx)?;
    let mut ids: HashMap<(String, String), i64> = HashMap::new();
    let mut uid_by_id: HashMap<(String, i64), String> = HashMap::new();
    for ((tbl, uid), k) in &known {
        if let (Some(id), false) = (k.local_id, k.deleted) {
            ids.insert((tbl.clone(), uid.clone()), id);
            uid_by_id.insert((tbl.clone(), id), uid.clone());
        }
    }
    let mut out = Merged::default();
    let mut settings = app_settings(tx)?;
    let settings_before = settings.clone();
    let mut cols_cache: HashMap<&'static str, Vec<String>> = HashMap::new();
    let mut deferred: Vec<(&'static Table, String, &Map<String, Value>)> = Vec::new();

    let mut ups: Vec<&Item> = remote.iter().filter(|i| !i.deleted && i.row.is_some()).collect();
    ups.sort_by_key(|i| order(&i.tbl));
    for it in ups {
        let Some(t) = table(&it.tbl) else { continue };
        if t.name == "settings" {
            let field = it.uid.strip_prefix("settings:").unwrap_or_default();
            if field.is_empty() || LOCAL_SETTINGS.contains(&field) {
                continue;
            }
        }
        let row = it.row.as_ref().expect("filtered");
        let mut uid = it.uid.clone();
        if !known.contains_key(&(t.name.to_owned(), uid.clone())) && t.id {
            // The same thing made on both devices before they synced?
            let mut un = false;
            if let Ok(probe) = localize(t, row, &ids, &mut un) {
                if let Some(Some(lid)) = find_existing(tx, t, &probe)? {
                    if let Some(mine) = uid_by_id.get(&(t.name.to_owned(), lid)).cloned() {
                        if it.uid < mine {
                            // Take theirs: rename ours.
                            tx.execute(
                                "UPDATE sync_items SET uid = ?3 WHERE tbl = ?1 AND uid = ?2",
                                params![t.name, mine, it.uid],
                            )?;
                            if let Some(k) = known.remove(&(t.name.to_owned(), mine.clone())) {
                                known.insert((t.name.to_owned(), it.uid.clone()), k);
                            }
                            ids.remove(&(t.name.to_owned(), mine));
                            ids.insert((t.name.to_owned(), it.uid.clone()), lid);
                            uid_by_id.insert((t.name.to_owned(), lid), it.uid.clone());
                        } else {
                            // Keep ours; they rename theirs when they get our items.
                            uid = mine;
                        }
                    }
                }
            }
        }
        let key = (t.name.to_owned(), uid.clone());
        if let Some(k) = known.get(&key) {
            if (!k.deleted && k.hash == it.hash) || !wins(it.modified, &it.hash, k) {
                continue;
            }
        }
        let mut k_new =
            Known { local_id: None, local_key: None, hash: it.hash.clone(), modified: it.modified, deleted: false };
        if t.name == "settings" {
            let field = uid.strip_prefix("settings:").unwrap_or_default().to_owned();
            settings.insert(field, row.get("value").cloned().unwrap_or(Value::Null));
            k_new.local_key = Some(uid.clone());
            out.settings = true;
        } else {
            let cols = match cols_cache.get(t.name) {
                Some(c) => c.clone(),
                None => {
                    let c = columns(tx, t.name)?;
                    cols_cache.insert(t.name, c.clone());
                    c
                }
            };
            let mut unresolved = false;
            let Ok(local) = localize(t, row, &ids, &mut unresolved) else {
                out.skipped += 1;
                continue;
            };
            let existing = known.get(&key).filter(|k| !k.deleted).and_then(|k| k.local_id);
            let res = tx
                .execute_batch("SAVEPOINT sync_item")
                .map_err(crate::Error::from)
                .and_then(|_| write_row(tx, t, &cols, &local, existing));
            match res {
                Ok(id) => {
                    tx.execute_batch("RELEASE sync_item")?;
                    if t.id {
                        let Some(id) = id else { continue };
                        ids.insert(key.clone(), id);
                        uid_by_id.insert((t.name.to_owned(), id), uid.clone());
                        k_new.local_id = Some(id);
                    } else {
                        let parts: Vec<String> = t
                            .fks
                            .iter()
                            .filter_map(|(c, _, _)| local.get(*c).and_then(Value::as_i64))
                            .map(|v| v.to_string())
                            .collect();
                        k_new.local_key = Some(parts.join(":"));
                    }
                    if unresolved {
                        deferred.push((t, uid.clone(), row));
                    }
                }
                Err(e) => {
                    tx.execute_batch("ROLLBACK TO sync_item; RELEASE sync_item")?;
                    tracing::debug!(table = t.name, "sync: couldn't apply an item: {e}");
                    out.skipped += 1;
                    continue;
                }
            }
        }
        save_known(tx, t.name, &uid, &k_new)?;
        known.insert(key, k_new);
        out.applied += 1;
        out.touch(t.name);
    }

    // References to rows that arrived later in the same batch.
    for (t, uid, row) in deferred {
        let mut un = false;
        let (Ok(local), Some(&id)) = (localize(t, row, &ids, &mut un), ids.get(&(t.name.to_owned(), uid))) else {
            continue;
        };
        let fk: Map<String, Value> =
            t.fks.iter().filter_map(|(c, _, _)| local.get(*c).map(|v| ((*c).to_owned(), v.clone()))).collect();
        let cols: Vec<String> = fk.keys().cloned().collect();
        let _ = write_row(tx, t, &cols, &fk, Some(id));
    }

    let mut dels: Vec<&Item> = remote.iter().filter(|i| i.deleted).collect();
    dels.sort_by_key(|i| std::cmp::Reverse(order(&i.tbl)));
    for it in dels {
        let Some(t) = table(&it.tbl) else { continue };
        let key = (t.name.to_owned(), it.uid.clone());
        let tomb = Known { local_id: None, local_key: None, hash: String::new(), modified: it.modified, deleted: true };
        match known.get(&key) {
            None => {
                // Remember it, so an older copy from a third device doesn't bring it back.
                save_known(tx, t.name, &it.uid, &tomb)?;
                known.insert(key, tomb);
            }
            Some(k) if !k.deleted && wins(it.modified, "", k) => {
                if t.name == "settings" {
                    if let Some(f) = it.uid.strip_prefix("settings:") {
                        if !LOCAL_SETTINGS.contains(&f) {
                            settings.remove(f);
                            out.settings = true;
                        }
                    }
                } else if t.id {
                    if let Some(id) = k.local_id {
                        tx.execute(&format!("DELETE FROM {} WHERE id = ?1", t.name), [id])?;
                    }
                } else if let Some(lk) = &k.local_key {
                    let parts: Vec<i64> = lk.split(':').filter_map(|p| p.parse().ok()).collect();
                    if parts.len() == t.fks.len() {
                        let cond = t.fks.iter().map(|(c, _, _)| format!("{c} = ?")).collect::<Vec<_>>().join(" AND ");
                        tx.execute(&format!("DELETE FROM {} WHERE {cond}", t.name), params_from_iter(parts))?;
                    }
                }
                save_known(tx, t.name, &it.uid, &tomb)?;
                known.insert(key, tomb);
                out.deleted += 1;
                out.touch(t.name);
            }
            Some(_) => {}
        }
    }

    if out.settings && settings != settings_before {
        let v = Value::Object(settings);
        // Only settings this version can read and that are valid.
        let ok = serde_json::from_value::<crate::settings::AppSettings>(v.clone())
            .ok()
            .is_some_and(|s| s.validate().is_ok());
        if ok {
            tx.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES ('app', ?1, unixepoch())
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                [v.to_string()],
            )?;
        } else {
            tracing::warn!("sync: settings from the other device don't validate here; kept ours");
            out.settings = false;
        }
    } else {
        out.settings = false;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(s: &Store) {
        s.conn()
            .execute_batch(
                "INSERT INTO host_groups (id, name) VALUES (5, 'web');
                 INSERT INTO tags (id, name) VALUES (7, 'prod');
                 INSERT INTO hosts (id, name, hostname, group_id, ssh_alias) VALUES (3, 'server01', '10.0.0.1', 5, 's1'),
                                                                               (4, 'server02', '10.0.0.2', NULL, NULL);
                 UPDATE hosts SET wake_relay_id = 4 WHERE id = 3;
                 INSERT INTO host_tags VALUES (3, 7);
                 INSERT INTO bookmarks (host_id, path, label) VALUES (3, '/etc', 'etc');",
            )
            .unwrap();
        s.set_setting("app", &serde_json::json!({ "theme": "custom", "uiScale": 140 })).unwrap();
    }

    fn q<T: rusqlite::types::FromSql>(s: &Store, sql: &str) -> T {
        s.conn().query_row(sql, [], |r| r.get(0)).unwrap()
    }

    /// One automatic sync between two stores at `t`, like two apps would.
    fn exchange(a: &mut Store, b: &mut Store, t: i64) -> (Merged, Merged) {
        let ia = scan(a, t).unwrap().items;
        scan(b, t).unwrap();
        let mb = merge(b, &ia, t).unwrap();
        let ib = scan(b, t).unwrap().items;
        let ma = merge(a, &ib, t).unwrap();
        (mb, ma)
    }

    #[test]
    fn copies_everything_and_settles() {
        let mut a = Store::open_in_memory().unwrap();
        seed(&a);
        let mut z = Store::open_in_memory().unwrap();
        z.set_setting("app", &serde_json::json!({ "theme": "nightgrid", "uiScale": 110 })).unwrap();
        scan(&mut z, 500).unwrap();
        scan(&mut a, 500).unwrap();
        // A's theme was edited after both first looked.
        a.set_setting("app", &serde_json::json!({ "theme": "ice", "uiScale": 140 })).unwrap();
        a.conn().execute("UPDATE hosts SET port = 22 WHERE id = 3", []).unwrap();
        let (mz, ma) = exchange(&mut a, &mut z, 1000);
        assert!(mz.applied >= 6, "{mz:?}");
        assert!(!ma.any(), "nothing new for A: {ma:?}");
        assert_eq!(q::<i64>(&z, "SELECT count(*) FROM hosts"), 2);
        assert_eq!(q::<String>(&z, "SELECT h.name FROM host_tags x JOIN hosts h ON h.id = x.host_id"), "server01");
        assert_eq!(q::<String>(&z, "SELECT g.name FROM hosts h JOIN host_groups g ON g.id = h.group_id"), "web");
        let relay: String =
            q(&z, "SELECT r.name FROM hosts h JOIN hosts r ON r.id = h.wake_relay_id WHERE h.name = 'server01'");
        assert_eq!(relay, "server02", "a reference to a later row is fixed up");
        let v: Value = z.get_setting("app").unwrap().unwrap();
        assert_eq!((v["theme"].as_str(), v["uiScale"].as_i64()), (Some("ice"), Some(110)), "device settings stay");
        // Nothing moves on the next round.
        assert_eq!(scan(&mut z, 2000).unwrap().changed, 0);
        assert_eq!(scan(&mut a, 2000).unwrap().changed, 0);
        let (mz, ma) = exchange(&mut a, &mut z, 3000);
        assert!(!mz.any() && !ma.any());
    }

    #[test]
    fn edits_on_both_sides_survive_and_the_latest_wins() {
        let mut a = Store::open_in_memory().unwrap();
        seed(&a);
        let mut z = Store::open_in_memory().unwrap();
        exchange(&mut a, &mut z, 1000);

        a.conn().execute("UPDATE hosts SET port = 2222 WHERE name = 'server01'", []).unwrap();
        z.conn().execute("UPDATE hosts SET name = 'srv2' WHERE name = 'server02'", []).unwrap();
        scan(&mut a, 2000).unwrap();
        scan(&mut z, 2100).unwrap();
        exchange(&mut a, &mut z, 3000);
        for s in [&a, &z] {
            assert_eq!(q::<i64>(s, "SELECT port FROM hosts WHERE name = 'server01'"), 2222);
            assert_eq!(q::<i64>(s, "SELECT count(*) FROM hosts WHERE name = 'srv2'"), 1);
        }

        // The same host edited on both: the later edit wins everywhere.
        a.conn().execute("UPDATE hosts SET hostname = 'a.example' WHERE name = 'server01'", []).unwrap();
        z.conn().execute("UPDATE hosts SET hostname = 'z.example' WHERE name = 'server01'", []).unwrap();
        scan(&mut z, 4000).unwrap();
        scan(&mut a, 4500).unwrap();
        exchange(&mut a, &mut z, 5000);
        for s in [&a, &z] {
            assert_eq!(q::<String>(s, "SELECT hostname FROM hosts WHERE name = 'server01'"), "a.example");
        }

        // A setting changed on Z reaches A.
        let mut v: Value = z.get_setting("app").unwrap().unwrap();
        v["theme"] = "bloodline".into();
        z.set_setting("app", &v).unwrap();
        scan(&mut z, 6000).unwrap();
        let (_, ma) = exchange(&mut a, &mut z, 6500);
        assert!(ma.settings);
        assert_eq!(a.get_setting::<Value>("app").unwrap().unwrap()["theme"], "bloodline");
    }

    #[test]
    fn deletions_travel_and_stay_deleted() {
        let mut a = Store::open_in_memory().unwrap();
        seed(&a);
        let mut z = Store::open_in_memory().unwrap();
        exchange(&mut a, &mut z, 1000);
        let before = scan(&mut a, 1000).unwrap().items;
        z.conn().execute("DELETE FROM hosts WHERE name = 'server01'", []).unwrap();
        z.conn().execute("DELETE FROM tags", []).unwrap();
        scan(&mut z, 2000).unwrap();
        exchange(&mut z, &mut a, 3000);
        for s in [&a, &z] {
            assert_eq!(q::<i64>(s, "SELECT count(*) FROM hosts"), 1);
            assert_eq!(q::<i64>(s, "SELECT count(*) FROM bookmarks"), 0);
            assert_eq!(q::<i64>(s, "SELECT count(*) FROM host_tags"), 0);
            assert_eq!(q::<i64>(s, "SELECT count(*) FROM tags"), 0);
        }
        // An older copy (from a device that was offline) doesn't bring it back.
        let m = merge(&mut a, &before, 3500).unwrap();
        assert!(!m.any(), "{m:?}");
        assert_eq!(q::<i64>(&a, "SELECT count(*) FROM hosts"), 1);
        assert_eq!(scan(&mut a, 4000).unwrap().changed, 0);
    }

    #[test]
    fn the_same_host_made_twice_is_one_item() {
        let mut a = Store::open_in_memory().unwrap();
        seed(&a);
        let mut z = Store::open_in_memory().unwrap();
        z.conn()
            .execute_batch(
                "INSERT INTO hosts (id, name, hostname, ssh_alias) VALUES (1, 'mine', 'h.example', 's1');
                 INSERT INTO tags (id, name) VALUES (2, 'prod');",
            )
            .unwrap();
        scan(&mut z, 900).unwrap();
        exchange(&mut a, &mut z, 1000);
        exchange(&mut z, &mut a, 1100);
        for s in [&a, &z] {
            assert_eq!(q::<i64>(s, "SELECT count(*) FROM hosts WHERE ssh_alias = 's1'"), 1);
            assert_eq!(q::<i64>(s, "SELECT count(*) FROM tags"), 1);
        }
        // Both now use one uid for it, and settle.
        let ua: String = q(&a, "SELECT i.uid FROM sync_items i JOIN hosts h ON h.id = i.local_id WHERE i.tbl = 'hosts' AND h.ssh_alias = 's1'");
        let uz: String = q(&z, "SELECT i.uid FROM sync_items i JOIN hosts h ON h.id = i.local_id WHERE i.tbl = 'hosts' AND h.ssh_alias = 's1'");
        assert_eq!(ua, uz);
        let (m1, m2) = exchange(&mut a, &mut z, 2000);
        assert!(!m1.any() && !m2.any(), "{m1:?} {m2:?}");
        assert_eq!(
            q::<String>(&a, "SELECT name FROM hosts WHERE ssh_alias = 's1'"),
            q::<String>(&z, "SELECT name FROM hosts WHERE ssh_alias = 's1'")
        );
    }

    #[test]
    fn invalid_settings_from_the_other_side_are_ignored() {
        let mut z = Store::open_in_memory().unwrap();
        z.set_setting("app", &serde_json::json!({ "theme": "ice" })).unwrap();
        scan(&mut z, 100).unwrap();
        let bad = Item {
            tbl: "settings".into(),
            uid: "settings:helperModel".into(),
            modified: 200,
            deleted: false,
            hash: "x".into(),
            row: Some(serde_json::json!({ "value": "nope" }).as_object().unwrap().clone()),
        };
        let evil = Item {
            uid: "settings:disabledFeatures".into(),
            row: Some(serde_json::json!({ "value": ["BAD ID"] }).as_object().unwrap().clone()),
            ..bad.clone()
        };
        let m = merge(&mut z, &[bad, evil], 300).unwrap();
        assert!(!m.settings);
        assert_eq!(z.get_setting::<Value>("app").unwrap().unwrap(), serde_json::json!({ "theme": "ice" }));
    }
}
