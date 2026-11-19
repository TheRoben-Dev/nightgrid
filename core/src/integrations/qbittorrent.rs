//! qBittorrent (Web UI API v2): speeds, active and failed torrents, pause or
//! resume one torrent or all. Logs in with the Web UI login name and password
//! (`POST /api/v2/auth/login`, session cookie) and logs out after each read.
//! With no password saved it calls the API directly, for setups that bypass
//! authentication for the local network.

use serde_json::Value;

use super::app::{cap_rows, cookie, n, plural, rate, s, AppAction, AppHealth, AppRow, AppRowState, AppStatus, Client};
use super::Integration;

/// qBittorrent's "no estimate" ETA.
const ETA_INFINITE: f64 = 8_640_000.0;

async fn session<'a>(i: &'a Integration, password: &str) -> Result<Client<'a>, String> {
    let c = Client::new(i, "qBittorrent").header("Referer", i.base_url.clone());
    if password.is_empty() {
        return Ok(c);
    }
    let r = c.post_form("/api/v2/auth/login", &[("username", &i.username), ("password", password)]).await?;
    if r.status == 403 {
        return Err("qBittorrent banned this address after failed logins; wait or restart it".into());
    }
    let r = c.check(r)?;
    if r.text().trim() != "Ok." {
        return Err("qBittorrent refused the login name or password".into());
    }
    let sid = cookie(&r).ok_or("qBittorrent didn't start a session")?;
    Ok(c.header("Cookie", sid))
}

async fn logout(c: &Client<'_>) {
    let _ = c.post_form("/api/v2/auth/logout", &[]).await;
}

/// qBittorrent 5 renamed pause/resume to stop/start.
fn v5(version: &str) -> bool {
    version.trim_start_matches('v').split('.').next().and_then(|m| m.parse::<u32>().ok()).is_some_and(|m| m >= 5)
}

fn row(t: &Value, version: &str) -> AppRow {
    let st = s(t, "state");
    let state = match st.as_str() {
        "error" | "missingFiles" => AppRowState::Bad,
        "downloading" | "forcedDL" | "metaDL" | "forcedMetaDL" | "uploading" | "forcedUP" | "moving" | "checkingDL"
        | "checkingUP" | "checkingResumeData" => AppRowState::Active,
        "stalledDL" => AppRowState::Warn,
        "uploading_done" | "stalledUP" | "queuedUP" => AppRowState::Ok,
        _ => AppRowState::Idle,
    };
    let progress = n(t, "progress").unwrap_or(0.0) * 100.0;
    let mut meta = vec![st.clone()];
    let (dl, ul) = (n(t, "dlspeed").unwrap_or(0.0), n(t, "upspeed").unwrap_or(0.0));
    if dl > 0.0 {
        meta.push(format!("↓ {}", rate(dl)));
    }
    if ul > 0.0 {
        meta.push(format!("↑ {}", rate(ul)));
    }
    if let Some(eta) = n(t, "eta").filter(|e| *e > 0.0 && *e < ETA_INFINITE && progress < 100.0) {
        meta.push(format!("{} left", super::app::duration(eta)));
    }
    let paused = st.starts_with("paused") || st.starts_with("stopped");
    let (on, off) = if v5(version) { ("start", "stop") } else { ("resume", "pause") };
    let action = if paused { AppAction::new(on, "Resume") } else { AppAction::new(off, "Pause") };
    AppRow::new(s(t, "hash"), s(t, "name"), meta.join(" · "), state).progress(Some(progress)).action(action)
}

pub fn build(version: &str, transfer: &Value, active: &Value, errored: &Value) -> AppStatus {
    let list = |v: &Value| v.as_array().cloned().unwrap_or_default();
    let (active, errored) = (list(active), list(errored));
    let mut rows: Vec<AppRow> = errored.iter().chain(active.iter()).map(|t| row(t, version)).collect();
    rows.dedup_by(|a, b| a.id == b.id);
    let (dl, ul) = (n(transfer, "dl_info_speed").unwrap_or(0.0), n(transfer, "up_info_speed").unwrap_or(0.0));
    let mut summary = format!("↓ {} · ↑ {}", rate(dl), rate(ul));
    if !active.is_empty() {
        summary = format!("{} · {summary}", plural(active.len() as u64, "active torrent"));
    }
    let mut st = AppStatus::new(summary)
        .tile("Down", rate(dl))
        .tile("Up", rate(ul))
        .tile("Active", active.len())
        .tile("Errored", errored.len());
    if !errored.is_empty() {
        st.health = st.health.max(AppHealth::Warn);
        st.notes.push(format!("{} with errors", plural(errored.len() as u64, "torrent")));
    }
    match s(transfer, "connection_status").as_str() {
        "disconnected" => st = st.warn("qBittorrent is disconnected from the network"),
        "firewalled" => st = st.warn("qBittorrent is firewalled: incoming connections can't reach it"),
        _ => {}
    }
    st.version = Some(version.trim().to_owned()).filter(|v| !v.is_empty());
    st.more = cap_rows(&mut rows);
    st.rows_title = "Active".into();
    st.rows = rows;
    let (on, off) = if v5(version) { ("start", "stop") } else { ("resume", "pause") };
    st.actions = vec![
        AppAction::new(&format!("{off}-all"), "Pause all").confirm("Pause every torrent in qBittorrent?"),
        AppAction::new(&format!("{on}-all"), "Resume all"),
    ];
    st
}

pub async fn status(i: &Integration, password: &str) -> Result<AppStatus, String> {
    let c = session(i, password).await?;
    let out = async {
        let version = c.check(c.send(crate::http::Method::Get, "/api/v2/app/version", None).await?)?.text();
        let transfer = c.get("/api/v2/transfer/info").await?;
        let active = c.get("/api/v2/torrents/info?filter=active&sort=dlspeed&reverse=true&limit=40").await?;
        let errored = c.get("/api/v2/torrents/info?filter=errored&limit=10").await?;
        Ok(build(&version, &transfer, &active, &errored))
    }
    .await;
    if !password.is_empty() {
        logout(&c).await;
    }
    out
}

pub async fn act(i: &Integration, password: &str, target: &str, action: &str) -> Result<(), String> {
    let (verb, hashes) = match action.strip_suffix("-all") {
        Some(v) => (v, "all"),
        None => (action, target),
    };
    if !matches!(verb, "pause" | "resume" | "stop" | "start") {
        return Err(format!("unknown action {action}"));
    }
    if hashes != "all" && (hashes.is_empty() || !hashes.chars().all(|c| c.is_ascii_hexdigit())) {
        return Err("not a torrent".into());
    }
    let c = session(i, password).await?;
    let r = c.post_form(&format!("/api/v2/torrents/{verb}"), &[("hashes", hashes)]).await;
    if !password.is_empty() {
        logout(&c).await;
    }
    c.check(r?).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::app::testing::{integration, serve};
    use crate::integrations::IntegrationKind;
    use serde_json::json;

    #[test]
    fn torrents() {
        let active = json!([
            {"hash": "aa11", "name": "debian-13.iso", "state": "downloading", "progress": 0.5, "dlspeed": 2097152, "upspeed": 0, "eta": 300},
            {"hash": "bb22", "name": "ubuntu.iso", "state": "uploading", "progress": 1.0, "dlspeed": 0, "upspeed": 1024, "eta": 8640000}
        ]);
        let errored = json!([{"hash": "cc33", "name": "broken", "state": "missingFiles", "progress": 0.2}]);
        let st = build(
            "v5.0.2",
            &json!({"dl_info_speed": 2097152, "up_info_speed": 1024, "connection_status": "firewalled"}),
            &active,
            &errored,
        );
        assert_eq!(st.summary, "2 active torrents · ↓ 2.0 MB/s · ↑ 1.0 KB/s");
        assert_eq!(st.rows[0].title, "broken");
        assert_eq!(st.rows[0].state, AppRowState::Bad);
        assert_eq!(st.rows[1].meta, "downloading · ↓ 2.0 MB/s · 5m 00s left");
        assert_eq!(st.rows[1].actions[0].id, "stop");
        assert_eq!(st.rows[2].meta, "uploading · ↑ 1.0 KB/s");
        assert_eq!(st.health, AppHealth::Warn);
        assert_eq!(st.actions[0].id, "stop-all");
        let old = build("v4.6.7", &json!({}), &json!([{"hash": "aa", "name": "x", "state": "pausedDL"}]), &json!([]));
        assert_eq!(old.rows[0].actions[0].id, "resume");
        assert_eq!(old.actions[1].id, "resume-all");
    }

    #[tokio::test]
    async fn login_read_logout() {
        let (url, seen) = serve(6, |m, p, head, body| match (m, p) {
            ("POST", "/api/v2/auth/login") if body == "username=admin&password=pw" => {
                (200, vec![("set-cookie".into(), "SID=abc123; HttpOnly; path=/".into())], "Ok.".into())
            }
            ("POST", "/api/v2/auth/login") => (200, vec![], "Fails.".into()),
            (_, _) if !head.to_ascii_lowercase().contains("cookie: sid=abc123") => (403, vec![], String::new()),
            ("GET", "/api/v2/app/version") => (200, vec![], "v4.6.7".into()),
            ("GET", "/api/v2/transfer/info") => (200, vec![], r#"{"dl_info_speed":0,"up_info_speed":0}"#.into()),
            ("GET", _) => (200, vec![], "[]".into()),
            ("POST", "/api/v2/auth/logout") => (200, vec![], String::new()),
            _ => (404, vec![], String::new()),
        })
        .await;
        let mut i = integration(IntegrationKind::Qbittorrent, &url);
        i.username = "admin".into();
        let st = status(&i, "pw").await.unwrap();
        assert_eq!(st.version.as_deref(), Some("v4.6.7"));
        let seen = seen.await.unwrap();
        assert_eq!(seen[5].path, "/api/v2/auth/logout");
        assert!(seen.iter().all(|r| r.head.to_ascii_lowercase().contains("referer: http://127.0.0.1")));

        let (url, _) = serve(1, |_, _, _, _| (200, vec![], "Fails.".into())).await;
        let mut i = integration(IntegrationKind::Qbittorrent, &url);
        i.username = "admin".into();
        assert_eq!(status(&i, "nope").await.unwrap_err(), "qBittorrent refused the login name or password");
        assert!(act(&i, "pw", "../x", "pause").await.is_err());
        assert!(act(&i, "pw", "aa", "delete").await.is_err());
    }
}
