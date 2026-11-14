//! Host, group, ssh-config import and reachability commands.

use std::collections::HashSet;
use std::time::Duration;

use nightgrid_core::hosts::{Group, GroupId, GroupInput, Host, HostId, HostInput};
use nightgrid_core::ssh::client::tcp_ping;
use nightgrid_core::ssh::config::{read_user_config, SshConfigHost};
use serde::Serialize;
use specta::Type;
use tauri::State;

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn hosts_list(state: State<'_, AppState>) -> CmdResult<Vec<Host>> {
    Ok(state.store().list_hosts()?)
}

#[tauri::command]
#[specta::specta]
pub fn host_create(state: State<'_, AppState>, input: HostInput) -> CmdResult<Host> {
    Ok(state.store().create_host(&input)?)
}

#[tauri::command]
#[specta::specta]
pub async fn host_update(state: State<'_, AppState>, id: HostId, input: HostInput) -> CmdResult<Host> {
    let (host, conn_changed, dropped_remember) = {
        let mut store = state.store();
        let before = store.get_host(id)?.ok_or_else(|| CommandError::msg("host not found"))?;
        let after = store.update_host(id, &input)?;
        let conn_changed = before.hostname != after.hostname
            || before.port != after.port
            || before.username != after.username
            || before.auth_method != after.auth_method
            || before.identity_file != after.identity_file
            || before.proxy_jump != after.proxy_jump;
        (after, conn_changed, before.remember_secret && !input.remember_secret)
    };
    if dropped_remember || conn_changed {
        state.forget_host_secrets(id);
    }
    if conn_changed {
        state.pool.disconnect(id).await;
    }
    Ok(host)
}

#[tauri::command]
#[specta::specta]
pub async fn host_delete(state: State<'_, AppState>, id: HostId) -> CmdResult<()> {
    state.store().delete_host(id)?;
    state.forget_host_secrets(id);
    state.pool.disconnect(id).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn host_forget_secrets(state: State<'_, AppState>, id: HostId) -> CmdResult<()> {
    state.forget_host_secrets(id);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn groups_list(state: State<'_, AppState>) -> CmdResult<Vec<Group>> {
    Ok(state.store().list_groups()?)
}

#[tauri::command]
#[specta::specta]
pub fn group_create(state: State<'_, AppState>, input: GroupInput) -> CmdResult<Group> {
    Ok(state.store().create_group(&input)?)
}

#[tauri::command]
#[specta::specta]
pub fn group_update(state: State<'_, AppState>, id: GroupId, input: GroupInput) -> CmdResult<Group> {
    Ok(state.store().update_group(id, &input)?)
}

#[tauri::command]
#[specta::specta]
pub fn group_delete(state: State<'_, AppState>, id: GroupId) -> CmdResult<()> {
    Ok(state.store().delete_group(id)?)
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SshConfigCandidate {
    #[serde(flatten)]
    pub host: SshConfigHost,
    /// Id of a host already imported from this alias.
    pub existing_id: Option<HostId>,
}

/// Reads `~/.ssh/config` (never writes it).
#[tauri::command]
#[specta::specta]
pub fn ssh_config_scan(state: State<'_, AppState>) -> CmdResult<Vec<SshConfigCandidate>> {
    let home = state.home.clone().ok_or_else(|| CommandError::msg("no home directory"))?;
    let hosts = state.store().list_hosts()?;
    Ok(read_user_config(&home)
        .into_iter()
        .map(|h| {
            let existing_id = hosts.iter().find(|x| x.ssh_alias.as_deref() == Some(h.alias.as_str())).map(|x| x.id);
            SshConfigCandidate { host: h, existing_id }
        })
        .collect())
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub created: u32,
    pub updated: u32,
}

/// Imports (or re-syncs) the chosen aliases. Re-importing updates connection
/// fields only; names, groups, tags, colors and notes the user set are kept.
#[tauri::command]
#[specta::specta]
pub fn ssh_config_import(state: State<'_, AppState>, aliases: Vec<String>) -> CmdResult<ImportResult> {
    let home = state.home.clone().ok_or_else(|| CommandError::msg("no home directory"))?;
    let wanted: HashSet<String> = aliases.into_iter().collect();
    let candidates: Vec<SshConfigHost> =
        read_user_config(&home).into_iter().filter(|h| wanted.contains(&h.alias)).collect();
    let mut store = state.store();
    let existing = store.list_hosts()?;
    let (mut created, mut updated) = (0, 0);
    for c in candidates {
        let auth_method = nightgrid_core::hosts::AuthMethod::Agent;
        if let Some(h) = existing.iter().find(|h| h.ssh_alias.as_deref() == Some(c.alias.as_str())) {
            let input = HostInput {
                name: h.name.clone(),
                hostname: c.hostname,
                port: c.port.or(Some(22)),
                username: c.user,
                group_id: h.group_id,
                color: h.color.clone(),
                icon: h.icon.clone(),
                favorite: h.favorite,
                notes: h.notes.clone(),
                auth_method: if h.auth_method == nightgrid_core::hosts::AuthMethod::Password {
                    h.auth_method
                } else {
                    auth_method
                },
                identity_file: c.identity_file,
                proxy_jump: c.proxy_jump,
                remember_secret: h.remember_secret,
                ssh_alias: Some(c.alias),
                tags: h.tags.clone(),
            };
            store.update_host(h.id, &input)?;
            updated += 1;
        } else {
            store.create_host(&HostInput {
                name: c.alias.clone(),
                hostname: c.hostname,
                port: c.port,
                username: c.user,
                auth_method,
                identity_file: c.identity_file,
                proxy_jump: c.proxy_jump,
                ssh_alias: Some(c.alias),
                tags: vec!["ssh-config".into()],
                ..Default::default()
            })?;
            created += 1;
        }
    }
    Ok(ImportResult { created, updated })
}

/// Copies entries from `~/.ssh/known_hosts` into NIGHTGRID's own file.
#[tauri::command]
#[specta::specta]
pub fn known_hosts_import_system(state: State<'_, AppState>) -> CmdResult<u32> {
    Ok(state.known_hosts.import_system()? as u32)
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HostPing {
    pub id: HostId,
    /// `online`, `offline`, or `unknown` (reached only through a jump host).
    pub state: String,
    pub latency_ms: Option<u32>,
    pub connected: bool,
}

/// TCP-connect check to each host's SSH port (no ICMP, no auth).
#[tauri::command]
#[specta::specta]
pub async fn hosts_ping(state: State<'_, AppState>, ids: Vec<HostId>) -> CmdResult<Vec<HostPing>> {
    let hosts: Vec<Host> = state.store().list_hosts()?.into_iter().filter(|h| ids.contains(&h.id)).collect();
    let mut set = tokio::task::JoinSet::new();
    for h in hosts {
        let connected = state.pool.is_connected(h.id);
        set.spawn(async move {
            if h.proxy_jump.is_some() {
                let state = if connected { "online" } else { "unknown" };
                return HostPing { id: h.id, state: state.into(), latency_ms: None, connected };
            }
            match tcp_ping(&h.hostname, h.port, Duration::from_secs(2)).await {
                Some(d) => HostPing {
                    id: h.id,
                    state: "online".into(),
                    latency_ms: Some(d.as_millis().min(u32::MAX as u128) as u32),
                    connected,
                },
                None => HostPing { id: h.id, state: "offline".into(), latency_ms: None, connected },
            }
        });
    }
    let mut out = Vec::new();
    while let Some(r) = set.join_next().await {
        if let Ok(p) = r {
            out.push(p);
        }
    }
    Ok(out)
}
