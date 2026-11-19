//! DRIFT, the local helper: model download/state and streaming replies.
//!
//! The webview builds the prompt (retrieval over the bundled guide, the live
//! view context, the "show me" targets) and shows it to the user before it's
//! sent; this side only runs the model. Nothing leaves the machine: the only
//! network use is the model download the user starts, from a pinned URL with
//! a pinned SHA-256 (see `nightgrid_core::helper::model`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use nightgrid_core::helper::hardware::{self, Hardware};
use nightgrid_core::helper::model::{self, DownloadEnd, ModelSpec, ModelState, Policy, MODELS};
use nightgrid_core::helper::think::{self, ThinkFilter};
use nightgrid_llm::{Engine, Finish, GenOptions, LoadOptions, Message};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::state::AppState;

/// Runtime state of the helper (not persisted).
#[derive(Default)]
pub struct Helper {
    /// The loaded model and its id.
    engine: Mutex<Option<(&'static str, Arc<Engine>)>>,
    /// Serializes model loading so two first questions don't load twice.
    loading: tokio::sync::Mutex<()>,
    /// The model being downloaded (one at a time).
    downloading: Mutex<Option<&'static str>>,
    cancel_download: Arc<AtomicBool>,
    /// Set to stop the reply being generated.
    stop: Arc<AtomicBool>,
    busy: AtomicBool,
    /// Read once per run: hardware doesn't change while the app is open.
    hardware: OnceLock<Hardware>,
}

fn models_dir(state: &AppState) -> PathBuf {
    state.data_dir.join("models")
}

/// The model the settings pick (the default if the id is unknown).
fn active(state: &AppState) -> &'static ModelSpec {
    nightgrid_core::settings::AppSettings::load(&state.store())
        .ok()
        .and_then(|s| model::model_by_id(&s.helper_model))
        .unwrap_or(&MODELS[0])
}

fn spec(id: &str) -> Result<&'static ModelSpec, String> {
    model::model_by_id(id).ok_or_else(|| "That model isn't offered.".into())
}

fn state_of(dir: &std::path::Path, m: &ModelSpec) -> (&'static str, u64) {
    match model::state(dir, m) {
        ModelState::Missing => ("missing", 0),
        ModelState::Partial(n) => ("partial", n),
        ModelState::Ready => ("ready", m.bytes),
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HelperStatus {
    /// The model in use (from settings).
    pub id: String,
    pub model: String,
    pub license: String,
    /// Full size of the model file in bytes.
    pub bytes: f64,
    /// `missing`, `partial` or `ready`.
    pub state: String,
    /// Bytes already on disk for a partial download.
    pub have: f64,
    /// A download is running (of any model).
    pub downloading: bool,
    /// Which model is downloading.
    pub downloading_id: Option<String>,
    pub loaded: bool,
    /// The build offloads to the GPU (Metal on macOS); otherwise CPU.
    pub gpu: bool,
    /// The loaded model's context window in tokens (0 until it's loaded).
    pub context: u32,
}

fn status(state: &AppState, helper: &Helper) -> HelperStatus {
    let m = active(state);
    let (st, have) = state_of(&models_dir(state), m);
    let dl = *helper.downloading.lock().expect("poisoned");
    let loaded = helper.engine.lock().expect("poisoned").as_ref().filter(|(id, _)| *id == m.id).map(|(_, e)| e.clone());
    HelperStatus {
        id: m.id.into(),
        model: m.name.into(),
        license: m.license.into(),
        bytes: m.bytes as f64,
        state: st.into(),
        have: have as f64,
        downloading: dl.is_some(),
        downloading_id: dl.map(Into::into),
        loaded: loaded.is_some(),
        context: loaded.map_or(0, |e| e.context()),
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
pub struct GpuView {
    pub name: String,
    /// Dedicated memory in bytes (0: shared or unknown).
    pub vram: f64,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HardwareView {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub cores: u32,
    /// Bytes (0: couldn't read).
    pub memory: f64,
    pub unified: bool,
    pub gpus: Vec<GpuView>,
    pub gpu_build: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelOption {
    pub id: String,
    pub name: String,
    /// `light`, `balanced` or `best`.
    pub tier: String,
    pub params: String,
    pub blurb: String,
    pub license: String,
    pub bytes: f64,
    /// Memory it needs while answering.
    pub need: f64,
    /// `missing`, `partial` or `ready`.
    pub state: String,
    pub have: f64,
    /// `good`, `tight`, `no` or `unknown`.
    pub fit: String,
    /// Why, in a line.
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HelperModels {
    pub hardware: HardwareView,
    pub recommended: String,
    pub active: String,
    pub models: Vec<ModelOption>,
}

/// The models on offer, how each fits this computer, and which to pick.
#[tauri::command]
#[specta::specta]
pub async fn helper_models(app: AppHandle) -> Result<HelperModels, String> {
    let helper = app.state::<Helper>();
    let hw = match helper.hardware.get() {
        Some(h) => h.clone(),
        None => {
            let h = tauri::async_runtime::spawn_blocking(|| hardware::detect(nightgrid_llm::gpu_build()))
                .await
                .map_err(|e| e.to_string())?;
            tracing::info!(memory = h.memory, cpu = %h.cpu, gpus = h.gpus.len(), "hardware read for model picks");
            helper.hardware.get_or_init(|| h).clone()
        }
    };
    let state = app.state::<AppState>();
    let dir = models_dir(&state);
    let models = MODELS
        .iter()
        .map(|m| {
            let (st, have) = state_of(&dir, m);
            ModelOption {
                id: m.id.into(),
                name: m.name.into(),
                tier: m.tier.into(),
                params: m.params.into(),
                blurb: m.blurb.into(),
                license: m.license.into(),
                bytes: m.bytes as f64,
                need: hardware::needed(m) as f64,
                state: st.into(),
                have: have as f64,
                fit: hardware::fit(&hw, m).as_str().into(),
                reason: hardware::reason(&hw, m),
            }
        })
        .collect();
    Ok(HelperModels {
        recommended: hardware::recommend(&hw, MODELS).id.into(),
        active: active(&state).id.into(),
        models,
        hardware: HardwareView {
            os: hw.os,
            arch: hw.arch,
            cpu: hw.cpu,
            cores: hw.cores,
            memory: hw.memory as f64,
            unified: hw.unified,
            gpus: hw.gpus.into_iter().map(|g| GpuView { name: g.name, vram: g.vram as f64 }).collect(),
            gpu_build: hw.gpu_build,
        },
    })
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub done: f64,
    pub total: f64,
}

/// Downloads (or resumes) a model, then verifies its checksum. Returns
/// `false` if it was cancelled. The model in use stays in use.
#[tauri::command]
#[specta::specta]
pub async fn helper_download(
    app: AppHandle,
    model_id: String,
    on_progress: Channel<DownloadProgress>,
) -> Result<bool, String> {
    let m = spec(&model_id)?;
    let helper = app.state::<Helper>();
    {
        let mut d = helper.downloading.lock().expect("poisoned");
        if d.is_some() {
            return Err("A model is already downloading.".into());
        }
        *d = Some(m.id);
    }
    helper.cancel_download.store(false, Ordering::SeqCst);
    let dir = models_dir(&app.state::<AppState>());
    let cancel = helper.cancel_download.clone();
    let mut last = Instant::now() - Duration::from_secs(1);
    tracing::info!(url = m.url, "helper model download started");
    let r = model::download(&dir, m, &Policy::default(), &cancel, |done, total| {
        if done == total || last.elapsed() >= Duration::from_millis(200) {
            last = Instant::now();
            let _ = on_progress.send(DownloadProgress { done: done as f64, total: total as f64 });
        }
    })
    .await;
    *helper.downloading.lock().expect("poisoned") = None;
    match r {
        Ok(DownloadEnd::Done) => {
            tracing::info!(model = m.id, "helper model downloaded and verified");
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

/// Deletes a model's file (and any partial download), unloading it first.
#[tauri::command]
#[specta::specta]
pub fn helper_delete_model(
    state: State<'_, AppState>,
    helper: State<'_, Helper>,
    model_id: String,
) -> Result<(), String> {
    let m = spec(&model_id)?;
    if *helper.downloading.lock().expect("poisoned") == Some(m.id) {
        return Err("Stop the download first.".into());
    }
    let mut e = helper.engine.lock().expect("poisoned");
    if e.as_ref().is_some_and(|(id, _)| *id == m.id) {
        if helper.busy.load(Ordering::SeqCst) {
            return Err("Stop the reply first.".into());
        }
        *e = None;
    }
    drop(e);
    model::delete(&models_dir(&state), m).map_err(|e| format!("Couldn't delete the model: {e}"))
}

/// Frees the model before the app quits or restarts. llama.cpp's Metal
/// backend aborts in its static destructors (`ggml_metal_rsets_free`) when a
/// model's GPU buffers are still alive at `exit`, which crashed restarts
/// after an update or an import. Stops a reply in progress and waits briefly
/// for it to let go of the model.
pub fn shutdown(app: &AppHandle) {
    let Some(helper) = app.try_state::<Helper>() else { return };
    helper.stop.store(true, Ordering::SeqCst);
    let taken = helper.engine.lock().expect("poisoned").take();
    if let Some((_, e)) = taken {
        let until = Instant::now() + Duration::from_secs(3);
        while Arc::strong_count(&e) > 1 && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        if Arc::strong_count(&e) > 1 {
            tracing::warn!("helper model still in use at shutdown");
        }
        drop(e);
    }
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

async fn engine(app: &AppHandle, on_event: &Channel<HelperEvent>) -> Result<(&'static ModelSpec, Arc<Engine>), String> {
    let helper = app.state::<Helper>();
    let _guard = helper.loading.lock().await;
    let state = app.state::<AppState>();
    let m = active(&state);
    if let Some((id, e)) = helper.engine.lock().expect("poisoned").clone() {
        if id == m.id {
            return Ok((m, e));
        }
    }
    let dir = models_dir(&state);
    if model::state(&dir, m) != ModelState::Ready {
        return Err(format!("Download {} first (Settings → DRIFT).", m.name));
    }
    // Free the model that was in use before loading the next one.
    *helper.engine.lock().expect("poisoned") = None;
    let path = model::model_path(&dir, m);
    let _ = on_event.send(HelperEvent::Loading);
    let started = Instant::now();
    let hw = helper.hardware.get().cloned();
    let (e, context) = tauri::async_runtime::spawn_blocking(move || {
        // The window grows past 8k only when this computer has room for it.
        let hw = hw.unwrap_or_else(|| hardware::detect(nightgrid_llm::gpu_build()));
        let context = hardware::context_for(&hw, m);
        Engine::load(&path, LoadOptions { context, ..LoadOptions::default() }).map(|e| (e, context))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    tracing::info!(
        model = m.id,
        secs = started.elapsed().as_secs_f32(),
        context = e.context(),
        planned = context,
        gpu = nightgrid_llm::gpu_build(),
        "helper model loaded"
    );
    let e = Arc::new(e);
    *helper.engine.lock().expect("poisoned") = Some((m.id, e.clone()));
    Ok((m, e))
}

/// Generates DRIFT's reply to `messages`, streaming it over `on_event`.
#[tauri::command]
#[specta::specta]
pub async fn helper_ask(
    app: AppHandle,
    messages: Vec<ChatTurn>,
    on_event: Channel<HelperEvent>,
) -> Result<HelperReply, String> {
    let settings = nightgrid_core::settings::AppSettings::load(&app.state::<AppState>().store())
        .map_err(|e| e.to_string())?
        .effective();
    if !settings.helper_enabled {
        return Err("The helper is off (Settings → DRIFT).".into());
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
        let (spec, engine) = engine(&app, &on_event).await?;
        let stop = helper.stop.clone();
        let mut msgs: Vec<Message> =
            messages.into_iter().map(|m| Message { role: m.role, content: m.content }).collect();
        if spec.no_think {
            // Qwen3's soft switch: answer directly.
            if let Some(m) = msgs.iter_mut().find(|m| m.role == "system") {
                m.content.push_str("\n/no_think");
            }
        }
        let ch = on_event.clone();
        let started = Instant::now();
        let _ = on_event.send(HelperEvent::Thinking);
        let out = tauri::async_runtime::spawn_blocking(move || {
            let mut f = ThinkFilter::new();
            let gen = GenOptions::default();
            // The webview already fits the prompt to the window; this catches
            // what its estimate missed (older turns go first) instead of failing.
            let before = msgs.len();
            let fitted = engine.fit(&mut msgs, gen.max_tokens as usize)?;
            if msgs.len() < before {
                tracing::debug!(dropped = before - msgs.len(), tokens = fitted, "helper prompt trimmed to fit");
            }
            let r = engine.generate(&msgs, &gen, &stop, |t| {
                let t = f.push(t);
                if !t.is_empty() {
                    let _ = ch.send(HelperEvent::Text { text: t });
                }
            });
            let rest = f.finish();
            if !rest.is_empty() {
                let _ = ch.send(HelperEvent::Text { text: rest });
            }
            r
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
        let seconds = started.elapsed().as_secs_f64();
        tracing::debug!(prompt = out.prompt_tokens, tokens = out.tokens, seconds, "helper reply");
        Ok(HelperReply {
            text: think::strip(&out.text),
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

/// Commands DRIFT gets from a host's history, newest last.
const HISTORY_KEEP: usize = 60;
/// Older lines matching the question that DRIFT also gets (M10p).
const HISTORY_MATCHES: usize = 25;

/// Shell history for DRIFT: the newest lines plus older ones that match the question.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HostHistory {
    /// Newest lines, oldest first.
    pub recent: Vec<nightgrid_core::system::history::HistoryEntry>,
    /// Older lines that mention the question's words, best first.
    pub matches: Vec<nightgrid_core::system::history::HistoryEntry>,
}

/// Recent shell history from a host for DRIFT, when the host allows it
/// ("DRIFT may read shell history" in the host editor). Lines that look like
/// they hold a secret are dropped before they leave this side (same filter
/// as Tasks → History search).
#[tauri::command]
#[specta::specta]
pub async fn helper_host_history(
    app: AppHandle,
    host_id: nightgrid_core::hosts::HostId,
    terms: Vec<String>,
) -> crate::commands::CmdResult<HostHistory> {
    use crate::commands::CommandError;
    use nightgrid_core::system::history;
    let allowed = {
        let state = app.state::<AppState>();
        let store = state.store();
        store.get_host(host_id)?.is_some_and(|h| h.drift_history)
    };
    if !allowed {
        return Err(CommandError::msg("reading shell history is off for this host"));
    }
    let conn = crate::term_cmd::connect_posix(&app, host_id, "drift.history")
        .await
        .map_err(|e| CommandError::msg(e.to_string()))?;
    let out = tokio::time::timeout(Duration::from_secs(10), history::read(&conn))
        .await
        .map_err(|_| CommandError::msg("the host took too long to answer"))?
        .map_err(|e| CommandError::msg(e.to_string()))?;
    let all = history::parse(&out);
    let found = history::filter(&all, "");
    let mut recent: Vec<_> = found.entries.into_iter().take(HISTORY_KEEP).collect();
    recent.reverse();
    let terms: Vec<String> = terms.into_iter().take(8).collect();
    let matches = history::relevant(&all, &terms, HISTORY_MATCHES)
        .into_iter()
        .filter(|e| !recent.iter().any(|r| r.command == e.command))
        .collect();
    Ok(HostHistory { recent, matches })
}
