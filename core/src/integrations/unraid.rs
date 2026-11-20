//! Unraid (7.2+, or the Unraid Connect plugin's API): array state and
//! capacity, disks with temperatures and errors, Docker containers and VMs,
//! through the GraphQL API at `/graphql` with an API key (Settings →
//! Management Access → API Keys). Each part is its own query, so an older
//! API that lacks one still shows the rest.

use serde_json::{json, Value};

use super::app::{bytes, cap_rows, count, n, plural, s, AppHealth, AppRow, AppRowState, AppStatus, Client};
use super::Integration;

const Q_VERSION: &str = "{ info { versions { core { unraid } } } }";
const Q_VERSION_OLD: &str = "{ info { versions { unraid } } }";
const Q_ARRAY: &str = "{ array { state capacity { kilobytes { free used total } } \
    parities { name status temp numErrors } disks { name status temp numErrors fsSize fsUsed } \
    caches { name status temp numErrors fsSize fsUsed } } }";
const Q_DOCKER: &str = "{ docker { containers { names state } } }";
const Q_VMS: &str = "{ vms { domains { name state } } }";

async fn query(c: &Client<'_>, q: &str) -> Result<Value, String> {
    let v = c.post("/graphql", &json!({ "query": q })).await?;
    match v["errors"].as_array().and_then(|e| e.first()) {
        Some(e) if v["data"].is_null() => Err(format!("Unraid: {}", s(e, "message"))),
        _ => Ok(v["data"].clone()),
    }
}

fn disk_row(d: &Value) -> AppRow {
    let name = s(d, "name");
    let status = s(d, "status");
    let errors = count(d, "numErrors");
    let ok = status.is_empty() || status.starts_with("DISK_OK");
    let mut meta = vec![status.trim_start_matches("DISK_").to_lowercase()];
    if let Some(t) = n(d, "temp").filter(|t| *t > 0.0) {
        meta.push(format!("{t:.0} °C"));
    }
    if errors > 0 {
        meta.push(plural(errors, "error"));
    }
    // fs sizes are KiB.
    let (size, used) = (n(d, "fsSize").unwrap_or(0.0) * 1024.0, n(d, "fsUsed").unwrap_or(0.0) * 1024.0);
    let pct = (size > 0.0).then(|| used / size * 100.0);
    if size > 0.0 {
        meta.push(format!("{} of {}", bytes(used), bytes(size)));
    }
    meta.retain(|m| !m.is_empty());
    let state = if !ok {
        AppRowState::Bad
    } else if errors > 0 || n(d, "temp").unwrap_or(0.0) >= 55.0 {
        AppRowState::Warn
    } else {
        AppRowState::Ok
    };
    AppRow::new(&name, &name, meta.join(" · "), state).progress(pct)
}

pub fn build(version: &str, array: Option<&Value>, docker: Option<&Value>, vms: Option<&Value>) -> AppStatus {
    let a = array.map(|a| &a["array"]).unwrap_or(&Value::Null);
    let state = s(a, "state");
    let kb = &a["capacity"]["kilobytes"];
    let (used, total) = (n(kb, "used").unwrap_or(0.0) * 1024.0, n(kb, "total").unwrap_or(0.0) * 1024.0);
    let mut rows: Vec<AppRow> = ["parities", "disks", "caches"]
        .iter()
        .flat_map(|k| a[k].as_array().cloned().unwrap_or_default())
        .map(|d| disk_row(&d))
        .collect();
    let containers: Vec<Value> = docker.and_then(|d| d["docker"]["containers"].as_array().cloned()).unwrap_or_default();
    let running = containers.iter().filter(|c| s(c, "state").eq_ignore_ascii_case("running")).count();
    let domains: Vec<Value> = vms.and_then(|v| v["vms"]["domains"].as_array().cloned()).unwrap_or_default();
    let vms_up = domains.iter().filter(|d| s(d, "state").eq_ignore_ascii_case("running")).count();
    let mut summary = if state.is_empty() { "Reachable".to_owned() } else { format!("Array {}", state.to_lowercase()) };
    if total > 0.0 {
        summary.push_str(&format!(" · {} of {} used", bytes(used), bytes(total)));
    }
    let mut st = AppStatus::new(summary).tile("Disks", rows.len());
    if docker.is_some() {
        st = st.tile("Containers", format!("{running}/{}", containers.len()));
    }
    if vms.is_some() {
        st = st.tile("VMs", format!("{vms_up}/{}", domains.len()));
    }
    if !state.is_empty() && state != "STARTED" {
        st = st.warn(format!("The array is {}", state.to_lowercase()));
    }
    if rows.iter().any(|r| r.state == AppRowState::Bad) {
        st.health = AppHealth::Bad;
    } else if rows.iter().any(|r| r.state == AppRowState::Warn) {
        st.health = st.health.max(AppHealth::Warn);
    }
    if array.is_none() {
        st.notes.push("This API key can't read the array".into());
    }
    st.version = Some(version.to_owned()).filter(|v| !v.is_empty());
    st.more = cap_rows(&mut rows);
    st.rows_title = "Disks".into();
    st.rows = rows;
    st
}

pub async fn status(i: &Integration, key: &str) -> Result<AppStatus, String> {
    let c = Client::new(i, "Unraid").header("x-api-key", key);
    let version = match query(&c, Q_VERSION).await {
        Ok(v) => s(&v["info"]["versions"]["core"], "unraid"),
        Err(e) if e.contains("refused") || e.contains("HTTP") => return Err(e),
        Err(_) => query(&c, Q_VERSION_OLD).await.map(|v| s(&v["info"]["versions"], "unraid")).unwrap_or_default(),
    };
    let array = query(&c, Q_ARRAY).await.ok();
    let docker = query(&c, Q_DOCKER).await.ok();
    let vms = query(&c, Q_VMS).await.ok();
    Ok(build(&version, array.as_ref(), docker.as_ref(), vms.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::app::testing::{integration, serve};
    use crate::integrations::IntegrationKind;

    #[test]
    fn array() {
        let a = json!({"array": {
            "state": "STARTED",
            "capacity": {"kilobytes": {"free": "1000", "used": "3000", "total": "4000"}},
            "parities": [{"name": "parity", "status": "DISK_OK", "temp": 34}],
            "disks": [
                {"name": "disk1", "status": "DISK_OK", "temp": 36, "numErrors": 0, "fsSize": 1048576, "fsUsed": 524288},
                {"name": "disk2", "status": "DISK_DSBL", "temp": 0, "numErrors": 12}
            ],
            "caches": [{"name": "cache", "status": "DISK_OK", "temp": 58}]
        }});
        let d = json!({"docker": {"containers": [{"names": ["/plex"], "state": "RUNNING"}, {"names": ["/x"], "state": "EXITED"}]}});
        let st = build("7.2.0", Some(&a), Some(&d), None);
        assert_eq!(st.summary, "Array started · 2.9 MB of 3.9 MB used");
        assert_eq!(st.rows.len(), 4);
        assert_eq!(st.rows[1].meta, "ok · 36 °C · 512 MB of 1.0 GB");
        assert_eq!(st.rows[2].state, AppRowState::Bad);
        assert_eq!(st.rows[2].meta, "dsbl · 12 errors");
        assert_eq!(st.rows[3].state, AppRowState::Warn);
        assert_eq!(st.health, AppHealth::Bad);
        assert!(st.tiles.iter().any(|t| t.label == "Containers" && t.value == "1/2"));
        assert!(st.tiles.iter().all(|t| t.label != "VMs"));
    }

    #[tokio::test]
    async fn partial_api() {
        let (url, seen) = serve(5, |_, _, _, body| {
            if body.contains("core") {
                (200, vec![], r#"{"errors":[{"message":"Cannot query field core"}],"data":null}"#.into())
            } else if body.contains("versions") {
                (200, vec![], r#"{"data":{"info":{"versions":{"unraid":"6.12.13"}}}}"#.into())
            } else if body.contains("array") {
                (200, vec![], r#"{"data":{"array":{"state":"STOPPED"}}}"#.into())
            } else {
                (200, vec![], r#"{"errors":[{"message":"Forbidden"}],"data":null}"#.into())
            }
        })
        .await;
        let st = status(&integration(IntegrationKind::Unraid, &url), "k").await.unwrap();
        assert_eq!(st.version.as_deref(), Some("6.12.13"));
        assert_eq!(st.notes, ["The array is stopped"]);
        assert!(seen.await.unwrap().iter().all(|r| r.head.to_ascii_lowercase().contains("x-api-key: k")));
    }
}
