//! SQLite storage with forward-only migrations.
//!
//! The schema version is tracked in `PRAGMA user_version` (rusqlite_migration).
//! Add a migration by appending a new `M::up(include_str!(...))` to [`migrations`];
//! never edit a migration that has shipped.

use std::path::Path;
use std::sync::LazyLock;

use rusqlite::{Connection, OptionalExtension};
use rusqlite_migration::{Migrations, M};
use serde::{de::DeserializeOwned, Serialize};

use crate::Result;

static MIGRATIONS: LazyLock<Migrations<'static>> = LazyLock::new(|| {
    Migrations::new(vec![
        M::up(include_str!("migrations/0001_init.sql")),
        M::up(include_str!("migrations/0002_host_auth.sql")),
        M::up(include_str!("migrations/0003_bookmarks.sql")),
        M::up(include_str!("migrations/0004_stack_dirs.sql")),
        M::up(include_str!("migrations/0005_metrics.sql")),
        M::up(include_str!("migrations/0006_services.sql")),
        M::up(include_str!("migrations/0007_tunnels.sql")),
        M::up(include_str!("migrations/0008_integrations.sql")),
        M::up(include_str!("migrations/0009_automation.sql")),
        M::up(include_str!("migrations/0010_metrics_gpu.sql")),
        M::up(include_str!("migrations/0011_notes.sql")),
        M::up(include_str!("migrations/0012_stack_snapshots.sql")),
        M::up(include_str!("migrations/0013_backup_jobs.sql")),
        M::up(include_str!("migrations/0014_note_drift.sql")),
        M::up(include_str!("migrations/0015_device_labels.sql")),
        M::up(include_str!("migrations/0016_host_tailnet.sql")),
        M::up(include_str!("migrations/0017_host_power.sql")),
        M::up(include_str!("migrations/0018_host_events.sql")),
        M::up(include_str!("migrations/0019_watch_items.sql")),
        M::up(include_str!("migrations/0020_disk_usage.sql")),
        M::up(include_str!("migrations/0021_backup_tests.sql")),
        M::up(include_str!("migrations/0022_alert_deps.sql")),
        M::up(include_str!("migrations/0023_note_runbooks.sql")),
        M::up(include_str!("migrations/0024_ups_watches.sql")),
        M::up(include_str!("migrations/0025_uptime.sql")),
        M::up(include_str!("migrations/0026_metrics_hourly.sql")),
        M::up(include_str!("migrations/0027_log_searches.sql")),
        M::up(include_str!("migrations/0028_env_vault.sql")),
        M::up(include_str!("migrations/0029_git_stacks.sql")),
        M::up(include_str!("migrations/0030_metrics_gpu_more.sql")),
        M::up(include_str!("migrations/0031_net_tests.sql")),
        M::up(include_str!("migrations/0032_cmd_widgets.sql")),
        M::up(include_str!("migrations/0033_host_role.sql")),
        M::up(include_str!("migrations/0034_log_patterns.sql")),
        M::up(include_str!("migrations/0035_net_devices.sql")),
        M::up(include_str!("migrations/0036_netmap_layout.sql")),
        M::up(include_str!("migrations/0037_drift_lookups.sql")),
        M::up(include_str!("migrations/0038_host_alert_offline.sql")),
        M::up(include_str!("migrations/0039_net_networks.sql")),
        M::up(include_str!("migrations/0040_journal.sql")),
        M::up(include_str!("migrations/0041_host_power.sql")),
        M::up(include_str!("migrations/0042_node.sql")),
        M::up(include_str!("migrations/0043_notices.sql")),
        M::up(include_str!("migrations/0044_posture.sql")),
        M::up(include_str!("migrations/0045_secrets.sql")),
        M::up(include_str!("migrations/0046_drills.sql")),
        M::up(include_str!("migrations/0047_host_specs.sql")),
        M::up(include_str!("migrations/0048_quests.sql")),
        M::up(include_str!("migrations/0049_integration_kinds.sql")),
        M::up(include_str!("migrations/0050_integration_kinds_open.sql")),
        M::up(include_str!("migrations/0051_uptime_cleanup.sql")),
        M::up(include_str!("migrations/0052_expiry_prefs.sql")),
        M::up(include_str!("migrations/0053_host_os.sql")),
        M::up(include_str!("migrations/0054_update_holds.sql")),
        M::up(include_str!("migrations/0055_host_docker_user.sql")),
    ])
});

/// The migration list, exposed for tests and diagnostics.
pub fn migrations() -> &'static Migrations<'static> {
    &MIGRATIONS
}

/// Handle to the app database. Not `Sync`; wrap it in a `Mutex` when sharing.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Opens (or creates) the database file and runs pending migrations.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    /// In-memory database, for tests.
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        MIGRATIONS.to_latest(&mut conn)?;
        Ok(Self { conn })
    }

    /// Runs `f` inside a transaction that commits on `Ok`.
    pub(crate) fn tx<T>(&mut self, f: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T>) -> Result<T> {
        let tx = self.conn.transaction()?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }

    /// Current schema version (`PRAGMA user_version`).
    pub fn schema_version(&self) -> Result<i64> {
        Ok(self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
    }

    /// Reads a JSON setting. Returns `None` when the key is unset.
    pub fn get_setting<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let raw: Option<String> =
            self.conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).optional()?;
        raw.map(|s| serde_json::from_str(&s).map_err(Into::into)).transpose()
    }

    /// Writes a JSON setting (upsert).
    pub fn set_setting<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value)?;
        self.conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, unixepoch())
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            (key, json),
        )?;
        Ok(())
    }

    /// Raw connection access for domain modules inside the crate.
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    pub(crate) fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LATEST: i64 = 55;

    #[test]
    fn migrations_are_valid() {
        migrations().validate().expect("migrations validate");
    }

    #[test]
    fn opens_in_memory_at_latest_version() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.schema_version().unwrap(), LATEST);
    }

    #[test]
    fn opens_file_and_reopens_idempotently() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("nightgrid.db");
        {
            let s = Store::open(&path).unwrap();
            s.set_setting("k", &42u32).unwrap();
        }
        let s = Store::open(&path).unwrap();
        assert_eq!(s.schema_version().unwrap(), LATEST);
        assert_eq!(s.get_setting::<u32>("k").unwrap(), Some(42));
    }

    #[test]
    fn settings_roundtrip_and_upsert() {
        let s = Store::open_in_memory().unwrap();
        assert_eq!(s.get_setting::<String>("missing").unwrap(), None);
        s.set_setting("theme", &"ice").unwrap();
        s.set_setting("theme", &"nightgrid").unwrap();
        assert_eq!(s.get_setting::<String>("theme").unwrap().as_deref(), Some("nightgrid"));
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let s = Store::open_in_memory().unwrap();
        let err =
            s.conn().execute("INSERT INTO hosts (name, hostname, group_id) VALUES ('x', 'x', 999)", []).unwrap_err();
        assert!(err.to_string().contains("FOREIGN KEY"), "{err}");
    }

    #[test]
    fn schema_has_no_secret_columns() {
        // Guard rail for security rule 1: no plaintext secrets in SQLite.
        let s = Store::open_in_memory().unwrap();
        let mut stmt = s
            .conn()
            .prepare(
                "SELECT m.name, p.name FROM sqlite_master m JOIN pragma_table_info(m.name) p WHERE m.type = 'table'",
            )
            .unwrap();
        let cols: Vec<(String, String)> =
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<std::result::Result<_, _>>().unwrap();
        for (table, col) in cols {
            let c = col.to_lowercase();
            // Boolean opt-in flags (e.g. `remember_secret`) hold no secret material.
            if c.starts_with("remember_") {
                continue;
            }
            for bad in ["password", "passphrase", "secret", "token", "private_key"] {
                assert!(!c.contains(bad), "{table}.{col} looks like a secret column");
            }
        }
    }
}
