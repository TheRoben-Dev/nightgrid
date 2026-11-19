//! Syncthing: folders with their sync state, connected devices, errors;
//! rescan a folder or all, pause or resume syncing. Uses the API key from
//! Actions → Settings → General in an `X-API-Key` header.

use serde_json::Value;

use super::app::{
    bytes, cap_rows, count, enc, n, plural, s, AppAction, AppHealth, AppRow, AppRowState, AppStatus, Client,
};
use super::Integration;

fn client<'a>(i: &'a Integration, key: &str) -> Client<'a> {
    Client::new(i, "Syncthing").header("X-API-Key", key)
}

/// One folder row from its config and `db/status`.
pub fn folder_row(cfg: &Value, db: Option<&Value>) -> AppRow {
    let id = s(cfg, "id");
    let label = Some(s(cfg, "label")).filter(|l| !l.is_empty()).unwrap_or_else(|| id.clone());
    if cfg["paused"].as_bool() == Some(true) {
        return AppRow::new(&id, label, "paused", AppRowState::Idle).action(AppAction::new("rescan", "Rescan"));
    }
    let Some(db) = db else {
        return AppRow::new(&id, label, "unknown", AppRowState::Idle);
    };
    let state = s(db, "state");
    let need = count(db, "needFiles") + count(db, "needDeletes");
    let errors = count(db, "errors") + count(db, "pullErrors");
    let (text, rs) = match state.as_str() {
        _ if errors > 0 => (format!("{} · {state}", plural(errors, "error")), AppRowState::Bad),
        "error" => (format!("error: {}", s(db, "error")), AppRowState::Bad),
        "syncing" | "sync-preparing" => {
            (format!("syncing · {need} to go ({})", bytes(n(db, "needBytes").unwrap_or(0.0))), AppRowState::Active)
        }
        "scanning" | "scan-waiting" => ("scanning".into(), AppRowState::Active),
        "idle" if need > 0 => (format!("out of sync · {need} to go"), AppRowState::Warn),
        "idle" => (format!("up to date · {}", bytes(n(db, "globalBytes").unwrap_or(0.0))), AppRowState::Ok),
        other => (other.to_owned(), AppRowState::Idle),
    };
    let global = n(db, "globalBytes").unwrap_or(0.0);
    let progress = (rs == AppRowState::Active && global > 0.0)
        .then(|| (global - n(db, "needBytes").unwrap_or(0.0)) / global * 100.0);
    AppRow::new(&id, label, text, rs).progress(progress).action(AppAction::new("rescan", "Rescan"))
}

pub fn build(version: &Value, folders: Vec<AppRow>, devices: &Value, connections: &Value, errors: &Value) -> AppStatus {
    let my_id = s(version, "myID");
    let devs: Vec<&Value> =
        devices.as_array().map(|a| a.iter().filter(|d| s(d, "deviceID") != my_id).collect()).unwrap_or_default();
    let conns = &connections["connections"];
    let connected = devs.iter().filter(|d| conns[s(d, "deviceID")]["connected"].as_bool() == Some(true)).count();
    let synced = folders.iter().filter(|f| f.state == AppRowState::Ok).count();
    let mut st =
        AppStatus::new(format!("{synced}/{} folders up to date · {connected}/{} devices", folders.len(), devs.len()))
            .tile("Folders", folders.len())
            .tile("Devices online", format!("{connected}/{}", devs.len()));
    if let Some(total) = n(&connections["total"], "inBytesTotal") {
        st = st.tile("Received", bytes(total));
    }
    if folders.iter().any(|f| f.state == AppRowState::Bad) {
        st.health = AppHealth::Bad;
    } else if folders.iter().any(|f| f.state == AppRowState::Warn) {
        st.health = AppHealth::Warn;
    }
    for e in errors["errors"].as_array().map(|a| a.as_slice()).unwrap_or_default().iter().take(4) {
        st = st.warn(s(e, "message"));
    }
    st.version = Some(s(version, "version")).filter(|v| !v.is_empty());
    let mut rows = folders;
    st.more = cap_rows(&mut rows);
    st.rows_title = "Folders".into();
    st.rows = rows;
    st.actions = vec![
        AppAction::new("rescan-all", "Rescan all"),
        AppAction::new("pause-all", "Pause").confirm("Pause syncing with every device?"),
        AppAction::new("resume-all", "Resume"),
    ];
    st
}

pub async fn status(i: &Integration, key: &str) -> Result<AppStatus, String> {
    let c = client(i, key);
    let mut version = c.get("/rest/system/version").await?;
    let status = c.get("/rest/system/status").await?;
    version["myID"] = status["myID"].clone();
    let folders = c.get("/rest/config/folders").await?;
    let devices = c.get("/rest/config/devices").await?;
    let connections = c.get("/rest/system/connections").await?;
    let errors = c.get("/rest/system/error").await.unwrap_or(Value::Null);
    let mut rows = Vec::new();
    for f in folders.as_array().map(|a| a.as_slice()).unwrap_or_default().iter().take(super::app::MAX_ROWS) {
        let db = if f["paused"].as_bool() == Some(true) {
            None
        } else {
            c.get(&format!("/rest/db/status?folder={}", enc(&s(f, "id")))).await.ok()
        };
        rows.push(folder_row(f, db.as_ref()));
    }
    Ok(build(&version, rows, &devices, &connections, &errors))
}

pub async fn act(i: &Integration, key: &str, target: &str, action: &str) -> Result<(), String> {
    let c = client(i, key);
    let path = match action {
        "rescan" if !target.is_empty() => format!("/rest/db/scan?folder={}", enc(target)),
        "rescan-all" => "/rest/db/scan".into(),
        "pause-all" => "/rest/system/pause".into(),
        "resume-all" => "/rest/system/resume".into(),
        _ => return Err(format!("unknown action {action}")),
    };
    c.post_empty(&path).await.map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn folders_and_devices() {
        let rows = vec![
            folder_row(
                &json!({"id": "abcd-1234", "label": "Photos"}),
                Some(&json!({"state": "idle", "needFiles": 0, "globalBytes": 1073741824})),
            ),
            folder_row(
                &json!({"id": "docs"}),
                Some(&json!({"state": "syncing", "needFiles": 12, "needBytes": 268435456, "globalBytes": 1073741824})),
            ),
            folder_row(&json!({"id": "x", "label": "Music", "paused": true}), None),
            folder_row(&json!({"id": "y", "label": "Broken"}), Some(&json!({"state": "idle", "pullErrors": 2}))),
        ];
        assert_eq!(rows[0].meta, "up to date · 1.0 GB");
        assert_eq!(rows[1].meta, "syncing · 12 to go (256 MB)");
        assert!((rows[1].progress.unwrap() - 75.0).abs() < 0.01);
        assert_eq!(rows[2].meta, "paused");
        assert_eq!(rows[3].state, AppRowState::Bad);
        let devices =
            json!([{"deviceID": "ME"}, {"deviceID": "A", "name": "laptop"}, {"deviceID": "B", "name": "phone"}]);
        let conns = json!({"connections": {"A": {"connected": true}, "B": {"connected": false}}, "total": {"inBytesTotal": 2048}});
        let st = build(
            &json!({"version": "v1.29.2", "myID": "ME"}),
            rows,
            &devices,
            &conns,
            &json!({"errors": [{"message": "Folder Docs: no space"}]}),
        );
        assert_eq!(st.summary, "1/4 folders up to date · 1/2 devices");
        assert_eq!(st.health, AppHealth::Bad);
        assert_eq!(st.notes, ["Folder Docs: no space"]);
    }
}
