//! Storage systems: **TrueNAS** (REST API v2.0 with an API key), **Synology
//! DSM** (web API, login name and password; accounts with 2-step
//! verification sign in with a trusted-device token, D-199) and **Proxmox Backup Server**
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

/// Start of the error when DSM wants a 2-step code and no trusted device
/// token is saved. The UI looks for it to offer the code field.
pub const NEEDS_OTP: &str = "DSM asks for a 2-step code";

/// What Nightgrid calls itself in DSM's trusted-device list.
const DEVICE_NAME: &str = "Nightgrid";

struct DsmSession {
    sid: String,
    token: String,
    did: String,
}

/// Signs in to DSM (SYNO.API.Auth v6, DSM Login Web API Guide). `otp`
/// asks DSM to trust this device (enable_device_token); `device_id` is a
/// token from that earlier trust, so no code is needed.
async fn dsm_login(
    c: &Client<'_>,
    i: &Integration,
    password: &str,
    otp: Option<&str>,
    device_id: Option<&str>,
) -> Result<DsmSession, String> {
    let mut fields = vec![
        ("api", "SYNO.API.Auth"),
        ("version", "6"),
        ("method", "login"),
        ("account", i.username.as_str()),
        ("passwd", password),
        ("session", "Nightgrid"),
        ("format", "sid"),
        ("enable_syno_token", "yes"),
        ("device_name", DEVICE_NAME),
    ];
    if let Some(code) = otp {
        fields.push(("otp_code", code));
        fields.push(("enable_device_token", "yes"));
    }
    if let Some(d) = device_id.filter(|d| !d.is_empty()) {
        fields.push(("device_id", d));
    }
    let v: Value = c.check(c.post_form("/webapi/entry.cgi", &fields).await?)?.json()?;
    if v["success"].as_bool() != Some(true) {
        return Err(login_error(count(&v["error"], "code"), otp.is_some(), device_id.is_some_and(|d| !d.is_empty())));
    }
    let d = &v["data"];
    Ok(DsmSession { sid: s(d, "sid"), token: s(d, "synotoken"), did: s(d, "did") })
}

/// DSM's login error codes (DSM Login Web API Guide, SYNO.API.Auth).
pub fn login_error(code: u64, with_otp: bool, with_device: bool) -> String {
    match code {
        400 => "DSM refused the login name or password".into(),
        401 => "DSM: this account is disabled".into(),
        402 => "DSM denied this account: allow it the DSM app in Control Panel → Application Privileges".into(),
        403 if with_device => format!(
            "{NEEDS_OTP} again: DSM no longer trusts this device (the token expired or was removed). Enter a new code in the integration's settings"
        ),
        403 => format!(
            "{NEEDS_OTP}: enter one from your authenticator in the integration's settings, once. DSM then trusts Nightgrid and doesn't ask again"
        ),
        404 if with_otp => "DSM didn't accept that 2-step code. Codes change every 30 seconds; also check the clocks on this computer and the NAS".into(),
        404 => "DSM didn't accept the 2-step code".into(),
        406 => "DSM requires 2-step verification for this account, but it isn't set up yet: sign in to DSM once as this user to set it up, then enter a code here".into(),
        407 => "DSM blocked this address after failed sign-ins (Control Panel → Security → Protection)".into(),
        408..=410 => "DSM wants this account's password changed: sign in to DSM once as this user".into(),
        code => format!("DSM login failed (code {code})"),
    }
}

/// Signs in with a 2-step code and asks DSM to trust this device. Returns
/// the device token to keep in the keychain (empty when DSM gave none,
/// which happens when the account has no 2-step verification).
pub async fn synology_trust(i: &Integration, password: &str, otp: &str) -> Result<String, String> {
    let code: String = otp.chars().filter(|c| !c.is_whitespace()).collect();
    if code.is_empty() || code.len() > 12 || !code.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("enter the code from your authenticator app".into());
    }
    let c = Client::new(i, "Synology DSM");
    let sess = dsm_login(&c, i, password, Some(&code), None).await?;
    let _ = c.get(&dsm_call(&sess, "SYNO.API.Auth", "6", "logout")).await;
    Ok(sess.did)
}

fn dsm_call(sess: &DsmSession, api: &str, version: &str, method: &str) -> String {
    let mut url =
        format!("/webapi/entry.cgi?api={api}&version={version}&method={method}&_sid={}", super::app::enc(&sess.sid));
    if !sess.token.is_empty() {
        url.push_str(&format!("&SynoToken={}", super::app::enc(&sess.token)));
    }
    url
}

/// A DSM API answer: Ok(data) or Err(error code, 0 when the request failed).
fn dsm_result(r: Result<Value, String>) -> Result<Value, u64> {
    match r {
        Ok(v) if v["success"].as_bool() == Some(false) => Err(count(&v["error"], "code").max(1)),
        Ok(v) => Ok(v),
        Err(_) => Err(0),
    }
}

/// The parts of the card a non-admin account may not see, with what they are.
const PANELS: [(&str, &str); 3] =
    [("info", "Model, DSM version and temperature"), ("util", "CPU and memory"), ("storage", "Volumes and disks")];

async fn synology(i: &Integration, password: &str, device_id: Option<&str>) -> Result<AppStatus, String> {
    let c = Client::new(i, "Synology DSM");
    let sess = dsm_login(&c, i, password, None, device_id).await?;
    let info = dsm_result(c.get(&dsm_call(&sess, "SYNO.Core.System", "1", "info")).await);
    let util = dsm_result(c.get(&dsm_call(&sess, "SYNO.Core.System.Utilization", "1", "get")).await);
    let storage = dsm_result(c.get(&dsm_call(&sess, "SYNO.Storage.CGI.Storage", "1", "load_info")).await);
    let _ = c.get(&dsm_call(&sess, "SYNO.API.Auth", "6", "logout")).await;
    Ok(synology_card(info, util, storage))
}

/// The card from each panel's answer; panels DSM keeps from this account
/// (error 105, "no permission") are named instead of failing the card.
pub fn synology_card(info: Result<Value, u64>, util: Result<Value, u64>, storage: Result<Value, u64>) -> AppStatus {
    let results = [&info, &util, &storage];
    let denied: Vec<&str> =
        PANELS.iter().zip(results).filter(|(_, r)| matches!(r, Err(105))).map(|((_, what), _)| *what).collect();
    let failed: Vec<String> = PANELS
        .iter()
        .zip(results)
        .filter_map(|((_, what), r)| match r {
            Err(code) if *code != 105 => {
                Some(if *code == 0 { format!("{what}: no answer") } else { format!("{what}: DSM error {code}") })
            }
            _ => None,
        })
        .collect();
    let mut st = match &storage {
        Ok(v) => build_synology(info.as_ref().unwrap_or(&Value::Null), util.as_ref().ok(), v),
        Err(_) => {
            let mut st = build_synology(info.as_ref().unwrap_or(&Value::Null), util.as_ref().ok(), &Value::Null);
            st.summary = if denied.len() == PANELS.len() {
                "signed in · DSM shares no status with this account".into()
            } else {
                "signed in · volumes not shared with this account".into()
            };
            st.tiles.retain(|t| t.label != "Volumes" && t.label != "Disks");
            st.rows_title.clear();
            st
        }
    };
    if !denied.is_empty() {
        st.notes.push(format!(
            "Needs an administrator in DSM: {}. For these without admin, add an SNMP card (read-only)",
            denied.join(", ").to_lowercase()
        ));
        st.health = st.health.max(AppHealth::Warn);
    }
    for f in failed {
        st.notes.push(f);
        st.health = st.health.max(AppHealth::Warn);
    }
    st
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

/// `device_id`: DSM's trusted-device token, for a Synology account with
/// 2-step verification (ignored by the other kinds).
pub async fn status(i: &Integration, secret: &str, device_id: Option<&str>) -> Result<AppStatus, String> {
    match i.kind {
        IntegrationKind::Truenas => {
            let c = Client::new(i, "TrueNAS").header("Authorization", format!("Bearer {secret}"));
            let info = c.get("/api/v2.0/system/info").await?;
            let pools = c.get("/api/v2.0/pool").await?;
            let alerts = c.get("/api/v2.0/alert/list").await.unwrap_or(Value::Null);
            Ok(build_truenas(&info, &pools, &alerts))
        }
        IntegrationKind::Synology => synology(i, secret, device_id).await,
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
        let st = status(&i, "pw", None).await.unwrap();
        assert_eq!(st.summary, "0 volumes · 0 disks");
        let seen = seen.await.unwrap();
        assert!(seen[4].path.contains("method=logout"));
        let (url, _) = serve(1, |_, _, _, _| (200, vec![], r#"{"success":false,"error":{"code":403}}"#.into())).await;
        let mut i = integration(IntegrationKind::Synology, &url);
        i.username = "admin".into();
        assert!(status(&i, "pw", None).await.unwrap_err().starts_with(NEEDS_OTP));
    }

    #[tokio::test]
    async fn synology_two_step_trust_then_device_token() {
        // Trust: the code goes with enable_device_token and DSM hands back a did.
        let (url, seen) = serve(2, |m, _, _, body| match m {
            "POST" if body.contains("otp_code=123456") && body.contains("enable_device_token=yes") => {
                (200, vec![], r#"{"success":true,"data":{"sid":"S1","did":"DID-9","synotoken":"T1"}}"#.into())
            }
            "POST" => (200, vec![], r#"{"success":false,"error":{"code":404}}"#.into()),
            _ => (200, vec![], r#"{"success":true}"#.into()),
        })
        .await;
        let mut i = integration(IntegrationKind::Synology, &url);
        i.username = "nightgrid".into();
        assert_eq!(synology_trust(&i, "pw", " 123 456 ").await.unwrap(), "DID-9");
        let seen = seen.await.unwrap();
        assert!(seen[0].body.contains("device_name=Nightgrid"));
        assert!(seen[1].path.contains("method=logout") && seen[1].path.contains("SynoToken=T1"));
        assert!(synology_trust(&i, "pw", "abc!").await.is_err());

        // Later polls: device_id instead of a code, SynoToken on every call.
        let (url, seen) = serve(5, |m, p, _, body| match (m, p) {
            ("POST", _) if body.contains("device_id=DID-9") && !body.contains("otp_code") => {
                (200, vec![], r#"{"success":true,"data":{"sid":"S2","synotoken":"T2"}}"#.into())
            }
            ("POST", _) => (200, vec![], r#"{"success":false,"error":{"code":403}}"#.into()),
            (_, p) if !p.contains("SynoToken=T2") => (200, vec![], r#"{"success":false,"error":{"code":119}}"#.into()),
            (_, p) if p.contains("Storage") => {
                (200, vec![], r#"{"success":true,"data":{"volumes":[],"disks":[]}}"#.into())
            }
            _ => (200, vec![], r#"{"success":true,"data":{}}"#.into()),
        })
        .await;
        let mut i = integration(IntegrationKind::Synology, &url);
        i.username = "nightgrid".into();
        let st = status(&i, "pw", Some("DID-9")).await.unwrap();
        assert_eq!(st.summary, "0 volumes · 0 disks");
        assert!(st.notes.is_empty(), "{:?}", st.notes);
        seen.await.unwrap();

        // A token DSM dropped asks for a new code.
        let (url, _) = serve(1, |_, _, _, _| (200, vec![], r#"{"success":false,"error":{"code":403}}"#.into())).await;
        let mut i = integration(IntegrationKind::Synology, &url);
        i.username = "nightgrid".into();
        let e = status(&i, "pw", Some("OLD")).await.unwrap_err();
        assert!(e.starts_with(NEEDS_OTP) && e.contains("no longer trusts"), "{e}");
    }

    #[test]
    fn synology_non_admin_panels() {
        let denied = || Err(105);
        let st = synology_card(denied(), denied(), denied());
        assert_eq!(st.summary, "signed in · DSM shares no status with this account");
        assert_eq!(st.health, AppHealth::Warn);
        assert!(
            st.notes[0].contains("model, dsm version and temperature, cpu and memory, volumes and disks"),
            "{:?}",
            st.notes
        );
        assert!(st.tiles.is_empty());

        let util = Ok(json!({"data": {"cpu": {"user_load": 5, "system_load": 2}, "memory": {"real_usage": 31}}}));
        let st = synology_card(denied(), util, denied());
        assert_eq!(st.summary, "signed in · volumes not shared with this account");
        assert!(st.tiles.iter().any(|t| t.label == "CPU"));
        assert!(st.notes[0].contains("volumes and disks") && !st.notes[0].contains("cpu"));

        let st = synology_card(Ok(json!({})), Err(0), Ok(json!({"data": {"volumes": [], "disks": []}})));
        assert_eq!(st.notes, vec!["CPU and memory: no answer".to_string()]);
    }

    #[test]
    fn ui_knows_the_two_step_error() {
        let ts = include_str!("../../../ui/src/lib/stores/integrations.svelte.ts");
        assert!(ts.contains(&format!("DSM_NEEDS_OTP = \"{NEEDS_OTP}\"")));
    }

    #[test]
    fn synology_login_errors() {
        assert!(login_error(406, false, false).contains("isn't set up yet"));
        assert!(login_error(404, true, false).contains("30 seconds"));
        assert!(login_error(402, false, false).contains("Application Privileges"));
        assert!(login_error(409, false, false).contains("password changed"));
    }
}
