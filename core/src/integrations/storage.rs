//! Storage systems: **TrueNAS** (REST API v2.0 with an API key), **Synology
//! DSM** (web API, login name and password) and **Proxmox Backup Server**
//! (API token). Each shows pools or volumes with how full they are, plus the
//! system's own alerts, disk problems or failed tasks.

use serde_json::Value;

use super::app::{bytes, cap_rows, count, duration, n, plural, s, AppHealth, AppRow, AppRowState, AppStatus, Client};
use super::{Integration, IntegrationKind};

fn usage_row(id: &str, name: &str, used: f64, total: f64, status: &str, healthy: bool) -> AppRow {
    let pct = if total > 0.0 { used / total * 100.0 } else { 0.0 };
    let state = if !healthy {
        AppRowState::Bad
    } else if pct >= 90.0 {
        AppRowState::Warn
    } else {
        AppRowState::Ok
    };
    let mut meta = Vec::new();
    if !status.is_empty() {
        meta.push(status.to_lowercase());
    }
    if total > 0.0 {
        meta.push(format!("{} of {} used ({pct:.0} %)", bytes(used), bytes(total)));
    }
    AppRow::new(id, name, meta.join(" · "), state).progress((total > 0.0).then_some(pct))
}

fn rows_health(st: &mut AppStatus) {
    if st.rows.iter().any(|r| r.state == AppRowState::Bad) {
        st.health = AppHealth::Bad;
    } else if st.rows.iter().any(|r| r.state == AppRowState::Warn) {
        st.health = st.health.max(AppHealth::Warn);
    }
}

// ---------- TrueNAS ----------

pub fn build_truenas(info: &Value, pools: &Value, alerts: &Value) -> AppStatus {
    let mut rows: Vec<AppRow> = pools
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|p| {
            let status = s(p, "status");
            let healthy = p["healthy"].as_bool().unwrap_or(status == "ONLINE");
            let (used, total) = (n(p, "allocated").unwrap_or(0.0), n(p, "size").unwrap_or(0.0));
            usage_row(&s(p, "id"), &s(p, "name"), used, total, &status, healthy)
        })
        .collect();
    let active: Vec<&Value> = alerts
        .as_array()
        .map(|a| a.iter().filter(|x| x["dismissed"].as_bool() != Some(true)).collect())
        .unwrap_or_default();
    let serious =
        |x: &&&Value| matches!(s(x, "level").as_str(), "WARNING" | "ERROR" | "CRITICAL" | "ALERT" | "EMERGENCY");
    let summary = format!(
        "{} · {}",
        plural(rows.len() as u64, "pool"),
        match active.iter().filter(serious).count() {
            0 => "no alerts".to_owned(),
            k => plural(k as u64, "alert"),
        }
    );
    let mut st = AppStatus::new(summary).tile("Pools", rows.len());
    if let Some(up) = n(info, "uptime_seconds") {
        st = st.tile("Uptime", duration(up));
    }
    if let Some(load) = info["loadavg"].as_array().and_then(|a| a.first()).and_then(Value::as_f64) {
        st = st.tile("Load", format!("{load:.2}"));
    }
    if let Some(mem) = n(info, "physmem") {
        st = st.tile("Memory", bytes(mem));
    }
    for a in active.iter().filter(serious).take(5) {
        let level = if matches!(s(a, "level").as_str(), "WARNING") { AppHealth::Warn } else { AppHealth::Bad };
        st.health = st.health.max(level);
        let text = s(a, "formatted");
        st.notes.push(if text.is_empty() { s(a, "klass") } else { text.lines().next().unwrap_or("").to_owned() });
    }
    st.version = Some(s(info, "version")).filter(|v| !v.is_empty());
    st.more = cap_rows(&mut rows);
    st.rows_title = "Pools".into();
    st.rows = rows;
    rows_health(&mut st);
    st
}

// ---------- Synology DSM ----------

pub fn build_synology(info: &Value, util: Option<&Value>, storage: &Value) -> AppStatus {
    let d = &storage["data"];
    let mut rows: Vec<AppRow> = d["volumes"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|v| {
            let status = s(v, "status");
            let healthy = matches!(status.as_str(), "normal" | "");
            let (used, total) = (n(&v["size"], "used").unwrap_or(0.0), n(&v["size"], "total").unwrap_or(0.0));
            let name =
                Some(s(v, "display_name")).filter(|x| !x.is_empty()).unwrap_or_else(|| s(v, "id").replace('_', " "));
            usage_row(&s(v, "id"), &name, used, total, &status, healthy)
        })
        .collect();
    let disks: Vec<&Value> = d["disks"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    let mut st =
        AppStatus::new(format!("{} · {}", plural(rows.len() as u64, "volume"), plural(disks.len() as u64, "disk")))
            .tile("Volumes", rows.len())
            .tile("Disks", disks.len());
    let i = &info["data"];
    if let Some(t) = n(i, "sys_temp") {
        st = st.tile("Temp", format!("{t:.0} °C"));
    }
    if let Some(u) = util.map(|u| &u["data"]) {
        let cpu = n(&u["cpu"], "user_load").unwrap_or(0.0) + n(&u["cpu"], "system_load").unwrap_or(0.0);
        st = st.tile("CPU", format!("{cpu:.0} %"));
        if let Some(m) = n(&u["memory"], "real_usage") {
            st = st.tile("Memory", format!("{m:.0} %"));
        }
    }
    for dk in &disks {
        let (status, smart) = (s(dk, "status"), s(dk, "smart_status"));
        let bad_status = !matches!(status.as_str(), "normal" | "" | "not_use");
        let bad_smart = !matches!(smart.as_str(), "normal" | "" | "safe");
        if bad_status || bad_smart {
            st.health = st.health.max(AppHealth::Warn);
            st.notes.push(format!("{}: {}", s(dk, "name"), if bad_status { status } else { format!("SMART {smart}") }));
        }
    }
    let model = s(i, "model");
    st.version = Some(s(i, "firmware_ver")).filter(|v| !v.is_empty()).map(|v| {
        if model.is_empty() {
            v
        } else {
            format!("{v} ({model})")
        }
    });
    st.more = cap_rows(&mut rows);
    st.rows_title = "Volumes".into();
    st.rows = rows;
    rows_health(&mut st);
    st
}

async fn synology(i: &Integration, password: &str) -> Result<AppStatus, String> {
    let c = Client::new(i, "Synology DSM");
    let fields = [
        ("api", "SYNO.API.Auth"),
        ("version", "6"),
        ("method", "login"),
        ("account", &i.username),
        ("passwd", password),
        ("session", "Nightgrid"),
        ("format", "sid"),
    ];
    let v: Value = c.check(c.post_form("/webapi/entry.cgi", &fields).await?)?.json()?;
    if v["success"].as_bool() != Some(true) {
        return Err(match count(&v["error"], "code") {
            400 => "DSM refused the login name or password".into(),
            401 => "DSM: this account is disabled".into(),
            402 => "DSM: this account may not use the API".into(),
            403..=406 => "DSM asks this account for a 2-step code; use an account without 2-step verification".into(),
            code => format!("DSM login failed (code {code})"),
        });
    }
    let sid = s(&v["data"], "sid");
    let call = |api: &str, version: &str, method: &str| {
        format!("/webapi/entry.cgi?api={api}&version={version}&method={method}&_sid={}", super::app::enc(&sid))
    };
    let info = c.get(&call("SYNO.Core.System", "1", "info")).await;
    let util = c.get(&call("SYNO.Core.System.Utilization", "1", "get")).await.ok();
    let storage = c.get(&call("SYNO.Storage.CGI.Storage", "1", "load_info")).await;
    let _ = c.get(&call("SYNO.API.Auth", "6", "logout")).await;
    let storage = storage?;
    if storage["success"].as_bool() == Some(false) {
        return Err("DSM didn't share storage details: the account must be an administrator".into());
    }
    Ok(build_synology(&info.unwrap_or(Value::Null), util.as_ref(), &storage))
}

// ---------- Proxmox Backup Server ----------

pub fn build_pbs(version: &Value, stores: &Value, tasks: &Value) -> AppStatus {
    let mut rows: Vec<AppRow> = stores["data"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|d| {
            let name = s(d, "store");
            let err = s(d, "error");
            let (used, total) = (n(d, "used").unwrap_or(0.0), n(d, "total").unwrap_or(0.0));
            let mut r = usage_row(&name, &name, used, total, "", err.is_empty());
            if !err.is_empty() {
                r.meta = err;
            } else if let Some(full) = n(d, "estimated-full-date").filter(|x| *x > 0.0) {
                let days = (full - crate::services::check::now_unix() as f64) / 86_400.0;
                if (0.0..3650.0).contains(&days) {
                    r.meta.push_str(&format!(" · full in ~{days:.0} days"));
                    if days < 30.0 {
                        r.state = r.state.max_warn();
                    }
                }
            }
            r
        })
        .collect();
    let failed: Vec<&Value> = tasks["data"]
        .as_array()
        .map(|a| a.iter().filter(|t| !s(t, "status").is_empty() && s(t, "status") != "OK").collect())
        .unwrap_or_default();
    let mut st = AppStatus::new(format!(
        "{} · {}",
        plural(rows.len() as u64, "datastore"),
        if failed.is_empty() {
            "no failed tasks".to_owned()
        } else {
            format!("{} failed today", plural(failed.len() as u64, "task"))
        }
    ))
    .tile("Datastores", rows.len())
    .tile("Failed tasks", failed.len());
    for t in failed.iter().take(4) {
        st = st.warn(format!("{} {}: {}", s(t, "worker_type"), s(t, "worker_id"), s(t, "status")));
    }
    let d = &version["data"];
    st.version = Some(s(d, "version")).filter(|v| !v.is_empty()).map(|v| match s(d, "release") {
        r if r.is_empty() => v,
        r => format!("{v}-{r}"),
    });
    st.more = cap_rows(&mut rows);
    st.rows_title = "Datastores".into();
    st.rows = rows;
    rows_health(&mut st);
    st
}

impl AppRowState {
    /// At least a warning.
    fn max_warn(self) -> Self {
        if self == AppRowState::Bad {
            self
        } else {
            AppRowState::Warn
        }
    }
}

pub async fn status(i: &Integration, secret: &str) -> Result<AppStatus, String> {
    match i.kind {
        IntegrationKind::Truenas => {
            let c = Client::new(i, "TrueNAS").header("Authorization", format!("Bearer {secret}"));
            let info = c.get("/api/v2.0/system/info").await?;
            let pools = c.get("/api/v2.0/pool").await?;
            let alerts = c.get("/api/v2.0/alert/list").await.unwrap_or(Value::Null);
            Ok(build_truenas(&info, &pools, &alerts))
        }
        IntegrationKind::Synology => synology(i, secret).await,
        _ => {
            if !i.username.contains('!') {
                return Err("enter the API token id as user@realm!tokenname".into());
            }
            let c = Client::new(i, "Proxmox Backup Server")
                .header("Authorization", format!("PBSAPIToken={}:{secret}", i.username));
            let version = c.get("/api2/json/version").await?;
            let stores = c.get("/api2/json/status/datastore-usage").await?;
            let since = crate::services::check::now_unix() - 86_400;
            let tasks = c
                .get(&format!("/api2/json/nodes/localhost/tasks?errors=1&limit=50&since={since}"))
                .await
                .unwrap_or(Value::Null);
            Ok(build_pbs(&version, &stores, &tasks))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::app::testing::{integration, serve};
    use serde_json::json;

    #[test]
    fn truenas() {
        let pools = json!([
            {"id": 1, "name": "tank", "status": "ONLINE", "healthy": true, "size": 1000.0, "allocated": 950.0},
            {"id": 2, "name": "fast", "status": "DEGRADED", "healthy": false, "size": 100.0, "allocated": 10.0}
        ]);
        let alerts = json!([
            {"level": "CRITICAL", "formatted": "Pool fast state is DEGRADED\nmore", "dismissed": false},
            {"level": "INFO", "formatted": "ignored", "dismissed": false},
            {"level": "WARNING", "formatted": "old", "dismissed": true}
        ]);
        let st = build_truenas(
            &json!({"version": "TrueNAS-SCALE-24.10.2", "uptime_seconds": 7500, "loadavg": [0.5]}),
            &pools,
            &alerts,
        );
        assert_eq!(st.summary, "2 pools · 1 alert");
        assert_eq!(st.rows[0].state, AppRowState::Warn);
        assert_eq!(st.rows[0].meta, "online · 950 B of 1000 B used (95 %)");
        assert_eq!(st.rows[1].state, AppRowState::Bad);
        assert_eq!(st.notes, ["Pool fast state is DEGRADED"]);
        assert_eq!(st.health, AppHealth::Bad);
    }

    #[test]
    fn synology_volumes() {
        let storage = json!({"success": true, "data": {
            "volumes": [{"id": "volume_1", "status": "normal", "size": {"total": "4000", "used": "1000"}}],
            "disks": [{"name": "Drive 1", "status": "normal", "smart_status": "normal"}, {"name": "Drive 2", "status": "normal", "smart_status": "failing"}]
        }});
        let st = build_synology(
            &json!({"data": {"model": "DS920+", "firmware_ver": "DSM 7.2.2-72806", "sys_temp": 41}}),
            Some(&json!({"data": {"cpu": {"user_load": 5, "system_load": 2}, "memory": {"real_usage": 31}}})),
            &storage,
        );
        assert_eq!(st.summary, "1 volume · 2 disks");
        assert_eq!(st.rows[0].title, "volume 1");
        assert_eq!(st.notes, ["Drive 2: SMART failing"]);
        assert_eq!(st.version.as_deref(), Some("DSM 7.2.2-72806 (DS920+)"));
        assert!(st.tiles.iter().any(|t| t.label == "CPU" && t.value == "7 %"));
    }

    #[test]
    fn pbs() {
        let full = crate::services::check::now_unix() + 10 * 86_400;
        let stores = json!({"data": [{"store": "main", "total": 1000, "used": 500, "estimated-full-date": full}, {"store": "usb", "error": "datastore is not mounted"}]});
        let tasks = json!({"data": [{"worker_type": "backup", "worker_id": "main:vm/100", "status": "connection reset"}, {"worker_type": "gc", "status": "OK"}]});
        let st = build_pbs(&json!({"data": {"version": "3.3", "release": "2"}}), &stores, &tasks);
        assert_eq!(st.summary, "2 datastores · 1 task failed today");
        assert!(st.rows[0].meta.ends_with("full in ~10 days"), "{}", st.rows[0].meta);
        assert_eq!(st.rows[0].state, AppRowState::Warn);
        assert_eq!((st.rows[1].meta.as_str(), st.rows[1].state), ("datastore is not mounted", AppRowState::Bad));
        assert_eq!(st.version.as_deref(), Some("3.3-2"));
        assert_eq!(st.notes, ["backup main:vm/100: connection reset"]);
    }

    #[tokio::test]
    async fn synology_login_and_logout() {
        let (url, seen) = serve(5, |m, p, _, body| match (m, p) {
            ("POST", "/webapi/entry.cgi") if body.contains("passwd=pw") => {
                (200, vec![], r#"{"success":true,"data":{"sid":"S1"}}"#.into())
            }
            ("POST", _) => (200, vec![], r#"{"success":false,"error":{"code":400}}"#.into()),
            (_, p) if !p.contains("_sid=S1") => (200, vec![], r#"{"success":false}"#.into()),
            (_, p) if p.contains("Storage") => {
                (200, vec![], r#"{"success":true,"data":{"volumes":[],"disks":[]}}"#.into())
            }
            _ => (200, vec![], r#"{"success":true,"data":{}}"#.into()),
        })
        .await;
        let mut i = integration(IntegrationKind::Synology, &url);
        i.username = "admin".into();
        let st = status(&i, "pw").await.unwrap();
        assert_eq!(st.summary, "0 volumes · 0 disks");
        let seen = seen.await.unwrap();
        assert!(seen[4].path.contains("method=logout"));
        let (url, _) = serve(1, |_, _, _, _| (200, vec![], r#"{"success":false,"error":{"code":403}}"#.into())).await;
        let mut i = integration(IntegrationKind::Synology, &url);
        i.username = "admin".into();
        assert!(status(&i, "pw").await.unwrap_err().contains("2-step"));
    }
}
