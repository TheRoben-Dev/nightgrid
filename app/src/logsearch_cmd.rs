//! Cross-host log search (M10u): runs one query on several hosts at once
//! (journal over SSH, container logs through the Docker connection), and
//! checks saved searches with alerts on from the alerts loop.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

use futures_util::future::join_all;
use nightgrid_core::alerts::{AlertKind, Condition, Severity};
use nightgrid_core::docker::LogQuery;
use nightgrid_core::hosts::HostId;
use nightgrid_core::logsearch::{self, LogHit, LogSearchId, LogSearchQuery, Matcher, SavedSearch, SavedSearchInput};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, State};

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;

/// Saved searches with alerts are checked this often.
const ALERT_EVERY: i64 = 5 * 60;
/// Containers read per host.
const MAX_CONTAINERS: usize = 60;

#[derive(Default)]
pub struct LogSearchHub {
    /// Saved search → when it was last checked for alerts.
    last: Mutex<HashMap<LogSearchId, i64>>,
    /// When the last alert round ran.
    round: Mutex<i64>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HostLogError {
    pub host_id: HostId,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LogSearchResult {
    pub hits: Vec<LogHit>,
    /// More lines matched than are shown.
    pub truncated: bool,
    /// Hosts that couldn't be searched, and why.
    pub errors: Vec<HostLogError>,
    /// Hosts searched.
    pub hosts: Vec<HostId>,
}

/// One host's matching lines since `since` (unix seconds).
async fn search_host(
    app: &AppHandle,
    id: HostId,
    q: &LogSearchQuery,
    m: &Matcher,
    since: i64,
) -> Result<Vec<LogHit>, String> {
    let mut hits = Vec::new();
    let mut journal_err = None;
    if q.journal {
        match crate::term_cmd::connect_posix(app, id, "fleet.logs").await {
            Ok(conn) if conn.is_windows() => {
                // The Event Log stands in for the journal on Windows hosts.
                use nightgrid_core::win::{self, events};
                let ago = (nightgrid_core::services::check::now_unix() - since).clamp(1, i64::from(u32::MAX)) as u32;
                match events::search_script(q, ago) {
                    Ok(script) => match win::run(&conn, &script).await {
                        Ok(o) => {
                            hits.extend(events::search_hits(id, &o.stdout, m));
                            let err = o.error_text();
                            if o.code != 0 && !err.trim().is_empty() && !events::is_empty_notice(err.trim()) {
                                journal_err = Some(err);
                            }
                        }
                        Err(e) => journal_err = Some(e.to_string()),
                    },
                    Err(e) => return Err(e.to_string()),
                }
            }
            Ok(conn) => match conn.exec(&logsearch::journal_command(q, since)).await {
                Ok((_, out, _)) => hits.extend(logsearch::journal_hits(id, &String::from_utf8_lossy(&out), m)),
                Err(e) => journal_err = Some(e.to_string()),
            },
            Err(e) => return Err(e.to_string()),
        }
    }
    if q.containers {
        let filter = q.container.as_deref().map(str::trim).filter(|c| !c.is_empty()).map(str::to_lowercase);
        match crate::docker_cmd::docker_for(app, id).await {
            Ok(d) => {
                let list = d.containers().await.unwrap_or_default();
                let picked = list
                    .iter()
                    .filter(|c| match &filter {
                        Some(f) => c.name.to_lowercase().contains(f.as_str()),
                        None => true,
                    })
                    .filter(|c| c.state != "created")
                    .take(MAX_CONTAINERS);
                for c in picked {
                    let mut found = Vec::new();
                    let opts = LogQuery {
                        follow: false,
                        tail: Some(logsearch::CONTAINER_READ),
                        since: Some(since.max(0) as u32),
                    };
                    let read = d.logs(&c.id, opts, |batch| {
                        found.extend(
                            batch
                                .into_iter()
                                .filter_map(|l| logsearch::container_hit(id, &c.name, l.ts.as_deref(), &l.text, m)),
                        );
                        true
                    });
                    let _ = tokio::time::timeout(Duration::from_secs(15), read).await;
                    hits.extend(found);
                }
            }
            // No Docker on a host is normal when the journal was searched.
            Err(e) if !q.journal => return Err(e.to_string()),
            Err(_) => {}
        }
    }
    if hits.is_empty() {
        if let Some(e) = journal_err {
            return Err(e);
        }
    }
    Ok(hits)
}

async fn run(app: &AppHandle, q: &LogSearchQuery, hosts: &[HostId], since: i64) -> CmdResult<LogSearchResult> {
    logsearch::validate(q)?;
    let m = Matcher::new(q)?;
    let jobs = hosts.iter().map(|&id| {
        let m = &m;
        async move {
            let r = tokio::time::timeout(Duration::from_secs(90), search_host(app, id, q, m, since)).await;
            (id, r.unwrap_or_else(|_| Err("timed out".into())))
        }
    });
    let mut hits = Vec::new();
    let mut errors = Vec::new();
    for (id, r) in join_all(jobs).await {
        match r {
            Ok(h) => hits.extend(h),
            Err(message) => errors.push(HostLogError { host_id: id, message }),
        }
    }
    let (hits, truncated) = logsearch::merge(hits);
    Ok(LogSearchResult { hits, truncated, errors, hosts: hosts.to_vec() })
}

fn pick_hosts(app: &AppHandle, wanted: &[HostId]) -> Vec<HostId> {
    let all: Vec<HostId> =
        app.state::<AppState>().store().list_hosts().unwrap_or_default().into_iter().map(|h| h.id).collect();
    if wanted.is_empty() {
        all
    } else {
        all.into_iter().filter(|id| wanted.contains(id)).collect()
    }
}

/// Searches the chosen hosts (all when none are chosen), connecting where needed.
#[tauri::command]
#[specta::specta]
pub async fn log_search(app: AppHandle, query: LogSearchQuery) -> CmdResult<LogSearchResult> {
    let hosts = pick_hosts(&app, &query.hosts);
    if hosts.is_empty() {
        return Err(CommandError::msg("no hosts to search"));
    }
    let since = nightgrid_core::services::check::now_unix()
        - i64::from(query.since_mins.clamp(1, logsearch::MAX_SINCE_MINS)) * 60;
    run(&app, &query, &hosts, since).await
}

#[tauri::command]
#[specta::specta]
pub fn log_searches_list(state: State<'_, AppState>) -> CmdResult<Vec<SavedSearch>> {
    Ok(state.store().log_searches()?)
}

#[tauri::command]
#[specta::specta]
pub fn log_search_save(state: State<'_, AppState>, input: SavedSearchInput) -> CmdResult<SavedSearch> {
    let s = state.store().log_search_save(&input)?;
    state.alerts.wake();
    Ok(s)
}

#[tauri::command]
#[specta::specta]
pub fn log_search_delete(state: State<'_, AppState>, id: LogSearchId) -> CmdResult<()> {
    state.store().log_search_delete(id)?;
    state.logsearch.last.lock().expect("poisoned").remove(&id);
    state.alerts.wake();
    Ok(())
}

/// Saved searches with alerts on, run together every [`ALERT_EVERY`] on
/// connected hosts over the lines since each one's last check. One
/// condition per search and host with new matches. Returns `None` when no
/// round is due; the scope is the hosts checked, or `None` (every host) when
/// no search has alerts on, so old alerts clear.
pub async fn evaluate(
    app: &AppHandle,
    connected: &[HostId],
    now: i64,
) -> Option<(Vec<Condition>, Option<HashSet<HostId>>)> {
    let state = app.state::<AppState>();
    let hub = state.logsearch.clone();
    {
        let mut round = hub.round.lock().expect("poisoned");
        if now - *round < ALERT_EVERY {
            return None;
        }
        *round = now;
    }
    let saved: Vec<SavedSearch> =
        state.store().log_searches().unwrap_or_default().into_iter().filter(|s| s.alert).collect();
    hub.last.lock().expect("poisoned").retain(|id, _| saved.iter().any(|s| s.id == *id));
    if saved.is_empty() {
        return Some((vec![], None));
    }
    let names: HashMap<HostId, String> =
        state.store().list_hosts().unwrap_or_default().into_iter().map(|h| (h.id, h.name)).collect();
    let mut conds = Vec::new();
    let mut covered = HashSet::new();
    for s in saved {
        let since = hub.last.lock().expect("poisoned").insert(s.id, now).unwrap_or(now - ALERT_EVERY);
        let hosts: Vec<HostId> =
            pick_hosts(app, &s.query.hosts).into_iter().filter(|h| connected.contains(h)).collect();
        if hosts.is_empty() {
            continue;
        }
        let Ok(res) = run(app, &s.query, &hosts, since).await else { continue };
        let failed: HashSet<HostId> = res.errors.iter().map(|e| e.host_id).collect();
        covered.extend(hosts.iter().filter(|h| !failed.contains(h)));
        let mut per: HashMap<HostId, (usize, String)> = HashMap::new();
        for h in &res.hits {
            let e = per.entry(h.host_id).or_insert((0, h.text.clone()));
            e.0 += 1;
        }
        for (host, (n, newest)) in per {
            let host_name = names.get(&host).cloned().unwrap_or_else(|| format!("host {host}"));
            conds.push(Condition {
                key: format!("logsearch:{}:{host}", s.id),
                kind: AlertKind::LogMatch,
                severity: Severity::Warn,
                title: format!("{} on {host_name}", s.name),
                detail: format!(
                    "{n} new matching line{}: {}",
                    if n == 1 { "" } else { "s" },
                    newest.chars().take(160).collect::<String>()
                ),
                host_id: Some(host),
            });
        }
    }
    Some((conds, Some(covered)))
}
