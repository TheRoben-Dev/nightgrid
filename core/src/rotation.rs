//! Secrets & rotation (1.1, D-169): every secret Nightgrid knows about in one
//! list, with how old it is and when it's due for rotation. Values are never
//! read here: only names, places and dates. Sources are the SSH keys in
//! `~/.ssh` (their file dates), `.env` vault values marked secret (when they
//! were last written), integration tokens, Nightgrid's own tokens (MCP,
//! status page, ntfy, webhook), host passwords kept in the keychain,
//! certificates and dates from the expiry watch, and secrets kept outside
//! Nightgrid that the user lists by hand (a router password, a DNS API
//! token). When Nightgrid sets a secret it stamps the date; "Mark rotated"
//! does the same for anything rotated elsewhere.

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

const DAY: i64 = 86_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SecretKind {
    SshKey,
    /// A `.env` vault value marked secret.
    Vault,
    Integration,
    /// Nightgrid's own tokens: MCP, status page, ntfy, webhook.
    AppToken,
    /// A host's SSH or sudo password kept in the keychain.
    HostPassword,
    /// A certificate, domain or token date from the expiry watch.
    Expiry,
    /// Listed by hand: kept outside Nightgrid.
    Manual,
}

impl SecretKind {
    /// Rotate every this many days unless the user picks another policy.
    pub fn default_days(self) -> Option<u32> {
        match self {
            SecretKind::SshKey => Some(365),
            SecretKind::Vault => Some(365),
            SecretKind::Integration => Some(180),
            SecretKind::AppToken => Some(180),
            SecretKind::HostPassword => Some(365),
            SecretKind::Manual => Some(365),
            // Expiry items have their own dates.
            SecretKind::Expiry => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum RotationState {
    Ok,
    /// Due within 30 days.
    DueSoon,
    Overdue,
    /// No date known: never stamped.
    Unknown,
    /// No rotation policy (turned off for this item).
    NoPolicy,
}

/// One secret in the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SecretItem {
    /// Stable id: `ssh:<fingerprint>`, `vault:<id>`, `integration:<id>`,
    /// `app:mcp`, `host:<id>`, `expiry:<id>`, `manual:<id>`.
    pub key: String,
    pub kind: SecretKind,
    pub name: String,
    /// Where it's used, in words (hosts, a stack, a server).
    pub place: String,
    pub host_id: Option<HostId>,
    /// When it was created or last rotated (unix seconds).
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub rotated_at: Option<i64>,
    /// Rotate every this many days; none = no policy.
    pub policy_days: Option<u32>,
    /// When it's due (or expires, for expiry items).
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub due_at: Option<i64>,
    pub state: RotationState,
    pub note: String,
}

/// What a source knows about one secret, before the policy is applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub key: String,
    pub kind: SecretKind,
    pub name: String,
    pub place: String,
    pub host_id: Option<HostId>,
    /// Date from the source itself (file time, `updated_at`).
    pub dated: Option<i64>,
    /// A hard expiry date (certificates).
    pub expires_at: Option<i64>,
    pub warn_days: u32,
    pub note: String,
}

impl Found {
    pub fn new(key: impl Into<String>, kind: SecretKind, name: impl Into<String>, place: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            kind,
            name: name.into(),
            place: place.into(),
            host_id: None,
            dated: None,
            expires_at: None,
            warn_days: 30,
            note: String::new(),
        }
    }
}

/// Stored per key: when it was last rotated (stamped) and its policy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Meta {
    pub rotated_at: Option<i64>,
    /// `Some(0)` = no policy; `None` = the kind's default.
    pub policy_days: Option<u32>,
    /// Due date that was last notified, so each due date notifies once.
    pub notified_due: Option<i64>,
}

/// Applies stamps and policy to what a source found.
pub fn item(f: Found, meta: Option<&Meta>, now: i64) -> SecretItem {
    let m = meta.cloned().unwrap_or_default();
    // A stamp newer than the source's own date wins (rotated elsewhere).
    let rotated_at = match (f.dated, m.rotated_at) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    };
    if f.kind == SecretKind::Expiry {
        let state = match f.expires_at {
            None => RotationState::Unknown,
            Some(e) if e <= now => RotationState::Overdue,
            Some(e) if e - now <= i64::from(f.warn_days) * DAY => RotationState::DueSoon,
            Some(_) => RotationState::Ok,
        };
        return SecretItem {
            key: f.key,
            kind: f.kind,
            name: f.name,
            place: f.place,
            host_id: f.host_id,
            rotated_at,
            policy_days: None,
            due_at: f.expires_at,
            state,
            note: f.note,
        };
    }
    let policy = match m.policy_days {
        Some(0) => None,
        Some(d) => Some(d),
        None => f.kind.default_days(),
    };
    let due_at = policy.zip(rotated_at).map(|(d, r)| r + i64::from(d) * DAY);
    let state = match (policy, due_at) {
        (None, _) => RotationState::NoPolicy,
        (Some(_), None) => RotationState::Unknown,
        (Some(_), Some(d)) if d <= now => RotationState::Overdue,
        (Some(_), Some(d)) if d - now <= 30 * DAY => RotationState::DueSoon,
        _ => RotationState::Ok,
    };
    SecretItem {
        key: f.key,
        kind: f.kind,
        name: f.name,
        place: f.place,
        host_id: f.host_id,
        rotated_at,
        policy_days: policy,
        due_at,
        state,
        note: f.note,
    }
}

/// Overdue first, then due soon, unknown, ok; by due date within each.
pub fn sort(items: &mut [SecretItem]) {
    let rank = |s: RotationState| match s {
        RotationState::Overdue => 0,
        RotationState::DueSoon => 1,
        RotationState::Unknown => 2,
        RotationState::Ok => 3,
        RotationState::NoPolicy => 4,
    };
    items.sort_by(|a, b| {
        rank(a.state).cmp(&rank(b.state)).then(a.due_at.unwrap_or(i64::MAX).cmp(&b.due_at.unwrap_or(i64::MAX)))
    });
}

/// Items whose due date deserves a notice now (overdue or due soon, and
/// not notified for this due date yet).
pub fn to_notify<'a>(items: &'a [SecretItem], metas: &[(String, Meta)]) -> Vec<&'a SecretItem> {
    items
        .iter()
        .filter(|i| matches!(i.state, RotationState::Overdue | RotationState::DueSoon))
        .filter(|i| {
            let notified = metas.iter().find(|(k, _)| *k == i.key).and_then(|(_, m)| m.notified_due);
            notified != i.due_at
        })
        .collect()
}

// ---------------------------------------------------------- manual items

/// A secret kept outside Nightgrid, listed by hand. No value is stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ManualSecret {
    /// `None` adds a new one.
    pub id: Option<u32>,
    pub name: String,
    /// Where it's used ("router admin", "Cloudflare DNS API").
    pub place: String,
    pub host_id: Option<HostId>,
    /// When it was last rotated (unix seconds), if known.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub rotated_at: Option<i64>,
    pub note: String,
}

fn check_manual(m: &ManualSecret) -> Result<()> {
    let bad = |field: &'static str, reason: &str| Err(Error::Invalid { field, reason: reason.into() });
    if m.name.trim().is_empty() || m.name.chars().count() > 120 {
        return bad("name", "1 to 120 characters");
    }
    if m.place.chars().count() > 120 || m.note.chars().count() > 500 {
        return bad("note", "too long");
    }
    for t in [&m.name, &m.place, &m.note] {
        if crate::redact::line_has_secret(t) {
            return bad("note", "that looks like the secret itself; only describe it here");
        }
    }
    Ok(())
}

/// A `.env` vault value marked secret: (id, host, stack, name, updated_at).
pub type VaultDate = (i64, HostId, String, String, i64);

impl Store {
    /// Stamps a secret as set or rotated now.
    pub fn secret_stamp(&self, key: &str, at: i64) -> Result<()> {
        self.conn().execute(
            "INSERT INTO secret_meta (key, rotated_at) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET rotated_at = excluded.rotated_at, notified_due = NULL",
            params![key, at],
        )?;
        Ok(())
    }

    /// `Some(0)` turns the policy off, `None` goes back to the default.
    pub fn secret_policy_set(&self, key: &str, days: Option<u32>) -> Result<()> {
        if days.is_some_and(|d| d > 3650) {
            return Err(Error::Invalid { field: "days", reason: "at most 10 years".into() });
        }
        self.conn().execute(
            "INSERT INTO secret_meta (key, policy_days) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET policy_days = excluded.policy_days, notified_due = NULL",
            params![key, days],
        )?;
        Ok(())
    }

    pub fn secret_notified(&self, key: &str, due: i64) -> Result<()> {
        self.conn().execute(
            "INSERT INTO secret_meta (key, notified_due) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET notified_due = excluded.notified_due",
            params![key, due],
        )?;
        Ok(())
    }

    pub fn secret_metas(&self) -> Result<Vec<(String, Meta)>> {
        let mut st = self.conn().prepare("SELECT key, rotated_at, policy_days, notified_due FROM secret_meta")?;
        let rows = st.query_map([], |r| {
            Ok((r.get(0)?, Meta { rotated_at: r.get(1)?, policy_days: r.get(2)?, notified_due: r.get(3)? }))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn manual_secrets(&self) -> Result<Vec<ManualSecret>> {
        let mut st = self
            .conn()
            .prepare("SELECT id, name, place, host_id, rotated_at, note FROM manual_secrets ORDER BY name")?;
        let rows = st.query_map([], |r| {
            Ok(ManualSecret {
                id: Some(r.get(0)?),
                name: r.get(1)?,
                place: r.get(2)?,
                host_id: r.get(3)?,
                rotated_at: r.get(4)?,
                note: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn manual_secret_save(&self, m: &ManualSecret) -> Result<u32> {
        check_manual(m)?;
        let c = self.conn();
        match m.id {
            Some(id) => {
                let n = c.execute(
                    "UPDATE manual_secrets SET name = ?2, place = ?3, host_id = ?4, rotated_at = ?5, note = ?6 WHERE id = ?1",
                    params![id, m.name.trim(), m.place.trim(), m.host_id, m.rotated_at, m.note.trim()],
                )?;
                if n == 0 {
                    return Err(Error::NotFound("secret"));
                }
                Ok(id)
            }
            None => {
                c.execute(
                    "INSERT INTO manual_secrets (name, place, host_id, rotated_at, note) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![m.name.trim(), m.place.trim(), m.host_id, m.rotated_at, m.note.trim()],
                )?;
                Ok(c.last_insert_rowid() as u32)
            }
        }
    }

    pub fn manual_secret_delete(&self, id: u32) -> Result<()> {
        let c = self.conn();
        c.execute("DELETE FROM manual_secrets WHERE id = ?1", [id])?;
        c.execute("DELETE FROM secret_meta WHERE key = ?1", [format!("manual:{id}")])?;
        Ok(())
    }

    /// `.env` vault values marked secret: (id, host, stack, name, updated_at).
    pub fn vault_secret_dates(&self) -> Result<Vec<VaultDate>> {
        let mut st = self
            .conn()
            .prepare("SELECT id, host_id, stack, name, updated_at FROM env_vars WHERE masked = 1 ORDER BY host_id, stack, position")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Integrations: (id, kind, name, created_at).
    pub fn integration_dates(&self) -> Result<Vec<(i64, String, String, i64)>> {
        let mut st = self.conn().prepare("SELECT id, kind, name, created_at FROM integrations ORDER BY id")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn secret_meta(&self, key: &str) -> Result<Option<Meta>> {
        Ok(self
            .conn()
            .query_row("SELECT rotated_at, policy_days, notified_due FROM secret_meta WHERE key = ?1", [key], |r| {
                Ok(Meta { rotated_at: r.get(0)?, policy_days: r.get(1)?, notified_due: r.get(2)? })
            })
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000;

    #[test]
    fn states_from_dates_and_policy() {
        let mut f = Found::new("ssh:SHA256:x", SecretKind::SshKey, "id_ed25519", "");
        assert_eq!(item(f.clone(), None, NOW).state, RotationState::Unknown);
        f.dated = Some(NOW - 400 * DAY);
        let i = item(f.clone(), None, NOW);
        assert_eq!((i.state, i.policy_days), (RotationState::Overdue, Some(365)));
        f.dated = Some(NOW - 350 * DAY);
        assert_eq!(item(f.clone(), None, NOW).state, RotationState::DueSoon);
        // A stamp newer than the file date wins.
        let m = Meta { rotated_at: Some(NOW - DAY), ..Default::default() };
        assert_eq!(item(f.clone(), Some(&m), NOW).state, RotationState::Ok);
        // Policy off.
        let m = Meta { policy_days: Some(0), ..Default::default() };
        assert_eq!(item(f.clone(), Some(&m), NOW).state, RotationState::NoPolicy);
        // Custom policy.
        let m = Meta { policy_days: Some(90), ..Default::default() };
        let i = item(f, Some(&m), NOW);
        assert_eq!((i.state, i.policy_days), (RotationState::Overdue, Some(90)));
    }

    #[test]
    fn expiry_items_use_their_date() {
        let mut f = Found::new("expiry:1", SecretKind::Expiry, "example.org", "server01");
        f.warn_days = 14;
        f.expires_at = Some(NOW + 10 * DAY);
        assert_eq!(item(f.clone(), None, NOW).state, RotationState::DueSoon);
        f.expires_at = Some(NOW - 1);
        assert_eq!(item(f.clone(), None, NOW).state, RotationState::Overdue);
        f.expires_at = Some(NOW + 60 * DAY);
        let i = item(f, None, NOW);
        assert_eq!((i.state, i.policy_days, i.due_at), (RotationState::Ok, None, Some(NOW + 60 * DAY)));
    }

    #[test]
    fn sorts_and_notifies_each_due_date_once() {
        let mk = |k: &str, days_old: i64| {
            let mut f = Found::new(k, SecretKind::AppToken, k, "");
            f.dated = Some(NOW - days_old * DAY);
            item(f, None, NOW)
        };
        let mut v = vec![mk("ok", 10), mk("late", 400), mk("soon", 170)];
        sort(&mut v);
        assert_eq!(v.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(), vec!["late", "soon", "ok"]);
        let metas = vec![("late".to_owned(), Meta { notified_due: v[0].due_at, ..Default::default() })];
        let n = to_notify(&v, &metas);
        assert_eq!(n.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(), vec!["soon"]);
    }

    #[test]
    fn store_round_trip() {
        let s = Store::open_in_memory().unwrap();
        s.secret_stamp("app:mcp", 100).unwrap();
        s.secret_policy_set("app:mcp", Some(30)).unwrap();
        s.secret_notified("app:mcp", 500).unwrap();
        let m = s.secret_meta("app:mcp").unwrap().unwrap();
        assert_eq!((m.rotated_at, m.policy_days, m.notified_due), (Some(100), Some(30), Some(500)));
        // A new stamp clears the notice.
        s.secret_stamp("app:mcp", 200).unwrap();
        assert_eq!(s.secret_meta("app:mcp").unwrap().unwrap().notified_due, None);
        assert!(s.secret_policy_set("x", Some(5000)).is_err());
        let id = s
            .manual_secret_save(&ManualSecret {
                id: None,
                name: "Router admin".into(),
                place: "the router".into(),
                host_id: None,
                rotated_at: Some(50),
                note: String::new(),
            })
            .unwrap();
        assert_eq!(s.manual_secrets().unwrap()[0].id, Some(id));
        let leak = ManualSecret {
            id: None,
            name: "API".into(),
            place: String::new(),
            host_id: None,
            rotated_at: None,
            note: "password=hunter2hunter2".into(),
        };
        assert!(s.manual_secret_save(&leak).is_err());
        s.manual_secret_delete(id).unwrap();
        assert!(s.manual_secrets().unwrap().is_empty());
    }
}
