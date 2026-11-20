//! DRIFT's read-only checks on a host (D-151). The UI (or the model, through
//! a `[[check …]]` line) asks for checks by kind; the commands themselves are
//! fixed in `nightgrid_core::helper::lookup`, run over the host's SSH
//! connection as the login user (never sudo), and come back masked and clipped.

use std::time::{Duration, Instant};

use nightgrid_core::helper::lookup::{self, Lookup, LookupKind, LookupResult};
use tauri::{AppHandle, Manager};

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;

/// Longest one check may take.
const PER_CHECK: Duration = Duration::from_secs(8);
/// Longest a whole batch may take; later checks are skipped after it.
const BUDGET: Duration = Duration::from_secs(20);

async fn run(conn: &nightgrid_core::ssh::client::Connection, cmd: &str) -> Result<(i32, String, String), String> {
    let wrapped = format!("LC_ALL=C; export LC_ALL; {cmd}");
    let (code, out, err) = tokio::time::timeout(PER_CHECK, conn.exec(&wrapped))
        .await
        .map_err(|_| "the host took too long to answer".to_string())?
        .map_err(|e| e.to_string())?;
    Ok((code, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned()))
}

/// Runs read-only checks on a host for DRIFT, when the host allows it
/// ("DRIFT may run read-only checks" in the host editor). `terms` are the
/// question's words: containers and services they name get their state and
/// recent logs checked too. Nothing is stored.
#[tauri::command]
#[specta::specta]
pub async fn helper_host_lookups(
    app: AppHandle,
    host_id: nightgrid_core::hosts::HostId,
    lookups: Vec<Lookup>,
    terms: Vec<String>,
) -> CmdResult<Vec<LookupResult>> {
    let host = {
        let state = app.state::<AppState>();
        let store = state.store();
        store.get_host(host_id)?
    };
    let Some(host) = host.filter(|h| h.drift_lookups) else {
        return Err(CommandError::msg("read-only checks are off for this host"));
    };
    let extra = lookup::extra_paths(host.drift_paths.as_deref());
    let conn = crate::term_cmd::connect_posix(&app, host_id, "drift.lookups")
        .await
        .map_err(|e| CommandError::msg(e.to_string()))?;
    let start = Instant::now();
    let terms: Vec<String> = terms.into_iter().filter(|t| lookup::check_name(t).is_ok()).take(8).collect();
    let found = if terms.is_empty() {
        Vec::new()
    } else {
        match run(&conn, lookup::NAMES_COMMAND).await {
            Ok((_, out, _)) => {
                let (c, u) = lookup::parse_names(&out);
                lookup::targets(&terms, &c, &u)
            }
            Err(_) => Vec::new(),
        }
    };
    let mut results = Vec::new();
    for l in lookup::plan(lookups, found) {
        if start.elapsed() > BUDGET {
            results.push(LookupResult {
                lookup: l,
                command: String::new(),
                ok: false,
                output: "skipped: out of time".into(),
                clipped: false,
            });
            continue;
        }
        let mut l = l;
        // Files: resolve symlinks on the host first and check the real path.
        if l.kind == LookupKind::File {
            let asked = l.target.clone().unwrap_or_default();
            if let Err(e) = lookup::path_allowed(&asked, &extra) {
                results.push(LookupResult { lookup: l, command: String::new(), ok: false, output: e, clipped: false });
                continue;
            }
            if let Ok((_, out, _)) = run(&conn, &lookup::resolve_command(&asked)).await {
                let real = out.trim();
                if !real.is_empty() && real != asked {
                    if let Err(e) = lookup::path_allowed(real, &extra) {
                        results.push(LookupResult {
                            lookup: l,
                            command: String::new(),
                            ok: false,
                            output: format!("it points at {real}: {e}"),
                            clipped: false,
                        });
                        continue;
                    }
                    l.target = Some(real.to_string());
                }
            }
        }
        let cmd = match lookup::command(&l, &extra) {
            Ok(c) => c,
            Err(e) => {
                results.push(LookupResult { lookup: l, command: String::new(), ok: false, output: e, clipped: false });
                continue;
            }
        };
        let r = match run(&conn, &cmd).await {
            Ok((code, out, err)) => {
                // `systemctl status` exits 3 for a stopped unit: output on stdout is still an answer.
                let ok = code == 0 || !out.trim().is_empty();
                let text = if out.trim().is_empty() {
                    err
                } else if err.trim().is_empty() {
                    out
                } else {
                    format!("{out}\n{err}")
                };
                let (output, clipped) = lookup::finish(&l, &text);
                let output = if output.trim().is_empty() { format!("(no output, exit code {code})") } else { output };
                LookupResult { lookup: l, command: cmd, ok, output, clipped }
            }
            Err(e) => LookupResult { lookup: l, command: cmd, ok: false, output: e, clipped: false },
        };
        results.push(r);
    }
    Ok(results)
}
