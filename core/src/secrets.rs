//! Secret storage: a thin wrapper over the OS keychain.
//!
//! - macOS: Keychain · Windows: Credential Manager · Linux: Secret Service
//!   (GNOME Keyring / KWallet), reached via the portal inside Flatpak.
//!
//! Secrets are addressed by a [`SecretRef`] (kind + owner id) and never touch
//! SQLite, config files, logs or `Debug` output. Values are returned in
//! [`Zeroizing`] buffers so they are wiped from memory on drop.

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

use zeroize::Zeroizing;

use crate::{Error, Result};

/// Keychain service name for every NIGHTGRID entry.
pub const SERVICE: &str = "dev.nightgrid.Nightgrid";

/// What a secret is for. Each kind is scoped to an owner id (host id,
/// integration id, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretKind {
    /// SSH password for a host.
    HostPassword,
    /// Passphrase for a private key file used by a host.
    KeyPassphrase,
    /// Remembered sudo password for a host (opt-in, off by default).
    SudoPassword,
    /// API token for an integration (Plex, AMP, ...).
    IntegrationToken,
}

impl SecretKind {
    /// Kinds stored per host.
    pub const HOST: [SecretKind; 3] = [Self::HostPassword, Self::KeyPassphrase, Self::SudoPassword];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::HostPassword => "host-password",
            Self::KeyPassphrase => "key-passphrase",
            Self::SudoPassword => "sudo-password",
            Self::IntegrationToken => "integration-token",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Self::HostPassword, Self::KeyPassphrase, Self::SudoPassword, Self::IntegrationToken]
            .into_iter()
            .find(|k| k.as_str() == s)
    }
}

/// Address of a secret in the keychain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SecretRef {
    pub kind: SecretKind,
    pub owner: String,
}

impl SecretRef {
    pub fn new(kind: SecretKind, owner: impl Into<String>) -> Self {
        Self { kind, owner: owner.into() }
    }

    /// A per-host secret (password, key passphrase, sudo password).
    pub fn host(kind: SecretKind, id: crate::hosts::HostId) -> Self {
        Self::new(kind, id.to_string())
    }

    /// An integration's token or password.
    pub fn integration(id: crate::integrations::IntegrationId) -> Self {
        Self::new(SecretKind::IntegrationToken, format!("integration-{id}"))
    }

    /// The alerts webhook URL (it can carry a token).
    pub fn alerts_webhook() -> Self {
        Self::new(SecretKind::IntegrationToken, "alerts-webhook")
    }

    /// The keychain "account"/"username" field, e.g. `host-password:42`.
    pub fn account(&self) -> String {
        format!("{}:{}", self.kind.as_str(), self.owner)
    }
}

/// A secret value. `Debug`/`Display` are redacted.
pub struct Secret(Zeroizing<String>);

impl Secret {
    pub fn new(s: impl Into<String>) -> Self {
        Self(Zeroizing::new(s.into()))
    }

    /// Borrow the plaintext. Keep the borrow short and never log it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// Backend-agnostic secret store.
pub trait SecretStore: Send + Sync {
    fn set(&self, r: &SecretRef, value: &Secret) -> Result<()>;
    /// `Ok(None)` when no entry exists.
    fn get(&self, r: &SecretRef) -> Result<Option<Secret>>;
    /// Deleting a missing entry is not an error.
    fn delete(&self, r: &SecretRef) -> Result<()>;
}

/// The OS keychain via the `keyring` crate.
#[derive(Debug, Default)]
pub struct KeyringStore;

impl KeyringStore {
    pub fn new() -> Self {
        Self
    }

    /// Whether a platform credential store could be initialised. Useful for
    /// a diagnostics readout (e.g. no Secret Service running on a bare WM).
    pub fn available() -> std::result::Result<(), String> {
        keyring::Entry::store_status().as_ref().map(|_| ()).map_err(|e| e.to_string())
    }

    fn entry(r: &SecretRef) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, &r.account()).map_err(map_err)
    }
}

fn map_err(e: keyring::Error) -> Error {
    // keyring errors don't include secret values, but be conservative and
    // drop any attached raw bytes (BadEncoding/BadDataFormat carry them).
    let msg = match &e {
        keyring::Error::BadEncoding(_) => "stored secret is not valid UTF-8".to_string(),
        keyring::Error::BadDataFormat(_, _) => "stored secret has an unexpected format".to_string(),
        other => other.to_string(),
    };
    Error::Secrets(msg)
}

impl SecretStore for KeyringStore {
    fn set(&self, r: &SecretRef, value: &Secret) -> Result<()> {
        Self::entry(r)?.set_password(value.expose()).map_err(map_err)
    }

    fn get(&self, r: &SecretRef) -> Result<Option<Secret>> {
        match Self::entry(r)?.get_password() {
            Ok(s) => Ok(Some(Secret::new(s))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(map_err(e)),
        }
    }

    fn delete(&self, r: &SecretRef) -> Result<()> {
        match Self::entry(r)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(map_err(e)),
        }
    }
}

/// In-memory store for tests and for a "don't remember anything" session.
#[derive(Default)]
pub struct MemoryStore {
    inner: Mutex<HashMap<SecretRef, Zeroizing<String>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemoryStore {
    fn set(&self, r: &SecretRef, value: &Secret) -> Result<()> {
        self.inner.lock().expect("secret store poisoned").insert(r.clone(), Zeroizing::new(value.expose().to_owned()));
        Ok(())
    }

    fn get(&self, r: &SecretRef) -> Result<Option<Secret>> {
        Ok(self.inner.lock().expect("secret store poisoned").get(r).map(|s| Secret::new(s.as_str())))
    }

    fn delete(&self, r: &SecretRef) -> Result<()> {
        self.inner.lock().expect("secret store poisoned").remove(r);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exercise(store: &dyn SecretStore) {
        let r = SecretRef::new(SecretKind::HostPassword, "test-host");
        store.delete(&r).unwrap();
        assert!(store.get(&r).unwrap().is_none());
        store.set(&r, &Secret::new("hunter2")).unwrap();
        assert_eq!(store.get(&r).unwrap().unwrap().expose(), "hunter2");
        store.set(&r, &Secret::new("rotated")).unwrap();
        assert_eq!(store.get(&r).unwrap().unwrap().expose(), "rotated");
        store.delete(&r).unwrap();
        assert!(store.get(&r).unwrap().is_none());
        // Deleting twice is fine.
        store.delete(&r).unwrap();
    }

    #[test]
    fn memory_store_roundtrip() {
        exercise(&MemoryStore::new());
    }

    #[test]
    fn kinds_are_isolated() {
        let s = MemoryStore::new();
        s.set(&SecretRef::new(SecretKind::HostPassword, "1"), &Secret::new("a")).unwrap();
        assert!(s.get(&SecretRef::new(SecretKind::SudoPassword, "1")).unwrap().is_none());
    }

    #[test]
    fn account_format() {
        assert_eq!(SecretRef::new(SecretKind::KeyPassphrase, "7").account(), "key-passphrase:7");
        assert_eq!(SecretRef::new(SecretKind::IntegrationToken, "plex").account(), "integration-token:plex");
    }

    #[test]
    fn secret_is_redacted_in_debug_and_display() {
        let s = Secret::new("hunter2");
        assert!(!format!("{s:?}").contains("hunter2"));
        assert!(!format!("{s}").contains("hunter2"));
    }

    /// Hits the real OS keychain. Opt-in because CI runners often have no
    /// unlocked keychain / Secret Service: `NIGHTGRID_TEST_KEYRING=1 cargo test`.
    #[test]
    fn keyring_store_roundtrip() {
        if std::env::var_os("NIGHTGRID_TEST_KEYRING").is_none() {
            eprintln!("skipping: set NIGHTGRID_TEST_KEYRING=1 to test the OS keychain");
            return;
        }
        exercise(&KeyringStore::new());
    }
}
