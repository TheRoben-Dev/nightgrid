//! The shared shape for most integrations: a summary line, a few number
//! tiles, a list of rows (streams, downloads, requests, alerts, ...) and the
//! actions an app allows. One card in the UI renders every kind that reports
//! an [`AppStatus`], so a new kind is a Rust module plus a line of metadata in
//! the UI, with no new component.
//!
//! Also holds the small JSON client the per-app modules use.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{http_error, Integration};
use crate::http::{send, Method, Request, Response};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum AppHealth {
    Ok,
    Warn,
    Bad,
}

impl AppHealth {
    /// The worse of two.
    pub fn max(self, o: Self) -> Self {
        match (self, o) {
            (Self::Bad, _) | (_, Self::Bad) => Self::Bad,
            (Self::Warn, _) | (_, Self::Warn) => Self::Warn,
            _ => Self::Ok,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum AppRowState {
    /// Fine / done / up.
    Ok,
    /// Playing, downloading, running.
    Active,
    /// Paused, queued, waiting.
    Idle,
    Warn,
    Bad,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AppTile {
    pub label: String,
    pub value: String,
}

/// Something the user can do. `confirm` is the question asked first, for
/// anything that stops, removes or declines.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AppAction {
    pub id: String,
    pub label: String,
    pub confirm: Option<String>,
}

impl AppAction {
    pub fn new(id: &str, label: &str) -> Self {
        Self { id: id.into(), label: label.into(), confirm: None }
    }

    pub fn confirm(mut self, question: impl Into<String>) -> Self {
        self.confirm = Some(question.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AppRow {
    /// The app's id for it (queue id, request id, ...): the target of row actions.
    pub id: String,
    pub title: String,
    pub meta: String,
    pub state: AppRowState,
    /// 0–100.
    pub progress: Option<f32>,
    pub actions: Vec<AppAction>,
}

impl AppRow {
    pub fn new(id: impl Into<String>, title: impl Into<String>, meta: impl Into<String>, state: AppRowState) -> Self {
        Self { id: id.into(), title: title.into(), meta: meta.into(), state, progress: None, actions: Vec::new() }
    }

    pub fn progress(mut self, p: Option<f64>) -> Self {
        self.progress = p.filter(|p| p.is_finite()).map(|p| p.clamp(0.0, 100.0) as f32);
        self
    }

    pub fn action(mut self, a: AppAction) -> Self {
        self.actions.push(a);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub version: Option<String>,
    /// One line: "2 streams · 1 transcoding", "4 in queue · 12 missing".
    pub summary: String,
    pub health: AppHealth,
    pub tiles: Vec<AppTile>,
    /// What the rows are: "Now playing", "Queue", "Pending requests".
    pub rows_title: String,
    pub rows: Vec<AppRow>,
    /// Rows the app has beyond those listed.
    pub more: u32,
    /// Health messages and warnings from the app.
    pub notes: Vec<String>,
    pub actions: Vec<AppAction>,
}

impl AppStatus {
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            version: None,
            summary: summary.into(),
            health: AppHealth::Ok,
            tiles: Vec::new(),
            rows_title: String::new(),
            rows: Vec::new(),
            more: 0,
            notes: Vec::new(),
            actions: Vec::new(),
        }
    }

    pub fn tile(mut self, label: &str, value: impl ToString) -> Self {
        self.tiles.push(AppTile { label: label.into(), value: value.to_string() });
        self
    }

    pub fn warn(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self.health = self.health.max(AppHealth::Warn);
        self
    }
}

/// Most rows a card lists; the rest are counted in [`AppStatus::more`].
pub const MAX_ROWS: usize = 25;

/// Keeps at most [`MAX_ROWS`] rows and returns how many were dropped.
pub fn cap_rows(rows: &mut Vec<AppRow>) -> u32 {
    let extra = rows.len().saturating_sub(MAX_ROWS);
    rows.truncate(MAX_ROWS);
    extra as u32
}

pub fn plural(n: u64, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

/// 1536 → "1.5 KB" (1024-based, like the apps show it).
pub fn bytes(n: f64) -> String {
    const U: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut v = n.max(0.0);
    let mut u = 0;
    while v >= 1024.0 && u < U.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{v:.0} B")
    } else if v >= 100.0 {
        format!("{v:.0} {}", U[u])
    } else {
        format!("{v:.1} {}", U[u])
    }
}

/// Bytes per second → "12.3 MB/s".
pub fn rate(bps: f64) -> String {
    format!("{}/s", bytes(bps))
}

/// Seconds → "2h 05m", "4m 10s", "12s".
pub fn duration(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    match s {
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{}m {:02}s", s / 60, s % 60),
        3600..=86_399 => format!("{}h {:02}m", s / 3600, s % 3600 / 60),
        _ => format!("{}d {:02}h", s / 86_400, s % 86_400 / 3600),
    }
}

/// .NET TimeSpan text ("00:12:30", "1.02:03:04") → [`duration`] text.
pub fn duration_hms(t: &str) -> String {
    let (days, rest) = match t.split_once('.') {
        Some((d, r)) if r.contains(':') && !d.contains(':') => (d.parse::<f64>().unwrap_or(0.0), r),
        _ => (0.0, t),
    };
    let rest = rest.split('.').next().unwrap_or(rest);
    let mut secs = 0.0;
    for part in rest.split(':') {
        secs = secs * 60.0 + part.parse::<f64>().unwrap_or(0.0);
    }
    duration(days * 86_400.0 + secs)
}

/// A string field, or a number as text, or "".
pub fn s(v: &Value, k: &str) -> String {
    match &v[k] {
        Value::String(x) => x.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

/// A number field, also when the app sends it as a string.
pub fn n(v: &Value, k: &str) -> Option<f64> {
    match &v[k] {
        Value::Number(x) => x.as_f64(),
        Value::String(x) => x.trim().parse().ok(),
        _ => None,
    }
}

pub fn count(v: &Value, k: &str) -> u64 {
    n(v, k).unwrap_or(0.0).max(0.0) as u64
}

/// Ids that go into a URL path: digits, letters, `-`, `_`, `.` and `:`.
pub fn valid_target(t: &str) -> bool {
    !t.is_empty()
        && t.len() <= 128
        && t.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
}

/// A JSON client for one integration: base URL, auth headers and the app's
/// name for error messages.
pub(crate) struct Client<'a> {
    pub i: &'a Integration,
    pub app: &'static str,
    pub headers: Vec<(String, String)>,
}

impl<'a> Client<'a> {
    pub fn new(i: &'a Integration, app: &'static str) -> Self {
        Self { i, app, headers: vec![("Accept".into(), "application/json".into())] }
    }

    pub fn header(mut self, k: &str, v: impl Into<String>) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.i.base_url)
    }

    pub async fn send(&self, method: Method, path: &str, body: Option<&Value>) -> Result<Response, String> {
        let mut r = match method {
            Method::Get => Request::get(self.url(path)),
            Method::Post => Request::post(self.url(path), Vec::new()),
            Method::Put => Request::put(self.url(path), Vec::new()),
            Method::Delete => Request::delete(self.url(path)),
        };
        for (k, v) in &self.headers {
            r = r.header(k.clone(), v.clone());
        }
        if let Some(b) = body {
            r = r.json(b);
        }
        send(r.allow_untrusted(self.i.allow_untrusted)).await
    }

    fn parse(&self, r: &Response) -> Result<Value, String> {
        if !r.ok() {
            return Err(http_error(self.app, r.status));
        }
        if r.body.iter().all(u8::is_ascii_whitespace) {
            return Ok(Value::Null);
        }
        r.json().map_err(|_| format!("{} sent something that isn't JSON: check the URL", self.app))
    }

    pub async fn get(&self, path: &str) -> Result<Value, String> {
        let r = self.send(Method::Get, path, None).await?;
        self.parse(&r)
    }

    /// `None` when the app answers 404 (an older version without the endpoint).
    pub async fn get_opt(&self, path: &str) -> Result<Option<Value>, String> {
        let r = self.send(Method::Get, path, None).await?;
        if r.status == 404 {
            return Ok(None);
        }
        self.parse(&r).map(Some)
    }

    pub async fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
        let r = self.send(Method::Post, path, Some(body)).await?;
        self.parse(&r)
    }

    pub async fn post_empty(&self, path: &str) -> Result<Value, String> {
        let r = self.send(Method::Post, path, None).await?;
        self.parse(&r)
    }

    pub async fn delete(&self, path: &str) -> Result<Value, String> {
        let r = self.send(Method::Delete, path, None).await?;
        self.parse(&r)
    }
}

/// Percent-encodes a query value.
pub fn enc(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for b in v.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod testing {
    //! A tiny HTTP server for integration tests: answers each request from a
    //! route table and records what it saw.

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use crate::integrations::{Integration, IntegrationKind};

    pub struct Seen {
        pub path: String,
        pub head: String,
        pub body: String,
    }

    /// Starts a server on 127.0.0.1. `route(method, path_with_query, head, body)`
    /// returns (status, extra headers, body). Returns the base URL and a
    /// handle yielding everything seen once `n` requests were served.
    pub async fn serve<F>(n: usize, route: F) -> (String, tokio::task::JoinHandle<Vec<Seen>>)
    where
        F: Fn(&str, &str, &str, &str) -> (u16, Vec<(String, String)>, String) + Send + 'static,
    {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let h = tokio::spawn(async move {
            let mut seen = Vec::new();
            for _ in 0..n {
                let (mut s, _) = l.accept().await.unwrap();
                let mut buf = vec![0u8; 16384];
                let mut got = Vec::new();
                let (head, body) = loop {
                    let k = s.read(&mut buf).await.unwrap();
                    got.extend_from_slice(&buf[..k]);
                    let t = String::from_utf8_lossy(&got).to_string();
                    if let Some(end) = t.find("\r\n\r\n") {
                        let len: usize = t
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .map(|v| v.trim().parse().unwrap())
                            })
                            .unwrap_or(0);
                        if got.len() >= end + 4 + len {
                            break (t[..end].to_owned(), t[end + 4..].to_owned());
                        }
                    }
                    if k == 0 {
                        break (t, String::new());
                    }
                };
                let mut w = head.split_whitespace();
                let (method, path) = (w.next().unwrap_or("").to_owned(), w.next().unwrap_or("").to_owned());
                let (code, extra, rbody) = route(&method, &path, &head, &body);
                let mut resp = format!(
                    "HTTP/1.1 {code} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\n",
                    rbody.len()
                );
                for (k, v) in extra {
                    resp.push_str(&format!("{k}: {v}\r\n"));
                }
                resp.push_str("\r\n");
                resp.push_str(&rbody);
                s.write_all(resp.as_bytes()).await.unwrap();
                let _ = s.shutdown().await;
                seen.push(Seen { path, head, body });
            }
            seen
        });
        (format!("http://127.0.0.1:{port}"), h)
    }

    pub fn integration(kind: IntegrationKind, base_url: &str) -> Integration {
        Integration {
            id: 1,
            kind,
            name: kind.as_str().into(),
            base_url: base_url.into(),
            username: String::new(),
            slug: String::new(),
            endpoint_id: 0,
            host_id: None,
            allow_untrusted: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting() {
        assert_eq!(bytes(0.0), "0 B");
        assert_eq!(bytes(1536.0), "1.5 KB");
        assert_eq!(bytes(5.0 * 1024.0 * 1024.0 * 1024.0), "5.0 GB");
        assert_eq!(bytes(250.0 * 1024.0 * 1024.0), "250 MB");
        assert_eq!(duration(42.0), "42s");
        assert_eq!(duration(250.0), "4m 10s");
        assert_eq!(duration(7500.0), "2h 05m");
        assert_eq!(duration(90_000.0), "1d 01h");
        assert_eq!(duration_hms("00:12:30"), "12m 30s");
        assert_eq!(duration_hms("1.02:03:04"), "1d 02h");
        assert_eq!(duration_hms("00:00:05.1234"), "5s");
        assert_eq!(plural(1, "stream"), "1 stream");
        assert_eq!(plural(3, "stream"), "3 streams");
        assert_eq!(enc("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
        assert!(valid_target("123") && valid_target("abc-DEF_1.2:3"));
        assert!(!valid_target("") && !valid_target("../x") && !valid_target("a b") && !valid_target("a?b"));
        assert_eq!(AppHealth::Ok.max(AppHealth::Warn), AppHealth::Warn);
        assert_eq!(AppHealth::Bad.max(AppHealth::Warn), AppHealth::Bad);
        let v = serde_json::json!({"a": "12.5", "b": 3, "c": "x"});
        assert_eq!((n(&v, "a"), n(&v, "b"), n(&v, "c"), count(&v, "zz")), (Some(12.5), Some(3.0), None, 0));
        let mut rows: Vec<AppRow> = (0..30).map(|i| AppRow::new(i.to_string(), "t", "m", AppRowState::Ok)).collect();
        assert_eq!((cap_rows(&mut rows), rows.len()), (5, MAX_ROWS));
        assert_eq!(AppRow::new("1", "t", "", AppRowState::Ok).progress(Some(140.0)).progress, Some(100.0));
    }
}
