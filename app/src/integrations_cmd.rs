//! Integrations: CRUD, status reads and AMP instance actions. Tokens and
//! passwords are kept in the OS keychain under the integration's id; the
//! list only says whether one is saved.

use nightgrid_core::integrations::amp::{self, AmpAction};
use nightgrid_core::integrations::dns::{self, DnsAction};
use nightgrid_core::integrations::{self, Integration, IntegrationId, IntegrationInput, IntegrationStatus};
use nightgrid_core::secrets::{Secret, SecretRef, SecretStore};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, State};

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationView {
    pub integration: Integration,
    /// A token/password is saved in the keychain.
    pub has_secret: bool,
}

fn secret_ref(id: IntegrationId) -> SecretRef {
    SecretRef::integration(id)
}

fn read_secret(state: &AppState, id: IntegrationId) -> Option<String> {
    match state.keychain.get(&secret_ref(id)) {
        Ok(s) => s.map(|s| s.expose().to_owned()),
        Err(e) => {
            tracing::warn!("keychain read failed: {e}");
            None
        }
    }
}

fn view(state: &AppState, i: Integration) -> IntegrationView {
    let has_secret = i.kind.needs_secret() && read_secret(state, i.id).is_some();
    IntegrationView { integration: i, has_secret }
}

/// `None` keeps the saved secret, `Some("")` removes it, anything else replaces it.
fn store_secret(state: &AppState, id: IntegrationId, secret: Option<String>) -> CmdResult<()> {
    match secret.as_deref().map(str::trim) {
        None => Ok(()),
        Some("") => state.keychain.delete(&secret_ref(id)).map_err(|e| CommandError::msg(e.to_string())),
        Some(s) => state
            .keychain
            .set(&secret_ref(id), &Secret::new(s.to_owned()))
            .map_err(|e| CommandError::msg(format!("couldn't save to the keychain: {e}"))),
    }
}

#[tauri::command]
#[specta::specta]
pub fn integrations_list(state: State<'_, AppState>) -> CmdResult<Vec<IntegrationView>> {
    let list = state.store().list_integrations()?;
    Ok(list.into_iter().map(|i| view(&state, i)).collect())
}

#[tauri::command]
#[specta::specta]
pub fn integration_create(
    state: State<'_, AppState>,
    input: IntegrationInput,
    secret: Option<String>,
) -> CmdResult<IntegrationView> {
    let i = state.store().create_integration(&input)?;
    let stamped = secret.as_deref().is_some_and(|s| !s.trim().is_empty());
    if let Err(e) = store_secret(&state, i.id, secret) {
        let _ = state.store().delete_integration(i.id);
        return Err(e);
    }
    if stamped {
        crate::secrets_cmd::stamp(&state, &format!("integration:{}", i.id));
    }
    journal(&state, &format!("Added integration {}", input.name));
    Ok(view(&state, i))
}

#[tauri::command]
#[specta::specta]
pub fn integration_update(
    state: State<'_, AppState>,
    id: IntegrationId,
    input: IntegrationInput,
    secret: Option<String>,
) -> CmdResult<IntegrationView> {
    let i = state.store().update_integration(id, &input)?;
    let stamped = secret.as_deref().is_some_and(|s| !s.trim().is_empty());
    store_secret(&state, id, secret)?;
    if stamped {
        crate::secrets_cmd::stamp(&state, &format!("integration:{id}"));
    }
    journal(&state, &format!("Changed integration {}", input.name));
    Ok(view(&state, i))
}

#[tauri::command]
#[specta::specta]
pub fn integration_delete(state: State<'_, AppState>, id: IntegrationId) -> CmdResult<()> {
    state.store().delete_integration(id)?;
    journal(&state, "Removed an integration");
    if let Err(e) = state.keychain.delete(&secret_ref(id)) {
        tracing::warn!("keychain delete failed: {e}");
    }
    Ok(())
}

/// The saved token/password, for "click to reveal" in the editor.
#[tauri::command]
#[specta::specta]
pub fn integration_reveal_secret(state: State<'_, AppState>, id: IntegrationId) -> CmdResult<Option<String>> {
    Ok(read_secret(&state, id))
}

#[tauri::command]
#[specta::specta]
pub async fn integration_status(app: AppHandle, id: IntegrationId) -> CmdResult<IntegrationStatus> {
    let (i, secret) = {
        let state = app.state::<AppState>();
        let i = state.store().get_integration(id)?;
        let secret = read_secret(&state, id);
        (i, secret)
    };
    integrations::status(&i, secret.as_deref()).await.map_err(CommandError::msg)
}

/// Tries settings before saving them. Uses `secret` if given, otherwise
/// the one saved for `id`.
#[tauri::command]
#[specta::specta]
pub async fn integration_test(
    app: AppHandle,
    id: Option<IntegrationId>,
    input: IntegrationInput,
    secret: Option<String>,
) -> CmdResult<IntegrationStatus> {
    let (kind, n) = integrations::normalize(&input)?;
    let stored = id.and_then(|id| read_secret(&app.state::<AppState>(), id));
    let secret = secret.filter(|s| !s.trim().is_empty()).or(stored);
    let i = Integration {
        id: id.unwrap_or(0),
        kind,
        name: n.name,
        base_url: n.base_url,
        username: n.username,
        slug: n.slug,
        endpoint_id: n.endpoint_id,
        host_id: n.host_id,
        allow_untrusted: n.allow_untrusted,
    };
    integrations::status(&i, secret.as_deref()).await.map_err(CommandError::msg)
}

/// Starts, stops or restarts an AMP instance. The UI confirms stop and
/// restart with the instance named.
#[tauri::command]
#[specta::specta]
pub async fn amp_instance_action(
    app: AppHandle,
    id: IntegrationId,
    instance: String,
    action: AmpAction,
) -> CmdResult<()> {
    let (i, secret) = {
        let state = app.state::<AppState>();
        let i = state.store().get_integration(id)?;
        (i, read_secret(&state, id))
    };
    let secret = secret.ok_or_else(|| CommandError::msg("no AMP password saved"))?;
    if action.is_destructive() {
        tracing::info!(integration = id, %instance, ?action, "AMP instance action");
    }
    amp::act(&i, &secret, &instance, action).await.map_err(CommandError::msg)
}

/// Changes a Pi-hole or AdGuard Home: blocking, lists, local records. The UI
/// confirms turning blocking off and removals with the target named.
#[tauri::command]
#[specta::specta]
pub async fn dns_filter_action(app: AppHandle, id: IntegrationId, action: DnsAction) -> CmdResult<()> {
    let (i, secret) = {
        let state = app.state::<AppState>();
        let i = state.store().get_integration(id)?;
        (i, read_secret(&state, id))
    };
    let secret = secret.ok_or_else(|| CommandError::msg("no password saved"))?;
    tracing::info!(integration = id, change = %action.describe(), "DNS filter change");
    dns::act(&i, &secret, &action).await.map_err(CommandError::msg)
}

/// Runs an action an integration card offered (scan libraries, approve a
/// request, remove a queue item, ...). `target` is the row's id, empty for
/// card-level actions. The UI asks first for any action marked `confirm`.
#[tauri::command]
#[specta::specta]
pub async fn integration_action(app: AppHandle, id: IntegrationId, target: String, action: String) -> CmdResult<()> {
    let (i, secret) = {
        let state = app.state::<AppState>();
        let i = state.store().get_integration(id)?;
        (i, read_secret(&state, id))
    };
    tracing::info!(integration = id, %target, %action, "integration action");
    integrations::act(&i, secret.as_deref(), &target, &action).await.map_err(CommandError::msg)
}

/// Integrations aren't tied to a host; the journal gets them fleet-wide.
fn journal(state: &AppState, title: &str) {
    let store = state.store();
    if !nightgrid_core::settings::AppSettings::load(&store).unwrap_or_default().feature("fleet.journal") {
        return;
    }
    let e = nightgrid_core::journal::NewEntry {
        at: nightgrid_core::services::check::now_unix(),
        area: Some(nightgrid_core::journal::ChangeArea::Integrations),
        title: title.to_owned(),
        ok: true,
        ..Default::default()
    };
    if let Err(err) = store.journal_add(&e) {
        tracing::warn!("couldn't record a journal entry: {err}");
    }
}
