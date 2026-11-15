//! Uptime Kuma: monitors from a public status page.
//!
//! Kuma's own API is a websocket for its UI; status pages are the stable,
//! read-only HTTP surface and need no API key: `GET /api/status-page/<slug>`
//! for the groups and monitors, `GET /api/status-page/heartbeat/<slug>` for
//! the latest beats and 24 h uptime.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{http_error, Integration};
use crate::http::{send, Request};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct KumaStatus {
    pub title: String,
    pub monitors: Vec<KumaMonitor>,
    pub up: u32,
    pub down: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum KumaState {
    Up,
    Down,
    Pending,
    Maintenance,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct KumaMonitor {
    pub id: u32,
    pub name: String,
    pub group: String,
    pub state: KumaState,
    pub ping_ms: Option<u32>,
    pub message: String,
    /// 0–100 over the last 24 h.
    pub uptime_24h: Option<f32>,
}

pub fn parse(page: &Value, beats: &Value) -> KumaStatus {
    let title = page["config"]["title"].as_str().unwrap_or("").to_owned();
    let mut monitors = Vec::new();
    for g in page["publicGroupList"].as_array().cloned().unwrap_or_default() {
        let group = g["name"].as_str().unwrap_or("").to_owned();
        for m in g["monitorList"].as_array().cloned().unwrap_or_default() {
            let Some(id) = m["id"].as_u64() else { continue };
            let last = beats["heartbeatList"][id.to_string()].as_array().and_then(|b| b.last().cloned());
            let state = match last.as_ref().and_then(|b| b["status"].as_i64()) {
                Some(1) => KumaState::Up,
                Some(0) => KumaState::Down,
                Some(2) => KumaState::Pending,
                Some(3) => KumaState::Maintenance,
                _ => KumaState::Unknown,
            };
            monitors.push(KumaMonitor {
                id: id as u32,
                name: m["name"].as_str().unwrap_or("").to_owned(),
                group: group.clone(),
                state,
                ping_ms: last.as_ref().and_then(|b| b["ping"].as_f64()).map(|p| p.round() as u32),
                message: last.as_ref().and_then(|b| b["msg"].as_str()).unwrap_or("").to_owned(),
                uptime_24h: beats["uptimeList"][format!("{id}_24")].as_f64().map(|u| (u * 100.0) as f32),
            });
        }
    }
    let up = monitors.iter().filter(|m| m.state == KumaState::Up).count() as u32;
    let down = monitors.iter().filter(|m| m.state == KumaState::Down).count() as u32;
    KumaStatus { title, monitors, up, down }
}

async fn get(i: &Integration, path: &str) -> Result<Value, String> {
    let r = send(
        Request::get(format!("{}{path}", i.base_url))
            .header("Accept", "application/json")
            .allow_untrusted(i.allow_untrusted),
    )
    .await?;
    if r.status == 404 {
        return Err(format!("no status page \"{}\" (Kuma answered 404)", i.slug));
    }
    if !r.ok() {
        return Err(http_error("Uptime Kuma", r.status));
    }
    r.json()
}

pub async fn status(i: &Integration) -> Result<KumaStatus, String> {
    let page = get(i, &format!("/api/status-page/{}", i.slug)).await?;
    let beats = get(i, &format!("/api/status-page/heartbeat/{}", i.slug)).await?;
    Ok(parse(&page, &beats))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_and_beats() {
        let page: Value = serde_json::from_str(include_str!("fixtures/kuma_page.json")).unwrap();
        let beats: Value = serde_json::from_str(include_str!("fixtures/kuma_heartbeat.json")).unwrap();
        let s = parse(&page, &beats);
        assert_eq!((s.title.as_str(), s.monitors.len(), s.up, s.down), ("Home Lab", 4, 1, 1));
        let plex = &s.monitors[0];
        assert_eq!((plex.state, plex.ping_ms, plex.group.as_str()), (KumaState::Up, Some(25), "Media"));
        assert!((plex.uptime_24h.unwrap() - 99.65).abs() < 0.01);
        assert_eq!(
            (s.monitors[1].state, s.monitors[1].message.as_str(), s.monitors[1].ping_ms),
            (KumaState::Down, "connect ECONNREFUSED", None)
        );
        assert_eq!(s.monitors[2].state, KumaState::Maintenance);
        assert_eq!((s.monitors[3].state, s.monitors[3].uptime_24h), (KumaState::Unknown, None));
    }
}
