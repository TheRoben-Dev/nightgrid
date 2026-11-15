//! Automation (M8): snippets and playbooks CRUD, plus runs on many hosts.
//!
//! A run is one plan (an ad-hoc command, a snippet or a playbook) on a set
//! of hosts: up to [`PARALLEL`] hosts at once, steps in order on each, with
//! output streamed per host. Connections come from the pool and never
//! prompt: a host that needs a login or an unknown host key fails with that
//! reason. Root goes through the same sudo path as the System view; the
//! password typed for a run is used for the hosts that ask and then dropped.
//! Destructive plans were confirmed in the UI (security rule 5) and are
//! logged here with their hosts.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use nightgrid_core::automation::run::{self, OutStream};
use nightgrid_core::automation::template::{self, Variable};
use nightgrid_core::automation::{Playbook, PlaybookId, PlaybookInput, Snippet, SnippetId, SnippetInput, Step};
use nightgrid_core::hosts::{Host, HostId};
use nightgrid_core::ssh::client::ConnectError;
use nightgrid_core::system::sudo::SudoAuth;
use nightgrid_core::system::SystemError;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tokio::sync::{mpsc, watch, Semaphore};

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;
use crate::system_cmd::{resolve_sudo, SudoInput};
use crate::term_cmd::connect_host;

/// Hosts worked on at the same time.
const PARALLEL: usize = 8;
pub type AutomationRunId = u32;

#[tauri::command]
#[specta::specta]
pub fn snippets_list(state: State<'_, AppState>) -> CmdResult<Vec<Snippet>> {
    Ok(state.store().list_snippets()?)
}

/// Creates (`id` = null) or updates a snippet.
#[tauri::command]
#[specta::specta]
pub fn snippet_save(state: State<'_, AppState>, id: Option<SnippetId>, input: SnippetInput) -> CmdResult<Snippet> {
    let mut store = state.store();
    Ok(match id {
        Some(id) => store.update_snippet(id, &input)?,
        None => store.create_snippet(&input)?,
    })
}

#[tauri::command]
#[specta::specta]
pub fn snippet_delete(state: State<'_, AppState>, id: SnippetId) -> CmdResult<()> {
    Ok(state.store().delete_snippet(id)?)
}

#[tauri::command]
#[specta::specta]
pub fn playbooks_list(state: State<'_, AppState>) -> CmdResult<Vec<Playbook>> {
    Ok(state.store().list_playbooks()?)
}

#[tauri::command]
#[specta::specta]
pub fn playbook_save(state: State<'_, AppState>, id: Option<PlaybookId>, input: PlaybookInput) -> CmdResult<Playbook> {
    let mut store = state.store();
    Ok(match id {
        Some(id) => store.update_playbook(id, &input)?,
        None => store.create_playbook(&input)?,
    })
}

#[tauri::command]
#[specta::specta]
pub fn playbook_delete(state: State<'_, AppState>, id: PlaybookId) -> CmdResult<()> {
    Ok(state.store().delete_playbook(id)?)
}

/// What to run.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RunSpec {
    Command { command: String, sudo: bool },
    Snippet { id: SnippetId },
    Playbook { id: PlaybookId },
}

struct Plan {
    name: String,
    steps: Vec<Step>,
    stop_on_error: bool,
}

fn plan(state: &AppState, spec: &RunSpec) -> CmdResult<Plan> {
    Ok(match spec {
        RunSpec::Command { command, sudo } => {
            if command.trim().is_empty() {
                return Err(CommandError::msg("enter a command"));
            }
            Plan {
                name: "command".into(),
                steps: vec![Step::Command { command: command.clone(), sudo: *sudo }],
                stop_on_error: true,
            }
        }
        RunSpec::Snippet { id } => {
            let s = state.store().get_snippet(*id)?;
            Plan { name: s.name, steps: vec![Step::Command { command: s.command, sudo: s.sudo }], stop_on_error: true }
        }
        RunSpec::Playbook { id } => {
            let p = state.store().get_playbook(*id)?;
            Plan { name: p.name, steps: p.steps, stop_on_error: p.stop_on_error }
        }
    })
}

/// Renders a step for one host. Commands get shell-quoted values; paths,
/// hosts and names are used as typed.
fn render_step(step: &Step, values: &HashMap<String, String>) -> Result<Step, Vec<String>> {
    let r = |t: &str| template::render(t, values, false);
    Ok(match step {
        Step::Command { command, sudo } => {
            Step::Command { command: template::render(command, values, true)?, sudo: *sudo }
        }
        Step::Upload { local_path, remote_path, mode } => {
            Step::Upload { local_path: r(local_path)?, remote_path: r(remote_path)?, mode: *mode }
        }
        Step::WaitPort { host, port, timeout_secs } => {
            Step::WaitPort { host: r(host)?, port: *port, timeout_secs: *timeout_secs }
        }
        Step::Docker { action, target } => Step::Docker { action: *action, target: r(target)? },
    })
}

fn host_values(user: &HashMap<String, String>, h: &Host) -> HashMap<String, String> {
    let mut v = user.clone();
    v.extend(template::builtins(&h.name, &h.hostname, h.username.as_deref().unwrap_or("")));
    v
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunInspect {
    pub name: String,
    pub variables: Vec<Variable>,
    /// What the plan destroys or overwrites, "step N: ..." for playbooks.
    pub destructive: Vec<String>,
    pub needs_sudo: bool,
    /// Step labels, rendered for `host_id` where values allow.
    pub steps: Vec<String>,
    /// The first command rendered for `host_id` (commands and snippets).
    pub preview: Option<String>,
    /// Variables still without a value.
    pub missing: Vec<String>,
}

/// Variables, destructive actions and a preview, for the run form. Danger is
/// checked on the text as typed and as rendered, so a value can't hide it.
#[tauri::command]
#[specta::specta]
pub fn automation_inspect(
    state: State<'_, AppState>,
    spec: RunSpec,
    values: HashMap<String, String>,
    host_id: Option<HostId>,
) -> CmdResult<RunInspect> {
    let p = plan(&state, &spec)?;
    let host = match host_id {
        Some(id) => state.store().get_host(id)?,
        None => None,
    };
    let vals = match &host {
        Some(h) => host_values(&values, h),
        None => values.clone(),
    };
    let multi = p.steps.len() > 1;
    let mut destructive = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut steps = Vec::new();
    let mut preview = None;
    for (i, s) in p.steps.iter().enumerate() {
        let rendered = match render_step(s, &vals) {
            Ok(r) => Some(r),
            Err(m) => {
                for v in m {
                    if !missing.contains(&v) && !template::BUILTINS.contains(&v.as_str()) {
                        missing.push(v);
                    }
                }
                None
            }
        };
        let shown = rendered.as_ref().unwrap_or(s);
        steps.push(shown.label());
        if preview.is_none() && !multi {
            if let Step::Command { command, .. } = shown {
                preview = Some(command.clone());
            }
        }
        // The rendered step names the real targets; a command's own text is
        // checked too, so a value can't hide what it does.
        let mut found = match &rendered {
            Some(r) => r.destructive(),
            None => s.destructive(),
        };
        if rendered.is_some() && matches!(s, Step::Command { .. }) {
            for d in s.destructive() {
                if !found.contains(&d) {
                    found.push(d);
                }
            }
        }
        for d in found {
            let d = if multi { format!("step {}: {d}", i + 1) } else { d };
            if !destructive.contains(&d) {
                destructive.push(d);
            }
        }
    }
    Ok(RunInspect {
        name: p.name,
        variables: template::variables(p.steps.iter().flat_map(Step::texts)),
        destructive,
        needs_sudo: p.steps.iter().any(Step::needs_sudo),
        steps,
        preview,
        missing,
    })
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OutLine {
    pub stream: OutStream,
    pub text: String,
}

/// Why a host couldn't start, when the user can fix it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum HostNeeds {
    /// Connect once from a terminal (host key or password prompt).
    Login,
    /// A sudo password: type one for the run or remember it in System.
    Sudo,
}

/// What a run's channel carries.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AutomationEvent {
    HostStart {
        host_id: HostId,
    },
    StepStart {
        host_id: HostId,
        step: u32,
        label: String,
    },
    Lines {
        host_id: HostId,
        lines: Vec<OutLine>,
    },
    StepEnd {
        host_id: HostId,
        step: u32,
        code: Option<i32>,
        error: Option<String>,
        ms: u32,
    },
    HostEnd {
        host_id: HostId,
        ok: bool,
        /// Exit code of the last command step that ran.
        code: Option<i32>,
        error: Option<String>,
        needs: Option<HostNeeds>,
        ms: u32,
        stopped: bool,
    },
    Done {
        stopped: bool,
    },
}

fn ms(since: Instant) -> u32 {
    since.elapsed().as_millis().min(u32::MAX as u128) as u32
}

fn expand_home(state: &AppState, p: &str) -> PathBuf {
    match (p.strip_prefix("~/"), &state.home) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(p),
    }
}

/// Runs `spec` on `host_ids`. Destructive plans were confirmed in the UI
/// with the hosts named (and the group or host count typed for several).
#[tauri::command]
#[specta::specta]
pub async fn automation_run(
    app: AppHandle,
    state: State<'_, AppState>,
    spec: RunSpec,
    host_ids: Vec<HostId>,
    values: HashMap<String, String>,
    sudo_password: Option<String>,
    on_event: Channel<AutomationEvent>,
) -> CmdResult<AutomationRunId> {
    let p = plan(&state, &spec)?;
    let mut hosts = Vec::new();
    for id in &host_ids {
        if let Some(h) = state.store().get_host(*id)? {
            if !hosts.iter().any(|x: &Host| x.id == h.id) {
                hosts.push(h);
            }
        }
    }
    if hosts.is_empty() {
        return Err(CommandError::msg("choose at least one host"));
    }
    // Every variable needs a value before anything starts.
    for h in &hosts {
        for s in &p.steps {
            if let Err(m) = render_step(s, &host_values(&values, h)) {
                return Err(CommandError::msg(format!("fill in: {}", m.join(", "))));
            }
        }
    }
    let destructive: Vec<String> = p.steps.iter().flat_map(Step::destructive).collect();
    let names: Vec<&str> = hosts.iter().map(|h| h.name.as_str()).collect();
    if destructive.is_empty() {
        tracing::info!(plan = %p.name, hosts = ?names, "automation run");
    } else {
        tracing::info!(plan = %p.name, hosts = ?names, destructive = ?destructive, "automation run (destructive)");
    }

    let id = state.next_term_id();
    let cancel = state.system_runs.add(id);
    let plan = Arc::new(p);
    let values = Arc::new(values);
    let sudo_password = Arc::new(sudo_password.filter(|s| !s.is_empty()));
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let (tx, mut rx) = mpsc::unbounded_channel::<AutomationEvent>();
        let ch = on_event.clone();
        let forward = tauri::async_runtime::spawn(async move {
            while let Some(first) = rx.recv().await {
                let mut batch = vec![first];
                while batch.len() < 500 {
                    match rx.try_recv() {
                        Ok(e) => batch.push(e),
                        Err(_) => break,
                    }
                }
                for e in coalesce(batch) {
                    let _ = ch.send(e);
                }
            }
        });
        let gate = Arc::new(Semaphore::new(PARALLEL));
        let mut tasks = Vec::new();
        for h in hosts {
            let (app, plan, values, pw, tx, cancel, gate) = (
                app2.clone(),
                plan.clone(),
                values.clone(),
                sudo_password.clone(),
                tx.clone(),
                cancel.clone(),
                gate.clone(),
            );
            tasks.push(tauri::async_runtime::spawn(async move {
                let _permit = gate.acquire_owned().await;
                run_host(&app, &plan, &h, &values, pw.as_deref(), cancel, &tx).await;
            }));
        }
        drop(tx);
        for t in tasks {
            let _ = t.await;
        }
        let _ = forward.await;
        let stopped = *cancel.borrow();
        let _ = on_event.send(AutomationEvent::Done { stopped });
        app2.state::<AppState>().system_runs.remove(id);
    });
    Ok(id)
}

/// Merges consecutive line batches for the same host.
fn coalesce(batch: Vec<AutomationEvent>) -> Vec<AutomationEvent> {
    let mut out: Vec<AutomationEvent> = Vec::with_capacity(batch.len());
    for e in batch {
        if let AutomationEvent::Lines { host_id, lines } = &e {
            if let Some(AutomationEvent::Lines { host_id: h, lines: l }) = out.last_mut() {
                if h == host_id {
                    l.extend(lines.iter().cloned());
                    continue;
                }
            }
        }
        out.push(e);
    }
    out
}

async fn run_host(
    app: &AppHandle,
    plan: &Plan,
    h: &Host,
    values: &HashMap<String, String>,
    sudo_password: Option<&str>,
    cancel: watch::Receiver<bool>,
    tx: &mpsc::UnboundedSender<AutomationEvent>,
) {
    let host_id = h.id;
    let t0 = Instant::now();
    let send = |e: AutomationEvent| {
        let _ = tx.send(e);
    };
    let end = |ok: bool, code: Option<i32>, error: Option<String>, needs: Option<HostNeeds>, stopped: bool| {
        send(AutomationEvent::HostEnd { host_id, ok, code, error, needs, ms: ms(t0), stopped });
    };
    if *cancel.borrow() {
        return end(false, None, None, None, true);
    }
    send(AutomationEvent::HostStart { host_id });
    let conn = match connect_host(app, host_id).await {
        Ok(c) => c,
        Err(e) => {
            let needs = match e {
                ConnectError::HostKeyUnknown { .. }
                | ConnectError::HostKeyChanged { .. }
                | ConnectError::PassphraseRequired { .. }
                | ConnectError::PasswordRequired { .. } => Some(HostNeeds::Login),
                _ => None,
            };
            return end(false, None, Some(e.to_string()), needs, false);
        }
    };
    let auth = if plan.steps.iter().any(Step::needs_sudo) {
        let input = sudo_password.map(|p| SudoInput { password: p.to_owned(), remember: false });
        match resolve_sudo(app, host_id, &conn, input).await {
            Ok(a) => a,
            Err(e) => {
                let needs =
                    matches!(e.system.as_deref(), Some(SystemError::SudoRequired { .. })).then_some(HostNeeds::Sudo);
                return end(false, None, Some(e.message), needs, false);
            }
        }
    } else {
        SudoAuth::User
    };
    let vals = host_values(values, h);
    let (mut ok, mut last_code, mut first_error) = (true, None, None::<String>);
    for (i, step) in plan.steps.iter().enumerate() {
        if *cancel.borrow() {
            return end(false, last_code, first_error, None, true);
        }
        let step_no = i as u32;
        let ts = Instant::now();
        let step = match render_step(step, &vals) {
            Ok(s) => s,
            Err(m) => {
                return end(false, last_code, Some(format!("missing values: {}", m.join(", "))), None, false);
            }
        };
        send(AutomationEvent::StepStart { host_id, step: step_no, label: step.label() });
        let mut on_line = |stream: OutStream, text: String| {
            let _ = tx.send(AutomationEvent::Lines { host_id, lines: vec![OutLine { stream, text }] });
        };
        let info = |text: String| {
            let _ = tx.send(AutomationEvent::Lines { host_id, lines: vec![OutLine { stream: OutStream::Info, text }] });
        };
        let result: Result<Option<i32>, String> = match &step {
            Step::Command { command, sudo } => {
                let a = if *sudo { &auth } else { &SudoAuth::User };
                run::command(&conn, a, command, cancel.clone(), &mut on_line).await
            }
            Step::Docker { action, target } => match run::docker_command(*action, target) {
                Ok(cmd) => run::command(&conn, &SudoAuth::User, &cmd, cancel.clone(), &mut on_line).await,
                Err(e) => Err(e),
            },
            Step::Upload { local_path, remote_path, mode } => {
                let local = expand_home(&app.state::<AppState>(), local_path);
                run::upload(conn.clone(), &local, remote_path, *mode).await.map(|n| {
                    info(format!("uploaded {} to {remote_path}", run::human_bytes(n)));
                    Some(0)
                })
            }
            Step::WaitPort { host, port, timeout_secs } => {
                run::wait_port(&conn, host, *port, Duration::from_secs((*timeout_secs).into()), cancel.clone())
                    .await
                    .map(|d| {
                        d.map(|d| {
                            info(format!("{host}:{port} is open ({:.1} s)", d.as_secs_f32()));
                            0
                        })
                    })
            }
        };
        let (code, error) = match result {
            Ok(None) => {
                send(AutomationEvent::StepEnd { host_id, step: step_no, code: None, error: None, ms: ms(ts) });
                return end(false, last_code, first_error, None, true);
            }
            Ok(Some(c)) => (Some(c), None),
            Err(e) => (None, Some(e)),
        };
        if matches!(step, Step::Command { .. } | Step::Docker { .. }) {
            last_code = code;
        }
        send(AutomationEvent::StepEnd { host_id, step: step_no, code, error: error.clone(), ms: ms(ts) });
        let failed = error.is_some() || code != Some(0);
        if failed {
            ok = false;
            if first_error.is_none() {
                first_error = error.or_else(|| code.map(|c| format!("step {} exited with {c}", i + 1)));
            }
            if plan.stop_on_error {
                break;
            }
        }
    }
    // A single command's exit code speaks for itself.
    if plan.steps.len() == 1 && first_error.as_deref().is_some_and(|e| e.contains("exited with")) {
        first_error = None;
    }
    end(ok, last_code, first_error, None, false);
}

#[tauri::command]
#[specta::specta]
pub fn automation_stop(state: State<'_, AppState>, run: AutomationRunId) -> CmdResult<()> {
    state.system_runs.stop(run);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesces_lines_per_host() {
        let l = |h: HostId, t: &str| AutomationEvent::Lines {
            host_id: h,
            lines: vec![OutLine { stream: OutStream::Out, text: t.into() }],
        };
        let out = coalesce(vec![l(1, "a"), l(1, "b"), l(2, "c"), AutomationEvent::HostStart { host_id: 1 }, l(1, "d")]);
        assert_eq!(out.len(), 4);
        match &out[0] {
            AutomationEvent::Lines { lines, .. } => assert_eq!(lines.len(), 2),
            e => panic!("{e:?}"),
        }
    }
}
