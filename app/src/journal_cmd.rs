//! The homelab journal (0.10.83): every change made from Nightgrid, across
//! all hosts, plus hand-written entries. See `core/src/journal.rs`.

use std::path::PathBuf;

use nightgrid_core::hosts::HostId;
use nightgrid_core::journal::{self, ChangeArea, ChangeEntry, ChangeQuery, NewEntry};
use nightgrid_core::services::check::now_unix;
use nightgrid_core::settings::AppSettings;
use nightgrid_core::timeline::{EventKind, Level};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;

/// One change to record. `diff` is a journal diff (`journal::text_diff`).
pub(crate) struct Change<'a> {
    pub host: Option<HostId>,
    pub area: ChangeArea,
    pub title: &'a str,
    pub detail: &'a str,
    pub diff: Option<String>,
    pub ok: bool,
}

impl<'a> Change<'a> {
    pub fn new(host: impl Into<Option<HostId>>, area: ChangeArea, title: &'a str) -> Self {
        Change { host: host.into(), area, title, detail: "", diff: None, ok: true }
    }
    pub fn detail(mut self, d: &'a str) -> Self {
        self.detail = d;
        self
    }
    pub fn diff(mut self, d: Option<String>) -> Self {
        self.diff = d;
        self
    }
    pub fn ok(mut self, ok: bool) -> Self {
        self.ok = ok;
        self
    }
}

/// Records a change. Failures are logged, never raised: the journal must
/// not break the action it describes. Nothing is recorded while the
/// journal is turned off in Settings → Features.
pub(crate) fn record(app: &AppHandle, c: Change<'_>) {
    let state = app.state::<AppState>();
    let store = state.store();
    if !AppSettings::load(&store).unwrap_or_default().feature("fleet.journal") {
        return;
    }
    let host = c.host.and_then(|id| store.get_host(id).ok().flatten()).map(|h| h.name).unwrap_or_default();
    let e = NewEntry {
        at: now_unix(),
        host_id: c.host,
        host,
        area: Some(c.area),
        title: c.title.to_owned(),
        detail: c.detail.to_owned(),
        diff: c.diff,
        ok: c.ok,
        manual: false,
    };
    if let Err(err) = store.journal_add(&e) {
        tracing::warn!("couldn't record a journal entry: {err}");
    }
}

/// Timeline events Nightgrid itself caused are changes too: the journal
/// gets them with the matching area.
pub(crate) fn from_timeline(app: &AppHandle, host: HostId, kind: EventKind, level: Level, title: &str, detail: &str) {
    let area = if title.starts_with("Proxmox") {
        ChangeArea::Proxmox
    } else {
        match kind {
            EventKind::Power => ChangeArea::Power,
            EventKind::Container => ChangeArea::Container,
            EventKind::Unit | EventKind::Packages => ChangeArea::System,
            EventKind::Stack => ChangeArea::Stack,
            EventKind::Automation => ChangeArea::Automation,
            // Alerts, boots and OOM kills are things that happened, not changes.
            EventKind::Alert | EventKind::Recovered | EventKind::Boot | EventKind::Oom => return,
        }
    };
    let failed = level == Level::Critical || format!("{title} {detail}").to_lowercase().contains("fail");
    record(app, Change::new(host, area, title).detail(detail).ok(!failed));
}

#[tauri::command]
#[specta::specta]
pub fn journal_query(state: State<'_, AppState>, query: ChangeQuery) -> CmdResult<Vec<ChangeEntry>> {
    Ok(state.store().journal_query(&query)?)
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeStats {
    pub count: u32,
    /// Unix seconds of the oldest entry.
    #[specta(type = Option<u32>)]
    pub oldest: Option<i64>,
}

#[tauri::command]
#[specta::specta]
pub fn journal_stats(state: State<'_, AppState>) -> CmdResult<ChangeStats> {
    let (count, oldest) = state.store().journal_stats()?;
    Ok(ChangeStats { count, oldest })
}

/// Adds a hand-written entry (`at`: Unix seconds, default now).
#[tauri::command]
#[specta::specta]
pub fn journal_note(
    state: State<'_, AppState>,
    host_id: Option<HostId>,
    title: String,
    detail: String,
    at: Option<u32>,
) -> CmdResult<ChangeEntry> {
    let title = title.trim();
    if title.is_empty() {
        return Err(CommandError::msg("a journal entry needs a title"));
    }
    let store = state.store();
    let host = host_id.and_then(|id| store.get_host(id).ok().flatten()).map(|h| h.name).unwrap_or_default();
    let now = now_unix();
    let at = at.map(i64::from).filter(|&a| a > 0 && a <= now + 86_400).unwrap_or(now);
    let id = store.journal_add(&NewEntry {
        at,
        host_id,
        host,
        area: Some(ChangeArea::Note),
        title: title.to_owned(),
        detail: detail.trim().to_owned(),
        diff: None,
        ok: true,
        manual: true,
    })?;
    let q = ChangeQuery { since: Some(at), until: Some(at), ..Default::default() };
    store.journal_query(&q)?.into_iter().find(|e| e.id == id).ok_or_else(|| CommandError::msg("the entry wasn't saved"))
}

#[tauri::command]
#[specta::specta]
pub fn journal_edit(state: State<'_, AppState>, id: u32, title: String, detail: String) -> CmdResult<()> {
    if title.trim().is_empty() {
        return Err(CommandError::msg("a journal entry needs a title"));
    }
    if !state.store().journal_edit(i64::from(id), &title, &detail)? {
        return Err(CommandError::msg("only entries you wrote can be edited"));
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn journal_delete(state: State<'_, AppState>, id: u32) -> CmdResult<()> {
    if !state.store().journal_delete(i64::from(id))? {
        return Err(CommandError::msg("only entries you wrote can be deleted"));
    }
    Ok(())
}

/// Forgets the whole journal. The UI confirms first.
#[tauri::command]
#[specta::specta]
pub fn journal_clear(state: State<'_, AppState>) -> CmdResult<u32> {
    Ok(state.store().journal_clear()?)
}

/// What changed, as text for DRIFT: entries for the given hosts (or all
/// hosts when empty) since `since`, newest first.
#[tauri::command]
#[specta::specta]
pub fn journal_for_drift(state: State<'_, AppState>, host_ids: Vec<HostId>, since: u32, max: u32) -> CmdResult<String> {
    let store = state.store();
    let max = max.clamp(1, 60) as usize;
    let since = i64::from(since);
    let mut all: Vec<ChangeEntry> = if host_ids.is_empty() {
        store.journal_query(&ChangeQuery { since: Some(since), limit: max as u32, ..Default::default() })?
    } else {
        let mut v = Vec::new();
        for h in host_ids {
            v.extend(store.journal_query(&ChangeQuery {
                host_id: Some(h),
                since: Some(since),
                limit: max as u32,
                ..Default::default()
            })?);
        }
        v
    };
    all.sort_by(|a, b| b.at.cmp(&a.at).then(b.id.cmp(&a.id)));
    Ok(journal::for_drift(&all, max))
}

async fn pick_save(app: &AppHandle) -> Option<PathBuf> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let name = format!("nightgrid-journal-{}.md", chrono::Local::now().format("%Y-%m-%d"));
    app.dialog().file().set_title("Export the journal").set_file_name(name).add_filter("Markdown", &["md"]).save_file(
        move |p| {
            let _ = tx.send(p);
        },
    );
    rx.await.ok().flatten().and_then(|p| p.into_path().ok())
}

/// Saves the entries matching `query` as a Markdown changelog. `None` when
/// cancelled.
#[tauri::command]
#[specta::specta]
pub async fn journal_export(app: AppHandle, query: ChangeQuery) -> CmdResult<Option<String>> {
    let entries = app.state::<AppState>().store().journal_query(&ChangeQuery { limit: 0, ..query })?;
    let Some(path) = pick_save(&app).await else { return Ok(None) };
    tokio::fs::write(&path, journal::to_markdown(&entries))
        .await
        .map_err(|e| CommandError::msg(format!("Couldn't write {}: {e}", path.display())))?;
    Ok(Some(path.display().to_string()))
}
