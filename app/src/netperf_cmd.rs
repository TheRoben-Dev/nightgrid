//! Network tests between two of your hosts (M10x): a short-lived iperf3
//! server on the target, the client and ping on the source, results kept.

use std::time::Duration;

use nightgrid_core::hosts::HostId;
use nightgrid_core::netperf::{self, NetTest, PORT};
use nightgrid_core::system::SystemError;
use nightgrid_core::win;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, State};

use crate::commands::CmdResult;
use crate::state::AppState;
use crate::system_cmd::SysResult;
use crate::term_cmd::{connect_host, connect_posix};

fn other(e: impl std::fmt::Display) -> SystemError {
    SystemError::other(e)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NetTestResult {
    pub test: NetTest,
    /// Something worth a look (Fast Ethernet speed, slower than usual).
    pub finding: Option<String>,
}

/// Measures `from_host` → `to_host`: ping, then iperf3 up and down for
/// `secs` seconds each. `target` is the address the source connects to
/// (default: the target host's saved hostname).
#[tauri::command]
#[specta::specta]
pub async fn net_test_run(
    app: AppHandle,
    from_host: HostId,
    to_host: HostId,
    target: Option<String>,
    secs: u8,
) -> SysResult<NetTestResult> {
    if from_host == to_host {
        return Err(other("pick two different hosts").into());
    }
    let target = match target.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty()) {
        Some(t) => t,
        None => {
            let state = app.state::<AppState>();
            let h = state.store().get_host(to_host).map_err(other)?.ok_or_else(|| other("host not found"))?;
            netperf::default_target(&h.hostname)
        }
    };
    if !netperf::valid_target(&target) {
        return Err(other(format!("{target:?} isn't a host name or IP address")).into());
    }
    let secs = secs.clamp(2, 30);
    let server = connect_posix(&app, to_host, "fleet.speed").await?;
    let client = connect_host(&app, from_host).await?;
    let server_win = server.is_windows();
    let client_win = client.is_windows();
    let started = if server_win {
        let o = tokio::time::timeout(Duration::from_secs(20), win::run(&server, &win::netperf::server_script(PORT)))
            .await
            .map_err(|_| other("starting iperf3 on the target took too long"))?
            .map_err(other)?;
        win::netperf::parse_server(&o.stdout).map(|pid| (pid, String::new()))
    } else {
        let (_, out, err) = tokio::time::timeout(Duration::from_secs(20), server.exec(&netperf::server_script(PORT)))
            .await
            .map_err(|_| other("starting iperf3 on the target took too long"))?
            .map_err(other)?;
        netperf::parse_server(&format!("{}\n{}", String::from_utf8_lossy(&out), {
            let e = String::from_utf8_lossy(&err);
            e.lines().map(|l| format!("@@err {l}")).collect::<Vec<_>>().join("\n")
        }))
    };
    let (pid, file) = started.map_err(other)?;
    let budget = Duration::from_secs(u64::from(secs) * 2 + 30);
    let result = if client_win {
        tokio::time::timeout(budget, win::run(&client, &win::netperf::client_script(&target, PORT, secs)))
            .await
            .map(|r| r.map(|o| o.stdout).map_err(|e| e.to_string()))
    } else {
        tokio::time::timeout(budget, client.exec(&netperf::client_script(&target, PORT, secs)))
            .await
            .map(|r| r.map(|(_, out, _)| String::from_utf8_lossy(&out).into_owned()).map_err(|e| e.to_string()))
    };
    // Stop the server whatever happened (the watchdog would anyway).
    if server_win {
        let _ = win::run(&server, &win::netperf::stop_script(pid)).await;
    } else {
        let _ = server.exec(&netperf::stop_script(pid, &file)).await;
    }
    let out = result.map_err(|_| other("the test took too long"))?.map_err(other)?;
    let now = nightgrid_core::services::check::now_unix();
    let mut test = netperf::parse_client(from_host, to_host, &target, now, &out);
    if client_win && out.lines().any(|l| l.trim() == "@@noiperf") {
        test.error = Some(win::netperf::missing("source"));
    }
    let state = app.state::<AppState>();
    let saved = state.store().save_net_test(&test).map_err(other)?;
    let history = state.store().net_tests(1000).map_err(other)?;
    let finding = netperf::finding(&saved, &history);
    tracing::info!(from = %from_host, to = %to_host, "network test");
    Ok(NetTestResult { test: saved, finding })
}

/// Recent results, newest first.
#[tauri::command]
#[specta::specta]
pub fn net_tests_list(state: State<'_, AppState>) -> CmdResult<Vec<NetTest>> {
    Ok(state.store().net_tests(1000)?)
}

/// Forgets the results of one pair.
#[tauri::command]
#[specta::specta]
pub fn net_tests_clear(state: State<'_, AppState>, from_host: HostId, to_host: HostId) -> CmdResult<u32> {
    Ok(state.store().delete_net_tests(from_host, to_host)? as u32)
}
