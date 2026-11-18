//! Secrets & rotation (1.1, D-169): one list of every secret Nightgrid knows
//! about, how old each is and when it's due. Nothing here reads a secret's
//! value: names, places and dates only. See `core/src/rotation.rs`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, UNIX_EPOCH};

use nightgrid_core::hosts::{AuthMethod, Host, HostId};
use nightgrid_core::notify::{NewNotice, NoticeLevel, NoticeSource};
use nightgrid_core::rotation::{self, Found, ManualSecret, RotationState, SecretItem, SecretKind};
use nightgrid_core::services::check::now_unix;
use nightgrid_core::settings::AppSettings;
use nightgrid_core::ssh::keys;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager as _, State};

use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;

/// Rotation notices are looked at this often.
const EVERY: i64 = 3600;
static LAST_DUE: AtomicI64 = AtomicI64::new(0);

/// Records that a secret Nightgrid keeps was set or replaced just now.
pub fn stamp(state: &AppState, key: &str) {
    if let Err(e) = state.store().secret_stamp(key, now_unix()) {
        tracing::warn!("couldn't stamp {key}: {e}");
    }
}

fn mtime(p: &Path) -> Option<i64> {
    let t = std::fs::metadata(p).ok()?.modified().ok()?;
    t.duration_since(UNIX_EPOCH).ok().and_then(|d| i64::try_from(d.as_secs()).ok())
}

/// `~/x` and plain paths, for comparing a host's identity file with a key.
fn expand(p: &str, home: Option<&Path>) -> PathBuf {
    match (p.strip_prefix("~/"), home) {
        (Some(rest), Some(h)) => h.join(rest),
        _ => PathBuf::from(p),
    }
}

fn names(hosts: &[&Host]) -> String {
    match hosts.len() {
        0 => String::new(),
        1..=3 => hosts.iter().map(|h| h.name.as_str()).collect::<Vec<_>>().join(", "),
        n => format!("{}, {} and {} more", hosts[0].name, hosts[1].name, n - 2),
    }
}

/// Everything the sources know, before stamps and policy.
fn found(state: &AppState, cfg: &AppSettings) -> CmdResult<Vec<Found>> {
    let store = state.store();
    let hosts = store.list_hosts()?;
    let host_name =
        |id: HostId| hosts.iter().find(|h| h.id == id).map_or_else(|| format!("host {id}"), |h| h.name.clone());
    let home = state.home.as_deref();
    let mut out = Vec::new();

    // SSH keys in ~/.ssh: the private file's date is when it was made.
    if let Some(h) = home {
        for k in keys::scan_dir(&h.join(".ssh")) {
            let Some(file) = k.file.clone() else { continue };
            let path = PathBuf::from(&file);
            let users: Vec<&Host> =
                hosts.iter().filter(|x| x.identity_file.as_deref().is_some_and(|f| expand(f, home) == path)).collect();
            let mut f = Found::new(format!("ssh:{}", k.fingerprint), SecretKind::SshKey, k.label(), names(&users));
            f.dated = mtime(&path);
            f.note = format!("{} {}", k.algorithm, if k.encrypted { "· passphrase" } else { "· no passphrase" });
            if users.len() == 1 {
                f.host_id = Some(users[0].id);
            }
            out.push(f);
        }
    }

    // Host passwords and key passphrases kept in the keychain.
    for h in hosts.iter().filter(|h| h.remember_secret) {
        let what = if h.auth_method == AuthMethod::Password { "SSH password" } else { "key passphrase" };
        let mut f = Found::new(format!("host:{}", h.id), SecretKind::HostPassword, what, h.name.clone());
        f.host_id = Some(h.id);
        out.push(f);
    }

    // `.env` vault values marked secret.
    for (id, host, stack, name, updated) in store.vault_secret_dates()? {
        let mut f =
            Found::new(format!("vault:{id}"), SecretKind::Vault, name, format!("{stack} on {}", host_name(host)));
        f.host_id = Some(host);
        f.dated = Some(updated);
        out.push(f);
    }

    // Integration tokens.
    for (id, kind, name, created) in store.integration_dates()? {
        let mut f = Found::new(format!("integration:{id}"), SecretKind::Integration, name, kind);
        f.dated = Some(created);
        out.push(f);
    }

    // Nightgrid's own tokens, when in use.
    let metas = store.secret_metas()?;
    let stamped = |k: &str| metas.iter().any(|(m, _)| m == k);
    let own = [
        ("app:mcp", "MCP server token", "Settings → MCP", cfg.mcp.enabled),
        ("app:status", "Status page link token", "Settings → Status page", cfg.status_page.enabled),
        ("app:ntfy", "ntfy access token", "Settings → Alerts → Notifications", cfg.notify.ntfy),
        ("app:webhook", "Alert webhook URL", "Settings → Alerts → Notifications", false),
    ];
    for (key, name, place, on) in own {
        if on || stamped(key) {
            let mut f = Found::new(key, SecretKind::AppToken, name, place);
            if key == "app:mcp" {
                f.dated = mtime(&crate::mcp_cmd::token_path(&state.data_dir));
            }
            out.push(f);
        }
    }

    // Certificates, domains and tokens from the expiry watch.
    for w in store.watch_list()? {
        let place = w.host_id.map_or_else(|| w.target.clone(), host_name);
        let mut f = Found::new(format!("expiry:{}", w.id), SecretKind::Expiry, w.name, place);
        f.host_id = w.host_id;
        f.expires_at = w.expires_at;
        f.warn_days = u32::from(w.warn_days);
        f.note = w.kind.word().to_owned();
        out.push(f);
    }

    // Listed by hand.
    for m in store.manual_secrets()? {
        let Some(id) = m.id else { continue };
        let mut f = Found::new(format!("manual:{id}"), SecretKind::Manual, m.name, m.place);
        f.host_id = m.host_id;
        f.dated = m.rotated_at;
        f.note = m.note;
        out.push(f);
    }
    Ok(out)
}

fn gather(state: &AppState, cfg: &AppSettings, now: i64) -> CmdResult<Vec<SecretItem>> {
    let found = found(state, cfg)?;
    let metas = state.store().secret_metas()?;
    let mut items: Vec<SecretItem> = found
        .into_iter()
        .map(|f| {
            let m = metas.iter().find(|(k, _)| *k == f.key).map(|(_, m)| m);
            rotation::item(f, m, now)
        })
        .collect();
    rotation::sort(&mut items);
    Ok(items)
}

fn settings(state: &AppState) -> CmdResult<AppSettings> {
    Ok(AppSettings::load(&state.store())?)
}

/// Every known secret, overdue first.
#[tauri::command]
#[specta::specta]
pub fn secrets_list(state: State<'_, AppState>) -> CmdResult<Vec<SecretItem>> {
    let cfg = settings(&state)?;
    gather(&state, &cfg, now_unix())
}

/// "I rotated it": starts its clock again.
#[tauri::command]
#[specta::specta]
pub fn secret_mark_rotated(state: State<'_, AppState>, key: String) -> CmdResult<()> {
    if key.starts_with("expiry:") {
        return Err(CommandError::msg(
            "expiry items follow their own date: renew it, then check it again in Services → EXPIRY",
        ));
    }
    stamp(&state, &key);
    Ok(())
}

/// Rotate every `days` days; 0 turns the policy off, none restores the default.
#[tauri::command]
#[specta::specta]
pub fn secret_policy(state: State<'_, AppState>, key: String, days: Option<u32>) -> CmdResult<()> {
    Ok(state.store().secret_policy_set(&key, days)?)
}

#[tauri::command]
#[specta::specta]
pub fn secret_manual_save(state: State<'_, AppState>, secret: ManualSecret) -> CmdResult<u32> {
    Ok(state.store().manual_secret_save(&secret)?)
}

#[tauri::command]
#[specta::specta]
pub fn secret_manual_delete(state: State<'_, AppState>, id: u32) -> CmdResult<()> {
    Ok(state.store().manual_secret_delete(id)?)
}

/// Where a key is authorized: one row per server host.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct KeyHost {
    pub host_id: HostId,
    pub name: String,
    pub authorized: bool,
    pub error: Option<String>,
}

/// Reads `authorized_keys` on every server host (three at a time) and says
/// where this key is, so a rotation reaches every host before the old key
/// is removed.
#[tauri::command]
#[specta::specta]
pub async fn secret_key_hosts(app: AppHandle, fingerprint: String) -> CmdResult<Vec<KeyHost>> {
    let hosts: Vec<Host> = {
        let state = app.state::<AppState>();
        let list = state.store().list_hosts()?;
        list.into_iter().filter(|h| h.role != "client").collect()
    };
    let mut out = Vec::with_capacity(hosts.len());
    for chunk in hosts.chunks(3) {
        let reads = futures_util::future::join_all(chunk.iter().map(|h| {
            let app = app.clone();
            let fingerprint = fingerprint.as_str();
            async move {
                let r = tokio::time::timeout(
                    Duration::from_secs(20),
                    crate::keys_cmd::exec(&app, h.id, keys::READ_AUTHORIZED),
                )
                .await;
                match r {
                    Ok(Ok(text)) => KeyHost {
                        host_id: h.id,
                        name: h.name.clone(),
                        authorized: keys::parse_authorized(&text).iter().any(|k| k.fingerprint == fingerprint),
                        error: None,
                    },
                    Ok(Err(e)) => {
                        KeyHost { host_id: h.id, name: h.name.clone(), authorized: false, error: Some(e.to_string()) }
                    }
                    Err(_) => KeyHost {
                        host_id: h.id,
                        name: h.name.clone(),
                        authorized: false,
                        error: Some("no answer in 20 s".into()),
                    },
                }
            }
        }))
        .await;
        out.extend(reads);
    }
    Ok(out)
}

fn notice_for(i: &SecretItem) -> NewNotice {
    let overdue = i.state == RotationState::Overdue;
    let (level, title) = match (i.kind, overdue) {
        (SecretKind::Expiry, true) => (NoticeLevel::Critical, format!("{} has expired", i.name)),
        (SecretKind::Expiry, false) => (NoticeLevel::Warn, format!("{} expires soon", i.name)),
        (_, true) => (NoticeLevel::Warn, format!("Time to rotate {}", i.name)),
        (_, false) => (NoticeLevel::Info, format!("{} is due for rotation soon", i.name)),
    };
    let body = if i.place.is_empty() {
        "Open Security → SECRETS.".to_owned()
    } else {
        format!("Used by {}. Open Security → SECRETS.", i.place)
    };
    let mut n = NewNotice::new(NoticeSource::Rotation, level, title, body).view("security/secrets");
    if let Some(h) = i.host_id {
        n = n.host(h);
    }
    n
}

/// Hourly from the chores tick: a notice for each secret that turned due
/// or overdue, once per due date. Five or more fold into one notice.
pub(crate) fn due(app: &AppHandle, cfg: &AppSettings, now: i64) {
    if !cfg.feature("security.secrets") || !cfg.security.rotation_notices {
        return;
    }
    if now - LAST_DUE.load(Ordering::Relaxed) < EVERY {
        return;
    }
    LAST_DUE.store(now, Ordering::Relaxed);
    let state = app.state::<AppState>();
    let Ok(items) = gather(&state, cfg, now) else { return };
    let Ok(metas) = state.store().secret_metas() else { return };
    let todo = rotation::to_notify(&items, &metas);
    if todo.is_empty() {
        return;
    }
    if todo.len() >= 5 {
        let over = todo.iter().filter(|i| i.state == RotationState::Overdue).count();
        let names: Vec<&str> = todo.iter().take(6).map(|i| i.name.as_str()).collect();
        let level = if over > 0 { NoticeLevel::Warn } else { NoticeLevel::Info };
        crate::notify_cmd::send(
            app,
            NewNotice::new(
                NoticeSource::Rotation,
                level,
                format!("{} secrets need rotating ({over} overdue)", todo.len()),
                format!("{}…", names.join(", ")),
            )
            .view("security/secrets"),
        );
    } else {
        for i in &todo {
            crate::notify_cmd::send(app, notice_for(i));
        }
    }
    for i in todo {
        if let Some(d) = i.due_at {
            let _ = state.store().secret_notified(&i.key, d);
        }
    }
}
