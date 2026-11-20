//! Login & access audit (M10u): one read-only script per host, as root when
//! that needs no prompt (or the user asked and typed the password), otherwise
//! as the login user with what it may see.

use std::time::Duration;

use nightgrid_core::access::{self, AccessReport};
use nightgrid_core::hosts::HostId;
use nightgrid_core::system::sudo::{self, SudoAuth};
use nightgrid_core::system::SystemError;
use tauri::AppHandle;

use crate::system_cmd::{resolve_sudo, SudoInput, SysResult};
use crate::term_cmd::connect_posix;

/// Reads `host_id`'s access report. With `elevate` false, root is used only
/// when it needs no prompt (root login, NOPASSWD or a remembered password);
/// with `elevate` true a missing password is asked for.
#[tauri::command]
#[specta::specta]
pub async fn access_audit(
    app: AppHandle,
    host_id: HostId,
    elevate: bool,
    sudo: Option<SudoInput>,
) -> SysResult<AccessReport> {
    let conn = connect_posix(&app, host_id, "fleet.access").await?;
    if conn.is_windows() {
        let script = nightgrid_core::win::access::script();
        let o = tokio::time::timeout(Duration::from_secs(60), nightgrid_core::win::run(&conn, &script))
            .await
            .map_err(|_| SystemError::other("the audit took longer than a minute"))?
            .map_err(SystemError::other)?;
        if !o.stdout.contains("@@end") {
            return Err(SystemError::other(format!("the audit didn't finish: {}", o.error_text())).into());
        }
        let now = nightgrid_core::services::check::now_unix();
        return Ok(nightgrid_core::win::access::parse(host_id, &o.stdout, now));
    }
    let auth = if elevate {
        resolve_sudo(&app, host_id, &conn, sudo).await?
    } else {
        resolve_sudo(&app, host_id, &conn, None).await.unwrap_or(SudoAuth::User)
    };
    let as_root = !matches!(auth, SudoAuth::User);
    let now = nightgrid_core::services::check::now_unix();
    let script = access::script(now - i64::from(access::DAYS) * 86_400);
    let out = tokio::time::timeout(Duration::from_secs(60), sudo::run(&conn, &auth, &script))
        .await
        .map_err(|_| SystemError::other("the audit took longer than a minute"))??;
    if !out.stdout.contains("@@end") {
        let why = out.stderr.lines().last().unwrap_or("no output").to_owned();
        return Err(SystemError::other(format!("the audit didn't finish: {why}")).into());
    }
    Ok(access::parse(host_id, &out.stdout, as_root, now))
}
