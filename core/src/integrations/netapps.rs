//! Network gear and proxies: **OPNsense** (API key and secret), **UniFi
//! Network** (the official integration API with an API key) and **Nginx
//! Proxy Manager** (its login). pfSense has no built-in REST API, so it isn't
//! here.

use serde_json::{json, Value};

use super::app::{
    basic_auth, cap_rows, count, plural, s, valid_target, AppAction, AppHealth, AppRow, AppRowState, AppStatus, Client,
};
use super::{Integration, IntegrationKind};

// ---------- OPNsense ----------

fn opnsense<'a>(i: &'a Integration, secret: &str) -> Client<'a> {
    Client::new(i, "OPNsense").header("Authorization", basic_auth(&i.username, secret))
}

pub fn build_opnsense(firmware: Option<&Value>, gateways: &Value, services: &Value) -> AppStatus {
    let mut rows: Vec<AppRow> = gateways["items"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|g| {
            let status = s(g, "status");
            let state = match status.as_str() {
                "none" | "online" => AppRowState::Ok,
                "loss" | "delay" | "delay+loss" => AppRowState::Warn,
                _ => AppRowState::Bad,
            };
            let label = Some(s(g, "status_translated")).filter(|x| !x.is_empty()).unwrap_or(status);
            let meta = [label, s(g, "address"), s(g, "delay"), s(g, "loss")]
                .into_iter()
                .filter(|x| !x.is_empty() && x != "~")
                .collect::<Vec<_>>()
                .join(" · ");
            AppRow::new(s(g, "name"), format!("Gateway {}", s(g, "name")), meta, state)
        })
        .collect();
    let svc: Vec<&Value> = services["rows"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    let stopped: Vec<&&Value> = svc.iter().filter(|x| count(x, "running") == 0 && count(x, "locked") == 0).collect();
    for x in &stopped {
        let id = s(x, "id");
        let name = Some(s(x, "description")).filter(|d| !d.is_empty()).unwrap_or_else(|| s(x, "name"));
        let mut r = AppRow::new(&id, &name, "service stopped", AppRowState::Warn);
        if valid_target(&id) {
            r = r.action(AppAction::new("start", "Start"));
        }
        rows.push(r);
    }
    let gw_down = rows.iter().filter(|r| r.title.starts_with("Gateway ") && r.state == AppRowState::Bad).count();
    let mut st = AppStatus::new(format!(
        "{}{} · {}/{} services running",
        plural(rows.len().saturating_sub(stopped.len()) as u64, "gateway"),
        if gw_down > 0 { format!(" ({gw_down} down)") } else { String::new() },
        svc.len() - stopped.len(),
        svc.len()
    ));
    let gateways_n = rows.len() - stopped.len();
    st = st.tile("Gateways", gateways_n).tile("Services stopped", stopped.len());
    if let Some(f) = firmware {
        st.version = Some(s(f, "product_version")).filter(|v| !v.is_empty());
        let latest = s(f, "product_latest");
        if !latest.is_empty() && Some(&latest) != st.version.as_ref() {
            st = st.warn(format!("Firmware {latest} is available"));
        }
    }
    if gw_down > 0 {
        st.health = AppHealth::Bad;
    } else if rows.iter().any(|r| r.state == AppRowState::Warn) {
        st.health = st.health.max(AppHealth::Warn);
    }
    st.more = cap_rows(&mut rows);
    st.rows_title = "Gateways and stopped services".into();
    st.rows = rows;
    st
}

// ---------- UniFi ----------

/// UniFi OS consoles serve the Network app under `/proxy/network`; a
/// self-hosted Network server serves it at the root.
async fn unifi_root(c: &Client<'_>) -> Result<(&'static str, Value), String> {
    for root in ["/proxy/network/integration/v1", "/integration/v1"] {
        if let Some(v) = c.get_opt(&format!("{root}/info")).await? {
            return Ok((root, v));
        }
    }
    Err("UniFi Network's API wasn't found at this URL (needs Network 9.0 or newer)".into())
}

pub fn build_unifi(info: &Value, site: &str, devices: &Value, clients: &Value) -> AppStatus {
    let mut rows: Vec<AppRow> = devices["data"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|d| {
            let state = s(d, "state");
            let rs = match state.as_str() {
                "ONLINE" => AppRowState::Ok,
                "OFFLINE" | "DISCONNECTED" | "ISOLATED" => AppRowState::Bad,
                "PENDING_ADOPTION" | "UPDATING" | "GETTING_READY" | "ADOPTING" | "PROVISIONING" => AppRowState::Active,
                _ => AppRowState::Warn,
            };
            let name = Some(s(d, "name")).filter(|x| !x.is_empty()).unwrap_or_else(|| s(d, "model"));
            let meta = [state.to_lowercase().replace('_', " "), s(d, "model"), s(d, "ipAddress")]
                .into_iter()
                .filter(|x| !x.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            let mut r = AppRow::new(s(d, "id"), &name, meta, rs);
            if rs == AppRowState::Ok {
                r = r.action(
                    AppAction::new("restart", "Restart")
                        .confirm(format!("Restart {name}? Clients on it drop until it's back.")),
                );
            }
            r
        })
        .collect();
    rows.sort_by_key(|r| r.state != AppRowState::Bad);
    let total = rows.len();
    let offline = rows.iter().filter(|r| r.state == AppRowState::Bad).count();
    let clients_n = count(clients, "totalCount");
    let mut st = AppStatus::new(format!(
        "{} online · {}",
        format_args!("{}/{} devices", total - offline, total),
        plural(clients_n, "client")
    ))
    .tile("Devices", total)
    .tile("Offline", offline)
    .tile("Clients", clients_n)
    .tile("Site", site);
    if offline > 0 {
        st.health = AppHealth::Warn;
    }
    st.version = Some(s(info, "applicationVersion")).filter(|v| !v.is_empty());
    st.more = cap_rows(&mut rows);
    st.rows_title = "Devices".into();
    st.rows = rows;
    st
}

async fn unifi<'a>(i: &'a Integration, key: &str) -> Result<(Client<'a>, &'static str, String, Value), String> {
    let c = Client::new(i, "UniFi").header("X-API-KEY", key);
    let (root, info) = unifi_root(&c).await?;
    let sites = c.get(&format!("{root}/sites?limit=200")).await?;
    let list = sites["data"].as_array().cloned().unwrap_or_default();
    let want = i.slug.trim();
    let site = list
        .iter()
        .find(|x| {
            want.is_empty()
                || s(x, "name").eq_ignore_ascii_case(want)
                || s(x, "internalReference") == want
                || s(x, "id") == want
        })
        .ok_or_else(|| {
            if want.is_empty() {
                "UniFi has no sites".to_owned()
            } else {
                format!("UniFi has no site named {want}")
            }
        })?;
    Ok((c, root, s(site, "id"), json!({"info": info, "name": s(site, "name")})))
}

// ---------- Nginx Proxy Manager ----------

async fn npm<'a>(i: &'a Integration, password: &str) -> Result<Client<'a>, String> {
    let c = Client::new(i, "Nginx Proxy Manager");
    let r = c
        .send(crate::http::Method::Post, "/api/tokens", Some(&json!({"identity": i.username, "secret": password})))
        .await?;
    if r.status == 401 || r.status == 400 {
        return Err("Nginx Proxy Manager refused the email or password".into());
    }
    let v: Value = c.check(r)?.json()?;
    let token = s(&v, "token");
    if token.is_empty() {
        return Err("Nginx Proxy Manager didn't return a token (2FA accounts can't sign in from here)".into());
    }
    Ok(c.header("Authorization", format!("Bearer {token}")))
}

pub fn build_npm(hosts: &Value, certs: &Value, now: i64) -> AppStatus {
    let mut rows: Vec<AppRow> = hosts
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|h| {
            let domains: Vec<String> = h["domain_names"]
                .as_array()
                .map(|d| d.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                .unwrap_or_default();
            let title = domains.first().cloned().unwrap_or_else(|| format!("Host {}", s(h, "id")));
            let enabled = h["enabled"].as_bool().unwrap_or(count(h, "enabled") == 1);
            let online = h["meta"]["nginx_online"].as_bool().unwrap_or(true);
            let mut meta =
                vec![format!("→ {}://{}:{}", s(h, "forward_scheme"), s(h, "forward_host"), s(h, "forward_port"))];
            if domains.len() > 1 {
                meta.push(format!("+{} more", domains.len() - 1));
            }
            if count(h, "certificate_id") > 0 {
                meta.push("TLS".into());
            }
            let state = if !enabled {
                meta.push("disabled".into());
                AppRowState::Idle
            } else if !online {
                meta.push(
                    Some(s(&h["meta"], "nginx_err")).filter(|e| !e.is_empty()).unwrap_or_else(|| "nginx error".into()),
                );
                AppRowState::Bad
            } else {
                AppRowState::Ok
            };
            let action = if enabled {
                AppAction::new("disable", "Disable")
                    .confirm(format!("Disable the proxy host {title}? It stops answering until enabled."))
            } else {
                AppAction::new("enable", "Enable")
            };
            AppRow::new(s(h, "id"), title, meta.join(" · "), state).action(action)
        })
        .collect();
    rows.sort_by_key(|r| r.state != AppRowState::Bad);
    let total = rows.len();
    let mut st = AppStatus::new(format!(
        "{} · {} certificates",
        plural(total as u64, "proxy host"),
        certs.as_array().map(Vec::len).unwrap_or(0)
    ))
    .tile("Proxy hosts", total)
    .tile("Certificates", certs.as_array().map(Vec::len).unwrap_or(0));
    for c in certs.as_array().map(|a| a.as_slice()).unwrap_or_default() {
        let exp = s(c, "expires_on");
        let Some(at) = crate::expiry::parse_date(&exp) else {
            continue;
        };
        let days = (at - now).div_euclid(86_400);
        if days < 14 {
            let name = Some(s(c, "nice_name")).filter(|x| !x.is_empty()).unwrap_or_else(|| s(c, "id"));
            let level = if days < 3 { AppHealth::Bad } else { AppHealth::Warn };
            st.health = st.health.max(level);
            st.notes.push(if days < 0 {
                format!("Certificate {name} expired")
            } else {
                format!("Certificate {name} expires in {days} days")
            });
        }
    }
    if rows.iter().any(|r| r.state == AppRowState::Bad) {
        st.health = AppHealth::Bad;
    }
    st.more = cap_rows(&mut rows);
    st.rows_title = "Proxy hosts".into();
    st.rows = rows;
    st
}

pub async fn status(i: &Integration, secret: &str) -> Result<AppStatus, String> {
    match i.kind {
        IntegrationKind::Opnsense => {
            let c = opnsense(i, secret);
            let gateways = c.get("/api/routes/gateway/status").await?;
            let services = c.get("/api/core/service/search").await.unwrap_or(Value::Null);
            let firmware = c.get_opt("/api/core/firmware/info").await.ok().flatten();
            Ok(build_opnsense(firmware.as_ref(), &gateways, &services))
        }
        IntegrationKind::Unifi => {
            let (c, root, site, extra) = unifi(i, secret).await?;
            let devices = c.get(&format!("{root}/sites/{site}/devices?limit=200")).await?;
            let clients = c.get(&format!("{root}/sites/{site}/clients?limit=1")).await.unwrap_or(Value::Null);
            Ok(build_unifi(&extra["info"], &s(&extra, "name"), &devices, &clients))
        }
        _ => {
            let c = npm(i, secret).await?;
            let hosts = c.get("/api/nginx/proxy-hosts").await?;
            let certs = c.get("/api/nginx/certificates").await.unwrap_or(Value::Null);
            Ok(build_npm(&hosts, &certs, crate::services::check::now_unix()))
        }
    }
}

pub async fn act(i: &Integration, secret: &str, target: &str, action: &str) -> Result<(), String> {
    if target.is_empty() {
        return Err("pick an item".into());
    }
    match (i.kind, action) {
        (IntegrationKind::Opnsense, "start") => {
            let v = opnsense(i, secret).post(&format!("/api/core/service/start/{target}"), &json!({})).await?;
            match s(&v, "result").as_str() {
                "" | "ok" => Ok(()),
                r => Err(format!("OPNsense: {r}")),
            }
        }
        (IntegrationKind::Unifi, "restart") => {
            let (c, root, site, _) = unifi(i, secret).await?;
            c.post(&format!("{root}/sites/{site}/devices/{target}/actions"), &json!({"action": "RESTART"}))
                .await
                .map(drop)
        }
        (IntegrationKind::Npm, "enable" | "disable") if target.chars().all(|c| c.is_ascii_digit()) => {
            npm(i, secret).await?.post(&format!("/api/nginx/proxy-hosts/{target}/{action}"), &json!({})).await.map(drop)
        }
        _ => Err(format!("unknown action {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::app::testing::{integration, serve};

    #[test]
    fn opnsense() {
        let gw = json!({"items": [
            {"name": "WAN_DHCP", "address": "203.0.113.1", "status": "none", "status_translated": "Online", "delay": "1.2 ms", "loss": "0.0 %"},
            {"name": "WAN2", "address": "~", "status": "down", "status_translated": "Offline"}
        ]});
        let svc = json!({"rows": [{"id": "unbound", "name": "unbound", "description": "Unbound DNS", "running": 1, "locked": 0}, {"id": "openvpn/1", "name": "openvpn", "description": "OpenVPN server", "running": 0, "locked": 0}]});
        let st = build_opnsense(Some(&json!({"product_version": "25.1.3", "product_latest": "25.1.4"})), &gw, &svc);
        assert_eq!(st.summary, "2 gateways (1 down) · 1/2 services running");
        assert_eq!(st.rows[0].meta, "Online · 203.0.113.1 · 1.2 ms · 0.0 %");
        assert_eq!(st.rows[1].state, AppRowState::Bad);
        assert_eq!(st.rows[2].title, "OpenVPN server");
        assert!(st.rows[2].actions.is_empty(), "ids with / aren't targets");
        assert_eq!(st.notes, ["Firmware 25.1.4 is available"]);
        assert_eq!(st.health, AppHealth::Bad);
    }

    #[test]
    fn unifi_devices() {
        let d = json!({"data": [
            {"id": "11111111-aaaa", "name": "Office AP", "model": "U7 Pro", "state": "ONLINE", "ipAddress": "192.168.1.20"},
            {"id": "22222222-bbbb", "name": "", "model": "USW Lite 8", "state": "OFFLINE"}
        ]});
        let st = build_unifi(&json!({"applicationVersion": "9.0.114"}), "Default", &d, &json!({"totalCount": 31}));
        assert_eq!(st.summary, "1/2 devices online · 31 clients");
        assert_eq!(st.rows[0].title, "USW Lite 8");
        assert!(st.rows[0].actions.is_empty());
        assert_eq!(st.rows[1].actions[0].id, "restart");
        assert_eq!(st.health, AppHealth::Warn);
    }

    #[test]
    fn npm_hosts() {
        let hosts = json!([
            {"id": 1, "domain_names": ["app.example.com", "www.example.com"], "forward_scheme": "http", "forward_host": "10.0.0.5", "forward_port": 8080, "enabled": 1, "certificate_id": 3, "meta": {"nginx_online": true}},
            {"id": 2, "domain_names": ["old.example.com"], "forward_scheme": "http", "forward_host": "10.0.0.6", "forward_port": 80, "enabled": 0, "certificate_id": 0, "meta": {}},
            {"id": 3, "domain_names": ["bad.example.com"], "forward_scheme": "https", "forward_host": "10.0.0.7", "forward_port": 443, "enabled": true, "meta": {"nginx_online": false, "nginx_err": "host not found in upstream"}}
        ]);
        let now = crate::expiry::parse_date("2026-11-19").unwrap();
        let certs = json!([{"id": 3, "nice_name": "example.com", "expires_on": "2026-11-25 10:00:00"}, {"id": 4, "nice_name": "fine", "expires_on": "2027-02-01 00:00:00"}]);
        let st = build_npm(&hosts, &certs, now);
        assert_eq!(st.summary, "3 proxy hosts · 2 certificates");
        assert_eq!(st.rows[0].title, "bad.example.com");
        assert_eq!(st.rows[0].meta, "→ https://10.0.0.7:443 · host not found in upstream");
        assert_eq!(st.rows[1].meta, "→ http://10.0.0.5:8080 · +1 more · TLS");
        assert_eq!(st.rows[2].actions[0].id, "enable");
        assert_eq!(st.notes, ["Certificate example.com expires in 6 days"]);
        assert_eq!(st.health, AppHealth::Bad);
    }

    #[tokio::test]
    async fn unifi_finds_root_and_site() {
        let (url, seen) = serve(5, |_, p, _, _| match p {
            "/proxy/network/integration/v1/info" => (404, vec![], String::new()),
            "/integration/v1/info" => (200, vec![], r#"{"applicationVersion":"9.1"}"#.into()),
            p if p.starts_with("/integration/v1/sites?") => {
                (200, vec![], r#"{"data":[{"id":"s-1","name":"Default"},{"id":"s-2","name":"Lab"}]}"#.into())
            }
            p if p.starts_with("/integration/v1/sites/s-2/devices") => (200, vec![], r#"{"data":[]}"#.into()),
            _ => (200, vec![], r#"{"totalCount":4}"#.into()),
        })
        .await;
        let mut i = integration(IntegrationKind::Unifi, &url);
        i.slug = "lab".into();
        let st = status(&i, "k").await.unwrap();
        assert_eq!(st.summary, "0/0 devices online · 4 clients");
        assert!(st.tiles.iter().any(|t| t.label == "Site" && t.value == "Lab"));
        assert!(seen.await.unwrap()[4].path.starts_with("/integration/v1/sites/s-2/clients"));
    }
}
