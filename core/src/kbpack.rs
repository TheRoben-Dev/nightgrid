//! Library content packs (D-174): the docs/kb articles, figures and their
//! catalog lines as one signed file, published by CI whenever only Library
//! content changes, so new articles reach installed apps without a release.
//!
//! The pack is gzipped JSON (`kb-pack.json.gz`) with a minisign signature
//! made by the release key (`kb-pack.json.gz.sig`, the Tauri updater's
//! format), plus a small unsigned `kb-pack.meta.json` that says whether the
//! download is worth it. Nothing in the meta is trusted: the pack is only
//! stored after its signature verifies, and it is verified again every time
//! it is opened. A pack is used only when its format is known, the app is at
//! least its `minApp`, and its version (the content commit's time) is newer
//! than both the stored pack and the copy bundled with the app; anything else
//! leaves the bundled Library in place. The "Using Nightgrid" shelf is not in
//! a pack: it describes the installed app's features and ships with it.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The pack layout this app reads.
pub const FORMAT: u32 = 1;
/// The Library's ceiling (articles plus figures, uncompressed).
pub const MAX_CONTENT: usize = 25 * 1024 * 1024;
/// Largest download accepted (gzipped).
pub const MAX_DOWNLOAD: usize = 16 * 1024 * 1024;
const MAX_FILES: usize = 5000;

pub const PACK_FILE: &str = "kb-pack.json.gz";
pub const SIG_FILE: &str = "kb-pack.json.gz.sig";
pub const META_FILE: &str = "kb-pack.meta.json";

/// The shelves a pack may carry (never `nightgrid`).
pub const SHELVES: [&str; 6] = ["basics", "guides", "howto", "apps", "linux", "errors"];

/// One catalog line, as in ui/src/lib/kb/catalog.ts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct KbCatalogEntry {
    pub id: String,
    pub shelf: String,
    pub title: String,
    pub cat: String,
}

/// The unsigned pointer published next to the pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub format: u32,
    pub version: u32,
    pub min_app: String,
    #[serde(default)]
    pub bytes: u64,
}

/// A verified pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pack {
    pub format: u32,
    /// Unix time of the commit the content was built from.
    pub version: u32,
    /// Oldest app version the content is written for.
    pub min_app: String,
    #[serde(default)]
    pub commit: String,
    pub catalog: Vec<KbCatalogEntry>,
    /// `shelf/slug.md` → markdown, `figures/name.svg` → SVG text.
    pub files: BTreeMap<String, String>,
}

impl Pack {
    pub fn articles(&self) -> usize {
        self.files.keys().filter(|k| k.ends_with(".md")).count()
    }
    pub fn figures(&self) -> usize {
        self.files.keys().filter(|k| k.ends_with(".svg")).count()
    }
}

/// `1.2.3` (a pre-release or build suffix is ignored) → comparable triple.
pub fn semver(v: &str) -> Option<(u64, u64, u64)> {
    let core = v.trim().trim_start_matches('v').split(['-', '+']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u64>().ok());
    let t = (it.next()??, it.next().unwrap_or(Some(0))?, it.next().unwrap_or(Some(0))?);
    Some(t)
}

/// True when `app` is at least `min`.
pub fn app_ok(min: &str, app: &str) -> bool {
    matches!((semver(min), semver(app)), (Some(m), Some(a)) if a >= m)
}

/// Whether a published pack should be downloaded: a format this app reads,
/// an app new enough, and newer than what's installed and what's bundled.
pub fn wanted(meta: &Meta, installed: Option<u32>, bundled: u32, app: &str) -> bool {
    meta.format == FORMAT
        && app_ok(&meta.min_app, app)
        && meta.version > bundled
        && installed.map_or(true, |v| meta.version > v)
}

/// Checks a minisign signature in the Tauri updater's encoding (both the
/// public key and the `.sig` file are base64 of minisign's text format).
pub fn verify(pubkey_b64: &str, data: &[u8], sig_b64: &str) -> Result<(), String> {
    use base64::Engine;
    let b64 = |s: &str| -> Result<String, String> {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(s.trim())
            .map_err(|_| "signature data isn't base64".to_owned())?;
        String::from_utf8(raw).map_err(|_| "signature data isn't text".to_owned())
    };
    let pk = minisign_verify::PublicKey::decode(&b64(pubkey_b64)?).map_err(|e| format!("bad public key: {e}"))?;
    let sig = minisign_verify::Signature::decode(&b64(sig_b64)?).map_err(|e| format!("bad signature: {e}"))?;
    pk.verify(data, &sig, false).map_err(|_| "signature doesn't match the release key".to_owned())
}

fn path_ok(p: &str) -> bool {
    let Some((dir, name)) = p.split_once('/') else { return false };
    let stem_ok = |s: &str| {
        !s.is_empty() && s.len() <= 80 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    match name.rsplit_once('.') {
        Some((stem, "md")) => SHELVES.contains(&dir) && stem_ok(stem) && !stem.eq_ignore_ascii_case("readme"),
        Some((stem, "svg")) => dir == "figures" && stem_ok(stem),
        _ => false,
    }
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
    let mut total = 0usize;
    for (p, text) in &pack.files {
        if !path_ok(p) {
            return Err(format!("pack has an unexpected file: {p}"));
        }
        total += text.len();
    }
    if total > MAX_CONTENT {
        return Err("pack is over the Library's 25 MB ceiling".into());
    }
    for c in &pack.catalog {
        let file = format!("{}.md", c.id);
        if !SHELVES.contains(&c.shelf.as_str())
            || !c.id.starts_with(&format!("{}/", c.shelf))
            || !pack.files.contains_key(&file)
        {
            return Err(format!("catalog line {} has no article", c.id));
        }
    }
    let listed = pack.catalog.len();
    if listed != pack.articles() {
        return Err(format!("catalog lists {listed} articles but the pack has {}", pack.articles()));
    }
    Ok(pack)
}

/// Verifies, unzips and checks a pack for this app.
pub fn open(pubkey_b64: &str, gz: &[u8], sig_b64: &str, app: &str) -> Result<Pack, String> {
    verify(pubkey_b64, gz, sig_b64)?;
    let pack = decode(gz)?;
    if !app_ok(&pack.min_app, app) {
        return Err(format!("this content needs Nightgrid {} or later", pack.min_app));
    }
    Ok(pack)
}

/// Where a stored pack lives: `<data>/kb-pack/`.
pub fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("kb-pack")
}

/// The stored pack and signature, if both are there.
pub fn read_stored(data_dir: &Path) -> Option<(Vec<u8>, String)> {
    let d = dir(data_dir);
    let gz = std::fs::read(d.join(PACK_FILE)).ok()?;
    let sig = std::fs::read_to_string(d.join(SIG_FILE)).ok()?;
    Some((gz, sig))
}

/// Opens the stored pack, or None when there's none or it no longer verifies.
pub fn load_stored(data_dir: &Path, pubkey_b64: &str, app: &str) -> Option<Pack> {
    let (gz, sig) = read_stored(data_dir)?;
    match open(pubkey_b64, &gz, &sig, app) {
        Ok(p) => Some(p),
        Err(e) => {
            tracing::warn!("stored Library pack ignored: {e}");
            None
        }
    }
}

/// Saves a verified pack (write to temp files, then rename, signature last
/// so a half-written pair never verifies).
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

/// Removes the stored pack (back to the bundled Library).
pub fn remove(data_dir: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(dir(data_dir)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // A throwaway test key (not the release key): fixtures/kbpack/ holds a
    // pack signed with it by `tauri signer sign`.
    const TEST_PUB: &str = include_str!("../fixtures/kbpack/test.pub");
    const TEST_GZ: &[u8] = include_bytes!("../fixtures/kbpack/kb-pack.json.gz");
    const TEST_SIG: &str = include_str!("../fixtures/kbpack/kb-pack.json.gz.sig");

    fn gz(json: &str) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(json.as_bytes()).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn compares_versions() {
        assert_eq!(semver("1.2.3"), Some((1, 2, 3)));
        assert_eq!(semver("v1.10"), Some((1, 10, 0)));
        assert_eq!(semver("1.2.0-beta.1"), Some((1, 2, 0)));
        assert_eq!(semver("x"), None);
        assert!(app_ok("1.1.0", "1.2.0"));
        assert!(app_ok("1.2.0", "1.2.0"));
        assert!(!app_ok("1.3.0", "1.2.9"));
        assert!(!app_ok("garbage", "1.2.0"));
    }

    #[test]
    fn wants_only_newer_compatible_packs() {
        let m = Meta { format: FORMAT, version: 200, min_app: "1.2.0".into(), bytes: 0 };
        assert!(wanted(&m, None, 100, "1.2.0"));
        assert!(wanted(&m, Some(150), 100, "1.2.0"));
        assert!(!wanted(&m, Some(200), 100, "1.2.0"), "same version as installed");
        assert!(!wanted(&m, None, 200, "1.2.0"), "bundled is as new");
        assert!(!wanted(&m, None, 100, "1.1.9"), "app too old");
        assert!(!wanted(&Meta { format: 2, ..m.clone() }, None, 100, "1.2.0"), "unknown format");
    }

    #[test]
    fn verifies_the_signed_fixture_and_rejects_tampering() {
        let p = open(TEST_PUB, TEST_GZ, TEST_SIG, "1.2.0").expect("fixture verifies");
        assert_eq!(p.articles(), 1);
        assert_eq!(p.figures(), 1);
        assert_eq!(p.catalog[0].id, "basics/test-article");
        let mut bad = TEST_GZ.to_vec();
        let n = bad.len();
        bad[n - 9] ^= 1;
        assert!(open(TEST_PUB, &bad, TEST_SIG, "1.2.0").unwrap_err().contains("signature"));
        assert!(open(TEST_PUB, TEST_GZ, TEST_SIG, "1.1.0").unwrap_err().contains("needs Nightgrid"));
        // The release key doesn't accept the test signature.
        let release = include_str!("../../app/updater.pub");
        assert!(verify(release, TEST_GZ, TEST_SIG).is_err());
    }

    #[test]
    fn rejects_bad_shapes() {
        let ok = r##"{"format":1,"version":5,"minApp":"1.0.0","catalog":[{"id":"apps/x","shelf":"apps","title":"X","cat":"media"}],"files":{"apps/x.md":"# X\n\nhi","figures/f.svg":"<svg viewBox=\"0 0 1 1\"></svg>"}}"##;
        assert!(decode(&gz(ok)).is_ok());
        for (from, to, why) in [
            ("apps/x.md\":", "../x.md\":", "unexpected file"),
            ("figures/f.svg", "figures/f.js", "unexpected file"),
            ("\"apps/x.md\":\"# X", "\"nightgrid/x.md\":\"# X", "unexpected file"),
            ("\"format\":1", "\"format\":9", "format 9"),
            ("\"id\":\"apps/x\"", "\"id\":\"apps/y\"", "no article"),
        ] {
            let e = decode(&gz(&ok.replace(from, to))).unwrap_err();
            assert!(e.contains(why), "{from} → {e}");
        }
        let missing = ok.replace(r#"{"id":"apps/x","shelf":"apps","title":"X","cat":"media"}"#, "");
        assert!(decode(&gz(&missing)).unwrap_err().contains("catalog lists 0"));
        assert!(decode(b"not gzip").is_err());
    }

    #[test]
    fn stores_and_removes() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load_stored(tmp.path(), TEST_PUB, "1.2.0").is_none());
        store(tmp.path(), TEST_GZ, TEST_SIG).unwrap();
        assert!(load_stored(tmp.path(), TEST_PUB, "1.2.0").is_some());
        // A corrupted file on disk is ignored, not trusted.
        std::fs::write(dir(tmp.path()).join(PACK_FILE), b"x").unwrap();
        assert!(load_stored(tmp.path(), TEST_PUB, "1.2.0").is_none());
        remove(tmp.path()).unwrap();
        remove(tmp.path()).unwrap();
        assert!(read_stored(tmp.path()).is_none());
    }
}
