//! Component packs (feature store part 2, D-207): a store component's
//! interface as one signed file, published by CI to the rolling
//! `components-latest` release whenever that component's files change on
//! main, so a fix to one component reaches installed apps without an app
//! release. The app keeps the packs of installed components in
//! `<data>/components/`, verifies each again whenever it's opened, and
//! serves its files at `/components/<id>/<built>/…`; the interface decides
//! whether a pack fits the running app (`ui/src/lib/componentUi.ts`).
//!
//! Files: `<id>.json.gz` (gzipped JSON, see [`Pack`]), `<id>.json.gz.sig`
//! (the release key's minisign signature, as for app updates) and one
//! unsigned `index.json` listing what's published. Nothing in the index is
//! trusted: it only says what to download.
//!
//! A pack only adds web assets served from the app's own origin. The CSP,
//! the capabilities and the command allow-list are compiled into the app,
//! so a pack can't widen what the interface may do.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use crate::kbpack::{app_ok, verify};

/// The pack layout this app reads.
pub const FORMAT: u32 = 1;
/// Largest unpacked pack accepted.
pub const MAX_CONTENT: usize = 16 * 1024 * 1024;
/// Largest download accepted (gzipped).
pub const MAX_DOWNLOAD: usize = 8 * 1024 * 1024;
const MAX_FILES: usize = 200;
pub const INDEX_FILE: &str = "index.json";

pub fn pack_file(id: &str) -> String {
    format!("{id}.json.gz")
}

pub fn sig_file(id: &str) -> String {
    format!("{id}.json.gz.sig")
}

/// One line of the unsigned index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexEntry {
    pub id: String,
    pub built: u64,
    #[serde(default)]
    pub min_app: String,
    #[serde(default)]
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub format: u32,
    #[serde(default)]
    pub components: Vec<IndexEntry>,
}

/// A verified pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pack {
    pub format: u32,
    /// Store id (`proxmox` …), one of [`crate::components::COMPONENTS`].
    pub id: String,
    /// Unix time of the newest commit to the component's files.
    pub built: u64,
    /// Oldest app version it may be offered to (the version it was built on).
    pub min_app: String,
    #[serde(default)]
    pub commit: String,
    /// What changed (shown in Settings → STORE).
    #[serde(default)]
    pub notes: Vec<String>,
    /// The module the interface imports, relative to the pack.
    pub entry: String,
    #[serde(default)]
    pub style: bool,
    /// What the pack needs from the running interface, checked there.
    pub requires: serde_json::Value,
    /// Path → base64 of the file.
    pub files: BTreeMap<String, String>,
}

fn id_ok(id: &str) -> bool {
    crate::components::by_id(id).is_some()
}

fn path_ok(p: &str) -> bool {
    let segs: Vec<&str> = p.split('/').collect();
    p.len() <= 160
        && segs.len() <= 4
        && segs.iter().all(|s| {
            !s.is_empty()
                && !s.starts_with('.')
                && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        })
        && [".js", ".css", ".svg", ".png", ".woff2", ".json"].iter().any(|e| p.ends_with(e))
}

/// Whether a published pack should be downloaded for this app.
pub fn wanted(e: &IndexEntry, have: Option<u64>, app: &str) -> bool {
    id_ok(&e.id) && (e.min_app.is_empty() || app_ok(&e.min_app, app)) && have.map_or(true, |h| e.built > h)
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
    if !id_ok(&pack.id) {
        return Err(format!("{} isn't a store component", pack.id));
    }
    if pack.files.len() > MAX_FILES {
        return Err("pack has too many files".into());
    }
    if !pack.entry.ends_with(".js") || !pack.files.contains_key(&pack.entry) {
        return Err("pack has no entry module".into());
    }
    if pack.style && !pack.files.contains_key("style.css") {
        return Err("pack has no style.css".into());
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

/// Verifies, unzips and checks a pack for this app and component.
pub fn open(pubkey_b64: &str, id: &str, gz: &[u8], sig_b64: &str, app: &str) -> Result<Pack, String> {
    verify(pubkey_b64, gz, sig_b64)?;
    let pack = decode(gz)?;
    if pack.id != id {
        return Err(format!("pack is for {}, not {id}", pack.id));
    }
    if !app_ok(&pack.min_app, app) {
        return Err(format!("this patch needs Nightgrid {} or later", pack.min_app));
    }
    Ok(pack)
}

/// The pack's files decoded, keyed by their path in the pack.
pub fn unpack(pack: &Pack) -> Result<BTreeMap<String, Vec<u8>>, String> {
    use base64::Engine;
    pack.files
        .iter()
        .map(|(p, b64)| {
            base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map(|bytes| (p.clone(), bytes))
                .map_err(|_| format!("{p} isn't base64"))
        })
        .collect()
}

/// Where packs live: `<data>/components/`.
pub fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("components")
}

/// Opens the stored pack of one component, or why it can't be used.
pub fn load_stored(data_dir: &Path, pubkey_b64: &str, id: &str, app: &str) -> Result<Option<Pack>, String> {
    let d = dir(data_dir);
    let (Ok(gz), Ok(sig)) = (std::fs::read(d.join(pack_file(id))), std::fs::read_to_string(d.join(sig_file(id))))
    else {
        return Ok(None);
    };
    open(pubkey_b64, id, &gz, &sig, app).map(Some)
}

/// Saves a verified pack (temp files then rename, signature last).
pub fn store(data_dir: &Path, id: &str, gz: &[u8], sig: &str) -> std::io::Result<()> {
    let d = dir(data_dir);
    std::fs::create_dir_all(&d)?;
    let _ = std::fs::remove_file(d.join(sig_file(id)));
    let tmp = d.join(format!("{}.tmp", pack_file(id)));
    std::fs::write(&tmp, gz)?;
    std::fs::rename(&tmp, d.join(pack_file(id)))?;
    let tmp = d.join(format!("{}.tmp", sig_file(id)));
    std::fs::write(&tmp, sig)?;
    std::fs::rename(&tmp, d.join(sig_file(id)))
}

/// Removes a component's stored pack (back to the built-in copy).
pub fn remove(data_dir: &Path, id: &str) -> std::io::Result<()> {
    let d = dir(data_dir);
    for f in [sig_file(id), pack_file(id)] {
        match std::fs::remove_file(d.join(f)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

/// Splits a served path `components/<id>/<built>/<file>` (leading `/`
/// allowed) into its parts.
pub fn served_path(key: &str) -> Option<(&str, u64, &str)> {
    let rest = key.trim_start_matches('/').strip_prefix("components/")?;
    let (id, rest) = rest.split_once('/')?;
    let (built, file) = rest.split_once('/')?;
    Some((id, built.parse().ok()?, file))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::io::Write;

    fn pack(id: &str) -> Pack {
        let b = |s: &str| base64::engine::general_purpose::STANDARD.encode(s);
        Pack {
            format: FORMAT,
            id: id.into(),
            built: 100,
            min_app: "1.0.0".into(),
            commit: "abc".into(),
            notes: vec!["fix".into()],
            entry: "entry.js".into(),
            style: true,
            requires: serde_json::json!({ "svelte": "5" }),
            files: [("entry.js".to_owned(), b("export default 1")), ("style.css".to_owned(), b("a{}"))].into(),
        }
    }

    fn gz(p: &Pack) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        e.write_all(&serde_json::to_vec(p).unwrap()).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn decodes_and_checks_shape() {
        let p = pack("proxmox");
        let back = decode(&gz(&p)).unwrap();
        assert_eq!(back, p);
        assert_eq!(unpack(&back).unwrap()["entry.js"], b"export default 1");

        let mut bad = pack("not-a-component");
        assert!(decode(&gz(&bad)).unwrap_err().contains("isn't a store component"));
        bad = pack("proxmox");
        bad.files.insert("../x.js".into(), String::new());
        assert!(decode(&gz(&bad)).unwrap_err().contains("unexpected file"));
        bad = pack("proxmox");
        bad.files.insert("run.sh".into(), String::new());
        assert!(decode(&gz(&bad)).is_err());
        bad = pack("proxmox");
        bad.entry = "missing.js".into();
        assert!(decode(&gz(&bad)).unwrap_err().contains("entry"));
        bad = pack("proxmox");
        bad.files.remove("style.css");
        assert!(decode(&gz(&bad)).unwrap_err().contains("style.css"));
        assert!(decode(b"nope").is_err());
    }

    #[test]
    fn unsigned_packs_are_refused() {
        let p = gz(&pack("proxmox"));
        assert!(open("bm90IGEga2V5", "proxmox", &p, "bm90IGEgc2ln", "9.9.9").is_err());
    }

    #[test]
    fn wanted_rules() {
        let e = IndexEntry { id: "ups".into(), built: 200, min_app: "1.5.0".into(), bytes: 1 };
        assert!(wanted(&e, None, "1.5.0"));
        assert!(wanted(&e, Some(199), "2.0.0"));
        assert!(!wanted(&e, Some(200), "2.0.0"));
        assert!(!wanted(&e, None, "1.4.9"));
        assert!(!wanted(&IndexEntry { id: "nope".into(), ..e.clone() }, None, "2.0.0"));
    }

    #[test]
    fn served_paths() {
        assert_eq!(served_path("/components/proxmox/123/entry.js"), Some(("proxmox", 123, "entry.js")));
        assert_eq!(served_path("components/ups/9/a/b.svg"), Some(("ups", 9, "a/b.svg")));
        assert_eq!(served_path("/components/ups/x/entry.js"), None);
        assert_eq!(served_path("/assets/x.js"), None);
    }

    #[test]
    fn store_and_remove() {
        let dir = tempfile::tempdir().unwrap();
        store(dir.path(), "ups", b"gz", "sig").unwrap();
        assert!(load_stored(dir.path(), "bm90IGEga2V5", "ups", "1.0.0").is_err(), "stored but not signed");
        remove(dir.path(), "ups").unwrap();
        assert_eq!(load_stored(dir.path(), "bm90IGEga2V5", "ups", "1.0.0"), Ok(None));
        remove(dir.path(), "ups").unwrap();
    }
}
