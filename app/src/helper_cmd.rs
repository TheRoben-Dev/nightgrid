//! DRIFT, the local helper: model download/state and streaming replies.
//!
//! The webview builds the prompt (retrieval over the bundled guide, the live
//! view context, the "show me" targets) and shows it to the user before it's
//! sent; this side only runs the model. Nothing leaves the machine: the only
//! network use is the model download the user starts, from a pinned URL with
//! a pinned SHA-256 (see `nightgrid_core::helper::model`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nightgrid_core::helper::model::{self, DownloadEnd, ModelState, Policy, DEFAULT_MODEL};
use nightgrid_llm::{Engine, Finish, GenOptions, LoadOptions, Message};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::state::AppState;

/// Runtime state of the helper (not persisted).
#[derive(Default)]
pub struct Helper {
    engine: Mutex<Option<Arc<Engine>>>,
    /// Serializes model loading so two first questions don't load twice.
    loading: tokio::sync::Mutex<()>,
    downloading: AtomicBool,
    cancel_download: Arc<AtomicBool>,
    /// Set to stop the reply being generated.
    stop: Arc<AtomicBool>,
    busy: AtomicBool,
}

fn models_dir(state: &AppState) -> PathBuf {
    state.data_dir.join("models")
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HelperStatus {
    pub model: String,
    pub license: String,
    /// Full size of the model file in bytes.
    pub bytes: f64,
    /// `missing`, `partial` or `ready`.
    pub state: String,
    /// Bytes already on disk for a partial download.
    pub have: f64,
    pub downloading: bool,
    pub loaded: bool,
    /// The build offloads to the GPU (Metal on macOS); otherwise CPU.
    pub gpu: bool,
}

fn status(state: &AppState, helper: &Helper) -> HelperStatus {
    let (st, have) = match model::state(&models_dir(state), &DEFAULT_MODEL) {
        ModelState::Missing => ("missing", 0),
        ModelState::Partial(n) => ("partial", n),
        ModelState::Ready => ("ready", DEFAULT_MODEL.bytes),
    };
    HelperStatus {
        model: DEFAULT_MODEL.name.into(),
        license: DEFAULT_MODEL.license.into(),
        bytes: DEFAULT_MODEL.bytes as f64,
        state: st.into(),
        have: have as f64,
        downloading: helper.downloading.load(Ordering::Relaxed),
        loaded: helper.engine.lock().expect("poisoned").is_some(),
        gpu: nightgrid_llm::gpu_build(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn helper_status(state: State<'_, AppState>, helper: State<'_, Helper>) -> HelperStatus {
    status(&state, &helper)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub done: f64,
    pub total: f64,
}

/// Downloads (or resumes) the model, then verifies its checksum. Returns
/// `false` if it was cancelled.
#[tauri::command]
#[specta::specta]
pub async fn helper_download(app: AppHandle, on_progress: Channel<DownloadProgress>) -> Result<bool, String> {
    let helper = app.state::<Helper>();
    if helper.downloading.swap(true, Ordering::SeqCst) {
        return Err("The model is already downloading.".into());
    }
    helper.cancel_download.store(false, Ordering::SeqCst);
    let dir = models_dir(&app.state::<AppState>());
    let cancel = helper.cancel_download.clone();
    let mut last = Instant::now() - Duration::from_secs(1);
    tracing::info!(url = DEFAULT_MODEL.url, "helper model download started");
    let r = model::download(&dir, &DEFAULT_MODEL, &Policy::default(), &cancel, |done, total| {
        if done == total || last.elapsed() >= Duration::from_millis(200) {
            last = Instant::now();
            let _ = on_progress.send(DownloadProgress { done: done as f64, total: total as f64 });
        }
    })
    .await;
    helper.downloading.store(false, Ordering::SeqCst);
    match r {
        Ok(DownloadEnd::Done) => {
            tracing::info!("helper model downloaded and verified");
            Ok(true)
        }
        Ok(DownloadEnd::Cancelled) => Ok(false),
        Err(e) => {
            tracing::warn!("helper model download failed: {e}");
            Err(e)
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn helper_cancel_download(helper: State<'_, Helper>) {
    helper.cancel_download.store(true, Ordering::SeqCst);
}

/// Unloads and deletes the model (and any partial download).
#[tauri::command]
#[specta::specta]
pub fn helper_delete_model(state: State<'_, AppState>, helper: State<'_, Helper>) -> Result<(), String> {
    if helper.downloading.load(Ordering::SeqCst) || helper.busy.load(Ordering::SeqCst) {
        return Err("Stop the download or reply first.".into());
    }
    *helper.engine.lock().expect("poisoned") = None;
    model::delete(&models_dir(&state), &DEFAULT_MODEL).map_err(|e| format!("Couldn't delete the model: {e}"))
}

/// Frees the model's memory (it loads again on the next question).
#[tauri::command]
#[specta::specta]
pub fn helper_unload(helper: State<'_, Helper>) {
    helper.stop.store(true, Ordering::SeqCst);
    *helper.engine.lock().expect("poisoned") = None;
}

#[derive(Debug, Clone, Deserialize, Type)]
pub struct ChatTurn {
    /// `system`, `user` or `assistant`.
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HelperEvent {
    /// The model is being loaded (first question after start).
    Loading,
    /// The model is reading the prompt.
    Thinking,
    /// More of the reply.
    Text { text: String },
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HelperReply {
    pub text: String,
    /// `done`, `length` or `stopped`.
    pub finish: String,
    pub prompt_tokens: u32,
    pub tokens: u32,
    pub seconds: f64,
}

async fn engine(app: &AppHandle, on_event: &Channel<HelperEvent>) -> Result<Arc<Engine>, String> {
    let helper = app.state::<Helper>();
    let _guard = helper.loading.lock().await;
    if let Some(e) = helper.engine.lock().expect("poisoned").clone() {
        return Ok(e);
    }
    let path = model::model_path(&models_dir(&app.state::<AppState>()), &DEFAULT_MODEL);
    if model::state(&models_dir(&app.state::<AppState>()), &DEFAULT_MODEL) != ModelState::Ready {
        return Err("Download the model first (Settings → Helper).".into());
    }
    let _ = on_event.send(HelperEvent::Loading);
    let started = Instant::now();
    let e = tauri::async_runtime::spawn_blocking(move || Engine::load(&path, LoadOptions::default()))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    tracing::info!(secs = started.elapsed().as_secs_f32(), gpu = nightgrid_llm::gpu_build(), "helper model loaded");
    let e = Arc::new(e);
    *helper.engine.lock().expect("poisoned") = Some(e.clone());
    Ok(e)
}

/// Generates DRIFT's reply to `messages`, streaming it over `on_event`.
#[tauri::command]
#[specta::specta]
pub async fn helper_ask(
    app: AppHandle,
    messages: Vec<ChatTurn>,
    on_event: Channel<HelperEvent>,
) -> Result<HelperReply, String> {
    let settings =
        nightgrid_core::settings::AppSettings::load(&app.state::<AppState>().store()).map_err(|e| e.to_string())?;
    if !settings.helper_enabled {
        return Err("The helper is off (Settings → Helper).".into());
    }
    if messages.is_empty() || messages.iter().any(|m| !matches!(m.role.as_str(), "system" | "user" | "assistant")) {
        return Err("bad conversation".into());
    }
    let helper = app.state::<Helper>();
    if helper.busy.swap(true, Ordering::SeqCst) {
        return Err("DRIFT is still answering.".into());
    }
    helper.stop.store(false, Ordering::SeqCst);
    let r = async {
        let engine = engine(&app, &on_event).await?;
        let stop = helper.stop.clone();
        let msgs: Vec<Message> = messages.into_iter().map(|m| Message { role: m.role, content: m.content }).collect();
        let ch = on_event.clone();
        let started = Instant::now();
        let _ = on_event.send(HelperEvent::Thinking);
        let out = tauri::async_runtime::spawn_blocking(move || {
            engine.generate(&msgs, &GenOptions::default(), &stop, |t| {
                let _ = ch.send(HelperEvent::Text { text: t.to_owned() });
            })
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
        let seconds = started.elapsed().as_secs_f64();
        tracing::debug!(prompt = out.prompt_tokens, tokens = out.tokens, seconds, "helper reply");
        Ok(HelperReply {
            text: out.text,
            finish: match out.finish {
                Finish::Done => "done",
                Finish::Length => "length",
                Finish::Cancelled => "stopped",
            }
            .into(),
            prompt_tokens: out.prompt_tokens as u32,
            tokens: out.tokens as u32,
            seconds,
        })
    }
    .await;
    helper.busy.store(false, Ordering::SeqCst);
    r
}

/// Stops the reply being generated (what's there so far is kept).
#[tauri::command]
#[specta::specta]
pub fn helper_stop(helper: State<'_, Helper>) {
    helper.stop.store(true, Ordering::SeqCst);
}
