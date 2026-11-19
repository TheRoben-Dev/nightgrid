//! Signed interface patches (D-183): the built frontend (`dist/`) as one
//! signed file, published by CI when main has interface-only changes since
//! the release of its version, so UI fixes reach installed apps without a
//! full app release. The app serves a verified pack in place of the UI built
//! into it on the next start.
//!
//! A pack is pinned to exactly one build of the app's core: the app version
//! and a hash of the typed command interface (`ui/src/lib/bindings.ts`) must
//! both equal the running app's, and it must be newer than the UI bundled
//! with the app. The pack is gzipped JSON (`ui-pack.json.gz`) signed with the
//! release key (`ui-pack.json.gz.sig`, the Tauri updater's minisign format),
//! plus an unsigned `ui-pack.meta.json` that only says whether to download.
//! Nothing in the meta is trusted, and the stored pack is verified again
//! every time it is opened.
//!
//! A pack only replaces web assets. Capabilities, the CSP and the command
//! allow-list are compiled into the app, so a pack can't widen them.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use crate::kbpack::verify;

/// The pack layout this app reads.
pub const FORMAT: u32 = 1;
/// Largest unpacked interface accepted.
pub const MAX_CONTENT: usize = 96 * 1024 * 1024;
/// Largest download accepted (gzipped).
pub const MAX_DOWNLOAD: usize = 48 * 1024 * 1024;
const MAX_FILES: usize = 4000;

pub const PACK_FILE: &str = "ui-pack.json.gz";
pub const SIG_FILE: &str = "ui-pack.json.gz.sig";
pub const META_FILE: &str = "ui-pack.meta.json";
/// Present when the user turned interface patches off.
const OFF_FILE: &str = "off";
const STATE_FILE: &str = "state.json";

/// Launches a pack may take without the interface reporting in before it is
/// set aside for good (the watchdog usually catches it on the first).
pub const MAX_TRIES: u32 = 2;

/// The hash a pack is pinned to: SHA-256 of the typed command interface,
/// with line endings normalised (a Windows checkout may have CRLF).
pub fn api_hash(bindings_ts: &str) -> String {
    let text = bindings_ts.replace("\r\n", "\n");
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// The unsigned pointer published next to the pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub format: u32,
    pub app_version: String,
    pub api_hash: String,
    pub built: u64,
    #[serde(default)]
    pub bytes: u64,
}

/// A verified pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pack {
    pub format: u32,
    /// The only app version this interface runs on.
    pub app_version: String,
    /// [`api_hash`] of the command interface it was built against.
    pub api_hash: String,
    /// Unix time of the commit it was built from.
    pub built: u64,
    #[serde(default)]
    pub commit: String,
    /// Short notes on what changed (shown in Settings).
    #[serde(default)]
    pub notes: Vec<String>,
    /// `index.html`, `assets/x.js` … → base64 of the file.
    pub files: BTreeMap<String, String>,
}

/// What this app is: a pack must match it exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct Target<'a> {
    pub app_version: &'a str,
    pub api_hash: &'a str,
    /// Commit time of the interface bundled with the app.
    pub bundled_built: u64,
}

impl Target<'_> {
    /// Same app, same command interface, newer than the bundled UI.
    pub fn fits(&self, app_version: &str, api_hash: &str, built: u64) -> Result<(), String> {
        if app_version != self.app_version {
            return Err(format!("built for Nightgrid {app_version}, this is {}", self.app_version));
        }
        if api_hash != self.api_hash {
            return Err("built against a different app core".into());
        }
        if built <= self.bundled_built {
            return Err("not newer than the interface built into this app".into());
        }
        Ok(())
    }
}

/// Whether a published pack should be downloaded.
pub fn wanted(meta: &Meta, target: &Target, have: Option<u64>, bad: &[u64]) -> bool {
    meta.format == FORMAT
        && target.fits(&meta.app_version, &meta.api_hash, meta.built).is_ok()
        && have.map_or(true, |h| meta.built > h)
        && !bad.contains(&meta.built)
}

/// A file path a pack may carry: relative, plain names, no dot-files.
fn path_ok(p: &str) -> bool {
    let segs: Vec<&str> = p.split('/').collect();
    p.len() <= 200
        && segs.len() <= 6
        && segs.iter().all(|s| {
            !s.is_empty()
                && !s.starts_with('.')
                && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@' | '~'))
        })
}

/// Unzips and checks a pack's shape (not its signature: see [`open`]).
pub fn decode(gz: &[u8]) -> Result<Pack, String> {
    let mut json = Vec::new();
    flate2::read::GzDecoder::new(gz)
        .take(MAX_CONTENT as u64 * 2)
        .read_to_end(&mut json)
        .map_err(|e| format!("pack isn't gzip: {e}"))?;
    let pack: Pack = serde_json::from_slice(&json).map_err(|e| format!("pack isn't readable: {e}"))?;
    if pack.format != FORMAT {
        return Err(format!("pack format {} isn't supported by this version", pack.format));
    }
    if pack.files.len() > MAX_FILES {
        return Err("pack has too many files".into());
    }
    if !pack.files.contains_key("index.html") {
        return Err("pack has no index.html".into());
    }
    let mut total = 0usize;
    for (p, b64) in &pack.files {
        if !path_ok(p) {
            return Err(format!("pack has an unexpected file: {p}"));
        }
        total += b64.len() / 4 * 3;
    }
    if total > MAX_CONTENT {
        return Err("pack is too large".into());
    }
    Ok(pack)
}

/// Verifies, unzips and checks a pack for this app.
pub fn open(pubkey_b64: &str, gz: &[u8], sig_b64: &str, target: &Target) -> Result<Pack, String> {
    verify(pubkey_b64, gz, sig_b64)?;
    let pack = decode(gz)?;
    target.fits(&pack.app_version, &pack.api_hash, pack.built)?;
    Ok(pack)
}

/// The pack's files decoded, keyed like Tauri's assets (`/index.html`).
pub fn unpack(pack: &Pack) -> Result<BTreeMap<String, Vec<u8>>, String> {
    use base64::Engine;
    pack.files
        .iter()
        .map(|(p, b64)| {
            base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map(|bytes| (format!("/{p}"), bytes))
                .map_err(|_| format!("{p} isn't base64"))
        })
        .collect()
}

/// Where packs live: `<data>/ui-pack/`.
pub fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("ui-pack")
}

/// The stored pack and signature, if both are there.
pub fn read_stored(data_dir: &Path) -> Option<(Vec<u8>, String)> {
    let d = dir(data_dir);
    let gz = std::fs::read(d.join(PACK_FILE)).ok()?;
    let sig = std::fs::read_to_string(d.join(SIG_FILE)).ok()?;
    Some((gz, sig))
}

/// Opens the stored pack, or why it can't be used.
pub fn load_stored(data_dir: &Path, pubkey_b64: &str, target: &Target) -> Result<Option<Pack>, String> {
    match read_stored(data_dir) {
        None => Ok(None),
        Some((gz, sig)) => open(pubkey_b64, &gz, &sig, target).map(Some),
    }
}

/// Saves a verified pack (temp files then rename, signature last, so a
/// half-written pair never verifies).
pub fn store(data_dir: &Path, gz: &[u8], sig: &str) -> std::io::Result<()> {
    let d = dir(data_dir);
    std::fs::create_dir_all(&d)?;
    let _ = std::fs::remove_file(d.join(SIG_FILE));
    let tmp = d.join(format!("{PACK_FILE}.tmp"));
    std::fs::write(&tmp, gz)?;
    std::fs::rename(&tmp, d.join(PACK_FILE))?;
    let tmp = d.join(format!("{SIG_FILE}.tmp"));
    std::fs::write(&tmp, sig)?;
    std::fs::rename(&tmp, d.join(SIG_FILE))
}

/// Removes the stored pack (back to the bundled interface on next start).
/// The on/off switch and the list of failed packs stay.
pub fn remove(data_dir: &Path) -> std::io::Result<()> {
    let d = dir(data_dir);
    for f in [SIG_FILE, PACK_FILE] {
        match std::fs::remove_file(d.join(f)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

/// Whether the user turned patches off (read before the app's database opens).
pub fn is_off(data_dir: &Path) -> bool {
    dir(data_dir).join(OFF_FILE).exists()
}

/// Mirrors the setting into a file the next start can read early.
pub fn set_off(data_dir: &Path, off: bool) -> std::io::Result<()> {
    let p = dir(data_dir).join(OFF_FILE);
    if off {
        std::fs::create_dir_all(dir(data_dir))?;
        std::fs::write(p, b"interface patches are turned off in Settings\n")
    } else {
        match std::fs::remove_file(p) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

/// Start-up bookkeeping: a pack that never reported in gets set aside.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BootState {
    /// The pack the counters are about.
    pub built: u64,
    /// Starts with this pack that haven't reported in yet.
    pub tries: u32,
    /// The interface reported in at least once.
    pub ok: bool,
    /// Packs (by `built`) that failed: never used or downloaded again.
    pub bad: Vec<u64>,
    /// Why the last pack was set aside.
    pub last_error: Option<String>,
}

impl BootState {
    pub fn load(data_dir: &Path) -> Self {
        std::fs::read(dir(data_dir).join(STATE_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, data_dir: &Path) -> std::io::Result<()> {
        let d = dir(data_dir);
        std::fs::create_dir_all(&d)?;
        let tmp = d.join(format!("{STATE_FILE}.tmp"));
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).unwrap_or_default())?;
        std::fs::rename(tmp, d.join(STATE_FILE))
    }

    /// Called when a start is about to use pack `built`: false means don't
    /// (it failed before, or took too many starts without reporting in).
    pub fn begin(&mut self, built: u64) -> bool {
        if self.bad.contains(&built) {
            return false;
        }
        if self.built != built {
            *self = Self { built, bad: std::mem::take(&mut self.bad), ..Self::default() };
        }
        if self.ok {
            return true;
        }
        if self.tries >= MAX_TRIES {
            self.fail(built, "the interface didn't start with this patch");
            return false;
        }
        self.tries += 1;
        true
    }

    /// The interface from pack `built` is up.
    pub fn ready(&mut self, built: u64) {
        if self.built == built {
            self.ok = true;
            self.tries = 0;
        }
    }

    /// Sets pack `built` aside for good.
    pub fn fail(&mut self, built: u64, why: &str) {
        if !self.bad.contains(&built) {
            self.bad.push(built);
        }
        // Keep the list short; old packs can't fit a newer app anyway.
        let n = self.bad.len();
        if n > 20 {
            self.bad.drain(..n - 20);
        }
        self.ok = false;
        self.tries = 0;
        self.last_error = Some(why.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // A throwaway test key (not the release key): fixtures/uipack/ holds a
    // pack signed with it by `tauri signer sign`.
    const TEST_PUB: &str = include_str!("../fixtures/uipack/test.pub");
    const TEST_GZ: &[u8] = include_bytes!("../fixtures/uipack/ui-pack.json.gz");
    const TEST_SIG: &str = include_str!("../fixtures/uipack/ui-pack.json.gz.sig");

    fn target() -> Target<'static> {
        Target { app_version: "9.9.9", api_hash: "abc", bundled_built: 100 }
    }

    fn gz(json: &str) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(json.as_bytes()).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn hashes_the_interface_the_same_on_every_os() {
        assert_eq!(api_hash("a\r\nb\n"), api_hash("a\nb\n"));
        assert_ne!(api_hash("a"), api_hash("b"));
        assert_eq!(api_hash("").len(), 64);
    }

    #[test]
    fn fits_only_the_exact_app() {
        let t = target();
        assert!(t.fits("9.9.9", "abc", 101).is_ok());
        assert!(t.fits("9.9.8", "abc", 101).unwrap_err().contains("built for Nightgrid 9.9.8"));
        assert!(t.fits("9.9.10", "abc", 101).is_err(), "newer app versions don't fit either");
        assert!(t.fits("9.9.9", "abd", 101).unwrap_err().contains("core"));
        assert!(t.fits("9.9.9", "abc", 100).unwrap_err().contains("not newer"));
    }

    #[test]
    fn wants_only_newer_fitting_packs() {
        let t = target();
        let m = Meta { format: FORMAT, app_version: "9.9.9".into(), api_hash: "abc".into(), built: 200, bytes: 0 };
        assert!(wanted(&m, &t, None, &[]));
        assert!(wanted(&m, &t, Some(150), &[]));
        assert!(!wanted(&m, &t, Some(200), &[]), "same as stored");
        assert!(!wanted(&m, &t, None, &[200]), "failed before");
        assert!(!wanted(&Meta { format: 2, ..m.clone() }, &t, None, &[]));
        assert!(!wanted(&Meta { api_hash: "x".into(), ..m.clone() }, &t, None, &[]));
    }

    #[test]
    fn verifies_the_signed_fixture_and_rejects_tampering() {
        let t = Target { app_version: "1.8.0", api_hash: "fixture", bundled_built: 1 };
        let p = open(TEST_PUB, TEST_GZ, TEST_SIG, &t).expect("fixture verifies");
        let files = unpack(&p).unwrap();
        assert!(String::from_utf8_lossy(&files["/index.html"]).contains("<html"));
        assert!(files.contains_key("/assets/app.js"));
        let mut bad = TEST_GZ.to_vec();
        let n = bad.len();
        bad[n - 9] ^= 1;
        assert!(open(TEST_PUB, &bad, TEST_SIG, &t).unwrap_err().contains("signature"));
        let other = Target { app_version: "1.8.1", ..t.clone() };
        assert!(open(TEST_PUB, TEST_GZ, TEST_SIG, &other).is_err());
        // The release key doesn't accept the test signature.
        let release = include_str!("../../app/updater.pub");
        assert!(verify(release, TEST_GZ, TEST_SIG).is_err());
    }

    #[test]
    fn rejects_bad_shapes() {
        let ok = r#"{"format":1,"appVersion":"9.9.9","apiHash":"abc","built":5,"files":{"index.html":"PGh0bWw+","assets/a.js":"eA=="}}"#;
        assert!(decode(&gz(ok)).is_ok());
        for (from, to, why) in [
            ("assets/a.js", "../a.js", "unexpected file"),
            ("assets/a.js", "/etc/a.js", "unexpected file"),
            ("assets/a.js", "assets/.hidden", "unexpected file"),
            ("assets/a.js", "a\\\\b.js", "unexpected file"),
            ("\"index.html\"", "\"main.html\"", "no index.html"),
            ("\"format\":1", "\"format\":9", "format 9"),
        ] {
            let e = decode(&gz(&ok.replace(from, to))).unwrap_err();
            assert!(e.contains(why), "{from} → {e}");
        }
        assert!(decode(b"not gzip").is_err());
        let p = decode(&gz(&ok.replace("eA==", "!!"))).unwrap();
        assert!(unpack(&p).is_err());
    }

    #[test]
    fn stores_switches_and_removes() {
        let tmp = tempfile::tempdir().unwrap();
        let t = Target { app_version: "1.8.0", api_hash: "fixture", bundled_built: 1 };
        assert_eq!(load_stored(tmp.path(), TEST_PUB, &t), Ok(None));
        store(tmp.path(), TEST_GZ, TEST_SIG).unwrap();
        assert!(load_stored(tmp.path(), TEST_PUB, &t).unwrap().is_some());
        std::fs::write(dir(tmp.path()).join(PACK_FILE), b"x").unwrap();
        assert!(load_stored(tmp.path(), TEST_PUB, &t).is_err(), "a corrupted file is never trusted");
        assert!(!is_off(tmp.path()));
        set_off(tmp.path(), true).unwrap();
        assert!(is_off(tmp.path()));
        set_off(tmp.path(), false).unwrap();
        set_off(tmp.path(), false).unwrap();
        assert!(!is_off(tmp.path()));
        remove(tmp.path()).unwrap();
        remove(tmp.path()).unwrap();
        assert!(read_stored(tmp.path()).is_none());
    }

    #[test]
    fn sets_aside_a_pack_that_never_reports_in() {
        let tmp = tempfile::tempdir().unwrap();
        let mut s = BootState::load(tmp.path());
        assert!(s.begin(7));
        assert!(s.begin(7), "second try");
        assert!(!s.begin(7), "third start without the UI reporting in");
        assert!(s.bad.contains(&7));
        assert!(!s.begin(7), "stays aside");
        // A newer pack starts fresh but remembers the failed one.
        assert!(s.begin(8));
        s.ready(8);
        assert!(s.begin(8) && s.begin(8) && s.begin(8), "a pack that reported in keeps working");
        s.fail(8, "watchdog");
        assert!(!s.begin(8));
        assert_eq!(s.last_error.as_deref(), Some("watchdog"));
        s.save(tmp.path()).unwrap();
        assert_eq!(BootState::load(tmp.path()), s);
    }
}
