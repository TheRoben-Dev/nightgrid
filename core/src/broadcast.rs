//! Transmissions (D-202): short notices from Nightgrid's developer to every
//! installed app, for problems people have to fix on their own end (a
//! provider changed something, a setting needs a nudge, a known issue with a
//! workaround). They arrive without an app update.
//!
//! The feed is `docs/notices/notices.json` in the repository, published by
//! CI as `notices.json` plus a minisign signature made with the release key
//! (`notices.json.sig`, the Tauri updater's format) on the rolling
//! `notices-latest` release. The app only shows a feed whose signature
//! verifies, verifies it again whenever it is read from disk, and never
//! replaces a stored feed with an older one, so nobody but the release key's
//! holder can put words in front of the user. The body is the markdown
//! subset the UI renders as text (never HTML).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The feed layout this app reads.
pub const FORMAT: u32 = 1;
/// Largest feed accepted (bytes of JSON).
pub const MAX_FEED: usize = 256 * 1024;
pub const MAX_NOTICES: usize = 50;
pub const MAX_TITLE: usize = 100;
pub const MAX_BODY: usize = 4000;
pub const MAX_LABEL: usize = 40;

pub const FEED_FILE: &str = "notices.json";
pub const SIG_FILE: &str = "notices.json.sig";

/// Operating systems a transmission can be aimed at (`std::env::consts::OS`).
pub const OSES: [&str; 3] = ["macos", "windows", "linux"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum BroadcastLevel {
    Info,
    Warn,
    Critical,
}

/// The one button a transmission may carry: `kb:shelf/slug` (a Library
/// article), `view:id` or `view:id/tab` (a view), or an `https://` page
/// opened in the browser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BroadcastLink {
    pub label: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Broadcast {
    /// Stable id; a dismissed id never shows again.
    pub id: String,
    pub level: BroadcastLevel,
    pub title: String,
    /// Markdown subset: paragraphs, lists, `code`, **bold**, `[x](kb:…)`, `[x](view:…)`.
    pub body: String,
    #[serde(default)]
    pub link: Option<BroadcastLink>,
    /// Only on these systems (empty: all).
    #[serde(default)]
    pub os: Vec<String>,
    /// Only on app versions in this range (inclusive).
    #[serde(default)]
    pub min_app: Option<String>,
    #[serde(default)]
    pub max_app: Option<String>,
    /// Only when at least one of these Settings → Features ids is on (empty: always).
    #[serde(default)]
    pub features: Vec<String>,
    /// `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SSZ` (UTC): shown from, and until before.
    #[serde(default)]
    pub starts: Option<String>,
    #[serde(default)]
    pub expires: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Feed {
    pub format: u32,
    /// Unix time it was published; a stored feed is never replaced by an older one.
    pub version: u64,
    pub notices: Vec<Broadcast>,
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `2026-12-01` (midnight UTC) or `2026-12-01T18:30:00Z` → unix seconds.
pub fn parse_time(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let t = s.get(r)?;
        t.bytes().all(|c| c.is_ascii_digit()).then(|| t.parse().ok())?
    };
    if b.len() != 10 && b.len() != 20 {
        return None;
    }
    if b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let dim = [
        31,
        if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=12).contains(&mo) || d < 1 || d > dim[(mo - 1) as usize] {
        return None;
    }
    let mut secs = days_from_civil(y, mo, d) * 86_400;
    if b.len() == 20 {
        if b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'Z' {
            return None;
        }
        let (h, mi, se) = (num(11..13)?, num(14..16)?, num(17..19)?);
        if h > 23 || mi > 59 || se > 59 {
            return None;
        }
        secs += h * 3600 + mi * 60 + se;
    }
    Some(secs)
}

fn id_ok(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(f) if f.is_ascii_lowercase() || f.is_ascii_digit())
        && s.len() <= 64
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Where a button may lead.
pub fn link_ok(to: &str) -> bool {
    let slug = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if let Some(id) = to.strip_prefix("kb:") {
        return matches!(id.split_once('/'), Some((shelf, slug_)) if shelf.chars().all(|c| c.is_ascii_lowercase()) && !shelf.is_empty() && slug(slug_));
    }
    if let Some(v) = to.strip_prefix("view:") {
        let mut parts = v.split('/');
        let view = parts.next().unwrap_or("");
        let tab = parts.next();
        return parts.next().is_none()
            && !view.is_empty()
            && view.chars().all(|c| c.is_ascii_lowercase())
            && tab.map_or(true, |t| !t.is_empty() && t.chars().all(|c| c.is_ascii_lowercase() || c == '-'));
    }
    if let Some(rest) = to.strip_prefix("https://") {
        let host = rest.split(['/', '?', '#']).next().unwrap_or("");
        return !host.is_empty()
            && !host.contains('@')
            && to.len() <= 500
            && !to.chars().any(|c| c.is_whitespace() || c.is_control());
    }
    false
}

/// Checks one transmission's fields.
pub fn check(b: &Broadcast) -> Result<(), String> {
    let id = &b.id;
    if !id_ok(id) {
        return Err(format!("id \"{id}\" must be 1–64 lowercase letters, digits and dashes"));
    }
    let title = b.title.trim();
    if title.is_empty() || title.chars().count() > MAX_TITLE {
        return Err(format!("{id}: the title must be 1–{MAX_TITLE} characters"));
    }
    if b.body.trim().is_empty() || b.body.chars().count() > MAX_BODY {
        return Err(format!("{id}: the body must be 1–{MAX_BODY} characters"));
    }
    if let Some(l) = &b.link {
        let n = l.label.trim().chars().count();
        if n == 0 || n > MAX_LABEL {
            return Err(format!("{id}: the link label must be 1–{MAX_LABEL} characters"));
        }
        if !link_ok(&l.to) {
            return Err(format!("{id}: the link must be kb:shelf/slug, view:id[/tab] or https://…"));
        }
    }
    if let Some(o) = b.os.iter().find(|o| !OSES.contains(&o.as_str())) {
        return Err(format!("{id}: unknown os \"{o}\" (macos, windows, linux)"));
    }
    for v in [&b.min_app, &b.max_app].into_iter().flatten() {
        if crate::kbpack::semver(v).is_none() {
            return Err(format!("{id}: \"{v}\" isn't a version like 1.22.0"));
        }
    }
    if let Some(f) = b.features.iter().find(|f| !id_ok(f)) {
        return Err(format!("{id}: \"{f}\" isn't a feature id"));
    }
    let mut times = [None, None];
    for (i, t) in [&b.starts, &b.expires].into_iter().enumerate() {
        if let Some(t) = t {
            times[i] =
                Some(parse_time(t).ok_or_else(|| format!("{id}: \"{t}\" isn't YYYY-MM-DD or YYYY-MM-DDTHH:MM:SSZ"))?);
        }
    }
    if let [Some(s), Some(e)] = times {
        if e <= s {
            return Err(format!("{id}: it expires before it starts"));
        }
    }
    Ok(())
}

/// Parses and checks a feed's shape (not its signature: see [`open`]).
pub fn decode(json: &[u8]) -> Result<Feed, String> {
    if json.len() > MAX_FEED {
        return Err("the notice feed is too large".into());
    }
    let feed: Feed = serde_json::from_slice(json).map_err(|e| format!("the notice feed isn't readable: {e}"))?;
    if feed.format != FORMAT {
        return Err(format!("notice feed format {} isn't supported by this version", feed.format));
    }
    if feed.notices.len() > MAX_NOTICES {
        return Err(format!("the feed has more than {MAX_NOTICES} notices"));
    }
    let mut seen = std::collections::HashSet::new();
    for b in &feed.notices {
        check(b)?;
        if !seen.insert(b.id.as_str()) {
            return Err(format!("id \"{}\" is used twice", b.id));
        }
    }
    Ok(feed)
}

/// Verifies and checks a downloaded or stored feed.
pub fn open(pubkey_b64: &str, json: &[u8], sig_b64: &str) -> Result<Feed, String> {
    crate::kbpack::verify(pubkey_b64, json, sig_b64)?;
    decode(json)
}

/// Whether a transmission is meant for this app right now. Feature targets
/// are left to the UI, which knows what's turned on.
pub fn applies(b: &Broadcast, app: &str, os: &str, now: i64) -> bool {
    (b.os.is_empty() || b.os.iter().any(|o| o == os))
        && b.min_app.as_deref().map_or(true, |m| crate::kbpack::app_ok(m, app))
        && b.max_app.as_deref().map_or(true, |m| crate::kbpack::app_ok(app, m))
        && b.starts.as_deref().and_then(parse_time).map_or(true, |s| now >= s)
        && b.expires.as_deref().and_then(parse_time).map_or(true, |e| now < e)
}

/// The transmissions for this app, most urgent first (feed order otherwise).
pub fn current(feed: &Feed, app: &str, os: &str, now: i64) -> Vec<Broadcast> {
    let mut out: Vec<Broadcast> = feed.notices.iter().filter(|b| applies(b, app, os, now)).cloned().collect();
    out.sort_by_key(|b| std::cmp::Reverse(b.level as u8));
    out
}

/// Where the stored feed lives: `<data>/broadcasts/`.
pub fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("broadcasts")
}

/// The stored feed, or None when there's none or it no longer verifies.
pub fn load_stored(data_dir: &Path, pubkey_b64: &str) -> Option<Feed> {
    let d = dir(data_dir);
    let json = std::fs::read(d.join(FEED_FILE)).ok()?;
    let sig = std::fs::read_to_string(d.join(SIG_FILE)).ok()?;
    match open(pubkey_b64, &json, &sig) {
        Ok(f) => Some(f),
        Err(e) => {
            tracing::warn!("stored notice feed ignored: {e}");
            None
        }
    }
}

/// Saves a verified feed (temp files then rename, signature last so a
/// half-written pair never verifies).
pub fn store(data_dir: &Path, json: &[u8], sig: &str) -> std::io::Result<()> {
    let d = dir(data_dir);
    std::fs::create_dir_all(&d)?;
    let _ = std::fs::remove_file(d.join(SIG_FILE));
    let tmp = d.join(format!("{FEED_FILE}.tmp"));
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, d.join(FEED_FILE))?;
    let tmp = d.join(format!("{SIG_FILE}.tmp"));
    std::fs::write(&tmp, sig)?;
    std::fs::rename(&tmp, d.join(SIG_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Signed with a throwaway test key (not the release key) by `tauri signer sign`.
    const TEST_PUB: &str = include_str!("../fixtures/broadcast/test.pub");
    const TEST_FEED: &[u8] = include_bytes!("../fixtures/broadcast/notices.json");
    const TEST_SIG: &str = include_str!("../fixtures/broadcast/notices.json.sig");
    /// The feed CI publishes, as committed.
    const REPO_FEED: &str = include_str!("../../docs/notices/notices.json");

    fn one(extra: &str) -> String {
        format!(
            r#"{{"format":1,"version":10,"notices":[{{"id":"a-1","level":"warn","title":"T","body":"Do **this**."{extra}}}]}}"#
        )
    }

    #[test]
    fn parses_times() {
        assert_eq!(parse_time("1970-01-01"), Some(0));
        assert_eq!(parse_time("2026-11-21"), Some(1_795_219_200));
        assert_eq!(parse_time("2026-11-21T10:30:05Z"), Some(1_795_219_200 + 37_805));
        assert_eq!(parse_time("2024-02-29"), Some(1_709_164_800));
        for bad in
            ["2025-02-29", "2026-13-01", "2026-1-01", "2026-11-21T25:00:00Z", "2026-11-21 10:00:00Z", "x", "+026-11-21"]
        {
            assert_eq!(parse_time(bad), None, "{bad}");
        }
    }

    #[test]
    fn checks_links() {
        for ok in ["kb:guides/tailnet-setup", "view:fleet", "view:fleet/disks", "https://tailscale.com/kb/1028"] {
            assert!(link_ok(ok), "{ok}");
        }
        for bad in [
            "kb:guides",
            "kb:../x",
            "view:Fleet",
            "view:a/b/c",
            "http://x.y",
            "https://",
            "https://u@evil",
            "javascript:alert(1)",
            "file:///etc",
        ] {
            assert!(!link_ok(bad), "{bad}");
        }
    }

    #[test]
    fn rejects_bad_feeds() {
        assert!(decode(one("").as_bytes()).is_ok());
        for (extra, why) in [
            (r#","os":["amiga"]"#, "unknown os"),
            (r#","minApp":"soon""#, "isn't a version"),
            (r#","expires":"2026-02-30""#, "isn't YYYY"),
            (r#","starts":"2026-12-02","expires":"2026-12-01""#, "expires before"),
            (r#","link":{"label":"Go","to":"javascript:x"}"#, "the link must"),
            (r#","link":{"label":"","to":"view:fleet"}"#, "label"),
            (r#","html":"<b>x</b>""#, "unknown field"),
        ] {
            let e = decode(one(extra).as_bytes()).unwrap_err();
            assert!(e.contains(why), "{extra} → {e}");
        }
        let twice = r#"{"format":1,"version":1,"notices":[{"id":"a","level":"info","title":"T","body":"b"},{"id":"a","level":"info","title":"T","body":"b"}]}"#;
        assert!(decode(twice.as_bytes()).unwrap_err().contains("twice"));
        assert!(decode(br#"{"format":2,"version":1,"notices":[]}"#).unwrap_err().contains("format 2"));
        assert!(decode(one("").replace("a-1", "A_1").as_bytes()).unwrap_err().contains("lowercase"));
    }

    #[test]
    fn targets_os_versions_and_dates() {
        let f = decode(
            one(r#","os":["macos"],"minApp":"1.20.0","maxApp":"1.21.9","starts":"2026-11-21","expires":"2026-12-01T12:00:00Z""#)
                .as_bytes(),
        )
        .unwrap();
        let b = &f.notices[0];
        let now = parse_time("2026-11-25").unwrap();
        assert!(applies(b, "1.21.2", "macos", now));
        assert!(!applies(b, "1.21.2", "windows", now), "other os");
        assert!(!applies(b, "1.19.0", "macos", now), "too old");
        assert!(!applies(b, "1.22.0", "macos", now), "fixed in a later version");
        assert!(!applies(b, "1.21.2", "macos", parse_time("2026-11-20").unwrap()), "not started");
        assert!(!applies(b, "1.21.2", "macos", parse_time("2026-12-01T12:00:00Z").unwrap()), "expired");
    }

    #[test]
    fn sorts_most_urgent_first() {
        let json = r#"{"format":1,"version":1,"notices":[{"id":"a","level":"info","title":"A","body":"b"},{"id":"b","level":"critical","title":"B","body":"b"},{"id":"c","level":"warn","title":"C","body":"b"}]}"#;
        let ids: Vec<String> =
            current(&decode(json.as_bytes()).unwrap(), "1.0.0", "linux", 0).into_iter().map(|b| b.id).collect();
        assert_eq!(ids, ["b", "c", "a"]);
    }

    #[test]
    fn verifies_the_signed_fixture_and_rejects_tampering() {
        let f = open(TEST_PUB, TEST_FEED, TEST_SIG).expect("fixture verifies");
        assert_eq!(f.notices.len(), 1);
        let mut bad = TEST_FEED.to_vec();
        let i = bad.iter().position(|&c| c == b'T').unwrap();
        bad[i] = b'X';
        assert!(open(TEST_PUB, &bad, TEST_SIG).unwrap_err().contains("signature"));
        let release = include_str!("../../app/updater.pub");
        assert!(open(release, TEST_FEED, TEST_SIG).is_err(), "the release key doesn't accept the test signature");
    }

    #[test]
    fn stores_and_ignores_a_corrupt_copy() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load_stored(tmp.path(), TEST_PUB).is_none());
        store(tmp.path(), TEST_FEED, TEST_SIG).unwrap();
        assert!(load_stored(tmp.path(), TEST_PUB).is_some());
        std::fs::write(dir(tmp.path()).join(FEED_FILE), b"{}").unwrap();
        assert!(load_stored(tmp.path(), TEST_PUB).is_none());
    }

    /// The repository's feed passes the same checks the app applies, so a
    /// bad edit fails CI instead of being refused by every installed app.
    #[test]
    fn the_repository_feed_is_valid() {
        let src: serde_json::Value = serde_json::from_str(REPO_FEED).expect("docs/notices/notices.json is JSON");
        let feed = serde_json::json!({ "format": FORMAT, "version": 1, "notices": src["notices"] });
        decode(feed.to_string().as_bytes()).expect("docs/notices/notices.json is a valid feed");
    }
}
