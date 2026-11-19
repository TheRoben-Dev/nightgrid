//! Monitoring stacks: **Grafana** (health, dashboards, firing and pending
//! alert rules, with a service account token) and **Prometheus** (scrape
//! targets up and down, firing alerts; login optional: Basic auth with a
//! login name, else a bearer token, else none).

use serde_json::Value;

use super::app::{basic_auth, cap_rows, plural, s, AppHealth, AppRow, AppRowState, AppStatus, Client};
use super::{Integration, IntegrationKind};

fn alert_state(state: &str) -> Option<AppRowState> {
    match state.to_ascii_lowercase().as_str() {
        "firing" | "alerting" => Some(AppRowState::Bad),
        "pending" => Some(AppRowState::Warn),
        _ => None,
    }
}

pub fn build_grafana(health: &Value, dashboards: &Value, rules: Option<&Value>) -> AppStatus {
    let mut rows = Vec::new();
    for g in rules.and_then(|r| r["data"]["groups"].as_array()).map(|a| a.as_slice()).unwrap_or_default() {
        for r in g["rules"].as_array().map(|a| a.as_slice()).unwrap_or_default() {
            let Some(st) = alert_state(&s(r, "state")) else { continue };
            let active = r["alerts"].as_array().map(Vec::len).unwrap_or(0);
            let meta = [
                s(g, "folder"),
                s(g, "name"),
                s(r, "state"),
                if active > 1 { format!("{active} instances") } else { String::new() },
            ]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
            rows.push(AppRow::new(s(r, "uid"), s(r, "name"), meta, st));
        }
    }
    rows.sort_by_key(|r| r.state != AppRowState::Bad);
    let firing = rows.iter().filter(|r| r.state == AppRowState::Bad).count() as u64;
    let pending = rows.len() as u64 - firing;
    let dash = dashboards.as_array().map(Vec::len).unwrap_or(0);
    let mut st = AppStatus::new(match (rules.is_some(), firing) {
        (false, _) => plural(dash as u64, "dashboard"),
        (true, 0) => format!("No alerts firing · {}", plural(dash as u64, "dashboard")),
        (true, f) => format!("{} firing · {}", plural(f, "alert"), plural(dash as u64, "dashboard")),
    })
    .tile("Dashboards", dash);
    if rules.is_some() {
        st = st.tile("Firing", firing).tile("Pending", pending);
    }
    if s(health, "database") != "ok" && !s(health, "database").is_empty() {
        st = st.warn(format!("Grafana's database reports {}", s(health, "database")));
    }
    if firing > 0 {
        st.health = AppHealth::Bad;
    } else if pending > 0 {
        st.health = st.health.max(AppHealth::Warn);
    }
    st.version = Some(s(health, "version")).filter(|v| !v.is_empty());
    st.more = cap_rows(&mut rows);
    st.rows_title = "Alerts".into();
    st.rows = rows;
    st
}

pub fn build_prometheus(build: &Value, targets: &Value, alerts: &Value) -> AppStatus {
    let list = targets["data"]["activeTargets"].as_array().map(|a| a.as_slice()).unwrap_or_default();
    let down: Vec<&Value> = list.iter().filter(|t| s(t, "health") != "up").collect();
    let mut rows: Vec<AppRow> = alerts["data"]["alerts"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter_map(|a| {
            let st = alert_state(&s(a, "state"))?;
            let l = &a["labels"];
            let summary = s(&a["annotations"], "summary");
            let meta = [s(l, "severity"), s(l, "instance"), summary]
                .into_iter()
                .filter(|x| !x.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            Some(AppRow::new(format!("{}:{}", s(l, "alertname"), s(l, "instance")), s(l, "alertname"), meta, st))
        })
        .collect();
    for t in &down {
        let l = &t["labels"];
        let meta =
            [s(t, "health"), s(t, "lastError")].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ");
        rows.push(AppRow::new(
            s(t, "scrapeUrl"),
            format!("{} {}", s(l, "job"), s(l, "instance")),
            meta,
            AppRowState::Bad,
        ));
    }
    rows.sort_by_key(|r| r.state != AppRowState::Bad);
    let firing = rows.iter().filter(|r| r.state == AppRowState::Bad && !r.id.contains("://")).count();
    let mut st = AppStatus::new(format!(
        "{}/{} targets up · {}",
        list.len() - down.len(),
        list.len(),
        match firing {
            0 => "no alerts firing".to_owned(),
            f => format!("{} firing", plural(f as u64, "alert")),
        }
    ))
    .tile("Targets", list.len())
    .tile("Down", down.len())
    .tile("Firing", firing);
    if !rows.is_empty() {
        st.health = if rows.iter().any(|r| r.state == AppRowState::Bad) { AppHealth::Bad } else { AppHealth::Warn };
    }
    st.version = Some(s(&build["data"], "version")).filter(|v| !v.is_empty());
    st.more = cap_rows(&mut rows);
    st.rows_title = "Alerts and down targets".into();
    st.rows = rows;
    st
}

pub async fn status(i: &Integration, secret: &str) -> Result<AppStatus, String> {
    if i.kind == IntegrationKind::Grafana {
        let c = Client::new(i, "Grafana").header("Authorization", format!("Bearer {secret}"));
        let health = c.get("/api/health").await?;
        let dashboards = c.get("/api/search?type=dash-db&limit=5000").await?;
        let rules = c.get("/api/prometheus/grafana/api/v1/rules").await.ok();
        return Ok(build_grafana(&health, &dashboards, rules.as_ref()));
    }
    let mut c = Client::new(i, "Prometheus");
    if !i.username.is_empty() {
        c = c.header("Authorization", basic_auth(&i.username, secret));
    } else if !secret.is_empty() {
        c = c.header("Authorization", format!("Bearer {secret}"));
    }
    let build = c.get("/api/v1/status/buildinfo").await.unwrap_or(Value::Null);
    let targets = c.get("/api/v1/targets?state=active").await?;
    let alerts = c.get("/api/v1/alerts").await.unwrap_or(Value::Null);
    Ok(build_prometheus(&build, &targets, &alerts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn grafana() {
        let rules = json!({"data": {"groups": [{"name": "disks", "folder": "Infra", "rules": [
            {"uid": "a", "name": "Disk almost full", "state": "firing", "alerts": [{}, {}]},
            {"uid": "b", "name": "High load", "state": "pending", "alerts": [{}]},
            {"uid": "c", "name": "Quiet", "state": "inactive"}
        ]}]}});
        let st = build_grafana(&json!({"version": "11.4.0", "database": "ok"}), &json!([{}, {}, {}]), Some(&rules));
        assert_eq!(st.summary, "1 alert firing · 3 dashboards");
        assert_eq!(st.rows[0].meta, "Infra · disks · firing · 2 instances");
        assert_eq!(st.rows.len(), 2);
        assert_eq!(st.health, AppHealth::Bad);
        let no_rules = build_grafana(&json!({}), &json!([]), None);
        assert_eq!(no_rules.summary, "0 dashboards");
    }

    #[test]
    fn prometheus() {
        let targets = json!({"data": {"activeTargets": [
            {"labels": {"job": "node", "instance": "server01:9100"}, "health": "up", "scrapeUrl": "http://server01:9100/metrics"},
            {"labels": {"job": "node", "instance": "server02:9100"}, "health": "down", "lastError": "connection refused", "scrapeUrl": "http://server02:9100/metrics"}
        ]}});
        let alerts = json!({"data": {"alerts": [
            {"labels": {"alertname": "InstanceDown", "instance": "server02:9100", "severity": "critical"}, "annotations": {"summary": "server02 is down"}, "state": "firing"}
        ]}});
        let st = build_prometheus(&json!({"data": {"version": "3.1.0"}}), &targets, &alerts);
        assert_eq!(st.summary, "1/2 targets up · 1 alert firing");
        assert_eq!(st.rows[0].title, "InstanceDown");
        assert_eq!(st.rows[0].meta, "critical · server02:9100 · server02 is down");
        assert_eq!(st.rows[1].title, "node server02:9100");
        assert_eq!(st.health, AppHealth::Bad);
    }
}
