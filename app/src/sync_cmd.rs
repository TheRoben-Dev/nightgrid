//! Profile sync between devices (D-205): the listener, pairing, push/pull
//! and the automatic sync loop. The protocol and the merge live in
//! `nightgrid_core::sync`.
//!
//! The listener is off until the user turns it on. It listens on every
//! interface (the channel itself is authenticated and encrypted, so where
//! a connection comes from doesn't matter) and, while the built-in
//! Tailscale client runs, on the tailnet too. Only paired devices get past
//! the handshake; pairing needs the one-time code shown on this screen.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nightgrid_core::backup::ImportMode;
use nightgrid_core::settings::AppSettings;
use nightgrid_core::sync::discover::{self, Beacon};
use nightgrid_core::sync::merge::{self, Merged};
use nightgrid_core::sync::wire::{self, Channel};
use nightgrid_core::sync::{
    self as core_sync, pair, Hello, Join, Keypair, Request, Response, SecretCategory, SyncMode, SyncPeer, SyncSettings,
};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Emitter as _, Manager as _, State};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::Semaphore;

use crate::backup_cmd::BackupImported;
use crate::commands::{CmdResult, CommandError};
use crate::state::AppState;

pub const EVENT: &str = "sync://event";
/// Seconds between checks for local changes while automatic sync is on.
const TICK: Duration = Duration::from_secs(15);
/// A device with automatic sync is asked for its changes at least this often.
const CATCH_UP: Duration = Duration::from_secs(300);
const DIAL_TIMEOUT: Duration = Duration::from_secs(6);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const MAX_CONN: usize = 8;
/// How long a LAN search listens for answers.
const SCAN_WAIT: Duration = Duration::from_millis(1200);
/// A device asking for a pairing code gets the same one again within this.
const ASK_GAP: Duration = Duration::from_secs(15);

struct Pending {
    code: String,
    expires: Instant,
    attempts: u32,
    accept_changes: bool,
    /// Shown because a device on the LAN asked for it (D-208).
    asked_by: Option<String>,
}

#[derive(Default)]
pub struct SyncHub {
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    looper: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    /// Port listened on, or why it couldn't.
    bound: Mutex<(Option<u16>, Option<String>)>,
    pairing: Mutex<Option<Pending>>,
    identity: Mutex<Option<Arc<Keypair>>>,
    /// Last automatic sync attempt per device.
    attempts: Mutex<HashMap<String, Instant>>,
    /// A device pushed a profile here: restart to load it.
    received: Mutex<Option<String>>,
    wake: tokio::sync::Notify,
    /// Answers LAN probes while the device is findable (D-208).
    answerer: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    /// Wrong passphrases in a row, and until when they're refused.
    pass_fails: Mutex<(u32, Option<Instant>)>,
    /// Last time a device asked for a pairing code.
    last_ask: Mutex<Option<Instant>>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PairingCode {
    pub code: String,
    /// Seconds left.
    pub expires_in: u32,
    pub accept_changes: bool,
    /// A device on the LAN asked for this code (shown as a dialog).
    pub asked_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncInfo {
    pub settings: SyncSettings,
    /// What other devices see this one as.
    pub device_name: String,
    /// Short id of this device's key, grouped for reading aloud.
    pub fingerprint: String,
    pub listening: bool,
    /// Addresses other devices can use (without the port).
    pub addresses: Vec<String>,
    pub error: Option<String>,
    pub peers: Vec<SyncPeer>,
    pub pairing: Option<PairingCode>,
    /// A device pushed its profile here; restart to load it.
    pub received_from: Option<String>,
    /// The built-in Tailscale client runs, so tailnet names work too.
    pub tailnet: bool,
    /// Pairing by profile passphrase is set up (its key is saved).
    pub passphrase_set: bool,
    /// Answering LAN searches right now.
    pub findable: bool,
}

/// A Nightgrid app that answered a LAN search (D-208).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FoundDevice {
    pub id: String,
    pub name: String,
    pub addr: String,
    pub port: u16,
    pub app_version: String,
    /// Same database version, so automatic sync can run.
    pub same_version: bool,
    pub hosts: u32,
    /// Takes pairing with the profile passphrase.
    pub passphrase: bool,
    pub salt: String,
    /// Already paired with this device.
    pub paired: bool,
}

/// Result of joining a device found on the LAN.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncJoined {
    pub peer: SyncPeer,
    /// What came over when its profile was imported (restart to load it).
    pub imported: Option<BackupImported>,
    /// Why the import didn't happen, when pairing worked but it failed.
    pub import_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SyncEvent {
    /// A device paired with this one (or the code expired).
    Peers,
    /// A device pushed its profile here.
    #[serde(rename_all = "camelCase")]
    Received { from: String },
    /// A device on the LAN asked for a pairing code: show it.
    #[serde(rename_all = "camelCase")]
    Asked { from: String, code: String },
    /// Automatic sync changed things here: reload.
    #[serde(rename_all = "camelCase")]
    Applied { from: String, merged: Merged },
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncPushed {
    pub added: u32,
    pub skipped: u32,
    pub secrets: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncRun {
    /// What changed here.
    pub merged: Merged,
    /// The other device took this one's changes.
    pub sent: bool,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn emit(app: &AppHandle, ev: SyncEvent) {
    let _ = app.emit(EVENT, ev);
}

fn err(e: impl std::fmt::Display) -> CommandError {
    CommandError::msg(e.to_string())
}

/// The computer's name, as people know it.
fn computer_name() -> String {
    #[cfg(target_os = "macos")]
    if let Ok(o) = std::process::Command::new("scutil").args(["--get", "ComputerName"]).output() {
        let s = String::from_utf8_lossy(&o.stdout).trim().to_owned();
        if o.status.success() && !s.is_empty() {
            return s;
        }
    }
    if let Ok(n) = std::env::var("COMPUTERNAME") {
        if !n.trim().is_empty() {
            return n.trim().to_owned();
        }
    }
    if let Ok(n) = std::fs::read_to_string("/etc/hostname") {
        if !n.trim().is_empty() {
            return n.trim().to_owned();
        }
    }
    std::process::Command::new("hostname")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Nightgrid".into())
}

fn device_name(s: &SyncSettings) -> String {
    let n = s.device_name.trim();
    if n.is_empty() {
        core_sync::clean_name(&computer_name())
    } else {
        core_sync::clean_name(n)
    }
}

fn identity(state: &AppState) -> CmdResult<Arc<Keypair>> {
    let mut g = state.sync.identity.lock().expect("poisoned");
    if let Some(k) = g.as_ref() {
        return Ok(k.clone());
    }
    let k = Arc::new(core_sync::identity(&state.keychain).map_err(|e| {
        CommandError::msg(format!("Sync keeps this device's key in the OS keychain, which isn't available: {e}"))
    })?);
    *g = Some(k.clone());
    Ok(k)
}

/// Addresses this device can be reached on.
fn my_addrs(state: &AppState) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    if let Some(a) = crate::status_cmd::lan_addr() {
        v.push(a.to_string());
    }
    if let Some(a) = crate::status_cmd::tailscale_addr() {
        v.push(a.to_string());
    }
    if let Some(n) = crate::tailnet_cmd::self_node(state) {
        let mut ips = n.ips.clone();
        ips.sort_by_key(|ip| ip.contains(':'));
        v.extend(ips);
        let dns = n.dns_name.trim_end_matches('.');
        if !dns.is_empty() {
            v.push(dns.to_owned());
        }
    }
    core_sync::clean_addrs(&v)
}

fn my_hello(state: &AppState) -> CmdResult<Hello> {
    let (settings, schema) = {
        let store = state.store();
        (AppSettings::load(&store)?.sync, store.schema_version()?)
    };
    Ok(Hello {
        name: device_name(&settings),
        app_version: env!("CARGO_PKG_VERSION").into(),
        schema,
        addrs: my_addrs(state),
        port: settings.port,
        join: None,
    })
}

fn info(state: &AppState) -> CmdResult<SyncInfo> {
    let (settings, peers) = {
        let store = state.store();
        (AppSettings::load(&store)?.sync, store.sync_peers()?)
    };
    let (port, error) = state.sync.bound.lock().expect("poisoned").clone();
    let pairing = {
        let mut g = state.sync.pairing.lock().expect("poisoned");
        if g.as_ref().is_some_and(|p| p.expires <= Instant::now() || p.attempts == 0) {
            *g = None;
        }
        g.as_ref().map(|p| PairingCode {
            code: p.code.clone(),
            expires_in: p.expires.saturating_duration_since(Instant::now()).as_secs() as u32,
            accept_changes: p.accept_changes,
            asked_by: p.asked_by.clone(),
        })
    };
    let fingerprint = match identity(state) {
        Ok(k) => core_sync::fingerprint(&core_sync::device_id(&k.public)),
        Err(_) => String::new(),
    };
    Ok(SyncInfo {
        device_name: device_name(&settings),
        settings,
        fingerprint,
        listening: port.is_some(),
        addresses: my_addrs(state),
        error,
        peers,
        pairing,
        received_from: state.sync.received.lock().expect("poisoned").clone(),
        tailnet: crate::tailnet_cmd::self_node(state).is_some(),
        passphrase_set: core_sync::pass_secret(&state.keychain).ok().flatten().is_some(),
        findable: state.sync.answerer.lock().expect("poisoned").is_some() && port.is_some(),
    })
}

// ---------------------------------------------------------------- listener

/// Starts, restarts or stops the listener (and the automatic loop) to match
/// the settings.
pub fn apply(app: &AppHandle) {
    let state = app.state::<AppState>();
    let hub = &state.sync;
    if let Some(t) = hub.task.lock().expect("poisoned").take() {
        t.abort();
    }
    if let Some(t) = hub.answerer.lock().expect("poisoned").take() {
        t.abort();
    }
    *hub.bound.lock().expect("poisoned") = (None, None);
    start_loop(app);
    let settings = match AppSettings::load(&state.store()) {
        Ok(s) => s.sync,
        Err(_) => return,
    };
    if !settings.enabled {
        *hub.pairing.lock().expect("poisoned") = None;
        let app2 = app.clone();
        tauri::async_runtime::spawn(async move {
            crate::tailnet_cmd::set_forward(&app2.state::<AppState>(), None).await;
        });
        return;
    }
    if let Err(e) = identity(&state) {
        hub.bound.lock().expect("poisoned").1 = Some(e.message);
        return;
    }
    if settings.discoverable {
        start_answerer(app);
    }
    let app2 = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        let port = settings.port;
        let listener = match TcpListener::bind((IpAddr::from([0, 0, 0, 0]), port)).await {
            Ok(l) => l,
            Err(e) => {
                app2.state::<AppState>().sync.bound.lock().expect("poisoned").1 =
                    Some(format!("Can't listen on port {port}: {e}"));
                return;
            }
        };
        tracing::info!(port, "sync listening");
        app2.state::<AppState>().sync.bound.lock().expect("poisoned").0 = Some(port);
        crate::tailnet_cmd::set_forward(&app2.state::<AppState>(), Some(port)).await;
        let slots = Arc::new(Semaphore::new(MAX_CONN));
        loop {
            let Ok((sock, peer)) = listener.accept().await else { continue };
            let Ok(slot) = slots.clone().try_acquire_owned() else { continue };
            let app3 = app2.clone();
            tokio::spawn(async move {
                let _slot = slot;
                let _ = sock.set_nodelay(true);
                match tokio::time::timeout(REQUEST_TIMEOUT, serve(&app3, sock, peer)).await {
                    Ok(Err(e)) => tracing::debug!(%peer, "sync connection: {}", e.message),
                    Err(_) => tracing::debug!(%peer, "sync connection timed out"),
                    Ok(Ok(())) => {}
                }
            });
        }
    });
    *hub.task.lock().expect("poisoned") = Some(task);
}

/// What this device says when a LAN search finds it (`None`: stay quiet).
fn beacon(state: &AppState) -> Option<Beacon> {
    let (settings, schema, hosts) = {
        let store = state.store();
        (AppSettings::load(&store).ok()?.sync, store.schema_version().ok()?, store.host_count().unwrap_or(0))
    };
    if !(settings.enabled && settings.discoverable) || state.sync.bound.lock().expect("poisoned").0.is_none() {
        return None;
    }
    let id = core_sync::device_id(&identity(state).ok()?.public);
    let pass = if settings.passphrase_pairing { core_sync::pass_secret(&state.keychain).ok().flatten() } else { None };
    Some(Beacon {
        id,
        name: device_name(&settings),
        port: settings.port,
        app_version: env!("CARGO_PKG_VERSION").into(),
        schema,
        hosts,
        passphrase: pass.is_some(),
        salt: pass.map(|(salt, _)| salt).unwrap_or_default(),
    })
}

/// Answers LAN searches on [`discover::DISCOVERY_PORT`] (D-208).
fn start_answerer(app: &AppHandle) {
    let app2 = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        let sock = match UdpSocket::bind((IpAddr::from([0, 0, 0, 0]), discover::DISCOVERY_PORT)).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("sync: can't answer LAN searches on UDP {}: {e}", discover::DISCOVERY_PORT);
                return;
            }
        };
        tracing::info!(port = discover::DISCOVERY_PORT, "sync: findable on the LAN");
        let app3 = app2.clone();
        discover::answer(sock, move || beacon(&app3.state::<AppState>())).await;
    });
    *app.state::<AppState>().sync.answerer.lock().expect("poisoned") = Some(task);
}

async fn serve(app: &AppHandle, mut sock: TcpStream, from: SocketAddr) -> CmdResult<()> {
    let kind = wire::read_header(&mut sock).await.map_err(err)?;
    match kind {
        wire::KIND_PAIR => serve_pair(app, sock, from).await,
        wire::KIND_PAIR_PASS => serve_pair_pass(app, sock, from).await,
        wire::KIND_ASK => serve_ask(app, sock).await,
        wire::KIND_SESSION => serve_session(app, sock).await,
        _ => Err(CommandError::msg("unknown connection kind")),
    }
}

async fn serve_pair(app: &AppHandle, mut sock: TcpStream, from: SocketAddr) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let (code, accept_changes, asked) = {
        let g = state.sync.pairing.lock().expect("poisoned");
        match g.as_ref() {
            Some(p) if p.expires > Instant::now() && p.attempts > 0 => {
                (p.code.clone(), p.accept_changes, p.asked_by.is_some())
            }
            _ => return Err(CommandError::msg("pairing isn't open")),
        }
    };
    let me = identity(&state)?;
    let attempt = async {
        let psk = pair::responder(&mut sock, &code).await?;
        let mut ch = wire::pair_responder(sock, &me, &psk).await?;
        let theirs: Hello = ch.recv_json().await?;
        Ok::<_, std::io::Error>((ch, theirs))
    }
    .await;
    let (ch, theirs) = match attempt {
        Ok(v) => v,
        Err(e) => {
            let mut g = state.sync.pairing.lock().expect("poisoned");
            if let Some(p) = g.as_mut() {
                p.attempts = p.attempts.saturating_sub(1);
                if p.attempts == 0 {
                    tracing::warn!(%from, "sync: too many wrong pairing codes; the code is void");
                    *g = None;
                }
            }
            drop(g);
            emit(app, SyncEvent::Peers);
            return Err(err(e));
        }
    };
    // A code the user put up themselves keeps their "can change" choice;
    // one shown because the device asked (and typed here) is the owner's.
    finish_pair(app, ch, theirs, from, accept_changes, asked || accept_changes).await?;
    *state.sync.pairing.lock().expect("poisoned") = None;
    Ok(())
}

/// The shared end of pairing on the listening side: say who we are, save
/// the device and apply what it asked for if `trust_join`.
async fn finish_pair(
    app: &AppHandle,
    mut ch: Channel<TcpStream>,
    mut theirs: Hello,
    from: SocketAddr,
    accept_changes: bool,
    trust_join: bool,
) -> CmdResult<()> {
    let state = app.state::<AppState>();
    ch.send_json(&my_hello(&state)?).await.map_err(err)?;
    // The address it came from works, at least on this network.
    let ip = from.ip();
    if !ip.is_loopback() {
        theirs.addrs.insert(0, ip.to_string());
    }
    let mut peer = state.store().sync_peer_add(&ch.remote_key(), &theirs, accept_changes)?;
    if let (true, Some(j)) = (trust_join, theirs.join.as_ref()) {
        peer = state.store().sync_peer_joined(&peer.id, j)?;
    }
    tracing::info!(peer = %peer.name, mode = peer.mode.as_str(), "sync: paired");
    emit(app, SyncEvent::Peers);
    Ok(())
}

/// Pairing with the profile passphrase (D-208).
async fn serve_pair_pass(app: &AppHandle, mut sock: TcpStream, from: SocketAddr) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let settings = AppSettings::load(&state.store())?.sync;
    if !(settings.discoverable && settings.passphrase_pairing) {
        return Err(CommandError::msg("pairing by passphrase is off"));
    }
    {
        let mut g = state.sync.pass_fails.lock().expect("poisoned");
        match g.1 {
            Some(until) if until > Instant::now() => return Err(CommandError::msg("too many wrong passphrases")),
            Some(_) => *g = (0, None),
            None => {}
        }
    }
    let Some((_, key)) = core_sync::pass_secret(&state.keychain)? else {
        return Err(CommandError::msg("pairing by passphrase isn't set up"));
    };
    let me = identity(&state)?;
    let attempt = async {
        let psk = pair::pass_responder(&mut sock, &key).await?;
        let mut ch = wire::pair_responder(sock, &me, &psk).await?;
        let theirs: Hello = ch.recv_json().await?;
        Ok::<_, std::io::Error>((ch, theirs))
    }
    .await;
    match attempt {
        Ok((ch, theirs)) => {
            *state.sync.pass_fails.lock().expect("poisoned") = (0, None);
            finish_pair(app, ch, theirs, from, true, true).await
        }
        Err(e) => {
            let mut g = state.sync.pass_fails.lock().expect("poisoned");
            g.0 += 1;
            if g.0 >= pair::PASS_ATTEMPTS {
                tracing::warn!(%from, "sync: too many wrong passphrases; refusing them for a while");
                g.1 = Some(Instant::now() + Duration::from_secs(pair::PASS_LOCKOUT_SECS));
            }
            Err(err(e))
        }
    }
}

/// Answers for [`wire::KIND_ASK`].
const ASK_SHOWN: u8 = 1;
const ASK_OFF: u8 = 2;
const ASK_FULL: u8 = 3;

/// A device on the LAN asks for a pairing code: put one on this screen.
async fn serve_ask(app: &AppHandle, mut sock: TcpStream) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let n = sock.read_u8().await.map_err(err)?;
    let mut name = vec![0u8; usize::from(n.min(64))];
    sock.read_exact(&mut name).await.map_err(err)?;
    let name = core_sync::clean_name(&String::from_utf8_lossy(&name));
    let settings = AppSettings::load(&state.store())?.sync;
    let answer = if !settings.discoverable {
        ASK_OFF
    } else if state.store().sync_peers()?.len() >= core_sync::MAX_PEERS {
        ASK_FULL
    } else {
        let code = {
            let mut g = state.sync.pairing.lock().expect("poisoned");
            let mut last = state.sync.last_ask.lock().expect("poisoned");
            let recent = last.is_some_and(|t| t.elapsed() < ASK_GAP);
            let live = g.as_ref().is_some_and(|p| p.expires > Instant::now() && p.attempts > 0);
            if !(live && (recent || g.as_ref().is_some_and(|p| p.asked_by.is_none()))) {
                *g = Some(Pending {
                    code: pair::new_code(),
                    expires: Instant::now() + Duration::from_secs(pair::CODE_TTL_SECS),
                    attempts: pair::CODE_ATTEMPTS,
                    accept_changes: true,
                    asked_by: Some(name.clone()),
                });
            }
            *last = Some(Instant::now());
            g.as_ref().map(|p| p.code.clone()).unwrap_or_default()
        };
        tracing::info!(from = %name, "sync: a device asked for a pairing code");
        emit(app, SyncEvent::Asked { from: name, code });
        ASK_SHOWN
    };
    sock.write_u8(answer).await.map_err(err)?;
    sock.flush().await.map_err(err)?;
    Ok(())
}

async fn serve_session(app: &AppHandle, sock: TcpStream) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let me = identity(&state)?;
    let keys: Vec<[u8; 32]> = state.store().sync_peers()?.iter().filter_map(|p| p.key().ok()).collect();
    let mut ch = wire::session_responder(sock, &me, |k| keys.contains(k)).await.map_err(err)?;
    let peer = state.store().sync_peer_by_key(&ch.remote_key())?.ok_or_else(|| CommandError::msg("not paired"))?;
    let req: Request = ch.recv_json().await.map_err(err)?;
    let resp = match handle(app, &peer, req).await {
        Ok(r) => r,
        Err(e) => Response::Error { message: e.message },
    };
    ch.send_json(&resp).await.map_err(err)?;
    Ok(())
}

fn allowed(asked: &[SecretCategory], mine: &[SecretCategory]) -> Vec<SecretCategory> {
    asked.iter().copied().filter(|c| mine.contains(c)).collect()
}

async fn handle(app: &AppHandle, peer: &SyncPeer, req: Request) -> CmdResult<Response> {
    let state = app.state::<AppState>();
    let me = device_name(&AppSettings::load(&state.store())?.sync);
    match req {
        Request::Hello(h) => {
            state.store().sync_peer_seen(&peer.id, Some(&h), None)?;
            emit(app, SyncEvent::Peers);
            Ok(Response::Hello(my_hello(&state)?))
        }
        Request::Pull { secrets } => {
            let cats = allowed(&secrets, &peer.secrets);
            let bundle = {
                let store = state.store();
                core_sync::bundle_for(&store, &state.keychain, &cats, env!("CARGO_PKG_VERSION"), now_ms() / 1000)?
            };
            state.store().sync_peer_seen(&peer.id, None, None)?;
            Ok(Response::Profile { bundle: Box::new(bundle) })
        }
        Request::Push { mode, bundle } => {
            if !peer.accept_changes {
                return Err(CommandError::msg(format!(
                    "{me} doesn't take changes from {}. Turn on \"Can change this device\" for it there.",
                    peer.name
                )));
            }
            let applied = {
                let mut store = state.store();
                let r = core_sync::apply_bundle(&mut store, &state.keychain, *bundle, mode, &peer.secrets)?;
                merge::reset(&mut store)?;
                store.sync_peer_synced(&peer.id)?;
                r
            };
            tracing::info!(from = %peer.name, ?mode, "sync: profile pushed here");
            *state.sync.received.lock().expect("poisoned") = Some(peer.name.clone());
            emit(app, SyncEvent::Received { from: peer.name.clone() });
            Ok(Response::Pushed {
                added: applied.outcome.added.values().sum(),
                skipped: applied.outcome.skipped.values().sum(),
                secrets: applied.secrets,
            })
        }
        Request::Sync { schema, items } => {
            if !peer.mode.automatic() {
                return Err(CommandError::msg(format!("Automatic sync with this device is off on {me}.")));
            }
            let ours = state.store().schema_version()?;
            if schema != ours {
                return Err(CommandError::msg(format!(
                    "Automatic sync needs the same Nightgrid version on both devices ({me} has {}).",
                    env!("CARGO_PKG_VERSION")
                )));
            }
            let take = peer.mode.receives() && peer.accept_changes && !items.is_empty();
            let (merged, out) = {
                let mut store = state.store();
                let merged = if take { merge::merge(&mut store, &items, now_ms())? } else { Merged::default() };
                let out = if peer.mode.sends() { merge::scan(&mut store, now_ms())?.items } else { Vec::new() };
                store.sync_peer_synced(&peer.id)?;
                (merged, out)
            };
            if merged.any() {
                emit(app, SyncEvent::Applied { from: peer.name.clone(), merged });
            }
            Ok(Response::Synced { items: out, took: take })
        }
        Request::Unpair => {
            state.store().sync_peer_delete(&peer.id)?;
            tracing::info!(peer = %peer.name, "sync: unpaired by the other device");
            emit(app, SyncEvent::Peers);
            Ok(Response::Done)
        }
    }
}

// ---------------------------------------------------------------- client

/// Splits `host`, `host:port`, `[v6]:port` into host and port.
fn split_addr(s: &str, default: u16) -> CmdResult<(String, u16)> {
    let s = s.trim();
    if s.is_empty() {
        return Err(CommandError::msg("Enter the other device's address."));
    }
    if let Ok(sa) = s.parse::<SocketAddr>() {
        return Ok((sa.ip().to_string(), sa.port()));
    }
    if s.parse::<IpAddr>().is_ok() {
        return Ok((s.to_owned(), default));
    }
    match s.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') => {
            let port = p.parse::<u16>().map_err(|_| CommandError::msg("That port isn't a number."))?;
            Ok((h.to_owned(), port))
        }
        _ => Ok((s.trim_matches(['[', ']']).to_owned(), default)),
    }
}

async fn dial(host: &str, port: u16) -> std::io::Result<TcpStream> {
    let s = nightgrid_core::tailnet::dial(host, port, DIAL_TIMEOUT).await?;
    let _ = s.set_nodelay(true);
    Ok(s)
}

async fn open(state: &AppState, peer: &SyncPeer) -> CmdResult<(Channel<TcpStream>, String)> {
    let me = identity(state)?;
    let key = peer.key()?;
    let mut last = String::from("no address to try");
    for addr in &peer.addrs {
        match dial(addr, peer.port).await {
            Ok(mut s) => {
                if let Err(e) = wire::write_header(&mut s, wire::KIND_SESSION).await {
                    last = format!("{addr}: {e}");
                    continue;
                }
                match wire::session_initiator(s, &me, &key).await {
                    Ok(ch) => return Ok((ch, addr.clone())),
                    Err(e) => last = format!("{addr}: {e}"),
                }
            }
            Err(e) => last = format!("{addr}: {e}"),
        }
    }
    Err(CommandError::msg(format!("Couldn't reach {} ({last}). Is Nightgrid open there with sync on?", peer.name)))
}

/// Sends one request to a paired device and returns its answer.
async fn request(state: &AppState, peer: &SyncPeer, req: &Request) -> CmdResult<Response> {
    let run = async {
        let (mut ch, addr) = open(state, peer).await?;
        ch.send_json(req).await.map_err(err)?;
        let r: Response = ch.recv_json().await.map_err(err)?;
        Ok::<_, CommandError>((r, addr))
    };
    let res = match tokio::time::timeout(REQUEST_TIMEOUT, run).await {
        Ok(r) => r,
        Err(_) => Err(CommandError::msg(format!("{} took too long to answer.", peer.name))),
    };
    match res {
        Ok((Response::Error { message }, addr)) => {
            let store = state.store();
            store.sync_peer_seen(&peer.id, None, Some(&addr))?;
            store.sync_peer_failed(&peer.id, &message)?;
            Err(CommandError::msg(message))
        }
        Ok((r, addr)) => {
            state.store().sync_peer_seen(&peer.id, None, Some(&addr))?;
            Ok(r)
        }
        Err(e) => {
            state.store().sync_peer_failed(&peer.id, &e.message)?;
            Err(e)
        }
    }
}

fn peer(state: &AppState, id: &str) -> CmdResult<SyncPeer> {
    state.store().sync_peer(id)?.ok_or_else(|| CommandError::msg("That device isn't paired any more."))
}

/// One automatic sync with a device. Returns what changed here.
async fn sync_with(app: &AppHandle, peer: &SyncPeer) -> CmdResult<SyncRun> {
    let state = app.state::<AppState>();
    if !peer.mode.automatic() {
        return Err(CommandError::msg("Automatic sync is off for this device."));
    }
    state.sync.attempts.lock().expect("poisoned").insert(peer.id.clone(), Instant::now());
    let (schema, items) = {
        let mut store = state.store();
        let items = merge::scan(&mut store, now_ms())?.items;
        (store.schema_version()?, if peer.mode.sends() { items } else { Vec::new() })
    };
    let Response::Synced { items: theirs, took } = request(&state, peer, &Request::Sync { schema, items }).await?
    else {
        return Err(CommandError::msg("unexpected answer"));
    };
    let merged = if peer.mode.receives() && peer.accept_changes && !theirs.is_empty() {
        let mut store = state.store();
        merge::merge(&mut store, &theirs, now_ms())?
    } else {
        Merged::default()
    };
    state.store().sync_peer_synced(&peer.id)?;
    if merged.any() {
        emit(app, SyncEvent::Applied { from: peer.name.clone(), merged: merged.clone() });
    }
    Ok(SyncRun { merged, sent: took })
}

/// The automatic loop: every few seconds, stamp local changes and send them
/// to devices with automatic sync; ask each for theirs now and then.
fn start_loop(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut g = state.sync.looper.lock().expect("poisoned");
    if g.is_some() {
        return;
    }
    let app2 = app.clone();
    *g = Some(tauri::async_runtime::spawn(async move {
        loop {
            {
                let state = app2.state::<AppState>();
                tokio::select! {
                    _ = tokio::time::sleep(TICK) => {}
                    _ = state.sync.wake.notified() => {}
                }
            }
            let state = app2.state::<AppState>();
            let peers: Vec<SyncPeer> = match state.store().sync_peers() {
                Ok(p) => p.into_iter().filter(|p| p.mode.automatic()).collect(),
                Err(_) => continue,
            };
            if peers.is_empty() {
                continue;
            }
            let changed = {
                let mut store = state.store();
                merge::scan(&mut store, now_ms()).map(|s| s.changed > 0).unwrap_or(false)
            };
            for p in peers {
                let last = state.sync.attempts.lock().expect("poisoned").get(&p.id).copied();
                let due = last.map_or(true, |t| t.elapsed() >= CATCH_UP);
                if !(due || (changed && p.mode.sends())) {
                    continue;
                }
                if let Err(e) = sync_with(&app2, &p).await {
                    tracing::debug!(peer = %p.name, "sync: {}", e.message);
                }
            }
        }
    }));
}

// ---------------------------------------------------------------- commands

#[tauri::command]
#[specta::specta]
pub fn sync_info(state: State<'_, AppState>) -> CmdResult<SyncInfo> {
    info(&state)
}

/// Turns the listener on or off, and sets the port and this device's name.
#[tauri::command]
#[specta::specta]
pub fn sync_configure(app: AppHandle, settings: SyncSettings) -> CmdResult<SyncInfo> {
    if settings.port < 1024 {
        return Err(CommandError::msg("Use a port from 1024 up."));
    }
    if settings.device_name.chars().count() > 64 {
        return Err(CommandError::msg("Keep the name under 64 characters."));
    }
    let state = app.state::<AppState>();
    {
        let store = state.store();
        let mut s = AppSettings::load(&store)?;
        let passphrase_pairing = settings.passphrase_pairing && core_sync::pass_secret(&state.keychain)?.is_some();
        s.sync = SyncSettings {
            device_name: settings.device_name.trim().to_owned(),
            // Being findable needs the listener.
            enabled: settings.enabled || settings.discoverable,
            passphrase_pairing,
            ..settings
        };
        s.save(&store)?;
    }
    apply(&app);
    info(&state)
}

/// Shows a one-time code another device can pair with (needs the listener).
#[tauri::command]
#[specta::specta]
pub fn sync_pair_start(state: State<'_, AppState>, accept_changes: bool) -> CmdResult<SyncInfo> {
    if state.sync.bound.lock().expect("poisoned").0.is_none() {
        return Err(CommandError::msg("Turn on \"Let paired devices reach this one\" first."));
    }
    if state.store().sync_peers()?.len() >= core_sync::MAX_PEERS {
        return Err(CommandError::msg(format!("At most {} paired devices.", core_sync::MAX_PEERS)));
    }
    *state.sync.pairing.lock().expect("poisoned") = Some(Pending {
        code: pair::new_code(),
        expires: Instant::now() + Duration::from_secs(pair::CODE_TTL_SECS),
        attempts: pair::CODE_ATTEMPTS,
        accept_changes,
        asked_by: None,
    });
    info(&state)
}

#[tauri::command]
#[specta::specta]
pub fn sync_pair_cancel(state: State<'_, AppState>) -> CmdResult<SyncInfo> {
    *state.sync.pairing.lock().expect("poisoned") = None;
    info(&state)
}

/// What proves the pairing: a code shown there, or the passphrase key.
enum PairSecret {
    Code(String),
    Pass(zeroize::Zeroizing<[u8; 32]>),
}

/// Pairs with the device at `host:port`; returns its key and what it said.
async fn pair_client(
    state: &AppState,
    host: &str,
    port: u16,
    secret: &PairSecret,
    hello: Hello,
) -> CmdResult<([u8; 32], Hello)> {
    let me = identity(state)?;
    let wrong = match secret {
        PairSecret::Code(_) => "Pairing failed: the code is wrong or has expired.",
        PairSecret::Pass(_) => "Pairing failed: that passphrase doesn't match the one set there (or it took too many wrong tries; wait 10 minutes).",
    };
    let run = async {
        let mut s = dial(host, port).await.map_err(|e| {
            CommandError::msg(format!(
                "Couldn't reach {host}:{port} ({e}). Check the address, and that sync is on there."
            ))
        })?;
        let psk = match secret {
            PairSecret::Code(code) => {
                wire::write_header(&mut s, wire::KIND_PAIR).await.map_err(err)?;
                pair::initiator(&mut s, code).await
            }
            PairSecret::Pass(key) => {
                wire::write_header(&mut s, wire::KIND_PAIR_PASS).await.map_err(err)?;
                pair::pass_initiator(&mut s, key).await
            }
        }
        .map_err(|_| CommandError::msg(wrong))?;
        let mut ch = wire::pair_initiator(s, &me, &psk).await.map_err(|_| CommandError::msg(wrong))?;
        ch.send_json(&hello).await.map_err(err)?;
        let mut theirs: Hello = ch.recv_json().await.map_err(|_| CommandError::msg(wrong))?;
        theirs.addrs.insert(0, host.to_owned());
        theirs.port = port;
        Ok::<_, CommandError>((ch.remote_key(), theirs))
    };
    tokio::time::timeout(Duration::from_secs(30), run).await.map_err(|_| CommandError::msg("Pairing timed out."))?
}

/// Pairs with a device that shows a code. `addr` is its address, with or
/// without the port.
#[tauri::command]
#[specta::specta]
pub async fn sync_pair_join(app: AppHandle, addr: String, code: String, accept_changes: bool) -> CmdResult<SyncPeer> {
    let state = app.state::<AppState>();
    if pair::normalize(&code).is_none() {
        return Err(CommandError::msg("A pairing code is 10 letters and digits, like ABCDE-12345."));
    }
    let default_port = AppSettings::load(&state.store())?.sync.port;
    let (host, port) = split_addr(&addr, default_port)?;
    let hello = my_hello(&state)?;
    let (key, theirs) = pair_client(&state, &host, port, &PairSecret::Code(code), hello).await?;
    let peer = state.store().sync_peer_add(&key, &theirs, accept_changes)?;
    tracing::info!(peer = %peer.name, "sync: paired");
    emit(&app, SyncEvent::Peers);
    Ok(peer)
}

/// Looks for other Nightgrid apps on the LAN (D-208). Works on a plain
/// network: no internet, tailnet or mDNS service involved.
#[tauri::command]
#[specta::specta]
pub async fn sync_discover(state: State<'_, AppState>) -> CmdResult<Vec<FoundDevice>> {
    let own = core_sync::device_id(&identity(&state)?.public);
    let (schema, peers) = {
        let store = state.store();
        (store.schema_version()?, store.sync_peers()?)
    };
    let targets = discover::targets(crate::status_cmd::lan_addr(), discover::DISCOVERY_PORT);
    let found = discover::scan(&targets, SCAN_WAIT, &own)
        .await
        .map_err(|e| CommandError::msg(format!("Couldn't search the network: {e}")))?;
    Ok(found
        .into_iter()
        .map(|f| FoundDevice {
            paired: peers.iter().any(|p| p.id == f.beacon.id),
            id: f.beacon.id,
            name: f.beacon.name,
            addr: f.addr.to_string(),
            port: f.beacon.port,
            app_version: f.beacon.app_version,
            same_version: f.beacon.schema == schema,
            hosts: f.beacon.hosts,
            passphrase: f.beacon.passphrase,
            salt: f.beacon.salt,
        })
        .collect())
}

/// Asks a device found on the LAN to show a pairing code on its screen.
#[tauri::command]
#[specta::specta]
pub async fn sync_ask_code(state: State<'_, AppState>, addr: String, port: u16) -> CmdResult<()> {
    let name = device_name(&AppSettings::load(&state.store())?.sync);
    let run = async {
        let mut s = dial(&addr, port).await?;
        wire::write_header(&mut s, wire::KIND_ASK).await?;
        let b = name.as_bytes();
        let b = &b[..b.len().min(64)];
        s.write_u8(b.len() as u8).await?;
        s.write_all(b).await?;
        s.flush().await?;
        s.read_u8().await
    };
    match tokio::time::timeout(Duration::from_secs(10), run).await {
        Ok(Ok(ASK_SHOWN)) => Ok(()),
        Ok(Ok(ASK_FULL)) => Err(CommandError::msg("That device has as many paired devices as it takes.")),
        Ok(Ok(_)) => Err(CommandError::msg("That device doesn't take pairing requests right now.")),
        Ok(Err(e)) => {
            Err(CommandError::msg(format!("Couldn't reach {addr}:{port} ({e}). Is Nightgrid still open there?")))
        }
        Err(_) => Err(CommandError::msg("It didn't answer in time.")),
    }
}

/// Pairs with a device found on the LAN (with a code it shows, or the
/// profile passphrase), optionally imports its profile, and turns on
/// two-way sync on both sides when asked.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn sync_join(
    app: AppHandle,
    addr: String,
    port: u16,
    code: Option<String>,
    passphrase: Option<String>,
    salt: Option<String>,
    import_mode: Option<ImportMode>,
    two_way: bool,
    secrets: Vec<SecretCategory>,
) -> CmdResult<SyncJoined> {
    let state = app.state::<AppState>();
    let secret = match (code, passphrase) {
        (Some(c), _) if !c.trim().is_empty() => {
            if pair::normalize(&c).is_none() {
                return Err(CommandError::msg("A pairing code is 10 letters and digits, like ABCDE-12345."));
            }
            PairSecret::Code(c)
        }
        (_, Some(p)) if !p.is_empty() => {
            let salt = salt
                .as_deref()
                .and_then(discover::decode_salt)
                .ok_or_else(|| CommandError::msg("That device doesn't take pairing by passphrase."))?;
            let key = tokio::task::spawn_blocking(move || discover::pass_key(&p, &salt)).await.map_err(err)?;
            PairSecret::Pass(key)
        }
        _ => return Err(CommandError::msg("Enter the code shown there, or your profile passphrase.")),
    };
    let mut hello = my_hello(&state)?;
    hello.join = Some(Join { two_way, secrets: secrets.clone() });
    let (key, theirs) = pair_client(&state, &addr, port, &secret, hello).await?;
    let mut peer = state.store().sync_peer_add(&key, &theirs, true)?;
    // Mode stays manual until the import is in, so the other side's first
    // automatic round doesn't race it.
    peer = state.store().sync_peer_set(&peer.id, &peer.name, SyncMode::Manual, &secrets, true)?;
    tracing::info!(peer = %peer.name, "sync: paired from the LAN list");
    let (mut imported, mut import_error) = (None, None);
    if let Some(mode) = import_mode {
        match request(&state, &peer, &Request::Pull { secrets: secrets.clone() }).await {
            Ok(Response::Profile { bundle }) => {
                let r = {
                    let mut store = state.store();
                    core_sync::apply_bundle(&mut store, &state.keychain, *bundle, mode, &secrets).and_then(|r| {
                        merge::reset(&mut store)?;
                        store.sync_peer_synced(&peer.id)?;
                        Ok(r)
                    })
                };
                match r {
                    Ok(a) => {
                        imported = Some(BackupImported {
                            added: a.outcome.added.values().sum(),
                            skipped: a.outcome.skipped.values().sum(),
                            secrets: a.secrets,
                            settings: a.outcome.settings_applied,
                            secrets_error: a.secrets_error,
                        })
                    }
                    Err(e) => import_error = Some(e.to_string()),
                }
            }
            Ok(_) => import_error = Some("unexpected answer".into()),
            Err(e) => import_error = Some(e.message),
        }
    }
    if two_way {
        peer = state.store().sync_peer_set(&peer.id, &peer.name, SyncMode::TwoWay, &secrets, true)?;
        state.sync.attempts.lock().expect("poisoned").remove(&peer.id);
        state.sync.wake.notify_one();
    }
    emit(&app, SyncEvent::Peers);
    Ok(SyncJoined { peer, imported, import_error })
}

/// Sets up (or with `None` turns off) pairing with the profile passphrase.
#[tauri::command]
#[specta::specta]
pub async fn sync_passphrase(app: AppHandle, passphrase: Option<String>) -> CmdResult<SyncInfo> {
    let state = app.state::<AppState>();
    let on = passphrase.is_some();
    core_sync::set_pass_secret(&state.keychain, passphrase.as_deref())?;
    {
        let store = state.store();
        let mut s = AppSettings::load(&store)?;
        s.sync.passphrase_pairing = on;
        s.save(&store)?;
    }
    *state.sync.pass_fails.lock().expect("poisoned") = (0, None);
    info(&state)
}

/// Changes how this device syncs with a paired one.
#[tauri::command]
#[specta::specta]
pub fn sync_peer_set(
    state: State<'_, AppState>,
    id: String,
    name: String,
    mode: SyncMode,
    secrets: Vec<SecretCategory>,
    accept_changes: bool,
) -> CmdResult<SyncPeer> {
    let p = state.store().sync_peer_set(&id, &name, mode, &secrets, accept_changes)?;
    if mode.automatic() {
        state.sync.attempts.lock().expect("poisoned").remove(&id);
        state.sync.wake.notify_one();
    }
    Ok(p)
}

/// Forgets a paired device (and asks it to forget this one).
#[tauri::command]
#[specta::specta]
pub async fn sync_unpair(app: AppHandle, id: String) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let p = peer(&state, &id)?;
    if let Err(e) = request(&state, &p, &Request::Unpair).await {
        tracing::info!(peer = %p.name, "sync: couldn't tell it about the unpairing: {}", e.message);
    }
    state.store().sync_peer_delete(&id)?;
    emit(&app, SyncEvent::Peers);
    Ok(())
}

/// Checks that a paired device answers.
#[tauri::command]
#[specta::specta]
pub async fn sync_check(app: AppHandle, id: String) -> CmdResult<SyncPeer> {
    let state = app.state::<AppState>();
    let p = peer(&state, &id)?;
    let hello = my_hello(&state)?;
    match request(&state, &p, &Request::Hello(hello)).await? {
        Response::Hello(h) => state.store().sync_peer_seen(&id, Some(&h), None)?,
        _ => return Err(CommandError::msg("unexpected answer")),
    }
    peer(&state, &id)
}

/// Sends this profile to a paired device (merged or replacing what's there).
#[tauri::command]
#[specta::specta]
pub async fn sync_push(app: AppHandle, id: String, mode: ImportMode) -> CmdResult<SyncPushed> {
    let state = app.state::<AppState>();
    let p = peer(&state, &id)?;
    let bundle = {
        let store = state.store();
        core_sync::bundle_for(&store, &state.keychain, &p.secrets, env!("CARGO_PKG_VERSION"), now_ms() / 1000)?
    };
    match request(&state, &p, &Request::Push { mode, bundle: Box::new(bundle) }).await? {
        Response::Pushed { added, skipped, secrets } => {
            state.store().sync_peer_synced(&id)?;
            Ok(SyncPushed { added, skipped, secrets })
        }
        _ => Err(CommandError::msg("unexpected answer")),
    }
}

/// Takes a paired device's profile (merged or replacing this one's). The
/// app should restart afterwards, like after an import.
#[tauri::command]
#[specta::specta]
pub async fn sync_pull(app: AppHandle, id: String, mode: ImportMode) -> CmdResult<BackupImported> {
    let state = app.state::<AppState>();
    let p = peer(&state, &id)?;
    let Response::Profile { bundle } = request(&state, &p, &Request::Pull { secrets: p.secrets.clone() }).await? else {
        return Err(CommandError::msg("unexpected answer"));
    };
    let applied = {
        let mut store = state.store();
        let r = core_sync::apply_bundle(&mut store, &state.keychain, *bundle, mode, &p.secrets)?;
        merge::reset(&mut store)?;
        store.sync_peer_synced(&id)?;
        r
    };
    tracing::info!(from = %p.name, ?mode, "sync: pulled a profile");
    Ok(BackupImported {
        added: applied.outcome.added.values().sum(),
        skipped: applied.outcome.skipped.values().sum(),
        secrets: applied.secrets,
        settings: applied.outcome.settings_applied,
        secrets_error: applied.secrets_error,
    })
}

/// Runs an automatic sync with a device now.
#[tauri::command]
#[specta::specta]
pub async fn sync_now(app: AppHandle, id: String) -> CmdResult<SyncRun> {
    let state = app.state::<AppState>();
    let p = peer(&state, &id)?;
    sync_with(&app, &p).await
}

#[cfg(test)]
mod tests {
    use super::split_addr;

    #[test]
    fn addresses_split() {
        assert_eq!(split_addr("10.0.0.5", 47920).unwrap(), ("10.0.0.5".into(), 47920));
        assert_eq!(split_addr("10.0.0.5:5000", 47920).unwrap(), ("10.0.0.5".into(), 5000));
        assert_eq!(split_addr("mac.tail1234.ts.net", 47920).unwrap(), ("mac.tail1234.ts.net".into(), 47920));
        assert_eq!(split_addr("ally:6000", 47920).unwrap(), ("ally".into(), 6000));
        assert_eq!(split_addr("[fd7a::1]:6000", 47920).unwrap(), ("fd7a::1".into(), 6000));
        assert_eq!(split_addr("fd7a::1", 47920).unwrap(), ("fd7a::1".into(), 47920));
        assert!(split_addr(" ", 1).is_err());
        assert!(split_addr("ally:x", 1).is_err());
    }
}
