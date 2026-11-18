//! Local notifications (1.1, D-167): the inbox, desktop notifications for
//! everything that isn't an alert, and an optional self-hosted ntfy server.
//! Everything that notifies goes through [`send`]; alerts keep their own
//! native notifications and folding, and hand their notices over with
//! [`from_alerts`].

use nightgrid_core::alerts::{self, Alert, Severity};
use nightgrid_core::notify::{self, NewNotice, Notice, NoticeLevel, NoticeSource};
use nightgrid_core::secrets::{Secret, SecretKind, SecretRef, SecretStore as _};
use nightgrid_core::services::check::now_unix;
use nightgrid_core::settings::AppSettings;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Emitter as _, Manager as _, State};
use tauri_plugin_notification::NotificationExt;

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;

/// Sent to the UI with each new notice.
pub const EVENT_NOTICE: &str = "notify://notice";
/// ntfy messages sent per round of alerts, at most.
const NTFY_MAX_PER_ROUND: usize = 10;

fn ntfy_url_ref() -> SecretRef {
    SecretRef::new(SecretKind::IntegrationToken, "notify-ntfy-url")
}

fn ntfy_token_ref() -> SecretRef {
    SecretRef::new(SecretKind::IntegrationToken, "notify-ntfy-token")
}

fn settings(app: &AppHandle) -> AppSettings {
    AppSettings::load(&app.state::<AppState>().store()).unwrap_or_default()
}

fn keychain_get(app: &AppHandle, r: &SecretRef) -> Option<String> {
    match app.state::<AppState>().keychain.get(r) {
        Ok(s) => s.map(|s| s.expose().to_owned()),
        Err(e) => {
            tracing::warn!("keychain read failed: {e}");
            None
        }
    }
}

fn ntfy_target(app: &AppHandle) -> Option<(String, Option<String>)> {
    let url = keychain_get(app, &ntfy_url_ref())?;
    Some((url, keychain_get(app, &ntfy_token_ref())))
}

fn native(app: &AppHandle, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        tracing::warn!("notification failed: {e}");
    }
}

fn store_and_emit(app: &AppHandle, n: &NewNotice, at: i64) -> Option<Notice> {
    let stored = app.state::<AppState>().store().notice_add(n, at);
    match stored {
        Ok(saved) => {
            let _ = app.emit(EVENT_NOTICE, &saved);
            Some(saved)
        }
        Err(e) => {
            tracing::warn!("couldn't keep the notice: {e}");
            None
        }
    }
}

fn post_ntfy(app: &AppHandle, list: Vec<NewNotice>) {
    if list.is_empty() {
        return;
    }
    let Some((url, token)) = ntfy_target(app) else { return };
    tauri::async_runtime::spawn(async move {
        for n in list.into_iter().take(NTFY_MAX_PER_ROUND) {
            match nightgrid_core::http::send(notify::ntfy_request(&url, token.as_deref(), &n)).await {
                Ok(r) if r.ok() => {}
                Ok(r) => tracing::warn!(status = r.status, "ntfy refused the notice"),
                Err(e) => tracing::warn!("ntfy failed: {e}"),
            }
        }
    });
}

/// Sends a notice: into the inbox always; to the desktop and ntfy when its
/// source and level are routed there and it isn't quiet hours.
pub fn send(app: &AppHandle, n: NewNotice) {
    let cfg = settings(app);
    store_and_emit(app, &n, now_unix());
    if crate::alerts_cmd::is_quiet(&cfg) {
        return;
    }
    if cfg.notify.to_native(n.source, n.level) {
        native(app, &format!("NIGHTGRID · {}", n.title), &n.body);
    }
    if cfg.notify.to_ntfy(n.source, n.level) {
        post_ntfy(app, vec![n]);
    }
}

fn alert_notice(a: &Alert, explains: &str) -> NewNotice {
    let level = if a.resolved.is_some() {
        NoticeLevel::Ok
    } else if a.severity == Severity::Critical {
        NoticeLevel::Critical
    } else {
        NoticeLevel::Warn
    };
    let title = if a.resolved.is_some() { format!("Resolved: {}", alerts::headline(a)) } else { alerts::headline(a) };
    NewNotice {
        source: NoticeSource::Alert,
        level,
        title,
        body: format!("{}{explains}", a.detail),
        host_id: a.host_id,
        view: Some("overview".into()),
    }
}

/// Alerts that fired or cleared this round (after folding): into the inbox,
/// and to ntfy when routed. Desktop notifications stay with the alert loop.
pub(crate) fn from_alerts(
    app: &AppHandle,
    cfg: &AppSettings,
    fired: &[(Alert, String)],
    resolved: &[Alert],
    quiet: bool,
) {
    let now = now_unix();
    let mut out = Vec::new();
    for (a, explains) in fired {
        let n = alert_notice(a, explains);
        store_and_emit(app, &n, now);
        if cfg.notify.to_ntfy(n.source, n.level) {
            out.push(n);
        }
    }
    for a in resolved {
        let n = alert_notice(a, "");
        store_and_emit(app, &n, now);
        if cfg.notify.to_ntfy(n.source, n.level) {
            out.push(n);
        }
    }
    if !quiet {
        post_ntfy(app, out);
    }
}

/// Moves an "ntfy" alert webhook from before 1.1 to the ntfy channel.
pub fn migrate(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut cfg = settings(app);
    if cfg.alerts.webhook != "ntfy" {
        return;
    }
    if let Some(url) = keychain_get(app, &SecretRef::alerts_webhook()) {
        if let Err(e) = state.keychain.set(&ntfy_url_ref(), &Secret::new(url)) {
            tracing::warn!("couldn't move the ntfy URL: {e}");
            return;
        }
        let _ = state.keychain.delete(&SecretRef::alerts_webhook());
        cfg.notify.ntfy = true;
        cfg.notify.ntfy_ok = cfg.alerts.webhook_resolved;
    }
    cfg.alerts.webhook = "none".into();
    let saved = cfg.save(&state.store());
    match saved {
        Err(e) => tracing::warn!("couldn't save the moved ntfy setting: {e}"),
        Ok(()) => tracing::info!("moved the ntfy alert webhook to Notifications"),
    }
}

// ------------------------------------------------------------- commands

#[tauri::command]
#[specta::specta]
pub fn notices_list(state: State<'_, AppState>, limit: u32) -> CmdResult<Vec<Notice>> {
    Ok(state.store().notices(limit.clamp(1, 1000))?)
}

/// Marks these notices read, or every one with `null`.
#[tauri::command]
#[specta::specta]
pub fn notices_read(state: State<'_, AppState>, ids: Option<Vec<u32>>) -> CmdResult<()> {
    let ids: Option<Vec<i64>> = ids.map(|v| v.into_iter().map(i64::from).collect());
    Ok(state.store().notices_mark_read(ids.as_deref())?)
}

#[tauri::command]
#[specta::specta]
pub fn notices_clear(state: State<'_, AppState>) -> CmdResult<()> {
    Ok(state.store().notices_clear()?)
}

/// What's saved for ntfy, without the secret parts.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NtfyInfo {
    pub configured: bool,
    /// `https://server/topic` without credentials.
    pub url: Option<String>,
    pub has_token: bool,
    /// The saved URL points at the public ntfy service (only possible for
    /// a webhook moved over from an older version).
    pub public: bool,
}

fn ntfy_info(app: &AppHandle) -> NtfyInfo {
    let url = keychain_get(app, &ntfy_url_ref());
    let shown = url.as_deref().map(|u| match u.split_once("://") {
        Some((scheme, rest)) => format!("{scheme}://{}", rest.rsplit_once('@').map_or(rest, |(_, r)| r)),
        None => u.to_owned(),
    });
    NtfyInfo {
        configured: url.is_some(),
        public: url.as_deref().is_some_and(notify::is_public_ntfy),
        url: shown,
        has_token: keychain_get(app, &ntfy_token_ref()).is_some(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn ntfy_get(app: AppHandle) -> CmdResult<NtfyInfo> {
    Ok(ntfy_info(&app))
}

/// Saves the topic URL (`null` removes it and the token) and, when given,
/// the access token (empty string removes it). Refuses the public ntfy.sh.
#[tauri::command]
#[specta::specta]
pub fn ntfy_set(app: AppHandle, url: Option<String>, token: Option<String>) -> CmdResult<NtfyInfo> {
    let state = app.state::<AppState>();
    let err = |e: nightgrid_core::Error| CommandError::msg(e.to_string());
    match url.map(|u| u.trim().to_owned()) {
        None => {
            state.keychain.delete(&ntfy_url_ref()).map_err(err)?;
            state.keychain.delete(&ntfy_token_ref()).map_err(err)?;
            return Ok(ntfy_info(&app));
        }
        Some(u) if u.is_empty() => {}
        Some(u) => {
            notify::check_ntfy_url(&u).map_err(CommandError::msg)?;
            state.keychain.set(&ntfy_url_ref(), &Secret::new(u)).map_err(err)?;
            crate::secrets_cmd::stamp(&app, "ntfy");
        }
    }
    match token.map(|t| t.trim().to_owned()) {
        None => {}
        Some(t) if t.is_empty() => state.keychain.delete(&ntfy_token_ref()).map_err(err)?,
        Some(t) => {
            if t.len() > 200 || t.chars().any(|c| c.is_whitespace() || c.is_control()) {
                return Err(CommandError::msg("that doesn't look like an ntfy access token"));
            }
            state.keychain.set(&ntfy_token_ref(), &Secret::new(t)).map_err(err)?;
            crate::secrets_cmd::stamp(&app, "ntfy");
        }
    }
    Ok(ntfy_info(&app))
}

/// What a test sent on each channel.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NotifyTest {
    pub native: Option<String>,
    pub ntfy: Option<String>,
}

/// Sends a test notice to the inbox, the desktop and ntfy (ignores quiet
/// hours and routes; only the channel switches count).
#[tauri::command]
#[specta::specta]
pub async fn notify_test(app: AppHandle) -> CmdResult<NotifyTest> {
    let cfg = settings(&app);
    let n =
        NewNotice::new(NoticeSource::System, NoticeLevel::Info, "Test", "Notifications from Nightgrid reach you here.")
            .view("settings");
    store_and_emit(&app, &n, now_unix());
    let native = (cfg.notify.native || cfg.alerts.native).then(|| {
        match app.notification().builder().title("NIGHTGRID · TEST").body(&n.body).show() {
            Ok(()) => "sent".to_owned(),
            Err(e) => format!("failed: {e}"),
        }
    });
    let ntfy = if !cfg.notify.ntfy {
        None
    } else if let Some((url, token)) = ntfy_target(&app) {
        Some(match nightgrid_core::http::send(notify::ntfy_request(&url, token.as_deref(), &n)).await {
            Ok(r) if r.ok() => format!("delivered (HTTP {})", r.status),
            Ok(r) if r.status == 401 || r.status == 403 => {
                format!("refused: HTTP {} (the topic needs an access token)", r.status)
            }
            Ok(r) => format!("refused: HTTP {}", r.status),
            Err(e) => format!("failed: {e}"),
        })
    } else {
        Some("no server saved".into())
    };
    Ok(NotifyTest { native, ntfy })
}
