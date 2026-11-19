//! Lab blueprints (0.10.84): capture a host's setup into an encrypted file
//! and rebuild it on another host, step by step. See `core/src/blueprint.rs`.

use std::path::PathBuf;
use std::sync::Mutex;

use nightgrid_core::blueprint::{
    self, Blueprint, BlueprintSummary, BpCaddy, BpCrontab, BpEnv, BpFile, BpFirewall, BpInclude, BpStack, BpStep,
    BpStepKind, BpStepState, BpTarget, BpTraefik, BpZone,
};
use nightgrid_core::docker::compose::{ComposeOp, Stack};
use nightgrid_core::docker::snapshot::stack_files;
use nightgrid_core::files::text;
use nightgrid_core::files::transfer::Side;
use nightgrid_core::hosts::HostId;
use nightgrid_core::journal::ChangeArea;
use nightgrid_core::services::check::now_unix;
use nightgrid_core::starters;
use nightgrid_core::system::firewall::Backend;
use nightgrid_core::system::sudo;
use nightgrid_core::system::SystemError;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use zeroize::Zeroizing;

use crate::journal_cmd::{record, Change};
use crate::state::AppState;
use crate::system_cmd::{resolve_sudo, SudoInput, SysResult};
use crate::term_cmd::connect_posix;

/// The blueprint just captured (waiting to be saved) and the one opened
/// from a file (waiting to be applied). Held in memory only, never on disk
/// unencrypted, and dropped (zeroized) when replaced or closed.
static CAPTURED: Mutex<Option<Blueprint>> = Mutex::new(None);
static OPENED: Mutex<Option<Blueprint>> = Mutex::new(None);

fn other(e: impl std::fmt::Display) -> SystemError {
    SystemError::other(e.to_string())
}

fn again(s: &Option<SudoInput>) -> Option<SudoInput> {
    s.as_ref().map(|s| SudoInput { password: s.password.clone(), remember: s.remember })
}

/// A sudo prompt has to reach the UI instead of becoming a note.
fn needs_password(e: &crate::system_cmd::SystemCmdError) -> bool {
    matches!(e.system.as_deref(), Some(SystemError::SudoRequired { .. }))
}

fn host_name(app: &AppHandle, id: HostId) -> String {
    app.state::<AppState>().store().get_host(id).ok().flatten().map(|h| h.name).unwrap_or_else(|| format!("host {id}"))
}

async fn read_file(app: &AppHandle, host_id: HostId, path: &str) -> Result<Option<String>, String> {
    let fs = crate::files_cmd::fs_for(app, Side::Host { id: host_id }).await.map_err(|e| e.message)?;
    match fs.stat(path).await {
        Ok(None) => return Ok(None),
        Ok(Some(st)) if st.size as usize > blueprint::MAX_STACK_FILE => {
            return Err(format!("{path} is over {} KB and was left out", blueprint::MAX_STACK_FILE / 1024))
        }
        Ok(Some(_)) => {}
        Err(e) => return Err(e.to_string()),
    }
    text::read_text(&fs, path).await.map(|t| Some(t.content)).map_err(|e| e.to_string())
}

fn firewall_of(r: &nightgrid_core::system::firewall::FirewallReport) -> Option<BpFirewall> {
    match r.backend {
        Backend::Ufw => {
            let u = r.ufw.as_ref();
            Some(BpFirewall {
                backend: "ufw".into(),
                active: r.active,
                ufw_rules: u.map(|u| u.rules.iter().map(|x| x.spec.clone()).collect()).unwrap_or_default(),
                default_in: u.map(|u| u.default_in.clone()),
                zones: Vec::new(),
            })
        }
        Backend::Firewalld => Some(BpFirewall {
            backend: "firewalld".into(),
            active: r.active,
            ufw_rules: Vec::new(),
            default_in: None,
            zones: r
                .zones
                .iter()
                .map(|z| BpZone { name: z.name.clone(), services: z.services.clone(), ports: z.ports.clone() })
                .collect(),
        }),
        _ => None,
    }
}

/// Reads `host_id`'s setup into a blueprint kept in memory; returns what's
/// in it (no secret values). Parts that can't be read become notes.
#[tauri::command]
#[specta::specta]
pub async fn blueprint_capture(
    app: AppHandle,
    host_id: HostId,
    include: BpInclude,
    sudo: Option<SudoInput>,
) -> SysResult<BlueprintSummary> {
    let name = host_name(&app, host_id);
    let conn = connect_posix(&app, host_id, "blueprints").await?;
    let mut bp = Blueprint::new(&name, now_unix());
    if let Ok((_, out, _)) = conn.exec(". /etc/os-release 2>/dev/null && printf '%s' \"$PRETTY_NAME\"").await {
        bp.os = String::from_utf8_lossy(&out).trim().chars().take(80).collect();
    }

    // Stacks: compose files and .env, or the vault's values.
    if include.all_stacks || !include.stacks.is_empty() {
        match crate::docker_cmd::docker_stacks(app.clone(), host_id).await {
            Err(e) => bp.notes.push(format!("Stacks: {}", e.message)),
            Ok(stacks) => {
                for s in stacks.into_iter().filter(|s| s.in_folder && include.wants_stack(&s.name)) {
                    let env: Vec<BpEnv> = if include.env {
                        let state = app.state::<AppState>();
                        let vars = state.store().env_vars(&state.keychain, host_id, &s.name).unwrap_or_default();
                        let values = state.store().env_values(&state.keychain, host_id, &s.name);
                        match values {
                            Ok(v) => v
                                .into_iter()
                                .map(|(n, val)| BpEnv {
                                    secret: vars.iter().any(|x| x.name == n && x.secret),
                                    name: n,
                                    value: val.to_string(),
                                })
                                .collect(),
                            Err(e) => {
                                bp.notes.push(format!("{}: .env vault: {e}", s.name));
                                Vec::new()
                            }
                        }
                    } else {
                        Vec::new()
                    };
                    let mut files = Vec::new();
                    for path in stack_files(&s) {
                        // The vault's values make the .env; don't carry the old file too.
                        if !env.is_empty() && path.ends_with("/.env") {
                            continue;
                        }
                        match read_file(&app, host_id, &path).await {
                            Ok(Some(text)) => files.push(BpFile { path, text }),
                            Ok(None) => {}
                            Err(e) => bp.notes.push(format!("{}: {e}", s.name)),
                        }
                    }
                    if files.iter().all(|f| f.path.ends_with("/.env")) {
                        bp.notes.push(format!("{}: no compose file could be read", s.name));
                        continue;
                    }
                    bp.stacks.push(BpStack { name: s.name, dir: s.dir, files, env });
                }
            }
        }
    }

    if include.cron {
        match crate::jobs_cmd::cron_read(app.clone(), host_id, None, None).await {
            Ok(c) => bp.crontabs.push(BpCrontab { user: None, text: c.text }),
            Err(e) => bp.notes.push(format!("Crontab: {}", e.message)),
        }
    }
    if include.timers {
        match crate::jobs_cmd::timer_jobs(app.clone(), host_id).await {
            Ok(v) => bp.timers = v.into_iter().map(|j| j.job).collect(),
            Err(e) => bp.notes.push(format!("Timers: {}", e.message)),
        }
    }
    if include.firewall {
        match crate::firewall_cmd::firewall_read(app.clone(), host_id, again(&sudo)).await {
            Ok(v) => {
                bp.firewall = firewall_of(&v.report);
                if bp.firewall.is_none() {
                    bp.notes.push("Firewall: no ufw or firewalld to capture".into());
                }
            }
            Err(e) if needs_password(&e) => return Err(e),
            Err(e) => bp.notes.push(format!("Firewall: {}", e.message)),
        }
    }
    if include.proxy {
        match crate::proxy_cmd::proxy_edit_targets(app.clone(), host_id).await {
            Ok(t) => {
                bp.caddy = t
                    .caddy
                    .into_iter()
                    .filter(|c| !c.text.trim().is_empty())
                    .map(|c| BpCaddy { proxy_name: c.proxy_name, path: c.path, text: c.text })
                    .collect();
                bp.traefik = t
                    .traefik
                    .into_iter()
                    .filter(|t| !t.routes.is_empty())
                    .map(|t| BpTraefik { proxy_name: t.proxy_name, routes: t.routes })
                    .collect();
            }
            Err(e) => bp.notes.push(format!("Proxy routes: {}", e.message)),
        }
    }
    let summary = bp.summary();
    *CAPTURED.lock().expect("poisoned") = Some(bp);
    Ok(summary)
}

async fn pick_save(app: &AppHandle, host: &str) -> Option<PathBuf> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let safe: String = host.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
    let name = format!("{safe}-{}.ngblueprint", chrono::Local::now().format("%Y-%m-%d"));
    app.dialog()
        .file()
        .set_title("Save the blueprint")
        .set_file_name(name)
        .add_filter("Nightgrid blueprint", &["ngblueprint"])
        .save_file(move |p| {
            let _ = tx.send(p);
        });
    rx.await.ok().flatten().and_then(|p| p.into_path().ok())
}

async fn pick_open(app: &AppHandle) -> Option<PathBuf> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Open a blueprint")
        .add_filter("Nightgrid blueprint", &["ngblueprint", "age"])
        .pick_file(move |p| {
            let _ = tx.send(p);
        });
    rx.await.ok().flatten().and_then(|p| p.into_path().ok())
}

/// Encrypts the captured blueprint with `passphrase` into a file picked in
/// a save dialog. `None` when cancelled.
#[tauri::command]
#[specta::specta]
pub async fn blueprint_save(app: AppHandle, host_id: HostId, passphrase: String) -> SysResult<Option<String>> {
    let pass = Zeroizing::new(passphrase);
    let (bytes, host, stacks) = {
        let g = CAPTURED.lock().expect("poisoned");
        let bp = g.as_ref().ok_or_else(|| other("capture the host first"))?;
        (blueprint::encrypt(bp, &pass).map_err(other)?, bp.host.clone(), bp.stacks.len())
    };
    let Some(path) = pick_save(&app, &host).await else { return Ok(None) };
    let tmp = path.with_extension("ngblueprint.partial");
    {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp).map_err(|e| other(format!("couldn't write {}: {e}", tmp.display())))?;
        f.write_all(&bytes).and_then(|_| f.sync_all()).map_err(|e| other(format!("couldn't write: {e}")))?;
    }
    std::fs::rename(&tmp, &path).map_err(|e| other(format!("couldn't write {}: {e}", path.display())))?;
    *CAPTURED.lock().expect("poisoned") = None;
    let title = format!("Blueprint saved ({stacks} stack{})", if stacks == 1 { "" } else { "s" });
    let file = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    record(&app, Change::new(host_id, ChangeArea::Blueprint, &title).detail(&file));
    Ok(Some(path.display().to_string()))
}

/// Opens and decrypts a blueprint file picked in a dialog. `None` when cancelled.
#[tauri::command]
#[specta::specta]
pub async fn blueprint_open(app: AppHandle, passphrase: String) -> SysResult<Option<BlueprintSummary>> {
    let pass = Zeroizing::new(passphrase);
    let Some(path) = pick_open(&app).await else { return Ok(None) };
    let meta = std::fs::metadata(&path).map_err(|e| other(format!("couldn't read the file: {e}")))?;
    if meta.len() > blueprint::MAX_FILE_BYTES {
        return Err(other("that file is too big to be a blueprint").into());
    }
    let bytes = std::fs::read(&path).map_err(|e| other(format!("couldn't read the file: {e}")))?;
    let bp = blueprint::decrypt(&bytes, &pass).map_err(other)?;
    let s = bp.summary();
    *OPENED.lock().expect("poisoned") = Some(bp);
    Ok(Some(s))
}

/// Forgets the captured and opened blueprints.
#[tauri::command]
#[specta::specta]
pub fn blueprint_close() {
    *CAPTURED.lock().expect("poisoned") = None;
    *OPENED.lock().expect("poisoned") = None;
}

fn opened() -> SysResult<Blueprint> {
    OPENED.lock().expect("poisoned").clone().ok_or_else(|| other("open a blueprint first").into())
}

/// Replaces the opened blueprint (a starter built for a host, D-177).
pub(crate) fn set_opened(bp: Blueprint) {
    *OPENED.lock().expect("poisoned") = Some(bp);
}

/// Reads the target and lists what applying the opened blueprint would do.
#[tauri::command]
#[specta::specta]
pub async fn blueprint_plan(app: AppHandle, host_id: HostId, sudo: Option<SudoInput>) -> SysResult<Vec<BpStep>> {
    plan_for(&app, host_id, sudo).await.map(|(_, steps)| steps)
}

/// The opened blueprint, fitted to the target's firewall, and its plan.
async fn plan_for(app: &AppHandle, host_id: HostId, sudo: Option<SudoInput>) -> SysResult<(Blueprint, Vec<BpStep>)> {
    let app = app.clone();
    let mut bp = opened()?;
    let conn = connect_posix(&app, host_id, "blueprints").await?;
    let mut t = BpTarget::default();
    let paths = blueprint::stack_paths(&bp);
    if !paths.is_empty() {
        let (_, out, _) = conn.exec(&blueprint::exists_script(&paths)).await.map_err(other)?;
        t.existing = blueprint::parse_exists(&String::from_utf8_lossy(&out));
    }
    if !bp.stacks.is_empty() {
        if let Ok(s) = crate::docker_cmd::docker_stacks(app.clone(), host_id).await {
            t.docker = true;
            t.stacks = s.into_iter().map(|s| s.name).collect();
        }
    }
    for c in &bp.crontabs {
        if let Ok(cur) = crate::jobs_cmd::cron_read(app.clone(), host_id, c.user.clone(), again(&sudo)).await {
            t.crontabs.push((c.user.clone(), cur.text));
        }
    }
    if !bp.timers.is_empty() {
        if let Ok(v) = crate::jobs_cmd::timer_jobs(app.clone(), host_id).await {
            t.timers = v.into_iter().map(|j| j.job.name).collect();
        }
    }
    if bp.firewall.is_some() {
        match crate::firewall_cmd::firewall_read(app.clone(), host_id, again(&sudo)).await {
            Ok(v) => {
                t.firewall = firewall_of(&v.report);
                let zone = v.report.zones.iter().find(|z| z.default).map(|z| z.name.as_str());
                starters::fit_firewall(&mut bp, t.firewall.as_ref(), zone);
            }
            Err(e) if needs_password(&e) => return Err(e),
            Err(_) => starters::fit_firewall(&mut bp, None, None),
        }
    }
    if !bp.caddy.is_empty() || !bp.traefik.is_empty() {
        if let Ok(x) = crate::proxy_cmd::proxy_edit_targets(app.clone(), host_id).await {
            t.caddy = x.caddy.into_iter().map(|c| (c.proxy_name, c.text)).collect();
            t.traefik =
                x.traefik.into_iter().map(|t| (t.proxy_name, t.routes.into_iter().map(|r| r.name).collect())).collect();
        }
    }
    let steps = blueprint::plan(&bp, &t);
    Ok((bp, steps))
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BpStepResult {
    pub id: String,
    pub ok: bool,
    pub message: String,
}

async fn write_text(app: &AppHandle, host_id: HostId, path: &str, content: &str) -> Result<(), String> {
    let fs = crate::files_cmd::fs_for(app, Side::Host { id: host_id }).await.map_err(|e| e.message)?;
    text::save_text(&fs, path, content, None).await.map(|_| ()).map_err(|e| e.to_string())
}

async fn apply_stack(app: &AppHandle, host_id: HostId, s: &BpStack) -> Result<String, String> {
    let conn = connect_posix(app, host_id, "blueprints").await.map_err(|e| e.to_string())?;
    let (_, out, err) = conn.exec(&blueprint::mkdir_script(s)).await.map_err(|e| e.to_string())?;
    if !String::from_utf8_lossy(&out).contains("@@ok") {
        return Err(format!(
            "couldn't create {}: {} (a folder owned by root needs to be created and handed to your user first)",
            s.dir,
            String::from_utf8_lossy(&err).trim()
        ));
    }
    for f in &s.files {
        write_text(app, host_id, &f.path, &f.text).await.map_err(|e| format!("{}: {e}", f.path))?;
    }
    let mut msg = format!("{} file{} written", s.files.len(), if s.files.len() == 1 { "" } else { "s" });
    if !s.env.is_empty() {
        let vars: Vec<(String, Zeroizing<String>)> =
            s.env.iter().map(|e| (e.name.clone(), Zeroizing::new(e.value.clone()))).collect();
        {
            let state = app.state::<AppState>();
            let mut store = state.store();
            store.import_env(&state.keychain, host_id, &s.name, &vars).map_err(|e| e.to_string())?;
        }
        let rendered = nightgrid_core::envvault::render(&vars);
        write_text(app, host_id, &blueprint::env_path(s), &rendered).await.map_err(|e| format!(".env: {e}"))?;
        msg.push_str(&format!(", {} .env value{} in the vault", s.env.len(), if s.env.len() == 1 { "" } else { "s" }));
    }
    // So Docker → STACKS lists it even while it's stopped.
    if let Some((parent, _)) = s.dir.rsplit_once('/').filter(|(p, _)| !p.is_empty()) {
        let _ = app.state::<AppState>().store().add_stack_dir(host_id, parent);
    }
    record(
        app,
        Change::new(host_id, ChangeArea::Blueprint, &format!("Stack {} written from a blueprint", s.name))
            .detail(&s.dir),
    );
    Ok(msg)
}

async fn start_stack(app: &AppHandle, host_id: HostId, s: &BpStack) -> Result<String, String> {
    let d = crate::docker_cmd::docker_for(app, host_id).await.map_err(|e| e.message)?;
    let files: Vec<String> = s.files.iter().filter(|f| !f.path.ends_with("/.env")).map(|f| f.path.clone()).collect();
    let stack = Stack { name: s.name.clone(), dir: s.dir.clone(), files, running: 0, total: 0, in_folder: true };
    let (_tx, rx) = tokio::sync::watch::channel(false);
    let mut tail: Vec<String> = Vec::new();
    let mut sink = |lines: Vec<String>| {
        tail.extend(lines);
        let n = tail.len();
        if n > 6 {
            tail.drain(..n - 6);
        }
    };
    let (code, _) = d.compose(&stack, ComposeOp::Up, rx, &mut sink).await.map_err(|e| e.to_string())?;
    let ok = code == 0;
    let title = format!("Started stack {} from a blueprint", s.name);
    record(app, Change::new(host_id, ChangeArea::Stack, &title).detail(&format!("exit {code}")).ok(ok));
    if ok {
        Ok("started".into())
    } else {
        Err(format!("docker compose exited with {code}: {}", tail.join(" ").trim()))
    }
}

async fn run_root(app: &AppHandle, host_id: HostId, sudo: Option<SudoInput>, cmd: &str) -> Result<(), String> {
    let conn = connect_posix(app, host_id, "blueprints").await.map_err(|e| e.to_string())?;
    let auth = resolve_sudo(app, host_id, &conn, sudo).await.map_err(|e| e.message)?;
    let o = sudo::run(&conn, &auth, cmd).await.map_err(|e| e.to_string())?;
    if o.code != 0 {
        return Err(o.stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("the command failed").to_owned());
    }
    Ok(())
}

async fn apply_step(
    app: &AppHandle,
    host_id: HostId,
    bp: &Blueprint,
    step: &BpStep,
    sudo: &Option<SudoInput>,
) -> Result<String, String> {
    let key = step.id.split_once(':').map(|(_, k)| k).unwrap_or("");
    match step.kind {
        BpStepKind::StackFiles => {
            let s = bp.stacks.iter().find(|s| s.name == key).ok_or("not in the blueprint")?;
            apply_stack(app, host_id, s).await
        }
        BpStepKind::StackStart => {
            let s = bp.stacks.iter().find(|s| s.name == key).ok_or("not in the blueprint")?;
            start_stack(app, host_id, s).await
        }
        BpStepKind::Cron => {
            let c =
                bp.crontabs.iter().find(|c| c.user.as_deref().unwrap_or("") == key).ok_or("not in the blueprint")?;
            let n = crate::jobs_cmd::cron_merge(app, host_id, c.user.clone(), &c.text, again(sudo))
                .await
                .map_err(|e| e.message)?;
            Ok(format!("{n} line{} added", if n == 1 { "" } else { "s" }))
        }
        BpStepKind::Timer => {
            let j = bp.timers.iter().find(|j| j.name == key).ok_or("not in the blueprint")?;
            crate::jobs_cmd::timer_job_save(app.clone(), host_id, j.clone(), None, again(sudo))
                .await
                .map(|i| format!("installed {}", i.unit))
                .map_err(|e| e.message)
        }
        BpStepKind::Firewall => {
            let f = bp.firewall.as_ref().ok_or("not in the blueprint")?;
            let cmd = if let Some(zone) = key.strip_prefix("zone:") {
                let z = f.zones.iter().find(|z| z.name == zone).ok_or("not in the blueprint")?;
                let current = crate::firewall_cmd::firewall_read(app.clone(), host_id, again(sudo))
                    .await
                    .ok()
                    .and_then(|v| firewall_of(&v.report));
                let (services, ports) =
                    blueprint::zone_missing(z, current.as_ref().and_then(|c| c.zones.iter().find(|x| x.name == zone)));
                blueprint::firewalld_add_command(zone, &services, &ports).ok_or("nothing to add")?
            } else {
                blueprint::ufw_add_command(key).ok_or("only allow rules are added from a blueprint")?
            };
            run_root(app, host_id, again(sudo), &cmd).await?;
            record(
                app,
                Change::new(host_id, ChangeArea::Firewall, &format!("Firewall: {} (from a blueprint)", step.title)),
            );
            Ok("added".into())
        }
        BpStepKind::Caddy => {
            let c = bp.caddy.iter().find(|c| c.proxy_name == key).ok_or("not in the blueprint")?;
            let targets = crate::proxy_cmd::proxy_edit_targets(app.clone(), host_id).await.map_err(|e| e.message)?;
            let cur = targets.caddy.into_iter().find(|t| t.proxy_name == key).ok_or("no Caddy there")?;
            let out = crate::proxy_cmd::caddy_save(
                app.clone(),
                host_id,
                key.to_owned(),
                c.text.clone(),
                cur.hash,
                "from a blueprint".into(),
                again(sudo),
            )
            .await
            .map_err(|e| e.message)?;
            if out.ok {
                Ok("written and reloaded".into())
            } else {
                Err(out.message)
            }
        }
        BpStepKind::Manual => Err(format!("done by hand: {}", step.detail)),
        BpStepKind::Traefik => {
            let tr = bp.traefik.iter().find(|t| t.proxy_name == key).ok_or("not in the blueprint")?;
            let targets = crate::proxy_cmd::proxy_edit_targets(app.clone(), host_id).await.map_err(|e| e.message)?;
            let cur = targets.traefik.into_iter().find(|t| t.proxy_name == key).ok_or("no Traefik there")?;
            let routes = blueprint::traefik_after(&cur.routes, &tr.routes);
            crate::proxy_cmd::traefik_save(app.clone(), host_id, key.to_owned(), routes, cur.hash)
                .await
                .map(|t| format!("{} routes", t.routes.len()))
                .map_err(|e| e.message)
        }
    }
}

/// Applies the chosen steps of the opened blueprint to `host_id`, in plan
/// order. Manual steps are never applied. Stops at nothing: each step
/// reports its own result.
#[tauri::command]
#[specta::specta]
pub async fn blueprint_apply(
    app: AppHandle,
    host_id: HostId,
    steps: Vec<String>,
    sudo: Option<SudoInput>,
) -> SysResult<Vec<BpStepResult>> {
    let (bp, plan) = plan_for(&app, host_id, again(&sudo)).await?;
    // Ask for the sudo password once, up front, when a chosen step needs root.
    let root = plan.iter().any(|s| {
        steps.contains(&s.id)
            && (matches!(s.kind, BpStepKind::Firewall | BpStepKind::Timer)
                || (s.kind == BpStepKind::Caddy && s.id == "caddy:host"))
    });
    if root {
        let conn = connect_posix(&app, host_id, "blueprints").await?;
        resolve_sudo(&app, host_id, &conn, again(&sudo)).await?;
    }
    let mut out = Vec::new();
    for step in plan.iter().filter(|s| steps.contains(&s.id)) {
        let r = if step.state == BpStepState::Manual {
            Err(format!("done by hand: {}", step.detail))
        } else {
            apply_step(&app, host_id, &bp, step, &sudo).await
        };
        out.push(match r {
            Ok(m) => BpStepResult { id: step.id.clone(), ok: true, message: m },
            Err(m) => BpStepResult { id: step.id.clone(), ok: false, message: m },
        });
    }
    let ok = out.iter().filter(|r| r.ok).count();
    let title = format!("Applied a blueprint of {}: {ok} of {} steps", bp.host, out.len());
    record(&app, Change::new(host_id, ChangeArea::Blueprint, &title).ok(ok == out.len()));
    Ok(out)
}
