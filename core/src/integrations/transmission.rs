//! Transmission: speeds, torrents and start/stop over its JSON RPC
//! (`POST /transmission/rpc`). The first call returns 409 with the session id
//! to send back (CSRF protection). The login is optional: only when RPC
//! authentication is turned on.

use serde_json::{json, Value};

use super::app::{
    basic_auth, cap_rows, count, duration, n, plural, rate, s, AppAction, AppHealth, AppRow, AppRowState, AppStatus,
    Client,
};
use super::Integration;

fn rpc_path(i: &Integration) -> &'static str {
    if i.base_url.ends_with("/transmission/rpc") {
        ""
    } else if i.base_url.ends_with("/transmission") {
        "/rpc"
    } else {
        "/transmission/rpc"
    }
}

struct Rpc<'a> {
    c: Client<'a>,
    path: &'static str,
}

impl<'a> Rpc<'a> {
    async fn open(i: &'a Integration, password: &str) -> Result<Self, String> {
        let mut c = Client::new(i, "Transmission");
        if !i.username.is_empty() || !password.is_empty() {
            c = c.header("Authorization", basic_auth(&i.username, password));
        }
        let path = rpc_path(i);
        // The first call answers 409 with the session id to send from then on.
        let probe = c.send(crate::http::Method::Post, path, Some(&json!({"method": "session-get"}))).await?;
        if probe.status == 401 {
            return Err("Transmission refused the login (HTTP 401)".into());
        }
        let c = match probe.header("x-transmission-session-id") {
            Some(id) => c.header("X-Transmission-Session-Id", id.to_owned()),
            None => c,
        };
        Ok(Self { c, path })
    }

    async fn call(&self, method: &str, args: Value) -> Result<Value, String> {
        let v = self.c.post(self.path, &json!({"method": method, "arguments": args})).await?;
        match s(&v, "result").as_str() {
            "success" => Ok(v["arguments"].clone()),
            r => Err(format!("Transmission: {r}")),
        }
    }
}

/// Transmission's torrent status codes.
fn state(code: u64, error: bool) -> (&'static str, AppRowState) {
    if error {
        return ("error", AppRowState::Bad);
    }
    match code {
        0 => ("stopped", AppRowState::Idle),
        1 | 3 | 5 => ("queued", AppRowState::Idle),
        2 => ("checking", AppRowState::Active),
        4 => ("downloading", AppRowState::Active),
        _ => ("seeding", AppRowState::Ok),
    }
}

pub fn build(version: &str, stats: &Value, torrents: &Value) -> AppStatus {
    let mut list: Vec<&Value> = torrents["torrents"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    let errored = list.iter().filter(|t| count(t, "error") > 0).count() as u64;
    // Errors first, then fastest.
    list.sort_by(|a, b| {
        (count(b, "error") > 0).cmp(&(count(a, "error") > 0)).then(
            (count(b, "rateDownload") + count(b, "rateUpload"))
                .cmp(&(count(a, "rateDownload") + count(a, "rateUpload"))),
        )
    });
    let mut rows: Vec<AppRow> = list
        .iter()
        .map(|t| {
            let err = count(t, "error") > 0;
            let (label, st) = state(count(t, "status"), err);
            let mut meta = vec![label.to_owned()];
            if err {
                meta.push(s(t, "errorString"));
            }
            let (dl, ul) = (n(t, "rateDownload").unwrap_or(0.0), n(t, "rateUpload").unwrap_or(0.0));
            if dl > 0.0 {
                meta.push(format!("↓ {}", rate(dl)));
            }
            if ul > 0.0 {
                meta.push(format!("↑ {}", rate(ul)));
            }
            if let Some(eta) = n(t, "eta").filter(|e| *e > 0.0) {
                meta.push(format!("{} left", duration(eta)));
            }
            meta.retain(|m| !m.is_empty());
            let id = s(t, "id");
            let action =
                if count(t, "status") == 0 { AppAction::new("start", "Start") } else { AppAction::new("stop", "Stop") };
            AppRow::new(&id, s(t, "name"), meta.join(" · "), st)
                .progress(n(t, "percentDone").map(|p| p * 100.0))
                .action(action)
        })
        .collect();
    let (dl, ul) = (n(stats, "downloadSpeed").unwrap_or(0.0), n(stats, "uploadSpeed").unwrap_or(0.0));
    let active = count(stats, "activeTorrentCount");
    let mut st = AppStatus::new(format!("{} · ↓ {} · ↑ {}", plural(active, "active torrent"), rate(dl), rate(ul)))
        .tile("Down", rate(dl))
        .tile("Up", rate(ul))
        .tile("Active", active)
        .tile("Paused", count(stats, "pausedTorrentCount"))
        .tile("Total", count(stats, "torrentCount"));
    if errored > 0 {
        st.health = AppHealth::Warn;
        st.notes.push(format!("{} with errors", plural(errored, "torrent")));
    }
    st.version = Some(version.split_whitespace().next().unwrap_or("").to_owned()).filter(|v| !v.is_empty());
    st.more = cap_rows(&mut rows);
    st.rows_title = "Torrents".into();
    st.rows = rows;
    st.actions = vec![
        AppAction::new("stop-all", "Stop all").confirm("Stop every torrent in Transmission?"),
        AppAction::new("start-all", "Start all"),
    ];
    st
}

pub async fn status(i: &Integration, password: &str) -> Result<AppStatus, String> {
    let rpc = Rpc::open(i, password).await?;
    let session = rpc.call("session-get", json!({"fields": ["version"]})).await?;
    let stats = rpc.call("session-stats", json!({})).await?;
    let fields = ["id", "name", "status", "percentDone", "rateDownload", "rateUpload", "eta", "error", "errorString"];
    let torrents = rpc.call("torrent-get", json!({ "fields": fields })).await?;
    Ok(build(&s(&session, "version"), &stats, &torrents))
}

pub async fn act(i: &Integration, password: &str, target: &str, action: &str) -> Result<(), String> {
    let (method, ids) = match action {
        "start-all" => ("torrent-start", None),
        "stop-all" => ("torrent-stop", None),
        "start" | "stop" => {
            let id: u64 = target.parse().map_err(|_| "not a torrent".to_owned())?;
            (if action == "start" { "torrent-start" } else { "torrent-stop" }, Some(id))
        }
        _ => return Err(format!("unknown action {action}")),
    };
    let rpc = Rpc::open(i, password).await?;
    let args = match ids {
        Some(id) => json!({ "ids": [id] }),
        None => json!({}),
    };
    rpc.call(method, args).await.map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::app::testing::{integration, serve};
    use crate::integrations::IntegrationKind;

    #[test]
    fn torrents() {
        let t = json!({"torrents": [
            {"id": 1, "name": "seeding.iso", "status": 6, "percentDone": 1.0, "rateUpload": 2048, "eta": -1, "error": 0},
            {"id": 2, "name": "broken.iso", "status": 0, "percentDone": 0.3, "error": 3, "errorString": "No data found"},
            {"id": 3, "name": "fast.iso", "status": 4, "percentDone": 0.5, "rateDownload": 1048576, "eta": 90, "error": 0}
        ]});
        let st = build(
            "4.0.6 (38c164933e)",
            &json!({"downloadSpeed": 1048576, "uploadSpeed": 2048, "activeTorrentCount": 2, "pausedTorrentCount": 1, "torrentCount": 3}),
            &t,
        );
        assert_eq!(st.summary, "2 active torrents · ↓ 1.0 MB/s · ↑ 2.0 KB/s");
        assert_eq!(st.version.as_deref(), Some("4.0.6"));
        let titles: Vec<_> = st.rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, ["broken.iso", "fast.iso", "seeding.iso"]);
        assert_eq!(st.rows[0].meta, "error · No data found");
        assert_eq!(st.rows[0].actions[0].id, "start");
        assert_eq!(st.rows[1].meta, "downloading · ↓ 1.0 MB/s · 1m 30s left");
        assert_eq!(st.health, AppHealth::Warn);
    }

    #[tokio::test]
    async fn session_id_dance() {
        let (url, seen) = serve(4, |_, _, head, body| {
            if !head.to_ascii_lowercase().contains("x-transmission-session-id: s1") {
                return (409, vec![("X-Transmission-Session-Id".into(), "s1".into())], String::new());
            }
            let m: Value = serde_json::from_str(body).unwrap();
            let args = match m["method"].as_str().unwrap() {
                "session-get" => json!({"version": "4.0.6"}),
                "session-stats" => json!({"activeTorrentCount": 0}),
                _ => json!({"torrents": []}),
            };
            (200, vec![], json!({"result": "success", "arguments": args}).to_string())
        })
        .await;
        let st = status(&integration(IntegrationKind::Transmission, &url), "").await.unwrap();
        assert_eq!(st.version.as_deref(), Some("4.0.6"));
        let seen = seen.await.unwrap();
        assert!(seen.iter().all(|r| r.path == "/transmission/rpc"));
        assert!(!seen[0].head.to_ascii_lowercase().contains("authorization"));
    }
}
