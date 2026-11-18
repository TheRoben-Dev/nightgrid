//! Local notifications (1.1): one place every part of Nightgrid sends a
//! notice through. Each notice lands in the in-app inbox (the bell), and,
//! when its source and level are routed there, as a desktop notification
//! and on a self-hosted ntfy server. Nothing goes to an outside service:
//! ntfy is off by default, and the public ntfy.sh is refused (D-167).
//!
//! Alerts keep their own rules and folding (`alerts.rs`); this module only
//! decides where a finished notice goes and keeps the inbox.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

/// The inbox keeps notices this long …
pub const KEEP_SECS: i64 = 30 * 86_400;
/// … and at most this many.
pub const KEEP_ROWS: i64 = 1000;
const TITLE_MAX: usize = 160;
const BODY_MAX: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum NoticeSource {
    /// An alert fired or cleared.
    Alert,
    /// A restore drill passed or failed.
    Drill,
    /// A secret or certificate is due for rotation.
    Rotation,
    /// A host's security posture score dropped.
    Posture,
    /// A host is running out of room (capacity planner).
    Capacity,
    /// Tests and Nightgrid's own messages.
    System,
}

impl NoticeSource {
    pub const ALL: [NoticeSource; 6] = [
        NoticeSource::Alert,
        NoticeSource::Drill,
        NoticeSource::Rotation,
        NoticeSource::Posture,
        NoticeSource::Capacity,
        NoticeSource::System,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            NoticeSource::Alert => "alert",
            NoticeSource::Drill => "drill",
            NoticeSource::Rotation => "rotation",
            NoticeSource::Posture => "posture",
            NoticeSource::Capacity => "capacity",
            NoticeSource::System => "system",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|x| x.as_str() == s)
    }

    pub fn label(self) -> &'static str {
        match self {
            NoticeSource::Alert => "Alert",
            NoticeSource::Drill => "Restore drill",
            NoticeSource::Rotation => "Rotation",
            NoticeSource::Posture => "Security posture",
            NoticeSource::Capacity => "Capacity",
            NoticeSource::System => "Nightgrid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum NoticeLevel {
    Info,
    /// Something went back to normal or passed.
    Ok,
    Warn,
    Critical,
}

impl NoticeLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            NoticeLevel::Info => "info",
            NoticeLevel::Ok => "ok",
            NoticeLevel::Warn => "warn",
            NoticeLevel::Critical => "critical",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "ok" => NoticeLevel::Ok,
            "warn" => NoticeLevel::Warn,
            "critical" => NoticeLevel::Critical,
            _ => NoticeLevel::Info,
        }
    }

    /// Ranks for the "send from this level up" choice: ok and info are the
    /// quiet end, then warn, then critical.
    fn rank(self) -> u8 {
        match self {
            NoticeLevel::Info | NoticeLevel::Ok => 0,
            NoticeLevel::Warn => 1,
            NoticeLevel::Critical => 2,
        }
    }
}

/// A notice to send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NewNotice {
    pub source: NoticeSource,
    pub level: NoticeLevel,
    pub title: String,
    pub body: String,
    pub host_id: Option<HostId>,
    /// Where clicking it in the inbox goes, as `view` or `view/tab`
    /// (`fleet/drills`, `security/posture`).
    pub view: Option<String>,
}

impl NewNotice {
    pub fn new(source: NoticeSource, level: NoticeLevel, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self { source, level, title: title.into(), body: body.into(), host_id: None, view: None }
    }

    pub fn host(mut self, id: HostId) -> Self {
        self.host_id = Some(id);
        self
    }

    pub fn view(mut self, v: &str) -> Self {
        self.view = Some(v.to_owned());
        self
    }
}

/// A notice in the inbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub id: i64,
    /// Unix seconds.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub at: i64,
    pub source: NoticeSource,
    pub level: NoticeLevel,
    pub title: String,
    pub body: String,
    pub host_id: Option<HostId>,
    pub view: Option<String>,
    pub read: bool,
}

/// Where notices go. Desktop notifications for alerts follow
/// `alerts.native`; everything else is routed here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct NotifySettings {
    /// Desktop notifications for the sources that aren't alerts.
    pub native: bool,
    /// Send to a self-hosted ntfy server (its URL lives in the keychain).
    pub ntfy: bool,
    /// Lowest level sent to ntfy.
    pub ntfy_level: NoticeLevel,
    /// Also send "resolved" and "passed" notices to ntfy.
    pub ntfy_ok: bool,
    /// Routes turned off, as `native:<source>` or `ntfy:<source>`.
    pub muted: Vec<String>,
}

impl Default for NotifySettings {
    fn default() -> Self {
        Self { native: true, ntfy: false, ntfy_level: NoticeLevel::Warn, ntfy_ok: true, muted: Vec::new() }
    }
}

/// A delivery channel besides the inbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Native,
    Ntfy,
}

impl Channel {
    fn as_str(self) -> &'static str {
        match self {
            Channel::Native => "native",
            Channel::Ntfy => "ntfy",
        }
    }
}

impl NotifySettings {
    pub fn validate(&self) -> Result<()> {
        if self.muted.len() > 32 {
            return Err(Error::Invalid { field: "notify.muted", reason: "too many entries".into() });
        }
        for m in &self.muted {
            let ok = m
                .split_once(':')
                .is_some_and(|(c, s)| matches!(c, "native" | "ntfy") && NoticeSource::parse(s).is_some());
            if !ok {
                return Err(Error::Invalid { field: "notify.muted", reason: format!("unknown route {m:?}") });
            }
        }
        Ok(())
    }

    pub fn routed(&self, ch: Channel, source: NoticeSource) -> bool {
        let key = format!("{}:{}", ch.as_str(), source.as_str());
        !self.muted.iter().any(|m| *m == key)
    }

    /// Whether a notice of this source and level goes to ntfy. `System`
    /// notices (tests) always do when ntfy is on.
    pub fn to_ntfy(&self, source: NoticeSource, level: NoticeLevel) -> bool {
        if !self.ntfy || !self.routed(Channel::Ntfy, source) {
            return false;
        }
        if source == NoticeSource::System {
            return true;
        }
        if level == NoticeLevel::Ok {
            return self.ntfy_ok;
        }
        level.rank() >= self.ntfy_level.rank()
    }

    /// Whether a non-alert notice shows as a desktop notification. Info
    /// notices stay in the inbox.
    pub fn to_native(&self, source: NoticeSource, level: NoticeLevel) -> bool {
        self.native && self.routed(Channel::Native, source) && level != NoticeLevel::Info
    }
}

// ---------------------------------------------------------------- ntfy

/// Hosts of the public ntfy service, which is an outside service.
const PUBLIC_NTFY: [&str; 2] = ["ntfy.sh", "ntfy.net"];

/// The host part of an http(s) URL, lowercased.
pub fn url_host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?;
    let host = if let Some(v6) = host.strip_prefix('[') { v6.split(']').next()? } else { host.split(':').next()? };
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// Whether the URL points at the public ntfy service.
pub fn is_public_ntfy(url: &str) -> bool {
    url_host(url).is_some_and(|h| PUBLIC_NTFY.iter().any(|p| h == *p || h.ends_with(&format!(".{p}"))))
}

/// Checks an ntfy topic URL: `http(s)://server[:port]/topic`, on your own
/// server.
pub fn check_ntfy_url(url: &str) -> std::result::Result<(), String> {
    let url = url.trim();
    if url_host(url).is_none() {
        return Err("the URL must start with https:// or http:// and name your ntfy server".into());
    }
    if is_public_ntfy(url) {
        return Err(
            "ntfy.sh is a public service run by someone else. Point this at your own ntfy server (it runs fine as a container on one of your hosts)."
                .into(),
        );
    }
    let rest = url.split_once("://").map_or("", |(_, r)| r);
    let path = rest.split_once('/').map_or("", |(_, p)| p);
    let topic = path.split(['?', '#']).next().unwrap_or("").trim_end_matches('/');
    if topic.is_empty() {
        return Err("add the topic to the URL, like https://ntfy.home.lan/nightgrid".into());
    }
    if !topic.chars().all(|c| c.is_ascii_alphanumeric() || "-_/".contains(c)) || topic.len() > 64 {
        return Err("a topic is letters, digits, - and _ (up to 64)".into());
    }
    Ok(())
}

/// A header value: plain when it's printable ASCII, otherwise an RFC 2047
/// encoded word (ntfy decodes those), never with control characters.
fn header_safe(s: &str) -> String {
    let s: String = s.chars().filter(|c| !c.is_control()).collect();
    if s.is_ascii() {
        return s;
    }
    use base64::Engine as _;
    format!("=?UTF-8?B?{}?=", base64::engine::general_purpose::STANDARD.encode(s.as_bytes()))
}

/// The POST that publishes a notice to an ntfy topic. `token` is an ntfy
/// access token (`tk_…`) for servers that need one.
pub fn ntfy_request(url: &str, token: Option<&str>, n: &NewNotice) -> crate::http::Request {
    use crate::http::Request;
    let (priority, tag) = match n.level {
        NoticeLevel::Critical => ("high", "rotating_light"),
        NoticeLevel::Warn => ("default", "warning"),
        NoticeLevel::Ok => ("low", "white_check_mark"),
        NoticeLevel::Info => ("low", "information_source"),
    };
    let title = format!("Nightgrid · {}", n.title);
    let mut r = Request::post(url.trim(), n.body.clone())
        .header("Title", header_safe(&tidy(&title, TITLE_MAX)))
        .header("Priority", priority)
        .header("Tags", format!("{tag},{}", n.source.as_str()));
    if let Some(t) = token.map(str::trim).filter(|t| !t.is_empty()) {
        r = r.header("Authorization", format!("Bearer {}", header_safe(t)));
    }
    r
}

// --------------------------------------------------------------- inbox

fn tidy(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut t: String = s.chars().take(max).collect();
    t.push('…');
    t
}

fn valid_view(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 48
        && v.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "/-".contains(c))
}

impl Store {
    /// Adds a notice to the inbox and trims it. Returns the stored notice.
    pub fn notice_add(&self, n: &NewNotice, at: i64) -> Result<Notice> {
        let title = tidy(&n.title, TITLE_MAX);
        let body = tidy(&n.body, BODY_MAX);
        let view = n.view.clone().filter(|v| valid_view(v));
        let c = self.conn();
        c.execute(
            "INSERT INTO notices (at, source, level, title, body, host_id, view) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![at, n.source.as_str(), n.level.as_str(), title, body, n.host_id, view],
        )?;
        let id = c.last_insert_rowid();
        c.execute(
            "DELETE FROM notices WHERE at < ?1 OR id NOT IN (SELECT id FROM notices ORDER BY id DESC LIMIT ?2)",
            params![at - KEEP_SECS, KEEP_ROWS],
        )?;
        Ok(Notice { id, at, source: n.source, level: n.level, title, body, host_id: n.host_id, view, read: false })
    }

    /// Newest first.
    pub fn notices(&self, limit: u32) -> Result<Vec<Notice>> {
        let mut st = self.conn().prepare(
            "SELECT id, at, source, level, title, body, host_id, view, read FROM notices ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = st.query_map([limit.min(KEEP_ROWS as u32)], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<HostId>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, bool>(8)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, at, source, level, title, body, host_id, view, read) = row?;
            let Some(source) = NoticeSource::parse(&source) else { continue };
            out.push(Notice { id, at, source, level: NoticeLevel::parse(&level), title, body, host_id, view, read });
        }
        Ok(out)
    }

    pub fn notices_unread(&self) -> Result<u32> {
        Ok(self.conn().query_row("SELECT COUNT(*) FROM notices WHERE read = 0", [], |r| r.get(0))?)
    }

    /// Marks these notices read, or all of them with `None`.
    pub fn notices_mark_read(&self, ids: Option<&[i64]>) -> Result<()> {
        match ids {
            None => {
                self.conn().execute("UPDATE notices SET read = 1 WHERE read = 0", [])?;
            }
            Some(ids) => {
                let mut st = self.conn().prepare("UPDATE notices SET read = 1 WHERE id = ?1")?;
                for id in ids {
                    st.execute([id])?;
                }
            }
        }
        Ok(())
    }

    pub fn notices_clear(&self) -> Result<()> {
        self.conn().execute("DELETE FROM notices", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_public_ntfy_and_needs_a_topic() {
        assert!(check_ntfy_url("https://ntfy.sh/mytopic").is_err());
        assert!(check_ntfy_url("https://NTFY.SH/mytopic").is_err());
        assert!(check_ntfy_url("https://eu.ntfy.sh/mytopic").is_err());
        assert!(check_ntfy_url("https://notntfy.sh/mytopic").is_ok());
        assert!(check_ntfy_url("http://10.0.0.5:8080/alerts").is_ok());
        assert!(check_ntfy_url("https://ntfy.home.lan/").is_err());
        assert!(check_ntfy_url("https://ntfy.home.lan").is_err());
        assert!(check_ntfy_url("ftp://ntfy.home.lan/x").is_err());
        assert!(check_ntfy_url("https://ntfy.home.lan/a b").is_err());
        assert!(check_ntfy_url("https://user:pw@ntfy.sh/x").is_err());
        assert_eq!(url_host("http://[fd00::1]:80/t").as_deref(), Some("fd00::1"));
    }

    #[test]
    fn routes_by_source_and_level() {
        let mut s = NotifySettings::default();
        assert!(!s.to_ntfy(NoticeSource::Drill, NoticeLevel::Critical), "ntfy is off by default");
        s.ntfy = true;
        assert!(s.to_ntfy(NoticeSource::Drill, NoticeLevel::Warn));
        assert!(!s.to_ntfy(NoticeSource::Drill, NoticeLevel::Info));
        assert!(s.to_ntfy(NoticeSource::Drill, NoticeLevel::Ok));
        s.ntfy_ok = false;
        assert!(!s.to_ntfy(NoticeSource::Drill, NoticeLevel::Ok));
        s.ntfy_level = NoticeLevel::Critical;
        assert!(!s.to_ntfy(NoticeSource::Drill, NoticeLevel::Warn));
        assert!(s.to_ntfy(NoticeSource::System, NoticeLevel::Info));
        s.muted.push("ntfy:posture".into());
        assert!(!s.to_ntfy(NoticeSource::Posture, NoticeLevel::Critical));
        assert!(s.to_native(NoticeSource::Posture, NoticeLevel::Warn));
        assert!(!s.to_native(NoticeSource::Posture, NoticeLevel::Info));
        s.muted.push("native:posture".into());
        assert!(!s.to_native(NoticeSource::Posture, NoticeLevel::Warn));
        assert!(s.validate().is_ok());
        s.muted.push("email:drill".into());
        assert!(s.validate().is_err());
    }

    #[test]
    fn ntfy_request_headers() {
        let n = NewNotice::new(
            NoticeSource::Drill,
            NoticeLevel::Critical,
            "Drill failed · server01",
            "restic: no snapshot",
        );
        let r = ntfy_request("https://ntfy.home.lan/ng", Some("tk_abc"), &n);
        let h = |k: &str| r.headers.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
        assert_eq!(h("Priority").as_deref(), Some("high"));
        assert_eq!(h("Authorization").as_deref(), Some("Bearer tk_abc"));
        assert_eq!(h("Tags").as_deref(), Some("rotating_light,drill"));
        let title = h("Title").unwrap();
        assert!(title.starts_with("=?UTF-8?B?") && title.ends_with("?="));
        use base64::Engine as _;
        let raw = base64::engine::general_purpose::STANDARD.decode(&title[10..title.len() - 2]).unwrap();
        assert_eq!(String::from_utf8(raw).unwrap(), "Nightgrid · Drill failed · server01");
        let plain =
            ntfy_request("https://x/t", None, &NewNotice::new(NoticeSource::System, NoticeLevel::Info, "Test", "b"));
        assert!(plain.headers.iter().any(|(k, v)| k == "Title" && v.starts_with("=?UTF-8?B?")));
        assert_eq!(header_safe("plain\nline"), "plainline");
        assert_eq!(r.body, b"restic: no snapshot");
        assert!(ntfy_request("https://x/t", Some(" "), &n).headers.iter().all(|(k, _)| k != "Authorization"));
    }

    #[test]
    fn inbox_round_trip_and_trim() {
        let s = Store::open_in_memory().unwrap();
        let a = s
            .notice_add(
                &NewNotice::new(NoticeSource::Posture, NoticeLevel::Warn, "Score fell", "82 → 61")
                    .view("security/posture"),
                1000,
            )
            .unwrap();
        let b = s
            .notice_add(
                &NewNotice::new(NoticeSource::Drill, NoticeLevel::Ok, "Drill passed", "").view("BAD VIEW"),
                2000,
            )
            .unwrap();
        assert_eq!(b.view, None);
        assert_eq!(s.notices_unread().unwrap(), 2);
        s.notices_mark_read(Some(&[a.id])).unwrap();
        let list = s.notices(10).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, b.id);
        assert!(list[1].read);
        assert_eq!(s.notices_unread().unwrap(), 1);
        // Older than 30 days is dropped on the next add.
        s.notice_add(&NewNotice::new(NoticeSource::System, NoticeLevel::Info, "t", ""), 1000 + KEEP_SECS + 1).unwrap();
        assert_eq!(s.notices(10).unwrap().len(), 2);
        s.notices_mark_read(None).unwrap();
        assert_eq!(s.notices_unread().unwrap(), 0);
        s.notices_clear().unwrap();
        assert!(s.notices(10).unwrap().is_empty());
    }
}
