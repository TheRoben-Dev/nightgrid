//! DNS filters (M10v): Pi-hole (v6 API) and AdGuard Home. Stats, blocking
//! on/off with a timer, block and allow lists, and local DNS records.
//!
//! - **Pi-hole**: `POST /api/auth` with the (app) password for a session id,
//!   sent as `X-FTL-SID`; the session is closed with `DELETE /api/auth`
//!   after each read or change, since Pi-hole allows only a few at a time.
//! - **AdGuard Home**: HTTP basic auth on `/control/*`.
//!
//! The password comes from the keychain and goes only to the configured URL.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{http_error, Integration, IntegrationKind};
use crate::http::{send, Method, Request, Response};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DomainCount {
    pub domain: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DnsList {
    pub url: String,
    pub name: String,
    pub enabled: bool,
    /// An allow list (Pi-hole "allow", AdGuard "whitelist").
    pub allow: bool,
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub entries: Option<u64>,
    /// Unix seconds of the last download.
    pub updated: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DnsRecord {
    pub domain: String,
    /// An IP address (or, on AdGuard, another name).
    pub answer: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DnsStatus {
    pub version: Option<String>,
    pub blocking: bool,
    /// Seconds until blocking turns itself back on, when paused with a timer.
    pub resumes_in: Option<u32>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub queries: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub blocked: u64,
    pub percent_blocked: f32,
    pub clients: Option<u32>,
    /// Domains (Pi-hole) or rules (AdGuard) on the enabled block lists.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub on_lists: Option<u64>,
    pub top_blocked: Vec<DomainCount>,
    pub lists: Vec<DnsList>,
    pub records: Vec<DnsRecord>,
}

/// One change. Turning blocking off, removing a list and removing a record
/// are confirmed in the UI with the target named (rule 5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum DnsAction {
    /// `seconds` pauses blocking for that long; `None` until turned back on.
    SetBlocking {
        enabled: bool,
        seconds: Option<u32>,
    },
    AddList {
        url: String,
        name: String,
        allow: bool,
    },
    RemoveList {
        url: String,
        allow: bool,
    },
    SetListEnabled {
        url: String,
        name: String,
        allow: bool,
        enabled: bool,
    },
    /// Download the lists again (Pi-hole: gravity).
    UpdateLists,
    AddRecord {
        domain: String,
        answer: String,
    },
    RemoveRecord {
        domain: String,
        answer: String,
    },
}

impl DnsAction {
    pub fn describe(&self) -> String {
        match self {
            Self::SetBlocking { enabled: true, .. } => "turn blocking on".into(),
            Self::SetBlocking { enabled: false, seconds: Some(s) } => format!("pause blocking for {s} s"),
            Self::SetBlocking { enabled: false, seconds: None } => "turn blocking off".into(),
            Self::AddList { url, .. } => format!("add list {url}"),
            Self::RemoveList { url, .. } => format!("remove list {url}"),
            Self::SetListEnabled { url, enabled, .. } => {
                format!("{} list {url}", if *enabled { "enable" } else { "disable" })
            }
            Self::UpdateLists => "update lists".into(),
            Self::AddRecord { domain, answer } => format!("add record {domain} → {answer}"),
            Self::RemoveRecord { domain, answer } => format!("remove record {domain} → {answer}"),
        }
    }
}

// ---- validation -------------------------------------------------------------------

fn valid_list_url(u: &str) -> bool {
    (u.starts_with("https://") || u.starts_with("http://"))
        && u.len() <= 500
        && !u.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '<' | '>' | '\\'))
}

fn valid_host_name(d: &str, wildcard: bool) -> bool {
    let d = if wildcard { d.strip_prefix("*.").unwrap_or(d) } else { d };
    !d.is_empty()
        && d.len() <= 253
        && d.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                && !l.starts_with('-')
        })
}

fn is_ip(s: &str) -> bool {
    s.parse::<std::net::IpAddr>().is_ok()
}

/// Checks an action for this kind of server.
pub fn validate(kind: IntegrationKind, a: &DnsAction) -> Result<(), String> {
    let adguard = kind == IntegrationKind::Adguard;
    match a {
        DnsAction::SetBlocking { seconds: Some(s), .. } if *s == 0 || *s > 7 * 86_400 => {
            Err("pause for 1 second to 7 days".into())
        }
        DnsAction::AddList { url, name, .. } | DnsAction::SetListEnabled { url, name, .. } => {
            if !valid_list_url(url) {
                Err(format!("{url:?} isn't a list URL (http or https)"))
            } else if name.len() > 120 || name.chars().any(char::is_control) {
                Err("the name is too long".into())
            } else {
                Ok(())
            }
        }
        DnsAction::RemoveList { url, .. } if !valid_list_url(url) => Err(format!("{url:?} isn't a list URL")),
        DnsAction::AddRecord { domain, answer } | DnsAction::RemoveRecord { domain, answer } => {
            if !valid_host_name(domain, adguard) {
                Err(format!("{domain:?} isn't a domain name"))
            } else if !(is_ip(answer) || (adguard && valid_host_name(answer, false))) {
                Err(format!("{answer:?} isn't an IP address{}", if adguard { " or a domain name" } else { "" }))
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

/// Percent-encodes a path segment.
fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

// ---- Pi-hole ----------------------------------------------------------------------------

/// Pi-hole status from its API answers. `top` and `version` may be absent.
pub fn parse_pihole(
    summary: &Value,
    blocking: &Value,
    lists: &Value,
    hosts: &Value,
    top: &Value,
    version: &Value,
) -> DnsStatus {
    let q = &summary["queries"];
    let lists: Vec<DnsList> = lists["lists"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|l| DnsList {
                    url: l["address"].as_str().unwrap_or("").to_owned(),
                    name: l["comment"].as_str().unwrap_or("").to_owned(),
                    enabled: l["enabled"].as_bool().unwrap_or(false),
                    allow: l["type"].as_str() == Some("allow"),
                    entries: l["number"].as_u64(),
                    updated: l["date_updated"].as_f64().filter(|t| *t > 0.0),
                })
                .collect()
        })
        .unwrap_or_default();
    let records = hosts["config"]["dns"]["hosts"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|h| {
                    let mut w = h.as_str()?.split_whitespace();
                    let ip = w.next()?;
                    // "ip name [alias…]": one record per name.
                    Some(w.map(|n| DnsRecord { domain: n.to_owned(), answer: ip.to_owned() }).collect::<Vec<_>>())
                })
                .flatten()
                .collect()
        })
        .unwrap_or_default();
    let top_blocked = top["domains"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|d| {
                    Some(DomainCount { domain: d["domain"].as_str()?.to_owned(), count: d["count"].as_u64()? })
                })
                .collect()
        })
        .unwrap_or_default();
    let v = &version["version"];
    let version =
        v["core"]["local"]["version"].as_str().or_else(|| v["ftl"]["local"]["version"].as_str()).map(str::to_owned);
    DnsStatus {
        version,
        blocking: blocking["blocking"].as_str() != Some("disabled"),
        resumes_in: blocking["timer"].as_f64().filter(|t| *t > 0.0).map(|t| t.ceil() as u32),
        queries: q["total"].as_u64().unwrap_or(0),
        blocked: q["blocked"].as_u64().unwrap_or(0),
        percent_blocked: q["percent_blocked"].as_f64().unwrap_or(0.0) as f32,
        clients: summary["clients"]["active"].as_u64().map(|n| n as u32),
        on_lists: summary["gravity"]["domains_being_blocked"].as_u64(),
        top_blocked,
        lists,
        records,
    }
}

/// The request(s) for an action: (method, path with query, JSON body).
pub fn pihole_requests(a: &DnsAction) -> Vec<(Method, String, Option<Value>)> {
    let kind = |allow: bool| if allow { "allow" } else { "block" };
    match a {
        DnsAction::SetBlocking { enabled, seconds } => vec![(
            Method::Post,
            "/api/dns/blocking".into(),
            Some(json!({ "blocking": enabled, "timer": seconds.filter(|_| !enabled) })),
        )],
        DnsAction::AddList { url, name, allow } => vec![(
            Method::Post,
            format!("/api/lists?type={}", kind(*allow)),
            Some(json!({ "address": url, "comment": name, "enabled": true, "groups": [0] })),
        )],
        DnsAction::RemoveList { url, allow } => {
            vec![(Method::Delete, format!("/api/lists/{}?type={}", enc(url), kind(*allow)), None)]
        }
        // Groups are filled in from the list's current settings before sending.
        DnsAction::SetListEnabled { url, name, allow, enabled } => vec![(
            Method::Put,
            format!("/api/lists/{}?type={}", enc(url), kind(*allow)),
            Some(json!({ "comment": name, "enabled": enabled })),
        )],
        DnsAction::UpdateLists => vec![(Method::Post, "/api/action/gravity".into(), None)],
        DnsAction::AddRecord { domain, answer } => {
            vec![(Method::Put, format!("/api/config/dns/hosts/{}", enc(&format!("{answer} {domain}"))), None)]
        }
        DnsAction::RemoveRecord { domain, answer } => {
            vec![(Method::Delete, format!("/api/config/dns/hosts/{}", enc(&format!("{answer} {domain}"))), None)]
        }
    }
}

fn request(i: &Integration, method: Method, path: &str, body: Option<&Value>) -> Request {
    let url = format!("{}{path}", i.base_url);
    let r = match method {
        Method::Get => Request::get(url),
        Method::Post => Request::post(url, Vec::new()),
        Method::Put => Request::put(url, Vec::new()),
        Method::Delete => Request::delete(url),
    };
    let r = match body {
        Some(b) => r.json(b),
        None if matches!(method, Method::Post | Method::Put) => r.json(&json!({})),
        None => r,
    };
    r.allow_untrusted(i.allow_untrusted)
}

/// Pi-hole's error text from a failed answer, if it gave one.
fn pihole_error(r: &Response) -> String {
    let v: Value = r.json().unwrap_or(Value::Null);
    match v["error"]["message"].as_str() {
        Some(m) => {
            format!("Pi-hole: {m}{}", v["error"]["hint"].as_str().map(|h| format!(" ({h})")).unwrap_or_default())
        }
        None => http_error("Pi-hole", r.status),
    }
}

struct PiSession<'a> {
    i: &'a Integration,
    sid: Option<String>,
}

impl<'a> PiSession<'a> {
    async fn open(i: &'a Integration, password: &str) -> Result<Self, String> {
        let r = send(request(i, Method::Post, "/api/auth", Some(&json!({ "password": password })))).await?;
        if r.status == 401 {
            return Err("Pi-hole refused the password (use the web password or an app password)".into());
        }
        if !r.ok() {
            return Err(pihole_error(&r));
        }
        let v: Value = r.json()?;
        if v["session"]["valid"].as_bool() != Some(true) {
            return Err("Pi-hole refused the password".into());
        }
        Ok(Self { i, sid: v["session"]["sid"].as_str().map(str::to_owned) })
    }

    async fn call(&self, method: Method, path: &str, body: Option<&Value>, secs: u64) -> Result<Response, String> {
        let mut r = request(self.i, method, path, body).timeout(std::time::Duration::from_secs(secs));
        if let Some(s) = &self.sid {
            r = r.header("X-FTL-SID", s.clone());
        }
        send(r).await
    }

    async fn get(&self, path: &str) -> Result<Value, String> {
        let r = self.call(Method::Get, path, None, 10).await?;
        if !r.ok() {
            return Err(pihole_error(&r));
        }
        r.json()
    }

    async fn close(self) {
        if self.sid.is_some() {
            let _ = self.call(Method::Delete, "/api/auth", None, 5).await;
        }
    }
}

async fn pihole_status(i: &Integration, password: &str) -> Result<DnsStatus, String> {
    let s = PiSession::open(i, password).await?;
    let read = async {
        let summary = s.get("/api/stats/summary").await?;
        let blocking = s.get("/api/dns/blocking").await?;
        let lists = s.get("/api/lists").await?;
        let hosts = s.get("/api/config/dns/hosts").await?;
        let top = s.get("/api/stats/top_domains?blocked=true&count=10").await.unwrap_or(Value::Null);
        let version = s.get("/api/info/version").await.unwrap_or(Value::Null);
        Ok(parse_pihole(&summary, &blocking, &lists, &hosts, &top, &version))
    };
    let out = read.await;
    s.close().await;
    out
}

async fn pihole_act(i: &Integration, password: &str, a: &DnsAction) -> Result<(), String> {
    let s = PiSession::open(i, password).await?;
    let run = async {
        for (method, path, mut body) in pihole_requests(a) {
            if let (DnsAction::SetListEnabled { .. }, Some(b)) = (a, body.as_mut()) {
                // PUT replaces the list's settings: keep its groups.
                let cur = s.get(&path).await?;
                b["groups"] = cur["lists"][0]["groups"].clone();
                if b["groups"].is_null() {
                    b["groups"] = json!([0]);
                }
            }
            let secs = if matches!(a, DnsAction::UpdateLists) { 600 } else { 15 };
            let r = s.call(method, &path, body.as_ref(), secs).await?;
            if !r.ok() {
                return Err(pihole_error(&r));
            }
        }
        Ok(())
    };
    let out = run.await;
    s.close().await;
    out
}

// ---- AdGuard Home ----------------------------------------------------------------------

/// `2026-11-15T10:00:00+01:00` / `…Z` / `….123Z` → unix seconds.
pub fn parse_rfc3339(s: &str) -> Option<f64> {
    let (date, time) = s.split_once('T')?;
    let mut d = date.split('-');
    let (y, mo, da): (i64, u32, u32) = (d.next()?.parse().ok()?, d.next()?.parse().ok()?, d.next()?.parse().ok()?);
    let (clock, offset) = if let Some(c) = time.strip_suffix('Z') {
        (c, 0i64)
    } else {
        let i = time.rfind(['+', '-'])?;
        let (c, o) = time.split_at(i);
        let sign = if o.starts_with('-') { -1 } else { 1 };
        let (oh, om) = o[1..].split_once(':')?;
        (c, sign * (oh.parse::<i64>().ok()? * 3600 + om.parse::<i64>().ok()? * 60))
    };
    let mut c = clock.split(':');
    let h: i64 = c.next()?.parse().ok()?;
    let mi: i64 = c.next()?.parse().ok()?;
    let sec: f64 = c.next()?.parse().ok()?;
    let days = crate::timeline::days_from_civil(y, mo, da);
    Some((days * 86_400 + h * 3600 + mi * 60 - offset) as f64 + sec)
}

fn pairs(v: &Value) -> Vec<DomainCount> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|o| {
                    o.as_object()?
                        .iter()
                        .next()
                        .map(|(k, n)| DomainCount { domain: k.clone(), count: n.as_u64().unwrap_or(0) })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_adguard(status: &Value, stats: &Value, filtering: &Value, rewrites: &Value) -> DnsStatus {
    let list = |v: &Value, allow: bool| -> Vec<DnsList> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .map(|f| DnsList {
                        url: f["url"].as_str().unwrap_or("").to_owned(),
                        name: f["name"].as_str().unwrap_or("").to_owned(),
                        enabled: f["enabled"].as_bool().unwrap_or(false),
                        allow,
                        entries: f["rules_count"].as_u64(),
                        updated: f["last_updated"].as_str().and_then(parse_rfc3339),
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut lists = list(&filtering["filters"], false);
    let on_lists = lists.iter().filter(|l| l.enabled).filter_map(|l| l.entries).sum::<u64>();
    lists.extend(list(&filtering["whitelist_filters"], true));
    let queries = stats["num_dns_queries"].as_u64().unwrap_or(0);
    let blocked = ["num_blocked_filtering", "num_replaced_safebrowsing", "num_replaced_parental"]
        .iter()
        .filter_map(|k| stats[*k].as_u64())
        .sum::<u64>();
    let records = rewrites
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    Some(DnsRecord {
                        domain: r["domain"].as_str()?.to_owned(),
                        answer: r["answer"].as_str()?.to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let disabled_ms = status["protection_disabled_duration"].as_f64().unwrap_or(0.0);
    DnsStatus {
        version: status["version"].as_str().map(|v| v.trim_start_matches('v').to_owned()),
        blocking: status["protection_enabled"].as_bool().unwrap_or(true),
        resumes_in: (disabled_ms > 0.0).then(|| (disabled_ms / 1000.0).ceil() as u32),
        queries,
        blocked,
        percent_blocked: if queries > 0 { (blocked as f64 * 100.0 / queries as f64) as f32 } else { 0.0 },
        clients: stats["top_clients"].as_array().map(|a| a.len() as u32),
        on_lists: Some(on_lists),
        top_blocked: pairs(&stats["top_blocked_domains"]).into_iter().take(10).collect(),
        lists,
        records,
    }
}

pub fn adguard_requests(a: &DnsAction) -> Vec<(String, Value)> {
    match a {
        DnsAction::SetBlocking { enabled, seconds } => {
            let mut b = json!({ "enabled": enabled });
            if let (false, Some(s)) = (enabled, seconds) {
                b["duration"] = json!(u64::from(*s) * 1000);
            }
            vec![("/control/protection".into(), b)]
        }
        DnsAction::AddList { url, name, allow } => {
            vec![("/control/filtering/add_url".into(), json!({ "name": name, "url": url, "whitelist": allow }))]
        }
        DnsAction::RemoveList { url, allow } => {
            vec![("/control/filtering/remove_url".into(), json!({ "url": url, "whitelist": allow }))]
        }
        DnsAction::SetListEnabled { url, name, allow, enabled } => vec![(
            "/control/filtering/set_url".into(),
            json!({ "url": url, "whitelist": allow, "data": { "name": name, "url": url, "enabled": enabled } }),
        )],
        DnsAction::UpdateLists => vec![
            ("/control/filtering/refresh".into(), json!({ "whitelist": false })),
            ("/control/filtering/refresh".into(), json!({ "whitelist": true })),
        ],
        DnsAction::AddRecord { domain, answer } => {
            vec![("/control/rewrite/add".into(), json!({ "domain": domain, "answer": answer }))]
        }
        DnsAction::RemoveRecord { domain, answer } => {
            vec![("/control/rewrite/delete".into(), json!({ "domain": domain, "answer": answer }))]
        }
    }
}

fn basic(i: &Integration, password: &str) -> String {
    use base64::Engine as _;
    format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{}:{password}", i.username)))
}

async fn adguard_get(i: &Integration, password: &str, path: &str) -> Result<Value, String> {
    let r = send(request(i, Method::Get, path, None).header("authorization", basic(i, password))).await?;
    if !r.ok() {
        return Err(http_error("AdGuard Home", r.status));
    }
    r.json()
}

async fn adguard_status(i: &Integration, password: &str) -> Result<DnsStatus, String> {
    let status = adguard_get(i, password, "/control/status").await?;
    let stats = adguard_get(i, password, "/control/stats").await?;
    let filtering = adguard_get(i, password, "/control/filtering/status").await?;
    let rewrites = adguard_get(i, password, "/control/rewrite/list").await?;
    Ok(parse_adguard(&status, &stats, &filtering, &rewrites))
}

async fn adguard_act(i: &Integration, password: &str, a: &DnsAction) -> Result<(), String> {
    for (path, body) in adguard_requests(a) {
        let secs = if matches!(a, DnsAction::UpdateLists) { 300 } else { 15 };
        let r = send(
            request(i, Method::Post, &path, Some(&body))
                .header("authorization", basic(i, password))
                .timeout(std::time::Duration::from_secs(secs)),
        )
        .await?;
        if !r.ok() {
            let why = r.text();
            let why = why.trim();
            return Err(if why.is_empty() || why.len() > 200 {
                http_error("AdGuard Home", r.status)
            } else {
                format!("AdGuard Home: {why}")
            });
        }
    }
    Ok(())
}

// ---- entry points ---------------------------------------------------------------------------

pub async fn status(i: &Integration, password: &str) -> Result<DnsStatus, String> {
    match i.kind {
        IntegrationKind::Pihole => pihole_status(i, password).await,
        IntegrationKind::Adguard => adguard_status(i, password).await,
        _ => Err("not a DNS filter".into()),
    }
}

pub async fn act(i: &Integration, password: &str, a: &DnsAction) -> Result<(), String> {
    validate(i.kind, a)?;
    match i.kind {
        IntegrationKind::Pihole => pihole_act(i, password, a).await,
        IntegrationKind::Adguard => adguard_act(i, password, a).await,
        _ => Err("not a DNS filter".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn parses_pihole() {
        let s = parse_pihole(
            &fx(include_str!("fixtures/pihole_summary.json")),
            &fx(r#"{"blocking":"disabled","timer":241.5}"#),
            &fx(include_str!("fixtures/pihole_lists.json")),
            &fx(include_str!("fixtures/pihole_hosts.json")),
            &fx(include_str!("fixtures/pihole_top.json")),
            &fx(r#"{"version":{"core":{"local":{"version":"v6.1"}}}}"#),
        );
        assert_eq!((s.queries, s.blocked, s.clients, s.on_lists), (48213, 6120, Some(14), Some(167_820)));
        assert!((s.percent_blocked - 12.69).abs() < 0.01);
        assert!(!s.blocking);
        assert_eq!(s.resumes_in, Some(242));
        assert_eq!(s.version.as_deref(), Some("v6.1"));
        assert_eq!(s.lists.len(), 2);
        assert_eq!(
            (s.lists[0].name.as_str(), s.lists[0].entries, s.lists[0].allow),
            ("Steven Black", Some(160_213), false)
        );
        assert!(s.lists[1].allow && !s.lists[1].enabled && s.lists[1].updated.is_none());
        assert_eq!(s.records[0], DnsRecord { domain: "nas.lan".into(), answer: "192.168.1.10".into() });
        assert_eq!(s.records.len(), 3);
        assert_eq!(s.top_blocked[0].domain, "ads.example.net");
        let on = parse_pihole(
            &Value::Null,
            &fx(r#"{"blocking":"enabled","timer":null}"#),
            &Value::Null,
            &Value::Null,
            &Value::Null,
            &Value::Null,
        );
        assert!(on.blocking && on.resumes_in.is_none() && on.lists.is_empty());
    }

    #[test]
    fn parses_adguard() {
        let s = parse_adguard(
            &fx(include_str!("fixtures/adguard_status.json")),
            &fx(include_str!("fixtures/adguard_stats.json")),
            &fx(include_str!("fixtures/adguard_filtering.json")),
            &fx(include_str!("fixtures/adguard_rewrites.json")),
        );
        assert_eq!(s.version.as_deref(), Some("0.107.57"));
        assert!(s.blocking && s.resumes_in.is_none());
        assert_eq!((s.queries, s.blocked), (91234, 15015));
        assert_eq!(s.on_lists, Some(61234));
        assert_eq!(s.lists.len(), 3);
        assert!(s.lists[2].allow);
        assert_eq!(s.lists[0].updated, Some(1_794_733_200.0));
        assert_eq!(s.lists[1].updated, None);
        assert_eq!(s.records[1].domain, "*.apps.lan");
        assert_eq!(s.top_blocked[0], DomainCount { domain: "ads.example.net".into(), count: 1200 });
        assert_eq!(s.clients, Some(2));
        let paused = parse_adguard(
            &fx(r#"{"protection_enabled":false,"protection_disabled_duration":299500}"#),
            &Value::Null,
            &Value::Null,
            &Value::Null,
        );
        assert_eq!((paused.blocking, paused.resumes_in), (false, Some(300)));
    }

    #[test]
    fn builds_requests() {
        let r = pihole_requests(&DnsAction::SetBlocking { enabled: false, seconds: Some(300) });
        assert_eq!(r[0].1, "/api/dns/blocking");
        assert_eq!(r[0].2, Some(json!({ "blocking": false, "timer": 300 })));
        let r = pihole_requests(&DnsAction::SetBlocking { enabled: true, seconds: Some(300) });
        assert_eq!(r[0].2, Some(json!({ "blocking": true, "timer": null })));
        let r = pihole_requests(&DnsAction::RemoveList { url: "https://x.example/a b?.txt".into(), allow: true });
        assert_eq!(
            (r[0].0, r[0].1.as_str()),
            (Method::Delete, "/api/lists/https%3A%2F%2Fx.example%2Fa%20b%3F.txt?type=allow")
        );
        let r = pihole_requests(&DnsAction::AddRecord { domain: "nas.lan".into(), answer: "192.168.1.10".into() });
        assert_eq!((r[0].0, r[0].1.as_str()), (Method::Put, "/api/config/dns/hosts/192.168.1.10%20nas.lan"));
        let r = adguard_requests(&DnsAction::SetBlocking { enabled: false, seconds: Some(60) });
        assert_eq!(r[0].1, json!({ "enabled": false, "duration": 60000 }));
        assert_eq!(adguard_requests(&DnsAction::UpdateLists).len(), 2);
        let r = adguard_requests(&DnsAction::SetListEnabled {
            url: "https://a.example/l".into(),
            name: "L".into(),
            allow: false,
            enabled: false,
        });
        assert_eq!(r[0].1["data"]["enabled"], json!(false));
    }

    #[test]
    fn validates_actions() {
        use IntegrationKind::{Adguard, Pihole};
        assert!(validate(
            Pihole,
            &DnsAction::AddList { url: "https://a.example/l.txt".into(), name: "x".into(), allow: false }
        )
        .is_ok());
        assert!(validate(Pihole, &DnsAction::AddList { url: "file:///etc/x".into(), name: "x".into(), allow: false })
            .is_err());
        assert!(validate(
            Pihole,
            &DnsAction::AddList { url: "https://a.example/l\".txt".into(), name: "x".into(), allow: false }
        )
        .is_err());
        assert!(
            validate(Pihole, &DnsAction::AddRecord { domain: "nas.lan".into(), answer: "192.168.1.10".into() }).is_ok()
        );
        assert!(
            validate(Pihole, &DnsAction::AddRecord { domain: "*.lan".into(), answer: "192.168.1.10".into() }).is_err()
        );
        assert!(
            validate(Pihole, &DnsAction::AddRecord { domain: "nas.lan".into(), answer: "nas2.lan".into() }).is_err()
        );
        assert!(
            validate(Adguard, &DnsAction::AddRecord { domain: "*.apps.lan".into(), answer: "nas2.lan".into() }).is_ok()
        );
        assert!(validate(Adguard, &DnsAction::SetBlocking { enabled: false, seconds: Some(0) }).is_err());
        assert_eq!(
            DnsAction::SetBlocking { enabled: false, seconds: Some(300) }.describe(),
            "pause blocking for 300 s"
        );
    }

    #[test]
    fn rfc3339() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Some(0.0));
        assert_eq!(parse_rfc3339("2026-11-15T10:00:00Z"), Some(1_794_736_800.0));
        assert_eq!(parse_rfc3339("2026-11-15T10:00:00+01:00"), Some(1_794_733_200.0));
        assert_eq!(parse_rfc3339("2026-11-15T10:00:00.5-00:30"), Some(1_794_738_600.5));
        assert_eq!(parse_rfc3339(""), None);
    }

    /// Login → reads → logout against a fake Pi-hole.
    #[tokio::test]
    async fn against_fake_pihole() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let mut seen = Vec::new();
            for _ in 0..8 {
                let (mut s, _) = l.accept().await.unwrap();
                let mut buf = vec![0u8; 16384];
                let mut got = Vec::new();
                loop {
                    let n = s.read(&mut buf).await.unwrap();
                    got.extend_from_slice(&buf[..n]);
                    let t = String::from_utf8_lossy(&got).to_string();
                    if let Some(end) = t.find("\r\n\r\n") {
                        let len: usize = t
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase().strip_prefix("content-length: ").map(|v| v.parse().unwrap())
                            })
                            .unwrap_or(0);
                        if got.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                let t = String::from_utf8_lossy(&got).to_string();
                let mut w = t.split_whitespace();
                let (method, path) = (w.next().unwrap().to_owned(), w.next().unwrap().to_owned());
                let authed = t.to_ascii_lowercase().contains("x-ftl-sid: s1");
                let (code, body) = match (method.as_str(), path.as_str()) {
                    ("POST", "/api/auth") if t.contains("\"password\":\"pw\"") => {
                        (200, r#"{"session":{"valid":true,"sid":"s1","validity":300}}"#.to_owned())
                    }
                    ("POST", "/api/auth") => {
                        (401, r#"{"error":{"key":"unauthorized","message":"Unauthorized"}}"#.to_owned())
                    }
                    (_, _) if !authed => (401, "{}".to_owned()),
                    ("GET", "/api/stats/summary") => (200, include_str!("fixtures/pihole_summary.json").to_owned()),
                    ("GET", "/api/dns/blocking") => (200, r#"{"blocking":"enabled","timer":null}"#.to_owned()),
                    ("GET", "/api/lists") => (200, include_str!("fixtures/pihole_lists.json").to_owned()),
                    ("GET", "/api/config/dns/hosts") => (200, include_str!("fixtures/pihole_hosts.json").to_owned()),
                    ("GET", p) if p.starts_with("/api/stats/top_domains") => {
                        (200, include_str!("fixtures/pihole_top.json").to_owned())
                    }
                    ("DELETE", "/api/auth") => (204, String::new()),
                    _ => (404, r#"{"error":{"key":"not_found","message":"Not found"}}"#.to_owned()),
                };
                let resp = format!(
                    "HTTP/1.1 {code} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
                    body.len()
                );
                s.write_all(resp.as_bytes()).await.unwrap();
                let _ = s.shutdown().await;
                seen.push(format!("{method} {}", path.split('?').next().unwrap()));
            }
            seen
        });
        let i = Integration {
            id: 1,
            kind: IntegrationKind::Pihole,
            name: "Pi-hole".into(),
            base_url: format!("http://127.0.0.1:{port}"),
            username: String::new(),
            slug: String::new(),
            endpoint_id: 0,
            host_id: None,
            allow_untrusted: false,
        };
        let st = status(&i, "pw").await.unwrap();
        assert_eq!(st.queries, 48213);
        assert!(st.version.is_none(), "version endpoint answered 404");
        assert_eq!(
            server.await.unwrap(),
            [
                "POST /api/auth",
                "GET /api/stats/summary",
                "GET /api/dns/blocking",
                "GET /api/lists",
                "GET /api/config/dns/hosts",
                "GET /api/stats/top_domains",
                "GET /api/info/version",
                "DELETE /api/auth"
            ]
        );
    }
}
