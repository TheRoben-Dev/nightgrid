//! Jellyfin and Emby: now playing, transcodes, library counts and a library
//! scan. Both speak the same API (Jellyfin forked Emby); Emby serves it under
//! `/emby`. Auth is an API key from the server's dashboard (Jellyfin:
//! Dashboard → API Keys; Emby: Settings → API Keys).

use serde_json::{json, Value};

use super::app::{cap_rows, count, n, plural, s, AppAction, AppRow, AppRowState, AppStatus, Client};
use super::{Integration, IntegrationKind};

fn client<'a>(i: &'a Integration, key: &str) -> (Client<'a>, &'static str) {
    if i.kind == IntegrationKind::Emby {
        let prefix = if i.base_url.to_ascii_lowercase().ends_with("/emby") { "" } else { "/emby" };
        (Client::new(i, "Emby").header("X-Emby-Token", key), prefix)
    } else {
        let auth = format!(
            r#"MediaBrowser Client="Nightgrid", Device="Nightgrid", DeviceId="nightgrid", Version="1", Token="{key}""#
        );
        (Client::new(i, "Jellyfin").header("Authorization", auth), "")
    }
}

/// "Show · S1E2 · Episode", "Film (2019)", "Artist · Track".
fn title(item: &Value) -> String {
    let name = s(item, "Name");
    match s(item, "Type").as_str() {
        "Episode" => {
            let (season, ep) = (s(item, "ParentIndexNumber"), s(item, "IndexNumber"));
            let code = if season.is_empty() || ep.is_empty() { String::new() } else { format!(" · S{season}E{ep}") };
            format!("{}{code} · {name}", s(item, "SeriesName"))
        }
        "Audio" => {
            let artist = item["Artists"].as_array().and_then(|a| a.first()).and_then(Value::as_str).map(str::to_owned);
            let artist = artist.unwrap_or_else(|| s(item, "AlbumArtist"));
            if artist.is_empty() {
                name
            } else {
                format!("{artist} · {name}")
            }
        }
        _ => match s(item, "ProductionYear") {
            y if y.is_empty() => name,
            y => format!("{name} ({y})"),
        },
    }
}

pub fn parse_sessions(v: &Value) -> Vec<AppRow> {
    v.as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|x| x["NowPlayingItem"].is_object())
        .map(|x| {
            let item = &x["NowPlayingItem"];
            let ps = &x["PlayState"];
            let transcode = s(ps, "PlayMethod") == "Transcode";
            let mut meta = vec![s(x, "UserName"), {
                let d = s(x, "DeviceName");
                if d.is_empty() {
                    s(x, "Client")
                } else {
                    d
                }
            }];
            meta.push(if transcode { "TRANSCODE".into() } else { s(ps, "PlayMethod") });
            meta.retain(|m| !m.is_empty());
            let paused = ps["IsPaused"].as_bool().unwrap_or(false);
            let progress =
                n(ps, "PositionTicks").zip(n(item, "RunTimeTicks").filter(|r| *r > 0.0)).map(|(p, r)| p / r * 100.0);
            AppRow::new(
                s(x, "Id"),
                title(item),
                meta.join(" · "),
                if paused { AppRowState::Idle } else { AppRowState::Active },
            )
            .progress(progress)
        })
        .collect()
}

pub fn build(info: &Value, sessions: &Value, counts: Option<&Value>) -> AppStatus {
    let mut rows = parse_sessions(sessions);
    let transcodes = rows.iter().filter(|r| r.meta.contains("TRANSCODE")).count() as u64;
    let streams = rows.len() as u64;
    let summary = if streams == 0 {
        "Nothing playing".to_owned()
    } else if transcodes > 0 {
        format!("{} · {transcodes} transcoding", plural(streams, "stream"))
    } else {
        plural(streams, "stream")
    };
    let more = cap_rows(&mut rows);
    let mut st = AppStatus::new(summary).tile("Streams", streams).tile("Transcoding", transcodes);
    if let Some(c) = counts {
        for (k, label) in [("MovieCount", "Movies"), ("SeriesCount", "Series"), ("SongCount", "Songs")] {
            if count(c, k) > 0 {
                st = st.tile(label, count(c, k));
            }
        }
    }
    let server = s(info, "ServerName");
    st.version = Some(s(info, "Version")).filter(|v| !v.is_empty());
    if info["HasPendingRestart"].as_bool() == Some(true) {
        st = st.warn(format!(
            "{} needs a restart to finish an update or plugin change",
            if server.is_empty() { "The server" } else { &server }
        ));
    }
    if info["HasUpdateAvailable"].as_bool() == Some(true) {
        st = st.warn("An update is available");
    }
    st.rows_title = "Now playing".into();
    st.rows = rows;
    st.more = more;
    st.actions = vec![AppAction::new("scan", "Scan libraries")];
    st
}

pub async fn status(i: &Integration, key: &str) -> Result<AppStatus, String> {
    let (c, p) = client(i, key);
    let info = c.get(&format!("{p}/System/Info")).await?;
    let sessions = c.get(&format!("{p}/Sessions?activeWithinSeconds=960")).await?;
    let counts = c.get_opt(&format!("{p}/Items/Counts")).await.ok().flatten();
    Ok(build(&info, &sessions, counts.as_ref()))
}

pub async fn act(i: &Integration, key: &str, action: &str) -> Result<(), String> {
    let (c, p) = client(i, key);
    match action {
        "scan" => c.post(&format!("{p}/Library/Refresh"), &json!({})).await.map(drop),
        _ => Err(format!("unknown action {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::app::testing::{integration, serve};

    #[test]
    fn sessions() {
        let v: Value = serde_json::from_str(include_str!("fixtures/jellyfin_sessions.json")).unwrap();
        let info = json!({"ServerName": "media01", "Version": "10.10.7", "HasPendingRestart": true});
        let counts = json!({"MovieCount": 412, "SeriesCount": 37, "SongCount": 0});
        let st = build(&info, &v, Some(&counts));
        assert_eq!(st.summary, "2 streams · 1 transcoding");
        assert_eq!(st.version.as_deref(), Some("10.10.7"));
        assert_eq!(st.rows[0].title, "Example Show · S2E5 · The Fifth One");
        assert_eq!(st.rows[0].meta, "alice · Living Room TV · TRANSCODE");
        assert_eq!(st.rows[0].state, AppRowState::Active);
        assert!((st.rows[0].progress.unwrap() - 25.0).abs() < 0.01);
        assert_eq!(st.rows[1].title, "A Film (2019)");
        assert_eq!(st.rows[1].state, AppRowState::Idle);
        assert_eq!(st.rows[1].meta, "bob · Web · DirectPlay");
        let labels: Vec<_> = st.tiles.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["Streams", "Transcoding", "Movies", "Series"]);
        assert_eq!(st.health, super::super::app::AppHealth::Warn);
        let idle = build(&json!({}), &json!([]), None);
        assert_eq!(idle.summary, "Nothing playing");
    }

    #[tokio::test]
    async fn emby_prefix_and_auth() {
        let (url, seen) = serve(3, |_, path, _, _| match path {
            "/emby/System/Info" => (200, vec![], r#"{"ServerName":"e","Version":"4.8"}"#.into()),
            p if p.starts_with("/emby/Sessions") => (200, vec![], "[]".into()),
            _ => (404, vec![], String::new()),
        })
        .await;
        let st = status(&integration(IntegrationKind::Emby, &url), "k1").await.unwrap();
        assert_eq!(st.version.as_deref(), Some("4.8"));
        let seen = seen.await.unwrap();
        assert!(seen.iter().all(|r| r.head.to_ascii_lowercase().contains("x-emby-token: k1")));
        assert_eq!(seen[2].path, "/emby/Items/Counts");
    }
}
