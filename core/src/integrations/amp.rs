//! AMP (CubeCoders) game server panel: instances, their state and players,
//! plus start / stop / restart.
//!
//! Logs in with `POST /API/Core/Login` for a session id, then calls
//! `/API/ADSModule/GetInstances` and `Start|Stop|RestartInstance`. The
//! password comes from the keychain; the session id is used for one status
//! read or one action and then dropped.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{http_error, Integration};
use crate::http::{send, Request};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AmpStatus {
    pub instances: Vec<AmpInstance>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AmpInstance {
    /// AMP's instance name (used for actions), e.g. `Minecraft01`.
    pub name: String,
    pub friendly_name: String,
    /// Game/module, e.g. "Minecraft".
    pub module: String,
    /// The instance process is running (the game itself may still be starting).
    pub running: bool,
    /// Application state: `ready`, `starting`, `stopped`, ...
    pub state: String,
    pub users: Option<u32>,
    pub max_users: Option<u32>,
    pub cpu_percent: Option<f32>,
    pub memory_mb: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum AmpAction {
    Start,
    Stop,
    Restart,
}

impl AmpAction {
    fn endpoint(self) -> &'static str {
        match self {
            Self::Start => "StartInstance",
            Self::Stop => "StopInstance",
            Self::Restart => "RestartInstance",
        }
    }

    pub fn is_destructive(self) -> bool {
        !matches!(self, Self::Start)
    }
}

/// AMP's `ApplicationState` codes.
pub fn state_label(code: i64) -> &'static str {
    match code {
        0 => "stopped",
        5 => "pre-start",
        7 => "configuring",
        10 => "starting",
        20 => "ready",
        30 => "restarting",
        40 => "stopping",
        45 => "preparing to sleep",
        50 => "sleeping",
        60 => "waiting",
        70 => "installing",
        75 => "updating",
        80 => "awaiting input",
        100 => "failed",
        200 => "suspended",
        250 => "maintenance",
        _ => "unknown",
    }
}

fn metric(m: &Value, name: &str) -> (Option<f64>, Option<f64>) {
    let x = &m[name];
    (x["RawValue"].as_f64(), x["MaxValue"].as_f64())
}

/// Parses GetInstances (a list of targets, optionally wrapped in `result`).
/// The ADS controller itself is left out.
pub fn parse_instances(v: &Value) -> Vec<AmpInstance> {
    let targets = v.as_array().or_else(|| v["result"].as_array()).cloned().unwrap_or_default();
    targets
        .iter()
        .flat_map(|t| t["AvailableInstances"].as_array().cloned().unwrap_or_default())
        .filter(|i| i["Module"].as_str() != Some("ADS"))
        .map(|i| {
            let str_of = |k: &str| i[k].as_str().unwrap_or("").to_owned();
            let running = i["Running"].as_bool().unwrap_or(false);
            let code = i["AppState"].as_i64().unwrap_or(-1);
            let m = &i["Metrics"];
            let (users, max_users) = metric(m, "Active Users");
            let (cpu, _) = metric(m, "CPU Usage");
            let (mem, _) = metric(m, "Memory Usage");
            let module = {
                let d = str_of("ModuleDisplayName");
                if d.is_empty() {
                    str_of("Module").trim_end_matches("Module").to_owned()
                } else {
                    d
                }
            };
            AmpInstance {
                name: str_of("InstanceName"),
                friendly_name: str_of("FriendlyName"),
                module,
                running,
                state: if running { state_label(code) } else { "stopped" }.to_owned(),
                users: users.map(|u| u as u32),
                max_users: max_users.map(|u| u as u32).filter(|m| *m > 0),
                cpu_percent: cpu.map(|c| c as f32),
                memory_mb: mem.map(|m| m as u32),
            }
        })
        .collect()
}

async fn call(i: &Integration, method: &str, body: Value) -> Result<Value, String> {
    let r = send(
        Request::post(format!("{}/API/{method}", i.base_url), Vec::new())
            .header("Accept", "application/json")
            .json(&body)
            .allow_untrusted(i.allow_untrusted),
    )
    .await?;
    if !r.ok() {
        return Err(http_error("AMP", r.status));
    }
    r.json()
}

async fn login(i: &Integration, password: &str) -> Result<String, String> {
    let v = call(
        i,
        "Core/Login",
        json!({ "username": i.username, "password": password, "token": "", "rememberMe": false }),
    )
    .await?;
    match (v["success"].as_bool(), v["sessionID"].as_str()) {
        (Some(true), Some(id)) if !id.is_empty() => Ok(id.to_owned()),
        _ => Err(match v["resultReason"].as_str().filter(|r| !r.is_empty()) {
            Some(r) => format!("AMP login failed: {r}"),
            None => "AMP login failed: check the login name and password".into(),
        }),
    }
}

/// Whether an action's response says it worked (AMP answers `{}`, `null`,
/// or an ActionResult with `Status`).
fn action_ok(v: &Value) -> Result<(), String> {
    match v.get("Status").and_then(Value::as_bool) {
        Some(false) => Err(v["Reason"].as_str().unwrap_or("AMP refused the action").to_owned()),
        _ => Ok(()),
    }
}

pub async fn status(i: &Integration, password: &str) -> Result<AmpStatus, String> {
    let session = login(i, password).await?;
    let v = call(i, "ADSModule/GetInstances", json!({ "SESSIONID": session })).await?;
    Ok(AmpStatus { instances: parse_instances(&v) })
}

pub fn valid_instance_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

pub async fn act(i: &Integration, password: &str, instance: &str, action: AmpAction) -> Result<(), String> {
    if !valid_instance_name(instance) {
        return Err("invalid instance name".into());
    }
    let session = login(i, password).await?;
    let v =
        call(i, &format!("ADSModule/{}", action.endpoint()), json!({ "InstanceName": instance, "SESSIONID": session }))
            .await?;
    action_ok(&v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instances() {
        let v: Value = serde_json::from_str(include_str!("fixtures/amp_instances.json")).unwrap();
        let list = parse_instances(&v);
        assert_eq!(list.len(), 3, "ADS controller left out");
        let mc = &list[0];
        assert_eq!(
            (mc.name.as_str(), mc.friendly_name.as_str(), mc.module.as_str()),
            ("Minecraft01", "Survival World", "Minecraft")
        );
        assert_eq!((mc.state.as_str(), mc.users, mc.max_users), ("ready", Some(3), Some(20)));
        assert_eq!((mc.cpu_percent, mc.memory_mb), (Some(12.0), Some(3072)));
        assert_eq!((list[1].module.as_str(), list[1].state.as_str(), list[1].users), ("Generic", "stopped", None));
        assert_eq!(list[2].state, "starting");
        // Wrapped in `result` (some AMP versions).
        assert_eq!(parse_instances(&json!({ "result": v })).len(), 3);
    }

    #[test]
    fn actions_and_names() {
        assert!(action_ok(&json!({})).is_ok());
        assert!(action_ok(&Value::Null).is_ok());
        assert_eq!(action_ok(&json!({"Status": false, "Reason": "no permission"})).unwrap_err(), "no permission");
        assert!(valid_instance_name("Minecraft01"));
        assert!(!valid_instance_name("x\"; rm"));
        assert!(AmpAction::Stop.is_destructive() && !AmpAction::Start.is_destructive());
        assert_eq!(state_label(20), "ready");
    }

    /// Login → GetInstances → action against a fake AMP.
    #[tokio::test]
    async fn against_fake_amp() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let mut paths = Vec::new();
            for _ in 0..4 {
                let (mut s, _) = l.accept().await.unwrap();
                let mut buf = vec![0u8; 16384];
                let mut got = Vec::new();
                loop {
                    let n = s.read(&mut buf).await.unwrap();
                    got.extend_from_slice(&buf[..n]);
                    let t = String::from_utf8_lossy(&got).to_string();
                    if let Some(end) = t.find("\r\n\r\n") {
                        let len: usize = t
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase().strip_prefix("content-length: ").map(|v| v.parse().unwrap())
                            })
                            .unwrap_or(0);
                        if got.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                let t = String::from_utf8_lossy(&got).to_string();
                let path = t.split_whitespace().nth(1).unwrap().to_owned();
                let body = match path.as_str() {
                    "/API/Core/Login" if t.contains("\"password\":\"pw\"") => {
                        r#"{"success":true,"sessionID":"abc"}"#.to_owned()
                    }
                    "/API/Core/Login" => r#"{"success":false,"resultReason":"Bad credentials"}"#.to_owned(),
                    "/API/ADSModule/GetInstances" if t.contains("\"SESSIONID\":\"abc\"") => {
                        include_str!("fixtures/amp_instances.json").to_owned()
                    }
                    "/API/ADSModule/StopInstance" if t.contains("Minecraft01") => "{}".to_owned(),
                    _ => r#"{"Title":"Unauthorized"}"#.to_owned(),
                };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
                    body.len()
                );
                s.write_all(resp.as_bytes()).await.unwrap();
                let _ = s.shutdown().await;
                paths.push(path);
            }
            paths
        });
        let i = Integration {
            id: 1,
            kind: super::super::IntegrationKind::Amp,
            name: "AMP".into(),
            base_url: format!("http://127.0.0.1:{port}"),
            username: "admin".into(),
            slug: String::new(),
            endpoint_id: 0,
            host_id: None,
            allow_untrusted: false,
        };
        let st = status(&i, "pw").await.unwrap();
        assert_eq!(st.instances.len(), 3);
        act(&i, "pw", "Minecraft01", AmpAction::Stop).await.unwrap();
        assert_eq!(
            server.await.unwrap(),
            ["/API/Core/Login", "/API/ADSModule/GetInstances", "/API/Core/Login", "/API/ADSModule/StopInstance"]
        );
    }
}
