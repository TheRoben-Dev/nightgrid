//! Usenet downloaders. **SABnzbd**: `GET /api?mode=...&output=json` with the
//! API key from Config → General. **NZBGet**: JSON-RPC at `/jsonrpc` with the
//! control login from Settings → Security. Both show speed, the queue with
//! progress, failures, and pause or resume for the queue or one item.

use serde_json::{json, Value};

use super::app::{
    basic_auth, cap_rows, count, enc, n, rate, s, valid_target, AppAction, AppHealth, AppRow, AppRowState, AppStatus,
    Client,
};
use super::{Integration, IntegrationKind};

const MB: f64 = 1024.0 * 1024.0;

// ---------- SABnzbd ----------

async fn sab(i: &Integration, key: &str, query: &str) -> Result<Value, String> {
    let v = Client::new(i, "SABnzbd").get(&format!("/api?output=json&apikey={}&{query}", enc(key))).await?;
    if v["status"].as_bool() == Some(false) {
        let e = s(&v, "error");
        return Err(if e.is_empty() { "SABnzbd refused the request".into() } else { format!("SABnzbd: {e}") });
    }
    Ok(v)
}

pub fn build_sab(queue: &Value, history: &Value) -> AppStatus {
    let q = &queue["queue"];
    let paused = q["paused"].as_bool().unwrap_or(false);
    let speed = n(q, "kbpersec").unwrap_or(0.0) * 1024.0;
    let mut rows: Vec<AppRow> = q["slots"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|x| {
            let st = s(x, "status");
            let state = match st.as_str() {
                "Downloading" | "Fetching" | "Grabbing" => AppRowState::Active,
                "Paused" | "Queued" => AppRowState::Idle,
                _ => AppRowState::Idle,
            };
            let mut meta = vec![st.to_lowercase()];
            if let (Some(mb), Some(left)) = (n(x, "mb"), n(x, "mbleft")) {
                meta.push(format!("{} of {}", super::app::bytes((mb - left) * MB), super::app::bytes(mb * MB)));
            }
            let tl = s(x, "timeleft");
            if st == "Downloading" && !tl.is_empty() && tl != "0:00:00" {
                meta.push(format!("{} left", super::app::duration_hms(&tl)));
            }
            let id = s(x, "nzo_id");
            let name = s(x, "filename");
            let toggle =
                if st == "Paused" { AppAction::new("resume", "Resume") } else { AppAction::new("pause", "Pause") };
            AppRow::new(&id, &name, meta.join(" · "), state).progress(n(x, "percentage")).action(toggle).action(
                AppAction::new("delete", "Delete").confirm(format!("Delete \"{name}\" from the SABnzbd queue?")),
            )
        })
        .collect();
    let failed: Vec<&Value> = history["history"]["slots"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    let items = count(q, "noofslots");
    let mut summary = if paused { "Paused".to_owned() } else { rate(speed) };
    summary.push_str(&format!(" · {} queued", items));
    let mut st = AppStatus::new(summary).tile("Speed", rate(speed)).tile("Queue", items).tile("Left", s(q, "sizeleft"));
    let free = s(q, "diskspace1_norm");
    if !free.is_empty() {
        st = st.tile("Free disk", free);
    }
    if paused {
        st.health = AppHealth::Warn;
        st.notes.push("The queue is paused".into());
    }
    for f in failed.iter().take(3) {
        st.health = st.health.max(AppHealth::Warn);
        st.notes.push(format!("Failed: {} ({})", s(f, "name"), s(f, "fail_message")));
    }
    st.version = Some(s(q, "version")).filter(|v| !v.is_empty());
    st.more = cap_rows(&mut rows).max((items as usize).saturating_sub(rows.len()) as u32);
    st.rows_title = "Queue".into();
    st.rows = rows;
    st.actions = vec![if paused {
        AppAction::new("resume-all", "Resume")
    } else {
        AppAction::new("pause-all", "Pause").confirm("Pause the SABnzbd queue?")
    }];
    st
}

// ---------- NZBGet ----------

fn nzbget<'a>(i: &'a Integration, password: &str) -> Client<'a> {
    Client::new(i, "NZBGet").header("Authorization", basic_auth(&i.username, password))
}

async fn rpc(c: &Client<'_>, method: &str, params: Value) -> Result<Value, String> {
    let v = c.post("/jsonrpc", &json!({"method": method, "params": params})).await?;
    if !v["error"].is_null() {
        return Err(format!("NZBGet: {}", s(&v["error"], "message")));
    }
    Ok(v["result"].clone())
}

pub fn build_nzbget(version: &str, status: &Value, groups: &Value) -> AppStatus {
    let paused = status["DownloadPaused"].as_bool().unwrap_or(false);
    let speed = n(status, "DownloadRate").unwrap_or(0.0);
    let list = groups.as_array().map(|a| a.as_slice()).unwrap_or_default();
    let mut rows: Vec<AppRow> =
        list.iter()
            .map(|g| {
                let st = s(g, "Status");
                let state = match st.as_str() {
                    "DOWNLOADING" | "FETCHING" | "PP_QUEUED" | "LOADING_PARS" | "VERIFYING_SOURCES" | "REPAIRING"
                    | "VERIFYING_REPAIRED" | "RENAMING" | "UNPACKING" | "MOVING" | "EXECUTING_SCRIPT" => {
                        AppRowState::Active
                    }
                    _ => AppRowState::Idle,
                };
                let (size, left) = (n(g, "FileSizeMB").unwrap_or(0.0), n(g, "RemainingSizeMB").unwrap_or(0.0));
                let progress = (size > 0.0).then(|| (size - left) / size * 100.0);
                let meta = format!("{} · {}", st.to_lowercase().replace('_', " "), super::app::bytes(size * MB));
                let id = s(g, "NZBID");
                let name = s(g, "NZBName");
                let toggle =
                    if st == "PAUSED" { AppAction::new("resume", "Resume") } else { AppAction::new("pause", "Pause") };
                AppRow::new(&id, &name, meta, state).progress(progress).action(toggle).action(
                    AppAction::new("delete", "Delete").confirm(format!("Delete \"{name}\" from the NZBGet queue?")),
                )
            })
            .collect();
    let mut summary = if paused { "Paused".to_owned() } else { rate(speed) };
    summary.push_str(&format!(" · {} queued", list.len()));
    let mut st = AppStatus::new(summary)
        .tile("Speed", rate(speed))
        .tile("Queue", list.len())
        .tile("Left", super::app::bytes(n(status, "RemainingSizeMB").unwrap_or(0.0) * MB));
    if let Some(free) = n(status, "FreeDiskSpaceMB") {
        st = st.tile("Free disk", super::app::bytes(free * MB));
    }
    if paused {
        st.health = AppHealth::Warn;
        st.notes.push("Downloading is paused".into());
    }
    st.version = Some(version.to_owned()).filter(|v| !v.is_empty());
    st.more = cap_rows(&mut rows);
    st.rows_title = "Queue".into();
    st.rows = rows;
    st.actions = vec![if paused {
        AppAction::new("resume-all", "Resume")
    } else {
        AppAction::new("pause-all", "Pause").confirm("Pause downloading in NZBGet?")
    }];
    st
}

pub async fn status(i: &Integration, secret: &str) -> Result<AppStatus, String> {
    if i.kind == IntegrationKind::Sabnzbd {
        let queue = sab(i, secret, "mode=queue&limit=40").await?;
        let history = sab(i, secret, "mode=history&limit=5&failed_only=1").await.unwrap_or(Value::Null);
        return Ok(build_sab(&queue, &history));
    }
    let c = nzbget(i, secret);
    let version = rpc(&c, "version", json!([])).await?;
    let status = rpc(&c, "status", json!([])).await?;
    let groups = rpc(&c, "listgroups", json!([0])).await?;
    Ok(build_nzbget(version.as_str().unwrap_or(""), &status, &groups))
}

pub async fn act(i: &Integration, secret: &str, target: &str, action: &str) -> Result<(), String> {
    let row = matches!(action, "pause" | "resume" | "delete");
    if row && (!valid_target(target) || target.contains(':')) {
        return Err("not a queue item".into());
    }
    if i.kind == IntegrationKind::Sabnzbd {
        let q = match action {
            "pause-all" => "mode=pause".to_owned(),
            "resume-all" => "mode=resume".to_owned(),
            a if row => format!("mode=queue&name={a}&value={}", enc(target)),
            _ => return Err(format!("unknown action {action}")),
        };
        return sab(i, secret, &q).await.map(drop);
    }
    let c = nzbget(i, secret);
    let r = match action {
        "pause-all" => rpc(&c, "pausedownload", json!([])).await?,
        "resume-all" => rpc(&c, "resumedownload", json!([])).await?,
        a if row => {
            let id: i64 = target.parse().map_err(|_| "not a queue item".to_owned())?;
            let cmd = match a {
                "pause" => "GroupPause",
                "resume" => "GroupResume",
                _ => "GroupDelete",
            };
            rpc(&c, "editqueue", json!([cmd, "", [id]])).await?
        }
        _ => return Err(format!("unknown action {action}")),
    };
    if r.as_bool() == Some(false) {
        return Err("NZBGet didn't apply it".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::app::testing::{integration, serve};

    #[test]
    fn sabnzbd() {
        let q = json!({"queue": {"version": "4.4.1", "paused": false, "kbpersec": "2048", "sizeleft": "1.2 GB", "noofslots": 2, "diskspace1_norm": "120.5 G",
        "slots": [
            {"nzo_id": "SABnzbd_nzo_ab12", "filename": "Some.Release", "status": "Downloading", "percentage": "45", "mb": "1000", "mbleft": "550", "timeleft": "0:05:00"},
            {"nzo_id": "SABnzbd_nzo_cd34", "filename": "Other.Release", "status": "Paused", "percentage": "0", "mb": "200", "mbleft": "200"}
        ]}});
        let h = json!({"history": {"slots": [{"name": "Bad.Release", "fail_message": "Repair failed"}]}});
        let st = build_sab(&q, &h);
        assert_eq!(st.summary, "2.0 MB/s · 2 queued");
        assert_eq!(st.rows[0].meta, "downloading · 450 MB of 1000 MB · 5m 00s left");
        assert_eq!(st.rows[1].actions[0].id, "resume");
        assert_eq!(st.notes, ["Failed: Bad.Release (Repair failed)"]);
        assert_eq!(st.health, AppHealth::Warn);
        assert_eq!(st.version.as_deref(), Some("4.4.1"));
    }

    #[test]
    fn nzbget_queue() {
        let g = json!([{"NZBID": 5, "NZBName": "Some.Release", "Status": "DOWNLOADING", "FileSizeMB": 1000, "RemainingSizeMB": 250}]);
        let st =
            build_nzbget("24.5", &json!({"DownloadRate": 1048576, "DownloadPaused": true, "RemainingSizeMB": 250}), &g);
        assert_eq!(st.summary, "Paused · 1 queued");
        assert!((st.rows[0].progress.unwrap() - 75.0).abs() < 0.01);
        assert_eq!(st.rows[0].meta, "downloading · 1000 MB");
        assert_eq!(st.actions[0].id, "resume-all");
    }

    #[tokio::test]
    async fn requests() {
        let (url, seen) = serve(3, |_, p, _, _| {
            if p.contains("mode=queue") && p.contains("apikey=bad") {
                (200, vec![], r#"{"status":false,"error":"API Key Incorrect"}"#.into())
            } else {
                (200, vec![], r#"{"status":true}"#.into())
            }
        })
        .await;
        let i = integration(IntegrationKind::Sabnzbd, &url);
        assert_eq!(status(&i, "bad").await.unwrap_err(), "SABnzbd: API Key Incorrect");
        act(&i, "k", "SABnzbd_nzo_ab12", "delete").await.unwrap();
        act(&i, "k", "", "pause-all").await.unwrap();
        assert!(act(&i, "k", "a&b", "pause").await.is_err());
        let seen = seen.await.unwrap();
        assert!(seen[1].path.ends_with("mode=queue&name=delete&value=SABnzbd_nzo_ab12"));
        assert!(seen[2].path.ends_with("mode=pause"));

        let (url, seen) = serve(1, |_, _, _, _| (200, vec![], r#"{"result":true}"#.into())).await;
        let mut n = integration(IntegrationKind::Nzbget, &url);
        n.username = "nzbget".into();
        act(&n, "tegbzn6789", "5", "pause").await.unwrap();
        let seen = seen.await.unwrap();
        assert_eq!(seen[0].body, r#"{"method":"editqueue","params":["GroupPause","",[5]]}"#);
        assert!(seen[0].head.to_ascii_lowercase().contains("authorization: basic"));
    }
}
