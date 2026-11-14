//! Host key verification: trust-on-first-use against NIGHTGRID's own
//! `known_hosts` file, with the system `~/.ssh/known_hosts` consulted
//! read-only as a second source.
//!
//! A key that differs from a recorded key of the same algorithm is reported as
//! [`HostKeyStatus::Changed`], and callers must block the connection until the
//! user explicitly accepts (security rule 3).

use std::io::Write;
use std::path::{Path, PathBuf};

use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use russh::keys::ssh_key::{HashAlg, PublicKey};
use serde::Serialize;
use sha1::Sha1;

use crate::{Error, Result};

/// A host key as shown to the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HostKeyInfo {
    pub algorithm: String,
    /// `SHA256:...`, as printed by `ssh-keygen -lf`.
    pub fingerprint: String,
    /// Full OpenSSH public key line (`ssh-ed25519 AAAA...`), used to accept it.
    pub openssh: String,
}

impl HostKeyInfo {
    pub fn from_key(key: &PublicKey) -> Self {
        Self {
            algorithm: key.algorithm().to_string(),
            fingerprint: key.fingerprint(HashAlg::Sha256).to_string(),
            openssh: key.to_openssh().unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyStatus {
    Trusted,
    Unknown,
    /// A different key of the same algorithm was recorded (or the key is revoked).
    Changed {
        previous: Vec<HostKeyInfo>,
    },
}

#[derive(Debug, Clone)]
struct Entry {
    line: usize,
    revoked: bool,
    key: PublicKey,
}

fn host_port(host: &str, port: u16) -> String {
    if port == 22 {
        host.to_owned()
    } else {
        format!("[{host}]:{port}")
    }
}

fn hashed_match(entry: &str, target: &str) -> bool {
    let mut parts = entry.trim_start_matches("|1|").split('|');
    let (Some(salt), Some(hash)) = (parts.next(), parts.next()) else { return false };
    let b64 = base64::engine::general_purpose::STANDARD;
    let (Ok(salt), Ok(hash)) = (b64.decode(salt), b64.decode(hash)) else { return false };
    let Ok(mut mac) = <Hmac<Sha1> as KeyInit>::new_from_slice(&salt) else { return false };
    mac.update(target.as_bytes());
    mac.verify_slice(&hash).is_ok()
}

/// Matches the host field of a known_hosts line against `target`
/// (`host` or `[host]:port`). Supports lists, hashing, `*`/`?` and `!negation`.
fn hosts_field_matches(field: &str, target: &str) -> bool {
    let mut matched = false;
    for pat in field.split(',') {
        if pat.starts_with("|1|") {
            if hashed_match(pat, target) {
                matched = true;
            }
        } else if let Some(neg) = pat.strip_prefix('!') {
            if super::config::wildcard_match(neg, target) {
                return false;
            }
        } else if super::config::wildcard_match(pat, target) {
            matched = true;
        }
    }
    matched
}

/// Parses a file and returns the entries that apply to host:port.
/// Unparseable lines and unknown key types are skipped, not fatal.
fn entries_for(path: &Path, host: &str, port: u16) -> Vec<Entry> {
    let Ok(text) = std::fs::read_to_string(path) else { return vec![] };
    let target = host_port(host, port);
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split_whitespace();
        let mut first = fields.next().unwrap_or_default();
        let mut revoked = false;
        if first.starts_with('@') {
            match first {
                "@revoked" => revoked = true,
                _ => continue, // @cert-authority: not supported yet
            }
            first = fields.next().unwrap_or_default();
        }
        if !hosts_field_matches(first, &target) {
            continue;
        }
        let rest: Vec<&str> = fields.collect();
        if rest.len() < 2 {
            continue;
        }
        if let Ok(key) = PublicKey::from_openssh(&format!("{} {}", rest[0], rest[1])) {
            out.push(Entry { line: i, revoked, key });
        }
    }
    out
}

fn classify(entries: &[Entry], key: &PublicKey) -> Option<HostKeyStatus> {
    if entries.iter().any(|e| e.revoked && e.key.key_data() == key.key_data()) {
        return Some(HostKeyStatus::Changed { previous: vec![] });
    }
    let same_alg: Vec<&Entry> = entries.iter().filter(|e| !e.revoked && e.key.algorithm() == key.algorithm()).collect();
    if same_alg.iter().any(|e| e.key.key_data() == key.key_data()) {
        return Some(HostKeyStatus::Trusted);
    }
    if !same_alg.is_empty() {
        return Some(HostKeyStatus::Changed {
            previous: same_alg.iter().map(|e| HostKeyInfo::from_key(&e.key)).collect(),
        });
    }
    None
}

#[derive(Debug, Clone)]
pub struct KnownHosts {
    /// NIGHTGRID's own file (read/write).
    path: PathBuf,
    /// The user's `~/.ssh/known_hosts` (read only).
    system: Option<PathBuf>,
}

impl KnownHosts {
    pub fn new(path: impl Into<PathBuf>, system: Option<PathBuf>) -> Self {
        Self { path: path.into(), system }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn check(&self, host: &str, port: u16, key: &PublicKey) -> Result<HostKeyStatus> {
        if let Some(status) = classify(&entries_for(&self.path, host, port), key) {
            return Ok(status);
        }
        if let Some(sys) = &self.system {
            if let Some(status) = classify(&entries_for(sys, host, port), key) {
                if status == HostKeyStatus::Trusted {
                    // Remember it so our file is self-sufficient.
                    self.append(host, port, key)?;
                }
                return Ok(status);
            }
        }
        Ok(HostKeyStatus::Unknown)
    }

    /// Records `key` as trusted for host:port. With `replace`, previously
    /// recorded keys of the same algorithm are removed first (used after the
    /// user explicitly accepts a changed key).
    pub fn trust(&self, host: &str, port: u16, key: &PublicKey, replace: bool) -> Result<()> {
        if replace {
            let drop: Vec<usize> = entries_for(&self.path, host, port)
                .into_iter()
                .filter(|e| e.key.algorithm() == key.algorithm())
                .map(|e| e.line)
                .collect();
            if !drop.is_empty() {
                let text = std::fs::read_to_string(&self.path)?;
                let kept: Vec<&str> =
                    text.lines().enumerate().filter(|(i, _)| !drop.contains(i)).map(|(_, l)| l).collect();
                let mut out = kept.join("\n");
                if !out.is_empty() {
                    out.push('\n');
                }
                atomic_write(&self.path, out.as_bytes())?;
            }
        } else if matches!(classify(&entries_for(&self.path, host, port), key), Some(HostKeyStatus::Changed { .. })) {
            return Err(Error::Invalid {
                field: "hostKey",
                reason: "host key changed; explicit replace required".into(),
            });
        }
        if classify(&entries_for(&self.path, host, port), key) != Some(HostKeyStatus::Trusted) {
            self.append(host, port, key)?;
        }
        Ok(())
    }

    fn append(&self, host: &str, port: u16, key: &PublicKey) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let openssh = key.to_openssh().map_err(|e| Error::Invalid { field: "hostKey", reason: e.to_string() })?;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        writeln!(f, "{} {}", host_port(host, port), openssh)?;
        Ok(())
    }

    /// Copies all lines from the system file that aren't already in ours.
    /// Returns the number of lines added.
    /// [`Self::trust`] for a key in OpenSSH `algo base64` form (as the UI got
    /// it in [`HostKeyInfo::openssh`]).
    pub fn trust_openssh(&self, host: &str, port: u16, openssh: &str, replace: bool) -> Result<()> {
        let key = PublicKey::from_openssh(openssh.trim())
            .map_err(|e| Error::Invalid { field: "hostKey", reason: e.to_string() })?;
        self.trust(host, port, &key, replace)
    }

    pub fn import_system(&self) -> Result<usize> {
        let Some(sys) = &self.system else { return Ok(0) };
        let Ok(theirs) = std::fs::read_to_string(sys) else { return Ok(0) };
        let ours = std::fs::read_to_string(&self.path).unwrap_or_default();
        let existing: std::collections::HashSet<&str> = ours.lines().map(str::trim).collect();
        let new: Vec<&str> = theirs
            .lines()
            .map(str::trim)
            .filter(|l| !l.starts_with('#') && l.split_whitespace().count() >= 3 && !existing.contains(l))
            .collect();
        if new.is_empty() {
            return Ok(0);
        }
        let mut out = ours;
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        for l in &new {
            out.push_str(l);
            out.push('\n');
        }
        atomic_write(&self.path, out.as_bytes())?;
        Ok(new.len())
    }
}

/// Write to a temp file in the same directory, then rename over the target.
pub(crate) fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".{}.tmp", path.file_name().and_then(|n| n.to_str()).unwrap_or("nightgrid")));
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use russh::keys::ssh_key::{Algorithm, PrivateKey};

    fn key() -> PublicKey {
        PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap().public_key().clone()
    }

    #[test]
    fn tofu_flow() {
        let dir = tempfile::tempdir().unwrap();
        let kh = KnownHosts::new(dir.path().join("known_hosts"), None);
        let k1 = key();
        assert_eq!(kh.check("lumen", 22, &k1).unwrap(), HostKeyStatus::Unknown);
        kh.trust("lumen", 22, &k1, false).unwrap();
        assert_eq!(kh.check("lumen", 22, &k1).unwrap(), HostKeyStatus::Trusted);
        // Different port is a different host.
        assert_eq!(kh.check("lumen", 2222, &k1).unwrap(), HostKeyStatus::Unknown);

        let k2 = key();
        let HostKeyStatus::Changed { previous } = kh.check("lumen", 22, &k2).unwrap() else {
            panic!("expected changed")
        };
        assert_eq!(previous[0].fingerprint, HostKeyInfo::from_key(&k1).fingerprint);

        // Accepting a changed key needs replace = true.
        assert!(kh.trust("lumen", 22, &k2, false).is_err());
        kh.trust("lumen", 22, &k2, true).unwrap();
        assert_eq!(kh.check("lumen", 22, &k2).unwrap(), HostKeyStatus::Trusted);
        assert!(matches!(kh.check("lumen", 22, &k1).unwrap(), HostKeyStatus::Changed { .. }));
        // Trusting twice doesn't duplicate lines.
        kh.trust("lumen", 22, &k2, false).unwrap();
        let text = std::fs::read_to_string(kh.path()).unwrap();
        assert_eq!(text.lines().count(), 1);
    }

    #[test]
    fn system_file_hashed_and_wildcards() {
        let dir = tempfile::tempdir().unwrap();
        let k = key();
        let kline = k.to_openssh().unwrap();
        // Hashed entry for "lumen" built the same way OpenSSH does.
        let salt = [7u8; 20];
        let mut mac = <Hmac<Sha1> as KeyInit>::new_from_slice(&salt).unwrap();
        mac.update(b"lumen");
        let hash = mac.finalize().into_bytes();
        let b64 = base64::engine::general_purpose::STANDARD;
        let hashed = format!("|1|{}|{}", b64.encode(salt), b64.encode(hash));
        let other = key().to_openssh().unwrap();
        let sys = dir.path().join("system_known_hosts");
        std::fs::write(
            &sys,
            format!(
                "# comment\n\n@cert-authority *.lan {other}\ngarbage line\n{hashed} {kline}\n*.corp,!bad.corp\t{other}\n"
            ),
        )
        .unwrap();
        let kh = KnownHosts::new(dir.path().join("ours"), Some(sys));
        assert_eq!(kh.check("lumen", 22, &k).unwrap(), HostKeyStatus::Trusted);
        // Learned into our file.
        assert!(std::fs::read_to_string(kh.path()).unwrap().contains(&kline));
        // Wildcard line applies to a.corp (different key → changed), not to bad.corp.
        assert!(matches!(kh.check("a.corp", 22, &k).unwrap(), HostKeyStatus::Changed { .. }));
        assert_eq!(kh.check("bad.corp", 22, &k).unwrap(), HostKeyStatus::Unknown);
        assert_eq!(kh.import_system().unwrap(), 3, "cert-authority, hashed and wildcard lines");
        assert_eq!(kh.import_system().unwrap(), 0, "idempotent");
    }

    #[test]
    fn revoked_key_is_blocked() {
        let dir = tempfile::tempdir().unwrap();
        let k = key();
        let path = dir.path().join("kh");
        std::fs::write(&path, format!("lumen {}\n@revoked * {}\n", k.to_openssh().unwrap(), k.to_openssh().unwrap()))
            .unwrap();
        let kh = KnownHosts::new(path, None);
        assert!(matches!(kh.check("lumen", 22, &k).unwrap(), HostKeyStatus::Changed { .. }));
    }
}
