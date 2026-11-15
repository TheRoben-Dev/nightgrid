//! Plex Media Server: name, version and what's playing.
//!
//! `GET /` for the server identity and `GET /status/sessions` for active
//! streams, both with `X-Plex-Token` and `Accept: application/json`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{http_error, Integration};
use crate::http::{send, Request};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PlexStatus {
    pub server: String,
    pub version: String,
    pub streams: Vec<PlexStream>,
    /// Streams being transcoded (video or audio).
    pub transcodes: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PlexStream {
    pub user: String,
    /// "Show · S1E2 · Episode", "Movie (2019)", "Artist · Track".
    pub title: String,
    /// movie, episode, track, ...
    pub media: String,
    pub player: String,
    /// playing, paused, buffering.
    pub state: String,
    /// 0–100, when duration is known.
    pub progress: Option<f32>,
    pub transcode: bool,
    /// "1080p" etc. from the media, when known.
    pub quality: Option<String>,
}

fn s(v: &Value, k: &str) -> String {
    match &v[k] {
        Value::String(x) => x.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

/// Parses `GET /` → (friendlyName, version).
pub fn parse_identity(v: &Value) -> (String, String) {
    let mc = &v["MediaContainer"];
    (s(mc, "friendlyName"), s(mc, "version"))
}

pub fn parse_sessions(v: &Value) -> Vec<PlexStream> {
    let items = v["MediaContainer"]["Metadata"].as_array().cloned().unwrap_or_default();
    items
        .iter()
        .map(|m| {
            let media = s(m, "type");
            let title = match media.as_str() {
                "episode" => {
                    let (season, ep) = (s(m, "parentIndex"), s(m, "index"));
                    let code =
                        if season.is_empty() || ep.is_empty() { String::new() } else { format!(" · S{season}E{ep}") };
                    format!("{}{code} · {}", s(m, "grandparentTitle"), s(m, "title"))
                }
                "track" => format!("{} · {}", s(m, "grandparentTitle"), s(m, "title")),
                _ => {
                    let year = s(m, "year");
                    if year.is_empty() {
                        s(m, "title")
                    } else {
                        format!("{} ({year})", s(m, "title"))
                    }
                }
            };
            let duration = m["duration"].as_f64().filter(|d| *d > 0.0);
            let offset = m["viewOffset"].as_f64();
            let progress = duration.zip(offset).map(|(d, o)| ((o / d) * 100.0).clamp(0.0, 100.0) as f32);
            let ts = &m["TranscodeSession"];
            let transcode =
                ts.is_object() && (s(ts, "videoDecision") == "transcode" || s(ts, "audioDecision") == "transcode");
            let quality = m["Media"]
                .as_array()
                .and_then(|a| a.first())
                .map(|x| s(x, "videoResolution"))
                .filter(|r| !r.is_empty())
                .map(|r| if r.chars().all(|c| c.is_ascii_digit()) { format!("{r}p") } else { r.to_uppercase() });
            PlexStream {
                user: s(&m["User"], "title"),
                title,
                media,
                player: {
                    let p = s(&m["Player"], "title");
                    if p.is_empty() {
                        s(&m["Player"], "product")
                    } else {
                        p
                    }
                },
                state: s(&m["Player"], "state"),
                progress,
                transcode,
                quality,
            }
        })
        .collect()
}

async fn get(i: &Integration, token: &str, path: &str) -> Result<Value, String> {
    let r = send(
        Request::get(format!("{}{path}", i.base_url))
            .header("Accept", "application/json")
            .header("X-Plex-Token", token)
            .header("X-Plex-Product", "Nightgrid")
            .allow_untrusted(i.allow_untrusted),
    )
    .await?;
    if !r.ok() {
        return Err(http_error("Plex", r.status));
    }
    r.json()
}

pub async fn status(i: &Integration, token: &str) -> Result<PlexStatus, String> {
    let (server, version) = parse_identity(&get(i, token, "/").await?);
    let streams = parse_sessions(&get(i, token, "/status/sessions").await?);
    let transcodes = streams.iter().filter(|s| s.transcode).count() as u32;
    Ok(PlexStatus { server, version, streams, transcodes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions() {
        let v: Value = serde_json::from_str(include_str!("fixtures/plex_sessions.json")).unwrap();
        let s = parse_sessions(&v);
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].title, "Example Show · S2E5 · The Fifth One");
        assert_eq!(
            (s[0].user.as_str(), s[0].player.as_str(), s[0].state.as_str()),
            ("alice", "Living Room TV", "playing")
        );
        assert!(s[0].transcode);
        assert_eq!(s[0].quality.as_deref(), Some("1080p"));
        assert!((s[0].progress.unwrap() - 25.0).abs() < 0.01);
        assert_eq!(s[1].title, "A Film (2019)");
        assert!(!s[1].transcode);
        assert_eq!(s[1].quality.as_deref(), Some("4K"));
        assert_eq!(
            (s[2].title.as_str(), s[2].state.as_str(), s[2].player.as_str()),
            ("Some Band · Song", "paused", "Plexamp")
        );
        assert_eq!(s[2].progress, None);

        assert!(parse_sessions(&serde_json::json!({"MediaContainer": {"size": 0}})).is_empty());
        let (name, ver) =
            parse_identity(&serde_json::json!({"MediaContainer": {"friendlyName": "media01", "version": "1.41.0"}}));
        assert_eq!((name.as_str(), ver.as_str()), ("media01", "1.41.0"));
    }
}
