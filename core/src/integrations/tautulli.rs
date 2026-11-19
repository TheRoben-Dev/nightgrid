//! Tautulli: Plex activity with bandwidth, from `GET /api/v2?cmd=...` with
//! the API key from Settings → Web Interface.

use serde_json::Value;

use super::app::{cap_rows, count, enc, n, plural, s, AppRow, AppRowState, AppStatus, Client};
use super::Integration;

async fn call(i: &Integration, key: &str, cmd: &str) -> Result<Value, String> {
    let v = Client::new(i, "Tautulli").get(&format!("/api/v2?apikey={}&cmd={cmd}", enc(key))).await?;
    let r = &v["response"];
    if s(r, "result") != "success" {
        let msg = s(r, "message");
        return Err(if msg.is_empty() { "Tautulli refused the request".into() } else { format!("Tautulli: {msg}") });
    }
    Ok(r["data"].clone())
}

/// kbps → "12.3 Mbps".
fn kbps(v: f64) -> String {
    if v >= 1000.0 {
        format!("{:.1} Mbps", v / 1000.0)
    } else {
        format!("{v:.0} kbps")
    }
}

pub fn build(activity: &Value, info: Option<&Value>) -> AppStatus {
    let mut rows: Vec<AppRow> = activity["sessions"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|x| {
            let mut title = s(x, "full_title");
            if s(x, "media_type") == "movie" && !s(x, "year").is_empty() {
                title = format!("{title} ({})", s(x, "year"));
            }
            let user = Some(s(x, "friendly_name")).filter(|u| !u.is_empty()).unwrap_or_else(|| s(x, "user"));
            let quality = Some(s(x, "stream_video_full_resolution"))
                .filter(|q| !q.is_empty())
                .unwrap_or_else(|| s(x, "video_resolution"));
            let decision = s(x, "transcode_decision");
            let mut meta = vec![user, s(x, "player"), quality];
            if decision == "transcode" {
                meta.push("TRANSCODE".into());
            }
            meta.retain(|m| !m.is_empty());
            let state = match s(x, "state").as_str() {
                "playing" => AppRowState::Active,
                "buffering" => AppRowState::Warn,
                _ => AppRowState::Idle,
            };
            AppRow::new(s(x, "session_key"), title, meta.join(" · "), state).progress(n(x, "progress_percent"))
        })
        .collect();
    let streams = count(activity, "stream_count");
    let transcodes = count(activity, "stream_count_transcode");
    let summary = match (streams, transcodes) {
        (0, _) => "Nothing playing".to_owned(),
        (s, 0) => plural(s, "stream"),
        (s, t) => format!("{} · {t} transcoding", plural(s, "stream")),
    };
    let mut st = AppStatus::new(summary)
        .tile("Streams", streams)
        .tile("Transcoding", transcodes)
        .tile("Direct play", count(activity, "stream_count_direct_play"))
        .tile("Bandwidth", kbps(n(activity, "total_bandwidth").unwrap_or(0.0)));
    st.version = info.map(|v| s(v, "tautulli_version")).filter(|v| !v.is_empty());
    st.more = cap_rows(&mut rows);
    st.rows_title = "Now playing".into();
    st.rows = rows;
    st
}

pub async fn status(i: &Integration, key: &str) -> Result<AppStatus, String> {
    let activity = call(i, key, "get_activity").await?;
    let info = call(i, key, "get_tautulli_info").await.ok();
    Ok(build(&activity, info.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::app::testing::{integration, serve};
    use crate::integrations::IntegrationKind;

    #[test]
    fn activity() {
        let v: Value = serde_json::from_str(include_str!("fixtures/tautulli_activity.json")).unwrap();
        let st = build(&v["response"]["data"], Some(&serde_json::json!({"tautulli_version": "v2.15.2"})));
        assert_eq!(st.summary, "2 streams · 1 transcoding");
        assert_eq!(st.rows[0].title, "Example Show - The Fifth One");
        assert_eq!(st.rows[0].meta, "alice · Living Room TV · 1080p · TRANSCODE");
        assert_eq!(st.rows[1].title, "A Film (2019)");
        assert_eq!(st.rows[1].state, AppRowState::Idle);
        assert_eq!(st.tiles[3].value, "24.5 Mbps");
        assert_eq!(st.version.as_deref(), Some("v2.15.2"));
    }

    #[tokio::test]
    async fn refused_key() {
        let (url, _seen) = serve(1, |_, _, _, _| {
            (200, vec![], r#"{"response":{"result":"error","message":"Invalid apikey","data":{}}}"#.into())
        })
        .await;
        let e = status(&integration(IntegrationKind::Tautulli, &url), "bad key").await.unwrap_err();
        assert_eq!(e, "Tautulli: Invalid apikey");
    }
}
