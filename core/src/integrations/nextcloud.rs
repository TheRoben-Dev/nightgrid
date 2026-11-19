//! Nextcloud: version, users, files, active users, free space, app updates
//! and server load, from the serverinfo app
//! (`/ocs/v2.php/apps/serverinfo/api/v1/info`). Needs an admin login name and
//! an app password (Personal settings → Security → Devices & sessions).

use serde_json::Value;

use super::app::{basic_auth, bytes, count, n, plural, s, AppHealth, AppStatus, Client};
use super::Integration;

pub fn build(v: &Value) -> AppStatus {
    let d = &v["ocs"]["data"];
    let sys = &d["nextcloud"]["system"];
    let storage = &d["nextcloud"]["storage"];
    let active = &d["activeUsers"];
    let users = count(storage, "num_users");
    let mut st =
        AppStatus::new(format!("{} · {} active in the last hour", plural(users, "user"), count(active, "last1hour")))
            .tile("Users", users)
            .tile("Files", count(storage, "num_files"))
            .tile("Active 24 h", count(active, "last24hours"))
            .tile("Shares", count(&d["nextcloud"]["shares"], "num_shares"));
    if let Some(free) = n(sys, "freespace") {
        st = st.tile("Free space", bytes(free));
    }
    if let Some(load) = sys["cpuload"].as_array().and_then(|a| a.first()).and_then(Value::as_f64) {
        st = st.tile("Load", format!("{load:.2}"));
    }
    let updates = count(&sys["apps"], "num_updates_available");
    if updates > 0 {
        st = st.warn(format!("{} available", plural(updates, "app update")));
    }
    if let (Some(total), Some(free)) = (n(sys, "mem_total"), n(sys, "mem_free")) {
        if total > 0.0 && free / total < 0.05 {
            st.health = st.health.max(AppHealth::Warn);
            st.notes.push("Under 5 % memory free".into());
        }
    }
    st.version = Some(s(sys, "version")).filter(|x| !x.is_empty());
    st
}

pub async fn status(i: &Integration, password: &str) -> Result<AppStatus, String> {
    let c = Client::new(i, "Nextcloud")
        .header("OCS-APIRequest", "true")
        .header("Authorization", basic_auth(&i.username, password));
    let v = c.get("/ocs/v2.php/apps/serverinfo/api/v1/info?format=json&skipApps=false&skipUpdate=false").await?;
    let meta = &v["ocs"]["meta"];
    if !meta.is_null() && count(meta, "statuscode") != 200 {
        return Err(format!("Nextcloud: {} (the login must be an admin)", s(meta, "message")));
    }
    Ok(build(&v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serverinfo() {
        let v = json!({"ocs": {"meta": {"statuscode": 200}, "data": {
            "nextcloud": {
                "system": {"version": "30.0.4.1", "freespace": 536870912000u64, "cpuload": [0.42, 0.3, 0.2], "mem_total": 8000000, "mem_free": 200000, "apps": {"num_installed": 50, "num_updates_available": 2}},
                "storage": {"num_users": 4, "num_files": 120345},
                "shares": {"num_shares": 17}
            },
            "activeUsers": {"last5minutes": 1, "last1hour": 2, "last24hours": 3}
        }}});
        let st = build(&v);
        assert_eq!(st.summary, "4 users · 2 active in the last hour");
        assert_eq!(st.version.as_deref(), Some("30.0.4.1"));
        assert_eq!(st.notes, ["2 app updates available", "Under 5 % memory free"]);
        assert!(st.tiles.iter().any(|t| t.label == "Free space" && t.value == "500 GB"));
    }
}
