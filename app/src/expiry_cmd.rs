//! Expiry watch (M10l): certificate files, domains, tokens and licences with
//! an end date. Dates of certificate files are read over SSH; domains are
//! looked up with `whois` on a host only when the user clicks.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use nightgrid_core::expiry::{self, WatchId, WatchInput, WatchItem, WatchKind};
use nightgrid_core::hosts::HostId;
use nightgrid_core::services::check::now_unix;
use tauri::{AppHandle, Manager, State};

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;
use crate::term_cmd::connect_host;

/// Certificate files are re-read in the background this often.
pub(crate) const REFRESH_EVERY: i64 = 12 * 3600;

/// A certificate path and its date, or why it couldn't be read.
type FileDate = (String, Result<i64, String>);

static REFRESHING: AtomicBool = AtomicBool::new(false);

#[tauri::command]
#[specta::specta]
pub fn watch_list(state: State<'_, AppState>) -> CmdResult<Vec<WatchItem>> {
    Ok(state.store().watch_list()?)
}

/// Adds or updates an item; a certificate file is read right away.
#[tauri::command]
#[specta::specta]
pub async fn watch_save(app: AppHandle, input: WatchInput) -> CmdResult<WatchItem> {
    let item = app.state::<AppState>().store().watch_save(&input)?;
    if item.kind == WatchKind::CertFile && item.expires_at.is_none() {
        read_cert_files(&app, std::slice::from_ref(&item)).await;
    }
    app.state::<AppState>().alerts.wake();
    let got = app.state::<AppState>().store().watch_get(item.id)?;
    Ok(got.unwrap_or(item))
}

#[tauri::command]
#[specta::specta]
pub fn watch_delete(state: State<'_, AppState>, id: WatchId) -> CmdResult<()> {
    state.store().watch_delete(id)?;
    state.alerts.wake();
    Ok(())
}

/// Re-reads every certificate file now.
#[tauri::command]
#[specta::specta]
pub async fn watch_check(app: AppHandle) -> CmdResult<Vec<WatchItem>> {
    let items = app.state::<AppState>().store().watch_list()?;
    let certs: Vec<WatchItem> = items.into_iter().filter(|i| i.kind == WatchKind::CertFile).collect();
    read_cert_files(&app, &certs).await;
    app.state::<AppState>().alerts.wake();
    Ok(app.state::<AppState>().store().watch_list()?)
}

/// Looks the domain's expiry up with `whois` on `via` (asked for by a click;
/// the host contacts the registry's whois server).
#[tauri::command]
#[specta::specta]
pub async fn watch_lookup(app: AppHandle, id: WatchId, via: HostId) -> CmdResult<WatchItem> {
    let item = app.state::<AppState>().store().watch_get(id)?.ok_or_else(|| CommandError::msg("item not found"))?;
    if item.kind != WatchKind::Domain {
        return Err(CommandError::msg("only domains can be looked up"));
    }
    let cmd = expiry::whois_command(&item.target).ok_or_else(|| CommandError::msg("not a domain name"))?;
    let conn = connect_host(&app, via).await.map_err(|e| CommandError::msg(e.to_string()))?;
    tracing::info!(host = %via, "whois lookup for a watched domain");
    let r = match tokio::time::timeout(std::time::Duration::from_secs(30), conn.exec(&cmd)).await {
        Ok(Ok((_, out, _))) => expiry::parse_whois(&String::from_utf8_lossy(&out)),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("whois took too long".into()),
    };
    let failed = r.as_ref().err().cloned();
    let state = app.state::<AppState>();
    state.store().watch_checked(id, r, now_unix())?;
    state.alerts.wake();
    if let Some(e) = failed {
        return Err(CommandError::msg(e));
    }
    let got = state.store().watch_get(id)?;
    Ok(got.unwrap_or(item))
}

/// Reads the given certificate items, one connection per host.
pub(crate) async fn read_cert_files(app: &AppHandle, items: &[WatchItem]) {
    let mut by_host: HashMap<HostId, Vec<&WatchItem>> = HashMap::new();
    for i in items.iter().filter(|i| i.kind == WatchKind::CertFile) {
        if let Some(h) = i.host_id {
            by_host.entry(h).or_default().push(i);
        }
    }
    for (host, list) in by_host {
        let paths: Vec<String> = list.iter().map(|i| i.target.clone()).collect();
        let result: Result<Vec<FileDate>, String> = async {
            let conn = connect_host(app, host).await.map_err(|e| e.to_string())?;
            let (_, out, _) = tokio::time::timeout(
                std::time::Duration::from_secs(20),
                conn.exec(&expiry::cert_files_command(&paths)),
            )
            .await
            .map_err(|_| "the host took too long".to_owned())?
            .map_err(|e| e.to_string())?;
            Ok(expiry::parse_cert_files(&String::from_utf8_lossy(&out)))
        }
        .await;
        let now = now_unix();
        let state = app.state::<AppState>();
        let store = state.store();
        for i in list {
            let r = match &result {
                Err(e) => Err(e.clone()),
                Ok(rows) => rows
                    .iter()
                    .find(|(p, _)| p == &i.target || p == "*")
                    .map(|(_, r)| r.clone())
                    .unwrap_or_else(|| Err("no answer for this file".into())),
            };
            if let Err(e) = store.watch_checked(i.id, r, now) {
                tracing::warn!("couldn't save a certificate date: {e}");
            }
        }
    }
}

/// From the alert loop: re-reads certificate files on connected hosts that
/// weren't read in the last [`REFRESH_EVERY`], in the background.
pub(crate) fn refresh_due(app: &AppHandle, items: &[WatchItem], connected: &[HostId], now: i64) {
    let due: Vec<WatchItem> = items
        .iter()
        .filter(|i| {
            i.kind == WatchKind::CertFile
                && i.host_id.is_some_and(|h| connected.contains(&h))
                && i.checked_at.map_or(true, |t| now - t >= REFRESH_EVERY)
        })
        .cloned()
        .collect();
    if due.is_empty() || REFRESHING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        read_cert_files(&app, &due).await;
        REFRESHING.store(false, Ordering::SeqCst);
        app.state::<AppState>().alerts.wake();
    });
}
