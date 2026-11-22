//! Profile sync between Nightgrid apps (D-205).
//!
//! Two apps that can reach each other (LAN, a system Tailscale or the
//! built-in one) pair once with a one-time code and then talk over an
//! encrypted, mutually authenticated channel ([`wire`]). Nothing goes
//! through a server.
//!
//! Per paired device the user picks how they sync:
//!
//! - **Manual**: push this profile there, or pull theirs here, when asked.
//!   The same bundle as a profile export ([`crate::backup`]), merged or
//!   replacing what's there.
//! - **Automatic** (two-way, send only or receive only): item-level sync on
//!   change, last edit wins per item (see `merge`).
//!
//! What never travels: settings that belong to one device
//! ([`LOCAL_SETTINGS`]), the paired-device list and this device's key, and
//! saved secrets unless the user ticks their category for that device.

pub mod discover;
pub mod history;
pub mod merge;
pub mod pair;
pub mod wire;

use base64::Engine as _;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest as _;

use crate::backup::{self, Bundle, ImportMode, ImportOutcome, SecretOwner};
use crate::secrets::{Secret, SecretKind, SecretRef, SecretStore};
use crate::store::Store;
use crate::{Error, Result};

pub use wire::Keypair;

/// Default port the sync listener uses.
pub const DEFAULT_PORT: u16 = 47920;
/// Paired devices kept at most.
pub const MAX_PEERS: usize = 16;

/// Sync settings of this device. Never synced or exported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct SyncSettings {
    /// Listen for paired devices (and pairing) on [`Self::port`].
    pub enabled: bool,
    pub port: u16,
    /// What other devices call this one; empty = the computer's name.
    pub device_name: String,
    /// Answer "is Nightgrid here?" probes on the LAN (D-208), so a new
    /// install can find this device. Needs `enabled`.
    pub discoverable: bool,
    /// Take pairing with the profile passphrase (its key is in the keychain).
    pub passphrase_pairing: bool,
    /// Snapshots of the synced profile kept for rolling back (D-209).
    pub keep_snapshots: u8,
}

impl Default for SyncSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            device_name: String::new(),
            discoverable: false,
            passphrase_pairing: false,
            keep_snapshots: history::DEFAULT_KEEP,
        }
    }
}

/// App settings (camelCase keys of `AppSettings`) that belong to one device:
/// kept as they are when a profile arrives, and never sent.
pub const LOCAL_SETTINGS: &[&str] = &[
    "sync",
    "backupFolder",
    "lastProfileExport",
    "profileReminderSnooze",
    "statusPage",
    "mcp",
    "tailnetEnabled",
    "tailnetAcceptRoutes",
    "tailnetHostname",
    "tailnetControlUrl",
    // Window, layout and input of this screen.
    "uiScale",
    "railCollapsed",
    "handheld",
    "controllerNav",
    "swipeNav",
    "trayEnabled",
    "closeToTray",
    "dashboardActive",
    // The DRIFT model is downloaded per device.
    "helperEnabled",
    "helperModel",
    "updateSkipVersion",
    "tourOffered",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SyncMode {
    /// Push and pull only when asked.
    Manual,
    /// Changes go both ways automatically; the latest edit of an item wins.
    TwoWay,
    /// This device's changes go there; theirs don't come back.
    SendOnly,
    /// Their changes come here; this device's don't go there.
    ReceiveOnly,
}

impl SyncMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::TwoWay => "twoWay",
            Self::SendOnly => "sendOnly",
            Self::ReceiveOnly => "receiveOnly",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "twoWay" => Self::TwoWay,
            "sendOnly" => Self::SendOnly,
            "receiveOnly" => Self::ReceiveOnly,
            _ => Self::Manual,
        }
    }

    pub fn automatic(self) -> bool {
        self != Self::Manual
    }

    /// Whether this device takes changes from the peer automatically.
    pub fn receives(self) -> bool {
        matches!(self, Self::TwoWay | Self::ReceiveOnly)
    }

    /// Whether this device's changes go to the peer automatically.
    pub fn sends(self) -> bool {
        matches!(self, Self::TwoWay | Self::SendOnly)
    }
}

/// Saved secrets, in the groups the user can opt in to per device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SecretCategory {
    /// SSH passwords, key passphrases and sudo passwords of hosts.
    Logins,
    /// Integration tokens and the alerts webhook.
    Tokens,
    /// .env vault keys (without them synced vault values can't be read).
    Vault,
}

impl SecretCategory {
    pub const ALL: [SecretCategory; 3] = [Self::Logins, Self::Tokens, Self::Vault];

    fn of(owner: &SecretOwner) -> Self {
        match owner {
            SecretOwner::Host { .. } => Self::Logins,
            SecretOwner::Integration { .. } | SecretOwner::AlertsWebhook => Self::Tokens,
            SecretOwner::Vault { .. } => Self::Vault,
        }
    }
}

/// A paired device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SyncPeer {
    /// Short id derived from its key.
    pub id: String,
    pub name: String,
    /// Its X25519 public key, base64.
    pub public_key: String,
    /// Addresses to reach it, the one that worked last first.
    pub addrs: Vec<String>,
    pub port: u16,
    pub mode: SyncMode,
    /// Secret groups exchanged with this device (sent and accepted).
    pub secrets: Vec<SecretCategory>,
    /// It may change this device's profile (push, automatic sync).
    pub accept_changes: bool,
    pub app_version: String,
    /// Unix seconds.
    pub paired_at: f64,
    pub last_sync: Option<f64>,
    pub last_seen: Option<f64>,
    pub last_error: Option<String>,
}

impl SyncPeer {
    pub fn key(&self) -> Result<[u8; 32]> {
        decode_key(&self.public_key)
    }
}

pub fn encode_key(k: &[u8; 32]) -> String {
    base64::engine::general_purpose::STANDARD.encode(k)
}

pub fn decode_key(s: &str) -> Result<[u8; 32]> {
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .ok()
        .and_then(|v| <[u8; 32]>::try_from(v.as_slice()).ok())
        .ok_or_else(|| Error::Invalid { field: "publicKey", reason: "not a device key".into() })
}

/// Short, stable id of a device key (16 hex characters).
pub fn device_id(public: &[u8; 32]) -> String {
    let h = sha2::Sha256::digest(public);
    h[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// Groups a device key's id in fours for reading aloud (`a1b2-c3d4-…`).
pub fn fingerprint(id: &str) -> String {
    id.as_bytes().chunks(4).map(|c| String::from_utf8_lossy(c).into_owned()).collect::<Vec<_>>().join("-")
}

fn identity_ref() -> SecretRef {
    SecretRef::new(SecretKind::IntegrationToken, "sync-identity")
}

/// This device's key pair, created on first use and kept in the keychain.
pub fn identity(keychain: &dyn SecretStore) -> Result<Keypair> {
    if let Some(s) = keychain.get(&identity_ref())? {
        if let Ok(raw) = base64::engine::general_purpose::STANDARD.decode(s.expose()) {
            if let Ok(k) = Keypair::from_private(&raw) {
                return Ok(k);
            }
        }
        tracing::warn!("sync: the saved device key is unreadable; making a new one");
    }
    let k = Keypair::generate()?;
    keychain.set(&identity_ref(), &Secret::new(base64::engine::general_purpose::STANDARD.encode(&k.private[..])))?;
    Ok(k)
}

/// Forgets this device's key (every pairing has to be redone).
pub fn reset_identity(keychain: &dyn SecretStore) -> Result<()> {
    keychain.delete(&identity_ref())
}

fn pass_ref() -> SecretRef {
    SecretRef::new(SecretKind::IntegrationToken, "sync-passphrase")
}

/// Saves (or with `None` forgets) the key derived from the profile
/// passphrase that pairing by passphrase checks against (D-208). The
/// passphrase itself is never stored.
pub fn set_pass_secret(keychain: &dyn SecretStore, passphrase: Option<&str>) -> Result<()> {
    match passphrase {
        None => keychain.delete(&pass_ref()),
        Some(p) => {
            if p.chars().count() < 8 {
                return Err(Error::Invalid { field: "passphrase", reason: "use at least 8 characters".into() });
            }
            let salt = discover::new_salt();
            let key = discover::pass_key(p, &discover::decode_salt(&salt).expect("fresh salt"));
            keychain.set(&pass_ref(), &Secret::new(discover::encode_pass(&salt, &key)))
        }
    }
}

/// The saved passphrase key and its salt, if pairing by passphrase is set up.
pub fn pass_secret(keychain: &dyn SecretStore) -> Result<Option<(String, zeroize::Zeroizing<[u8; 32]>)>> {
    Ok(keychain.get(&pass_ref())?.and_then(|s| discover::decode_pass(s.expose())))
}

/// What each side says about itself after the handshake.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub name: String,
    pub app_version: String,
    pub schema: i64,
    /// Where it can be reached.
    pub addrs: Vec<String>,
    pub port: u16,
    /// Sent by a device joining from the LAN list (D-208): how it'd like
    /// this side to set the pairing up. Older versions leave it out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join: Option<Join>,
}

/// What a joining device asks for at pairing time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Join {
    /// Keep syncing both ways from now on.
    pub two_way: bool,
    /// Secret groups it'd like to receive.
    #[serde(default)]
    pub secrets: Vec<SecretCategory>,
}

/// One request per session connection.
#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Request {
    Hello(Hello),
    /// Send me your profile (with these secret groups, where you allow them).
    Pull {
        secrets: Vec<SecretCategory>,
    },
    /// Take my profile.
    Push {
        mode: ImportMode,
        bundle: Box<Bundle>,
    },
    /// Automatic sync: my items (empty when I don't send to you).
    Sync {
        schema: i64,
        items: Vec<merge::Item>,
    },
    /// I'm forgetting you; forget me.
    Unpair,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "ok", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Response {
    Hello(Hello),
    Profile {
        bundle: Box<Bundle>,
    },
    Pushed {
        added: u32,
        skipped: u32,
        secrets: u32,
    },
    /// Automatic sync: my items after taking yours (empty when I don't send
    /// to you), and whether I took yours.
    Synced {
        items: Vec<merge::Item>,
        took: bool,
    },
    Done,
    Error {
        message: String,
    },
}

/// The profile to send a peer: the export bundle without this device's own
/// settings, with the secret groups in `secrets` only.
pub fn bundle_for(
    store: &Store,
    keychain: &dyn SecretStore,
    secrets: &[SecretCategory],
    app_version: &str,
    now: i64,
) -> Result<Bundle> {
    let mut b = backup::snapshot(store, app_version, now)?;
    if !secrets.is_empty() {
        backup::collect_secrets(store, keychain, &mut b)?;
        keep_secrets(&mut b, secrets);
    }
    strip_local_settings(&mut b);
    Ok(b)
}

/// Drops bundled secrets outside `allowed`.
pub fn keep_secrets(b: &mut Bundle, allowed: &[SecretCategory]) {
    b.secrets.retain(|s| allowed.contains(&SecretCategory::of(&s.owner)));
}

/// Removes [`LOCAL_SETTINGS`] from the bundle's `settings` rows.
pub fn strip_local_settings(b: &mut Bundle) {
    let Some(rows) = b.tables.get_mut("settings") else { return };
    for r in rows.iter_mut() {
        if r.get("key").and_then(Value::as_str) != Some("app") {
            continue;
        }
        let Some(Ok(mut v)) = r.get("value").and_then(Value::as_str).map(serde_json::from_str::<Value>) else {
            continue;
        };
        if let Some(m) = v.as_object_mut() {
            for k in LOCAL_SETTINGS {
                m.remove(*k);
            }
        }
        r.insert("value".into(), Value::String(v.to_string()));
    }
}

/// Puts [`LOCAL_SETTINGS`] from `before` back into the saved app settings.
pub fn restore_local_settings(store: &Store, before: &Value) -> Result<()> {
    let mut now: Value = store.get_setting("app")?.unwrap_or_else(|| Value::Object(Default::default()));
    let (Some(m), Some(b)) = (now.as_object_mut(), before.as_object()) else { return Ok(()) };
    for k in LOCAL_SETTINGS {
        match b.get(*k) {
            Some(v) => {
                m.insert((*k).into(), v.clone());
            }
            None => {
                m.remove(*k);
            }
        }
    }
    store.set_setting("app", &now)
}

/// What applying a profile from a peer did.
#[derive(Debug, Default)]
pub struct Applied {
    pub outcome: ImportOutcome,
    pub secrets: u32,
    /// The keychain step failed (the profile itself was applied).
    pub secrets_error: Option<String>,
}

/// Applies a peer's profile: import (merge or replace), then this device's
/// own settings back, then the secret groups `allowed` allows.
pub fn apply_bundle(
    store: &mut Store,
    keychain: &dyn SecretStore,
    mut bundle: Bundle,
    mode: ImportMode,
    allowed: &[SecretCategory],
) -> Result<Applied> {
    keep_secrets(&mut bundle, allowed);
    strip_local_settings(&mut bundle);
    let before: Value = store.get_setting("app")?.unwrap_or(Value::Null);
    let outcome = backup::import(store, &bundle, mode)?;
    if outcome.settings_applied && before.is_object() {
        restore_local_settings(store, &before)?;
    }
    let (secrets, secrets_error) = match backup::restore_secrets(keychain, &bundle, &outcome) {
        Ok(n) => (n as u32, None),
        Err(e) => (0, Some(e.to_string())),
    };
    Ok(Applied { outcome, secrets, secrets_error })
}

fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

fn json_list<T: serde::de::DeserializeOwned>(s: &str) -> Vec<T> {
    serde_json::from_str(s).unwrap_or_default()
}

fn row_to_peer(r: &rusqlite::Row<'_>) -> rusqlite::Result<SyncPeer> {
    let mode: String = r.get("mode")?;
    let addrs: String = r.get("addrs")?;
    let secrets: String = r.get("share_groups")?;
    Ok(SyncPeer {
        id: r.get("id")?,
        name: r.get("name")?,
        public_key: r.get("public_key")?,
        addrs: json_list(&addrs),
        port: r.get::<_, i64>("port")?.clamp(1, 65535) as u16,
        mode: SyncMode::parse(&mode),
        secrets: json_list(&secrets),
        accept_changes: r.get::<_, i64>("accept_changes")? != 0,
        app_version: r.get("app_version")?,
        paired_at: r.get::<_, i64>("paired_at")? as f64,
        last_sync: r.get::<_, Option<i64>>("last_sync")?.map(|v| v as f64),
        last_seen: r.get::<_, Option<i64>>("last_seen")?.map(|v| v as f64),
        last_error: r.get("last_error")?,
    })
}

/// Cleans a device name from the other side (it's shown in the UI).
pub fn clean_name(s: &str) -> String {
    let t: String = s.chars().filter(|c| !c.is_control()).take(64).collect();
    let t = t.trim();
    if t.is_empty() {
        "Nightgrid".into()
    } else {
        t.into()
    }
}

/// Keeps addresses that look like host names or IPs, at most 8.
pub fn clean_addrs(addrs: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for a in addrs {
        let a = a.trim().trim_matches(['[', ']']);
        let ok = !a.is_empty()
            && a.len() <= 253
            && a.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_' | '%'));
        if ok && !out.iter().any(|x| x.eq_ignore_ascii_case(a)) {
            out.push(a.to_owned());
        }
        if out.len() == 8 {
            break;
        }
    }
    out
}

impl Store {
    /// Hosts in this profile (for the LAN beacon).
    pub fn host_count(&self) -> Result<u32> {
        let n: i64 = self.conn().query_row("SELECT COUNT(*) FROM hosts", [], |r| r.get(0))?;
        Ok(u32::try_from(n).unwrap_or(u32::MAX))
    }

    pub fn sync_peers(&self) -> Result<Vec<SyncPeer>> {
        let mut st = self.conn().prepare("SELECT * FROM sync_peers ORDER BY paired_at, id")?;
        let v = st.query_map([], row_to_peer)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    pub fn sync_peer(&self, id: &str) -> Result<Option<SyncPeer>> {
        Ok(self.conn().query_row("SELECT * FROM sync_peers WHERE id = ?1", [id], row_to_peer).optional()?)
    }

    pub fn sync_peer_by_key(&self, key: &[u8; 32]) -> Result<Option<SyncPeer>> {
        self.sync_peer(&device_id(key))
    }

    /// Adds a newly paired device, or refreshes one paired before (its
    /// mode and secret choices stay).
    pub fn sync_peer_add(&self, key: &[u8; 32], hello: &Hello, accept_changes: bool) -> Result<SyncPeer> {
        let id = device_id(key);
        if self.sync_peer(&id)?.is_none() && self.sync_peers()?.len() >= MAX_PEERS {
            return Err(Error::Invalid { field: "peers", reason: format!("at most {MAX_PEERS} paired devices") });
        }
        let addrs = serde_json::to_string(&clean_addrs(&hello.addrs))?;
        self.conn().execute(
            "INSERT INTO sync_peers (id, name, public_key, addrs, port, accept_changes, app_version, paired_at, last_seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, addrs = excluded.addrs, port = excluded.port,
                 app_version = excluded.app_version, last_seen = excluded.last_seen, last_error = NULL",
            params![
                id,
                clean_name(&hello.name),
                encode_key(key),
                addrs,
                i64::from(hello.port.max(1)),
                i64::from(accept_changes),
                hello.app_version.chars().take(32).collect::<String>(),
                now_secs()
            ],
        )?;
        self.sync_peer(&id)?.ok_or(Error::NotFound("paired device"))
    }

    /// Changes what the user controls about a paired device.
    /// Applies what a device joining from the LAN asked for (D-208): it may
    /// change this device, sync two-way if asked, and get those secret groups.
    pub fn sync_peer_joined(&self, id: &str, join: &Join) -> Result<SyncPeer> {
        let p = self.sync_peer(id)?.ok_or(Error::NotFound("paired device"))?;
        let mode = if join.two_way { SyncMode::TwoWay } else { p.mode };
        self.sync_peer_set(id, &p.name, mode, &join.secrets, true)
    }

    pub fn sync_peer_set(
        &self,
        id: &str,
        name: &str,
        mode: SyncMode,
        secrets: &[SecretCategory],
        accept_changes: bool,
    ) -> Result<SyncPeer> {
        let mut cats: Vec<SecretCategory> = Vec::new();
        for c in secrets {
            if !cats.contains(c) {
                cats.push(*c);
            }
        }
        let n = self.conn().execute(
            "UPDATE sync_peers SET name = ?2, mode = ?3, share_groups = ?4, accept_changes = ?5 WHERE id = ?1",
            params![id, clean_name(name), mode.as_str(), serde_json::to_string(&cats)?, i64::from(accept_changes)],
        )?;
        if n == 0 {
            return Err(Error::NotFound("paired device"));
        }
        self.sync_peer(id)?.ok_or(Error::NotFound("paired device"))
    }

    /// Records that the device answered (with what it said about itself).
    pub fn sync_peer_seen(&self, id: &str, hello: Option<&Hello>, used_addr: Option<&str>) -> Result<()> {
        let Some(p) = self.sync_peer(id)? else { return Ok(()) };
        let mut addrs = p.addrs.clone();
        if let Some(h) = hello {
            for a in clean_addrs(&h.addrs) {
                if !addrs.iter().any(|x| x.eq_ignore_ascii_case(&a)) {
                    addrs.push(a);
                }
            }
        }
        if let Some(a) = used_addr {
            addrs.retain(|x| !x.eq_ignore_ascii_case(a));
            addrs.insert(0, a.to_owned());
        }
        let addrs = clean_addrs(&addrs);
        self.conn().execute(
            "UPDATE sync_peers SET last_seen = ?2, last_error = NULL, addrs = ?3,
                 app_version = COALESCE(?4, app_version), port = COALESCE(?5, port) WHERE id = ?1",
            params![
                id,
                now_secs(),
                serde_json::to_string(&addrs)?,
                hello.map(|h| h.app_version.chars().take(32).collect::<String>()),
                hello.map(|h| i64::from(h.port.max(1)))
            ],
        )?;
        Ok(())
    }

    pub fn sync_peer_synced(&self, id: &str) -> Result<()> {
        self.conn().execute(
            "UPDATE sync_peers SET last_sync = ?2, last_seen = ?2, last_error = NULL WHERE id = ?1",
            params![id, now_secs()],
        )?;
        Ok(())
    }

    pub fn sync_peer_failed(&self, id: &str, error: &str) -> Result<()> {
        let e: String = error.chars().take(300).collect();
        self.conn().execute("UPDATE sync_peers SET last_error = ?2 WHERE id = ?1", params![id, e])?;
        Ok(())
    }

    pub fn sync_peer_delete(&self, id: &str) -> Result<bool> {
        Ok(self.conn().execute("DELETE FROM sync_peers WHERE id = ?1", [id])? > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemoryStore;

    fn hello(name: &str) -> Hello {
        Hello {
            name: name.into(),
            app_version: "1.22.0".into(),
            schema: 58,
            addrs: vec!["10.0.0.5".into()],
            port: 47920,
            join: None,
        }
    }

    #[test]
    fn identity_is_created_once() {
        let kc = MemoryStore::new();
        let a = identity(&kc).unwrap();
        let b = identity(&kc).unwrap();
        assert_eq!(a.public, b.public);
        assert_eq!(device_id(&a.public).len(), 16);
        assert_eq!(fingerprint("a1b2c3d4e5f60718"), "a1b2-c3d4-e5f6-0718");
        reset_identity(&kc).unwrap();
        assert_ne!(identity(&kc).unwrap().public, a.public);
    }

    #[test]
    fn peers_crud() {
        let s = Store::open_in_memory().unwrap();
        let k = Keypair::generate().unwrap();
        let p = s.sync_peer_add(&k.public, &hello("Mac\u{7}"), true).unwrap();
        assert_eq!(p.name, "Mac");
        assert_eq!(p.mode, SyncMode::Manual);
        assert_eq!(p.key().unwrap(), k.public);
        let p = s
            .sync_peer_set(&p.id, "Ally", SyncMode::TwoWay, &[SecretCategory::Tokens, SecretCategory::Tokens], false)
            .unwrap();
        assert_eq!((p.mode, p.secrets.len(), p.accept_changes), (SyncMode::TwoWay, 1, false));
        // Pairing again keeps the user's choices.
        let again = s.sync_peer_add(&k.public, &hello("Ally2"), true).unwrap();
        assert_eq!((again.mode, again.accept_changes, again.name.as_str()), (SyncMode::TwoWay, false, "Ally2"));
        s.sync_peer_seen(
            &p.id,
            Some(&Hello { addrs: vec!["ally.tail.ts.net".into()], ..hello("x") }),
            Some("100.64.0.9"),
        )
        .unwrap();
        let p = s.sync_peer(&p.id).unwrap().unwrap();
        assert_eq!(p.addrs, vec!["100.64.0.9", "10.0.0.5", "ally.tail.ts.net"]);
        s.sync_peer_failed(&p.id, "timed out").unwrap();
        assert_eq!(s.sync_peer(&p.id).unwrap().unwrap().last_error.as_deref(), Some("timed out"));
        s.sync_peer_synced(&p.id).unwrap();
        assert!(s.sync_peer(&p.id).unwrap().unwrap().last_error.is_none());
        let j = s.sync_peer_joined(&p.id, &Join { two_way: true, secrets: vec![SecretCategory::Logins] }).unwrap();
        assert_eq!(
            (j.mode, j.accept_changes, j.secrets.clone()),
            (SyncMode::TwoWay, true, vec![SecretCategory::Logins])
        );
        assert!(s.sync_peer_delete(&p.id).unwrap());
        assert!(s.sync_peers().unwrap().is_empty());
    }

    #[test]
    fn passphrase_secret_round_trips() {
        let kc = MemoryStore::new();
        assert!(pass_secret(&kc).unwrap().is_none());
        assert!(set_pass_secret(&kc, Some("short")).is_err());
        set_pass_secret(&kc, Some("a long passphrase")).unwrap();
        let (salt, key) = pass_secret(&kc).unwrap().unwrap();
        assert_eq!(*key, *discover::pass_key("a long passphrase", &discover::decode_salt(&salt).unwrap()));
        set_pass_secret(&kc, None).unwrap();
        assert!(pass_secret(&kc).unwrap().is_none());
    }

    #[test]
    fn hello_without_join_still_reads() {
        let h: Hello =
            serde_json::from_str(r#"{"name":"a","appVersion":"1.24.0","schema":59,"addrs":[],"port":47920}"#).unwrap();
        assert!(h.join.is_none());
        assert!(!serde_json::to_string(&hello("x")).unwrap().contains("join"));
    }

    #[test]
    fn addresses_are_cleaned() {
        let a = clean_addrs(&["  [fe80::1%en0] ".into(), "bad host".into(), "Lumen".into(), "lumen".into(), "".into()]);
        assert_eq!(a, vec!["fe80::1%en0", "Lumen"]);
    }

    #[test]
    fn profiles_keep_device_settings_and_chosen_secrets() {
        let a = Store::open_in_memory().unwrap();
        a.conn()
            .execute_batch(
                "INSERT INTO hosts (id, name, hostname, remember_secret, auth_method) VALUES (3, 'server01', '10.0.0.1', 1, 'password');
                 INSERT INTO integrations (id, kind, name, base_url) VALUES (9, 'kuma', 'Kuma', 'https://kuma.example');",
            )
            .unwrap();
        a.set_setting(
            "app",
            &serde_json::json!({ "theme": "ice", "uiScale": 140, "handheld": true, "backupFolder": "/a" }),
        )
        .unwrap();
        let kc = MemoryStore::new();
        kc.set(&SecretRef::host(SecretKind::HostPassword, 3), &Secret::new("pw")).unwrap();
        kc.set(&SecretRef::integration(9), &Secret::new("tok")).unwrap();

        let b = bundle_for(&a, &kc, &[SecretCategory::Tokens], "1.22.0", 1).unwrap();
        assert_eq!(b.secrets.len(), 1, "only the token group");
        let v: Value = serde_json::from_str(b.tables["settings"][0]["value"].as_str().unwrap()).unwrap();
        assert_eq!(v["theme"], "ice");
        assert!(v.get("uiScale").is_none() && v.get("handheld").is_none() && v.get("backupFolder").is_none());
        assert!(bundle_for(&a, &kc, &[], "1.22.0", 1).unwrap().secrets.is_empty());

        // The receiving device keeps its own scale and folder.
        let mut z = Store::open_in_memory().unwrap();
        z.set_setting("app", &serde_json::json!({ "theme": "nightgrid", "uiScale": 110, "backupFolder": "/z" }))
            .unwrap();
        let kc2 = MemoryStore::new();
        let r = apply_bundle(&mut z, &kc2, b, ImportMode::Replace, &[SecretCategory::Tokens, SecretCategory::Logins])
            .unwrap();
        assert_eq!(r.secrets, 1);
        let v: Value = z.get_setting("app").unwrap().unwrap();
        assert_eq!(
            (v["theme"].as_str(), v["uiScale"].as_i64(), v["backupFolder"].as_str()),
            (Some("ice"), Some(110), Some("/z"))
        );
        assert!(v.get("handheld").is_none());
        let iid = r.outcome.integrations[&9] as crate::integrations::IntegrationId;
        assert_eq!(kc2.get(&SecretRef::integration(iid)).unwrap().unwrap().expose(), "tok");

        // A group the receiver didn't allow is dropped there too.
        let mut y = Store::open_in_memory().unwrap();
        let kc3 = MemoryStore::new();
        let b = bundle_for(&a, &kc, &SecretCategory::ALL, "1.22.0", 1).unwrap();
        assert_eq!(b.secrets.len(), 2);
        let r = apply_bundle(&mut y, &kc3, b, ImportMode::Merge, &[]).unwrap();
        assert_eq!(r.secrets, 0);
    }

    #[test]
    fn protocol_json_shape() {
        let r = Request::Pull { secrets: vec![SecretCategory::Logins] };
        assert_eq!(serde_json::to_value(&r).unwrap(), serde_json::json!({ "op": "pull", "secrets": ["logins"] }));
        let r = Response::Pushed { added: 1, skipped: 2, secrets: 0 };
        assert_eq!(serde_json::to_value(&r).unwrap()["ok"], "pushed");
    }
}
