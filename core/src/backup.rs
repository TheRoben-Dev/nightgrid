//! Encrypted config export/import.
//!
//! A backup is a JSON [`Bundle`] (the config tables as rows, plus keychain
//! secrets only when the user opts in) encrypted with an age scrypt
//! passphrase recipient. Nothing is ever written unencrypted. History tables
//! (metric points, service checks) are left out.
//!
//! Import either **merges** (rows that match an existing one by a natural key
//! are skipped, the rest get fresh ids with references remapped) or
//! **replaces** (every config table is cleared, then rows keep their ids).
//! Both run in one transaction.

use std::collections::HashMap;

use age::secrecy::SecretString;
use rusqlite::types::Value as SqlValue;
use rusqlite::{params_from_iter, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use zeroize::Zeroizing;

use crate::hosts::HostId;
use crate::integrations::IntegrationId;
use crate::secrets::{Secret, SecretKind, SecretRef, SecretStore};
use crate::store::Store;
use crate::{Error, Result};

pub const FORMAT: &str = "nightgrid-backup";
pub const VERSION: u32 = 1;
/// Default file name inside a sync folder.
pub const FILE_NAME: &str = "nightgrid-backup.age";
/// Refuse to read anything bigger (a config bundle is a few hundred KB).
pub const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
pub const MIN_PASSPHRASE: usize = 8;

/// What to do when a reference has no row to point at.
#[derive(Clone, Copy)]
enum Missing {
    /// The column is nullable: clear it.
    Null,
    /// The row can't exist without it: leave the row out.
    Skip,
}

struct Table {
    name: &'static str,
    /// Has an integer `id` primary key that other tables point at.
    id: bool,
    /// (column, referenced table, what to do if the target is missing).
    fks: &'static [(&'static str, &'static str, Missing)],
    /// Natural keys used to spot duplicates when merging; a key is ignored
    /// when its first column is null in the incoming row.
    keys: &'static [&'static [&'static str]],
}

/// Config tables in insert order (parents first).
const TABLES: &[Table] = &[
    Table { name: "host_groups", id: true, fks: &[], keys: &[&["name"]] },
    Table { name: "tags", id: true, fks: &[], keys: &[&["name"]] },
    Table {
        name: "hosts",
        id: true,
        // `wake_relay_id` points at another host, which may come later; it is
        // cleared here and restored once every host has its new id.
        fks: &[("group_id", "host_groups", Missing::Null), ("wake_relay_id", "hosts", Missing::Null)],
        keys: &[&["ssh_alias"], &["name", "hostname", "port"]],
    },
    Table {
        name: "host_tags",
        id: false,
        fks: &[("host_id", "hosts", Missing::Skip), ("tag_id", "tags", Missing::Skip)],
        keys: &[&["host_id", "tag_id"]],
    },
    Table { name: "bookmarks", id: true, fks: &[("host_id", "hosts", Missing::Skip)], keys: &[&["path", "host_id"]] },
    Table { name: "stack_dirs", id: true, fks: &[("host_id", "hosts", Missing::Skip)], keys: &[&["host_id", "path"]] },
    Table { name: "services", id: true, fks: &[("host_id", "hosts", Missing::Null)], keys: &[&["name", "url"]] },
    Table {
        name: "tunnels",
        id: true,
        fks: &[("host_id", "hosts", Missing::Skip)],
        keys: &[&["host_id", "kind", "bind_addr", "bind_port"]],
    },
    Table {
        name: "integrations",
        id: true,
        fks: &[("host_id", "hosts", Missing::Null)],
        keys: &[&["kind", "base_url", "name"]],
    },
    Table {
        name: "snippets",
        id: true,
        fks: &[("host_id", "hosts", Missing::Skip), ("group_id", "host_groups", Missing::Skip)],
        keys: &[&["name"]],
    },
    Table { name: "playbooks", id: true, fks: &[], keys: &[&["name"]] },
    Table {
        name: "notes",
        id: true,
        fks: &[("host_id", "hosts", Missing::Null), ("service_id", "services", Missing::Null)],
        keys: &[&["title"]],
    },
    Table { name: "backup_jobs", id: true, fks: &[("host_id", "hosts", Missing::Skip)], keys: &[&["host_id", "name"]] },
    Table { name: "device_labels", id: true, fks: &[], keys: &[&["key"]] },
    Table { name: "netmap_layout", id: true, fks: &[], keys: &[&["key"]] },
    Table {
        name: "watch_items",
        id: true,
        fks: &[("host_id", "hosts", Missing::Skip)],
        keys: &[&["kind", "name", "target"]],
    },
    Table {
        name: "expiry_prefs",
        id: true,
        fks: &[("service_id", "services", Missing::Skip)],
        keys: &[&["service_id"], &["proxy_domain"]],
    },
    Table {
        name: "alert_deps",
        id: true,
        fks: &[
            ("child_host_id", "hosts", Missing::Skip),
            ("child_service_id", "services", Missing::Skip),
            ("parent_host_id", "hosts", Missing::Skip),
            ("parent_service_id", "services", Missing::Skip),
        ],
        keys: &[
            &["child_host_id", "child_service_id", "parent_host_id", "parent_service_id"],
            &["child_service_id", "child_host_id", "parent_host_id", "parent_service_id"],
        ],
    },
    Table {
        name: "ups_watches",
        id: true,
        fks: &[("host_id", "hosts", Missing::Skip), ("group_id", "host_groups", Missing::Null)],
        keys: &[&["host_id", "target"]],
    },
    // Host ids inside the saved query aren't remapped; unknown ones are ignored.
    Table { name: "log_searches", id: true, fks: &[], keys: &[&["name"]] },
    // Ciphertext only; the vault keys travel as secrets (opt-in).
    Table {
        name: "env_vars",
        id: true,
        fks: &[("host_id", "hosts", Missing::Skip)],
        keys: &[&["host_id", "stack", "name"]],
    },
    Table {
        name: "git_stacks",
        id: true,
        fks: &[("host_id", "hosts", Missing::Skip)],
        keys: &[&["host_id", "dir", "subdir"]],
    },
    Table { name: "cmd_widgets", id: true, fks: &[("host_id", "hosts", Missing::Skip)], keys: &[&["host_id", "name"]] },
    Table {
        name: "quests",
        id: true,
        fks: &[("host_id", "hosts", Missing::Skip)],
        keys: &[&["article_id", "host_id"]],
    },
    Table {
        name: "update_holds",
        id: true,
        fks: &[("host_id", "hosts", Missing::Skip)],
        keys: &[&["host_id", "kind", "stack", "target"]],
    },
    Table { name: "settings", id: false, fks: &[], keys: &[&["key"]] },
];

/// Who a bundled secret belongs to, by the id it had when exported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "owner", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SecretOwner {
    Host {
        id: HostId,
    },
    Integration {
        id: IntegrationId,
    },
    AlertsWebhook,
    /// A .env vault key, by its id (the same on every computer).
    Vault {
        key_id: String,
    },
}

#[derive(Serialize, Deserialize)]
pub struct BundledSecret {
    #[serde(flatten)]
    pub owner: SecretOwner,
    pub kind: String,
    pub value: String,
}

impl Drop for BundledSecret {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.value.zeroize();
    }
}

/// The decrypted contents of a backup file.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bundle {
    pub format: String,
    pub version: u32,
    pub app_version: String,
    /// Schema version (`PRAGMA user_version`) of the exporting database.
    pub schema: i64,
    /// Unix seconds.
    pub created_at: i64,
    pub tables: HashMap<String, Vec<Map<String, Value>>>,
    #[serde(default)]
    pub secrets: Vec<BundledSecret>,
}

/// What a backup holds, shown before importing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub app_version: String,
    /// Unix seconds.
    pub created_at: f64,
    pub hosts: u32,
    pub groups: u32,
    pub tags: u32,
    pub snippets: u32,
    pub playbooks: u32,
    #[serde(default)]
    pub notes: u32,
    #[serde(default)]
    pub backup_jobs: u32,
    #[serde(default)]
    pub device_labels: u32,
    /// Library quests (1.4).
    #[serde(default)]
    pub quests: u32,
    pub services: u32,
    pub tunnels: u32,
    pub integrations: u32,
    pub bookmarks: u32,
    pub settings: bool,
    pub secrets: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ImportMode {
    /// Keep what's here; add what's new.
    Merge,
    /// Clear the config tables and load the backup as is.
    Replace,
}

/// What an import changed. The id maps let the caller move keychain secrets.
#[derive(Debug, Default)]
pub struct ImportOutcome {
    /// Rows added per table.
    pub added: HashMap<&'static str, u32>,
    /// Rows skipped as duplicates of existing ones (merge only).
    pub skipped: HashMap<&'static str, u32>,
    /// Old host id → new host id, for hosts that were inserted.
    pub hosts: HashMap<i64, i64>,
    /// Old integration id → new integration id, for inserted integrations.
    pub integrations: HashMap<i64, i64>,
    /// Hosts and integrations that existed before a replace (their keychain
    /// entries now belong to nothing, or worse, to a different row).
    pub removed_hosts: Vec<HostId>,
    pub removed_integrations: Vec<IntegrationId>,
    /// The bundle's `settings` rows were applied.
    pub settings_applied: bool,
    pub replaced: bool,
}

fn bad(reason: impl Into<String>) -> Error {
    Error::Invalid { field: "backup", reason: reason.into() }
}

fn to_json(v: SqlValue) -> Result<Value> {
    Ok(match v {
        SqlValue::Null => Value::Null,
        SqlValue::Integer(i) => Value::from(i),
        SqlValue::Real(f) => Value::from(f),
        SqlValue::Text(s) => Value::String(s),
        SqlValue::Blob(_) => return Err(bad("binary column in a config table")),
    })
}

fn to_sql(v: &Value) -> Result<SqlValue> {
    Ok(match v {
        Value::Null => SqlValue::Null,
        Value::Bool(b) => SqlValue::Integer(i64::from(*b)),
        Value::Number(n) => match n.as_i64() {
            Some(i) => SqlValue::Integer(i),
            None => SqlValue::Real(n.as_f64().ok_or_else(|| bad("number out of range"))?),
        },
        Value::String(s) => SqlValue::Text(s.clone()),
        _ => return Err(bad("nested value in a row")),
    })
}

fn columns(tx: &rusqlite::Connection, table: &str) -> Result<Vec<String>> {
    let mut st = tx.prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))?;
    let cols = st.query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(cols)
}

/// Reads the config tables into a bundle. Secrets are added separately with
/// [`collect_secrets`].
pub fn snapshot(store: &Store, app_version: &str, now: i64) -> Result<Bundle> {
    let conn = store.conn();
    let mut tables = HashMap::new();
    for t in TABLES {
        let cols = columns(conn, t.name)?;
        let mut st = conn.prepare(&format!("SELECT * FROM {} ORDER BY rowid", t.name))?;
        let mut rows = st.query([])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let mut m = Map::new();
            for (i, c) in cols.iter().enumerate() {
                m.insert(c.clone(), to_json(r.get::<_, SqlValue>(i)?)?);
            }
            out.push(m);
        }
        tables.insert(t.name.to_owned(), out);
    }
    Ok(Bundle {
        format: FORMAT.into(),
        version: VERSION,
        app_version: app_version.into(),
        schema: store.schema_version()?,
        created_at: now,
        tables,
        secrets: Vec::new(),
    })
}

fn ids(store: &Store, table: &str) -> Result<Vec<i64>> {
    let mut st = store.conn().prepare(&format!("SELECT id FROM {table} ORDER BY id"))?;
    let v = st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<i64>>>()?;
    Ok(v)
}

/// Every keychain entry that can belong to the current hosts and integrations.
pub fn secret_refs(store: &Store) -> Result<Vec<(SecretOwner, SecretRef)>> {
    let mut out = Vec::new();
    for id in ids(store, "hosts")? {
        let id = id as HostId;
        for k in SecretKind::HOST {
            out.push((SecretOwner::Host { id }, SecretRef::host(k, id)));
        }
    }
    for id in ids(store, "integrations")? {
        let id = id as IntegrationId;
        out.push((SecretOwner::Integration { id }, SecretRef::integration(id)));
    }
    out.push((SecretOwner::AlertsWebhook, SecretRef::alerts_webhook()));
    for key_id in store.env_key_ids()? {
        let r = SecretRef::vault(&key_id);
        out.push((SecretOwner::Vault { key_id }, r));
    }
    Ok(out)
}

/// Adds the saved keychain secrets to `bundle`. Returns how many were found.
pub fn collect_secrets(store: &Store, keychain: &dyn SecretStore, bundle: &mut Bundle) -> Result<usize> {
    for (owner, r) in secret_refs(store)? {
        if let Some(s) = keychain.get(&r)? {
            bundle.secrets.push(BundledSecret { owner, kind: r.kind.as_str().into(), value: s.expose().to_owned() });
        }
    }
    Ok(bundle.secrets.len())
}

fn passphrase(p: &str) -> SecretString {
    SecretString::from(p.to_owned())
}

/// Serializes and encrypts a bundle with an age passphrase.
pub fn encrypt(bundle: &Bundle, pass: &str) -> Result<Vec<u8>> {
    encrypt_with(bundle, pass, None)
}

fn encrypt_with(bundle: &Bundle, pass: &str, work_factor: Option<u8>) -> Result<Vec<u8>> {
    if pass.chars().count() < MIN_PASSPHRASE {
        return Err(Error::Invalid {
            field: "passphrase",
            reason: format!("use at least {MIN_PASSPHRASE} characters"),
        });
    }
    let json = Zeroizing::new(serde_json::to_vec(bundle)?);
    let mut r = age::scrypt::Recipient::new(passphrase(pass));
    if let Some(n) = work_factor {
        r.set_work_factor(n);
    }
    age::encrypt(&r, &json).map_err(|e| bad(format!("couldn't encrypt: {e}")))
}

/// Decrypts and parses a backup file.
pub fn decrypt(bytes: &[u8], pass: &str) -> Result<Bundle> {
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(bad("the file is too big to be a Nightgrid backup"));
    }
    if !bytes.starts_with(b"age-encryption.org/v1") {
        return Err(bad("not a Nightgrid backup (not an age file)"));
    }
    let id = age::scrypt::Identity::new(passphrase(pass));
    let json = Zeroizing::new(age::decrypt(&id, bytes).map_err(|e| match e {
        age::DecryptError::DecryptionFailed | age::DecryptError::NoMatchingKeys => {
            bad("wrong passphrase, or the file isn't a passphrase-encrypted backup")
        }
        age::DecryptError::ExcessiveWork { .. } => bad("the file asks for too much work to decrypt"),
        other => bad(format!("couldn't decrypt: {other}")),
    })?);
    let b: Bundle = serde_json::from_slice(&json).map_err(|_| bad("not a Nightgrid backup (unexpected contents)"))?;
    if b.format != FORMAT {
        return Err(bad("not a Nightgrid backup"));
    }
    if b.version > VERSION {
        return Err(bad("made by a newer Nightgrid; update this one first"));
    }
    Ok(b)
}

impl Bundle {
    fn count(&self, t: &str) -> u32 {
        self.tables.get(t).map_or(0, |v| v.len() as u32)
    }

    pub fn summary(&self) -> BackupSummary {
        BackupSummary {
            app_version: self.app_version.clone(),
            created_at: self.created_at as f64,
            hosts: self.count("hosts"),
            groups: self.count("host_groups"),
            tags: self.count("tags"),
            snippets: self.count("snippets"),
            playbooks: self.count("playbooks"),
            notes: self.count("notes"),
            backup_jobs: self.count("backup_jobs"),
            device_labels: self.count("device_labels"),
            quests: self.count("quests"),
            services: self.count("services"),
            tunnels: self.count("tunnels"),
            integrations: self.count("integrations"),
            bookmarks: self.count("bookmarks"),
            settings: self.count("settings") > 0,
            secrets: self.secrets.len() as u32,
        }
    }
}

fn find_existing(tx: &Transaction<'_>, t: &Table, row: &Map<String, Value>) -> Result<Option<Option<i64>>> {
    for key in t.keys {
        let first = row.get(key[0]).unwrap_or(&Value::Null);
        if first.is_null() {
            continue;
        }
        let cond = key.iter().map(|c| format!("{c} IS ?")).collect::<Vec<_>>().join(" AND ");
        let sel = if t.id { "id" } else { "NULL" };
        let vals = key.iter().map(|c| to_sql(row.get(*c).unwrap_or(&Value::Null))).collect::<Result<Vec<_>>>()?;
        let hit: Option<Option<i64>> = tx
            .query_row(&format!("SELECT {sel} FROM {} WHERE {cond} LIMIT 1", t.name), params_from_iter(vals), |r| {
                r.get(0)
            })
            .optional()?;
        if hit.is_some() {
            return Ok(hit);
        }
    }
    Ok(None)
}

/// Loads a bundle into the store. The caller handles keychain secrets with
/// the returned id maps (see [`restore_secrets`]).
pub fn import(store: &mut Store, bundle: &Bundle, mode: ImportMode) -> Result<ImportOutcome> {
    let ours = store.schema_version()?;
    if bundle.schema > ours {
        return Err(bad(format!(
            "made by a newer Nightgrid (data version {} > {ours}); update this one first",
            bundle.schema
        )));
    }
    let mut out = ImportOutcome { replaced: mode == ImportMode::Replace, ..Default::default() };
    if out.replaced {
        out.removed_hosts = ids(store, "hosts")?.into_iter().map(|i| i as HostId).collect();
        out.removed_integrations = ids(store, "integrations")?.into_iter().map(|i| i as IntegrationId).collect();
    }
    store.tx(|tx| {
        if mode == ImportMode::Replace {
            for t in TABLES.iter().rev() {
                tx.execute(&format!("DELETE FROM {}", t.name), [])?;
            }
        }
        // Old id → new id per table.
        let mut maps: HashMap<&str, HashMap<i64, i64>> = HashMap::new();
        for t in TABLES {
            let Some(rows) = bundle.tables.get(t.name) else { continue };
            let cols = columns(tx, t.name)?;
            let mut map = HashMap::new();
            'rows: for src in rows {
                let mut row = src.clone();
                for (col, target, missing) in t.fks {
                    let Some(old) = row.get(*col).and_then(Value::as_i64) else { continue };
                    match maps.get(target).and_then(|m| m.get(&old)) {
                        Some(new) => {
                            row.insert((*col).to_owned(), Value::from(*new));
                        }
                        None => match missing {
                            Missing::Null => {
                                row.insert((*col).to_owned(), Value::Null);
                            }
                            Missing::Skip => continue 'rows,
                        },
                    }
                }
                let old_id = row.get("id").and_then(Value::as_i64);
                if mode == ImportMode::Merge {
                    if let Some(hit) = find_existing(tx, t, &row)? {
                        if let (Some(o), Some(n)) = (old_id, hit) {
                            map.insert(o, n);
                        }
                        *out.skipped.entry(t.name).or_default() += 1;
                        continue;
                    }
                }
                let keep: Vec<&String> = cols
                    .iter()
                    .filter(|c| row.contains_key(c.as_str()) && !(mode == ImportMode::Merge && t.id && *c == "id"))
                    .collect();
                if keep.is_empty() {
                    continue;
                }
                let names = keep.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", ");
                let marks = vec!["?"; keep.len()].join(", ");
                let vals = keep.iter().map(|c| to_sql(&row[c.as_str()])).collect::<Result<Vec<_>>>()?;
                tx.execute(&format!("INSERT INTO {} ({names}) VALUES ({marks})", t.name), params_from_iter(vals))
                    .map_err(|e| bad(format!("{}: {e}", t.name)))?;
                if let Some(o) = old_id {
                    let new = tx.last_insert_rowid();
                    map.insert(o, new);
                    match t.name {
                        "hosts" => {
                            out.hosts.insert(o, new);
                        }
                        "integrations" => {
                            out.integrations.insert(o, new);
                        }
                        _ => {}
                    }
                }
                if t.name == "settings" {
                    out.settings_applied = true;
                }
                *out.added.entry(t.name).or_default() += 1;
            }
            maps.insert(t.name, map);
        }
        // Wake relays refer to hosts by id: restore them with the new ids.
        if let (Some(rows), Some(map)) = (bundle.tables.get("hosts"), maps.get("hosts")) {
            for r in rows {
                let (Some(old), Some(relay)) =
                    (r.get("id").and_then(Value::as_i64), r.get("wake_relay_id").and_then(Value::as_i64))
                else {
                    continue;
                };
                if let (Some(new), Some(new_relay)) = (map.get(&old), map.get(&relay)) {
                    if out.hosts.contains_key(&old) {
                        tx.execute("UPDATE hosts SET wake_relay_id = ?2 WHERE id = ?1", [new, new_relay])?;
                    }
                }
            }
        }
        Ok(())
    })?;
    Ok(out)
}

/// Keychain side of an import: after a replace, forgets entries of rows that
/// are gone; then saves bundled secrets for rows that were inserted (an
/// existing row's secret is never overwritten). Returns how many were saved.
pub fn restore_secrets(keychain: &dyn SecretStore, bundle: &Bundle, outcome: &ImportOutcome) -> Result<usize> {
    for id in &outcome.removed_hosts {
        for k in SecretKind::HOST {
            keychain.delete(&SecretRef::host(k, *id))?;
        }
    }
    for id in &outcome.removed_integrations {
        keychain.delete(&SecretRef::integration(*id))?;
    }
    let mut saved = 0;
    for s in &bundle.secrets {
        let Some(kind) = SecretKind::parse(&s.kind) else { continue };
        let r = match s.owner {
            SecretOwner::Host { id } if SecretKind::HOST.contains(&kind) => match outcome.hosts.get(&i64::from(id)) {
                Some(new) => SecretRef::host(kind, *new as HostId),
                None => continue,
            },
            SecretOwner::Integration { id } if kind == SecretKind::IntegrationToken => {
                match outcome.integrations.get(&i64::from(id)) {
                    Some(new) => SecretRef::integration(*new as IntegrationId),
                    None => continue,
                }
            }
            SecretOwner::AlertsWebhook if kind == SecretKind::IntegrationToken => {
                let r = SecretRef::alerts_webhook();
                if !outcome.replaced && keychain.get(&r)?.is_some() {
                    continue;
                }
                r
            }
            // Same id = same key: saving it again changes nothing.
            SecretOwner::Vault { ref key_id }
                if kind == SecretKind::VaultKey && crate::envvault::valid_key_id(key_id) =>
            {
                SecretRef::vault(key_id)
            }
            _ => continue,
        };
        keychain.set(&r, &Secret::new(s.value.as_str()))?;
        if let SecretOwner::Vault { key_id } = &s.owner {
            crate::envvault::adopt_key(keychain, key_id)?;
        }
        saved += 1;
    }
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemoryStore;

    fn seed(s: &Store) {
        let c = s.conn();
        c.execute_batch(
            "INSERT INTO host_groups (id, name) VALUES (5, 'web');
             INSERT INTO tags (id, name) VALUES (7, 'prod');
             INSERT INTO hosts (id, name, hostname, group_id, ssh_alias, remember_secret, auth_method)
                VALUES (3, 'server01', '10.0.0.1', 5, 's1', 1, 'password'),
                       (4, 'server02', '10.0.0.2', NULL, NULL, 0, 'agent');
             INSERT INTO host_tags VALUES (3, 7);
             INSERT INTO bookmarks (host_id, path, label) VALUES (3, '/etc', 'etc'), (NULL, '/tmp', 'tmp');
             INSERT INTO stack_dirs (host_id, path) VALUES (4, '/opt/stacks');
             INSERT INTO services (name, url, host_id) VALUES ('Wiki', 'https://wiki.example', 3);
             INSERT INTO tunnels (host_id, kind, bind_addr, bind_port, dest_host, dest_port)
                VALUES (3, 'local', '127.0.0.1', 8080, 'localhost', 80);
             INSERT INTO integrations (id, kind, name, base_url) VALUES (9, 'kuma', 'Kuma', 'https://kuma.example');
             INSERT INTO snippets (name, command, scope, group_id) VALUES ('Disk', 'df -h', 'group', 5);
             INSERT INTO playbooks (name, steps) VALUES ('Deploy', '[]');",
        )
        .unwrap();
        s.set_setting("app", &serde_json::json!({ "theme": "ice" })).unwrap();
    }

    fn count(s: &Store, t: &str) -> i64 {
        s.conn().query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get(0)).unwrap()
    }

    fn roundtrip(b: &Bundle) -> Bundle {
        let bytes = encrypt_with(b, "correct horse", Some(2)).unwrap();
        decrypt(&bytes, "correct horse").unwrap()
    }

    #[test]
    fn encrypts_and_rejects_wrong_passphrase() {
        let s = Store::open_in_memory().unwrap();
        seed(&s);
        let b = snapshot(&s, "0.9.0", 100).unwrap();
        let bytes = encrypt_with(&b, "correct horse", Some(2)).unwrap();
        assert!(!bytes.windows(8).any(|w| w == b"server01"), "plaintext leaked");
        assert!(decrypt(&bytes, "wrong horse").is_err());
        assert!(decrypt(b"{\"format\":\"nightgrid-backup\"}", "x").is_err());
        assert!(encrypt(&b, "short").is_err());
        let back = decrypt(&bytes, "correct horse").unwrap();
        let sum = back.summary();
        assert_eq!((sum.hosts, sum.groups, sum.snippets, sum.services, sum.integrations), (2, 1, 1, 1, 1));
        assert!(sum.settings);
    }

    #[test]
    fn replace_into_empty_store_keeps_everything() {
        let a = Store::open_in_memory().unwrap();
        seed(&a);
        let kc = MemoryStore::new();
        kc.set(&SecretRef::host(SecretKind::HostPassword, 3), &Secret::new("pw")).unwrap();
        kc.set(&SecretRef::integration(9), &Secret::new("tok")).unwrap();
        let mut b = snapshot(&a, "0.9.0", 1).unwrap();
        assert_eq!(collect_secrets(&a, &kc, &mut b).unwrap(), 2);
        let b = roundtrip(&b);

        let mut z = Store::open_in_memory().unwrap();
        // Something that exists only here and must go away.
        z.conn().execute("INSERT INTO hosts (id, name, hostname) VALUES (3, 'old', 'old.example')", []).unwrap();
        let kc2 = MemoryStore::new();
        kc2.set(&SecretRef::host(SecretKind::SudoPassword, 3), &Secret::new("stale")).unwrap();
        let out = import(&mut z, &b, ImportMode::Replace).unwrap();
        assert_eq!(out.removed_hosts, vec![3]);
        assert_eq!(restore_secrets(&kc2, &b, &out).unwrap(), 2);
        for t in TABLES {
            assert_eq!(count(&z, t.name), count(&a, t.name), "{}", t.name);
        }
        let name: String = z.conn().query_row("SELECT name FROM hosts WHERE id = 3", [], |r| r.get(0)).unwrap();
        assert_eq!(name, "server01");
        assert!(kc2.get(&SecretRef::host(SecretKind::SudoPassword, 3)).unwrap().is_none(), "stale secret kept");
        assert_eq!(kc2.get(&SecretRef::host(SecretKind::HostPassword, 3)).unwrap().unwrap().expose(), "pw");
        assert_eq!(kc2.get(&SecretRef::integration(9)).unwrap().unwrap().expose(), "tok");
        assert_eq!(z.get_setting::<Value>("app").unwrap().unwrap()["theme"], "ice");
    }

    #[test]
    fn merge_skips_duplicates_and_remaps_ids() {
        let a = Store::open_in_memory().unwrap();
        seed(&a);
        let kc = MemoryStore::new();
        kc.set(&SecretRef::host(SecretKind::HostPassword, 3), &Secret::new("pw3")).unwrap();
        kc.set(&SecretRef::host(SecretKind::HostPassword, 4), &Secret::new("pw4")).unwrap();
        let mut b = snapshot(&a, "0.9.0", 1).unwrap();
        collect_secrets(&a, &kc, &mut b).unwrap();
        let b = roundtrip(&b);

        let mut z = Store::open_in_memory().unwrap();
        z.conn()
            .execute_batch(
                "INSERT INTO host_groups (id, name) VALUES (1, 'other'), (2, 'web');
                 INSERT INTO hosts (id, name, hostname, ssh_alias) VALUES (1, 'mine', 'h.example', 's1'),
                                                                    (2, 'filler', 'f.example', NULL);",
            )
            .unwrap();
        z.set_setting("app", &serde_json::json!({ "theme": "plain" })).unwrap();
        let kc2 = MemoryStore::new();
        kc2.set(&SecretRef::host(SecretKind::HostPassword, 1), &Secret::new("keep")).unwrap();

        let out = import(&mut z, &b, ImportMode::Merge).unwrap();
        // server01 matched by alias s1 (existing id 1), server02 is new.
        assert_eq!(out.skipped["hosts"], 1);
        assert_eq!(out.added["hosts"], 1);
        assert_eq!(count(&z, "hosts"), 3);
        assert_eq!(count(&z, "host_groups"), 2);
        let new02 = out.hosts[&4];
        assert!(new02 > 2);
        // The tunnel and bookmark of server01 now point at the existing host 1.
        let th: i64 = z.conn().query_row("SELECT host_id FROM tunnels", [], |r| r.get(0)).unwrap();
        assert_eq!(th, 1);
        let sd: i64 = z.conn().query_row("SELECT host_id FROM stack_dirs", [], |r| r.get(0)).unwrap();
        assert_eq!(sd, new02);
        let sg: i64 = z.conn().query_row("SELECT group_id FROM snippets", [], |r| r.get(0)).unwrap();
        assert_eq!(sg, 2, "snippet follows the existing 'web' group");
        // Existing settings win when merging.
        assert!(!out.settings_applied);
        assert_eq!(z.get_setting::<Value>("app").unwrap().unwrap()["theme"], "plain");
        // Secrets: only the inserted host gets one; the existing one is untouched.
        assert_eq!(restore_secrets(&kc2, &b, &out).unwrap(), 1);
        assert_eq!(kc2.get(&SecretRef::host(SecretKind::HostPassword, 1)).unwrap().unwrap().expose(), "keep");
        assert_eq!(
            kc2.get(&SecretRef::host(SecretKind::HostPassword, new02 as HostId)).unwrap().unwrap().expose(),
            "pw4"
        );

        // Merging the same file again adds nothing.
        let again = import(&mut z, &b, ImportMode::Merge).unwrap();
        assert!(again.added.is_empty(), "{:?}", again.added);
    }

    #[test]
    fn refuses_newer_schema_and_tolerates_unknown_columns() {
        let a = Store::open_in_memory().unwrap();
        seed(&a);
        let mut b = snapshot(&a, "0.9.0", 1).unwrap();
        for r in b.tables.get_mut("hosts").unwrap() {
            r.insert("future_column".into(), Value::from(1));
        }
        let mut z = Store::open_in_memory().unwrap();
        import(&mut z, &b, ImportMode::Replace).unwrap();
        assert_eq!(count(&z, "hosts"), 2);
        b.schema += 1;
        assert!(import(&mut z, &b, ImportMode::Replace).is_err());
    }

    #[test]
    fn a_bad_row_rolls_back_everything() {
        let a = Store::open_in_memory().unwrap();
        seed(&a);
        let mut b = snapshot(&a, "0.9.0", 1).unwrap();
        b.tables.get_mut("playbooks").unwrap()[0].insert("stop_on_error".into(), Value::from(7));
        let mut z = Store::open_in_memory().unwrap();
        z.conn().execute("INSERT INTO hosts (name, hostname) VALUES ('keep', 'k')", []).unwrap();
        assert!(import(&mut z, &b, ImportMode::Replace).is_err());
        assert_eq!(count(&z, "hosts"), 1);
    }

    #[test]
    fn secret_owner_json_shape() {
        let s = BundledSecret { owner: SecretOwner::Host { id: 3 }, kind: "host-password".into(), value: "x".into() };
        let j = serde_json::to_value(&s).unwrap();
        assert_eq!(j, serde_json::json!({ "owner": "host", "id": 3, "kind": "host-password", "value": "x" }));
    }

    #[test]
    fn vault_values_travel_with_their_key() {
        let mut a = Store::open_in_memory().unwrap();
        seed(&a);
        let kc = MemoryStore::new();
        let input = crate::envvault::EnvVarInput { name: "DB_PASS".into(), value: Some("s3cr3t".into()), secret: true };
        a.set_env_var(&kc, 4, "app", None, &input).unwrap();
        let mut b = snapshot(&a, "0.10.37", 1).unwrap();
        assert_eq!(collect_secrets(&a, &kc, &mut b).unwrap(), 1);
        let b = roundtrip(&b);
        let j = serde_json::to_value(&b.secrets[0]).unwrap();
        assert_eq!(j["owner"], "vault");
        assert!(crate::envvault::valid_key_id(j["keyId"].as_str().unwrap()));

        let mut z = Store::open_in_memory().unwrap();
        let kc2 = MemoryStore::new();
        let out = import(&mut z, &b, ImportMode::Merge).unwrap();
        assert_eq!(restore_secrets(&kc2, &b, &out).unwrap(), 1);
        let host = out.hosts[&4] as HostId;
        let vals = z.env_values(&kc2, host, "app").unwrap();
        assert_eq!(vals[0].1.as_str(), "s3cr3t");
        // The imported key became this computer's key.
        assert_eq!(crate::envvault::current_key(&kc2).unwrap().0, j["keyId"].as_str().unwrap());
    }
}
