//! Editing scheduled jobs (M10v): crontabs and Nightgrid's own systemd
//! timers. Other users' crontabs and every timer change need root.

use nightgrid_core::hosts::HostId;
use nightgrid_core::journal::{self, ChangeArea as Area};
use nightgrid_core::ssh::client::Connection;
use nightgrid_core::system::jobs::{self, CalendarPreview, CronEdit, Crontab, InstalledJob, TimerJob};
use nightgrid_core::system::schedule::{describe, CronSpec};
use nightgrid_core::system::sudo::{self, SudoAuth};
use nightgrid_core::system::SystemError;
use tauri::AppHandle;

use crate::journal_cmd::{record, Change};
use crate::system_cmd::{resolve_sudo, SudoInput, SysResult};
use crate::term_cmd::connect_host;

async fn crontab_auth(
    app: &AppHandle,
    host_id: HostId,
    conn: &Connection,
    user: Option<&str>,
    sudo: Option<SudoInput>,
) -> SysResult<SudoAuth> {
    Ok(match user {
        None => SudoAuth::User,
        Some(_) => resolve_sudo(app, host_id, conn, sudo).await?,
    })
}

async fn read(conn: &Connection, auth: &SudoAuth, user: Option<&str>) -> SysResult<Crontab> {
    let o = sudo::run(conn, auth, &jobs::read_command(user).map_err(SystemError::other)?).await?;
    let (now, off, text) = jobs::parse_read(&o.stdout)
        .ok_or_else(|| SystemError::other(format!("couldn't read the crontab: {}", o.stderr.trim())))?;
    Ok(jobs::parse_crontab(user.map(str::to_owned), &text, now, off))
}

/// The login user's crontab (`user` empty) or another user's, as root.
#[tauri::command]
#[specta::specta]
pub async fn cron_read(
    app: AppHandle,
    host_id: HostId,
    user: Option<String>,
    sudo: Option<SudoInput>,
) -> SysResult<Crontab> {
    let user = user.filter(|u| !u.trim().is_empty());
    let conn = connect_host(&app, host_id).await?;
    let auth = crontab_auth(&app, host_id, &conn, user.as_deref(), sudo).await?;
    read(&conn, &auth, user.as_deref()).await
}

/// Applies one change to the crontab read with `hash`; refused when it
/// changed since. Removing a job was confirmed in the UI with the job shown
/// (security rule 5). The old crontab is saved on the host first.
#[tauri::command]
#[specta::specta]
pub async fn cron_apply(
    app: AppHandle,
    host_id: HostId,
    user: Option<String>,
    hash: String,
    edit: CronEdit,
    sudo: Option<SudoInput>,
) -> SysResult<Crontab> {
    let user = user.filter(|u| !u.trim().is_empty());
    let conn = connect_host(&app, host_id).await?;
    let auth = crontab_auth(&app, host_id, &conn, user.as_deref(), sudo).await?;
    let cur = read(&conn, &auth, user.as_deref()).await?;
    if cur.hash != hash {
        return Err(
            SystemError::other("the crontab changed on the host since it was read: reload it and try again").into()
        );
    }
    let text = jobs::apply(&cur.text, &edit).map_err(SystemError::other)?;
    let now = nightgrid_core::services::check::now_unix();
    tracing::info!(host = %host_id, user = ?user, op = edit.op(), "crontab edit");
    let o =
        sudo::run(&conn, &auth, &jobs::write_command(user.as_deref(), &text, now).map_err(SystemError::other)?).await?;
    let whose = format!("{}'s crontab", user.as_deref().unwrap_or("the login user"));
    let title = format!("Crontab: {} ({whose})", edit.op());
    if !o.stdout.contains("written") {
        let why = o.stderr.lines().last().unwrap_or("crontab refused it").to_owned();
        record(&app, Change::new(host_id, Area::Jobs, &title).detail(&why).ok(false));
        return Err(SystemError::other(format!("couldn't install the crontab: {why}")).into());
    }
    record(&app, Change::new(host_id, Area::Jobs, &title).diff(journal::text_diff(&cur.text, &text)));
    read(&conn, &auth, user.as_deref()).await
}

/// Plain words for a cron schedule, or `None` when it isn't one.
#[tauri::command]
#[specta::specta]
pub fn cron_describe(schedule: String) -> Option<String> {
    let s = schedule.split_whitespace().collect::<Vec<_>>().join(" ");
    (s == "@reboot" || CronSpec::parse(&s).is_some()).then(|| describe(&s))
}

#[tauri::command]
#[specta::specta]
pub async fn timer_jobs(app: AppHandle, host_id: HostId) -> SysResult<Vec<InstalledJob>> {
    let conn = connect_host(&app, host_id).await?;
    let (_, out, _) = conn.exec(jobs::LIST_COMMAND).await.map_err(SystemError::other)?;
    Ok(jobs::parse_list(&String::from_utf8_lossy(&out)))
}

/// Creates or replaces a Nightgrid timer (`replaces`: the unit being edited).
#[tauri::command]
#[specta::specta]
pub async fn timer_job_save(
    app: AppHandle,
    host_id: HostId,
    job: TimerJob,
    replaces: Option<String>,
    sudo: Option<SudoInput>,
) -> SysResult<InstalledJob> {
    let job = job.normalized().map_err(SystemError::other)?;
    let cmd = jobs::install_command(&job, replaces.as_deref()).map_err(SystemError::other)?;
    let conn = connect_host(&app, host_id).await?;
    let auth = resolve_sudo(&app, host_id, &conn, sudo).await?;
    tracing::info!(host = %host_id, unit = %job.unit(), "install timer job");
    let o = sudo::run(&conn, &auth, &cmd).await?;
    let title = format!("Timer {}: {}", if replaces.is_some() { "changed" } else { "added" }, job.unit());
    if !o.stdout.contains("installed") {
        let why = o.stderr.lines().last().unwrap_or("no output").to_owned();
        record(&app, Change::new(host_id, Area::Jobs, &title).detail(&why).ok(false));
        return Err(SystemError::other(format!("couldn't install the timer: {why}")).into());
    }
    record(&app, Change::new(host_id, Area::Jobs, &title));
    Ok(InstalledJob { unit: job.unit(), job })
}

/// Removes a Nightgrid timer; confirmed in the UI with its name (rule 5).
#[tauri::command]
#[specta::specta]
pub async fn timer_job_remove(app: AppHandle, host_id: HostId, unit: String, sudo: Option<SudoInput>) -> SysResult<()> {
    let cmd = jobs::remove_command(&unit).map_err(SystemError::other)?;
    let conn = connect_host(&app, host_id).await?;
    let auth = resolve_sudo(&app, host_id, &conn, sudo).await?;
    tracing::info!(host = %host_id, unit = %unit, "remove timer job");
    let o = sudo::run(&conn, &auth, &cmd).await?;
    if !o.stdout.contains("removed") {
        return Err(SystemError::other(format!("couldn't remove {unit}: {}", o.stderr.trim())).into());
    }
    record(&app, Change::new(host_id, Area::Jobs, &format!("Timer removed: {unit}")));
    Ok(())
}

/// What systemd on the host makes of an `OnCalendar=` expression.
#[tauri::command]
#[specta::specta]
pub async fn timer_calendar_preview(app: AppHandle, host_id: HostId, calendar: String) -> SysResult<CalendarPreview> {
    let cmd = jobs::preview_command(&calendar).map_err(SystemError::other)?;
    let conn = connect_host(&app, host_id).await?;
    let (_, out, _) = conn.exec(&cmd).await.map_err(SystemError::other)?;
    Ok(jobs::parse_preview(&String::from_utf8_lossy(&out)))
}
