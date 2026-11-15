//! Integrations: CRUD, status reads and AMP instance actions. Tokens and
//! passwords are kept in the OS keychain under the integration's id; the
//! list only says whether one is saved.

use nightgrid_core::integrations::amp::{self, AmpAction};
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
    if let Err(e) = store_secret(&state, i.id, secret) {
        let _ = state.store().delete_integration(i.id);
        return Err(e);
    }
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
    store_secret(&state, id, secret)?;
    Ok(view(&state, i))
}

#[tauri::command]
#[specta::specta]
pub fn integration_delete(state: State<'_, AppState>, id: IntegrationId) -> CmdResult<()> {
    state.store().delete_integration(id)?;
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
