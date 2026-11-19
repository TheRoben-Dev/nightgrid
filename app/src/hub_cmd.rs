//! Node setup and the hub (D-175): one host's node and hub in detail
//! (link, hub membership, units, `doctor`), installing the hub, making
//! nodes members of it, editing either program's config with a rollback,
//! and their logs. The logic lives in `nightgrid_core::node::hub`; this
//! runs it over the pooled SSH connections and the sudo flow.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use nightgrid_core::files::path::shell_quote;
use nightgrid_core::files::transfer::Side;
use nightgrid_core::hosts::HostId;
use nightgrid_core::journal::ChangeArea;
use nightgrid_core::node::client::NodeClient;
use nightgrid_core::node::hub::{
    self, ConfigSaveReport, DoctorFinding, HubMemberState, HubOverview, HubProbe, HubReport, NodePart, NodePeer,
    NodePeerList, NodePeerReading, NodePeersLatest, NodeUnit,
};
use nightgrid_core::node::{install, NodeEvent};
use nightgrid_core::ssh::client::Connection;
use nightgrid_core::system::sudo;
use serde::Serialize;
use serde_json::{json, Value};
use specta::Type;
use tauri::{AppHandle, Manager};
use tokio::io::AsyncWriteExt;

use crate::journal_cmd::{record, Change};
use crate::node_cmd::{ensure, err, host_name, pick_binary, reconnect, release_binary};
use crate::system_cmd::{resolve_sudo, SudoInput, SysResult};
use crate::term_cmd::connect_posix;

async fn exec(conn: &Connection, cmd: &str) -> SysResult<(i32, String, String)> {
    let (code, out, e) = tokio::time::timeout(Duration::from_secs(30), conn.exec(cmd))
        .await
        .map_err(|_| err("the host didn't answer in time"))??;
    Ok((code, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&e).into_owned()))
}

async fn hub_probe(conn: &Connection) -> SysResult<HubProbe> {
    Ok(hub::parse_probe(&exec(conn, &hub::probe_script()).await?.1))
}

/// One request to the hub's socket.
async fn hub_call(conn: &Connection, p: &HubProbe, method: &str, params: Value) -> SysResult<Value> {
    if !p.usable() {
        return Err(err("the hub isn't running here, or this login can't use it yet (Update fixes access)"));
    }
    let (code, out, e) = exec(conn, &hub::query_command(p, method, &params)).await?;
    if code != 0 {
        let why = e.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("the hub didn't answer").trim().to_owned();
        return Err(err(format!("hub {method}: {why}")));
    }
    serde_json::from_str(out.trim()).map_err(|e| err(format!("hub {method}: unexpected answer ({e})")))
}

async fn node_call<T: serde::de::DeserializeOwned>(c: &NodeClient, method: &str, params: Value) -> SysResult<T> {
    c.call(method, params).await.map_err(|e| err(format!("{method}: {e}")))
}

// ---- the setup view ----

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LinkView {
    /// `ok`, `off` (root hasn't allowed the link) or `denied`.
    pub access: String,
    pub fingerprint: Option<String>,
    pub listening: Option<String>,
    pub serve: Vec<String>,
    pub peers: Vec<NodePeer>,
    pub readings: Vec<NodePeerReading>,
    /// The hub this node names (`hub.member`), if any.
    pub hub_id: Option<String>,
    /// The node can be a hub member (0.26.0 or newer).
    pub can_join: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HubEventView {
    pub at: f64,
    pub level: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HubView {
    pub probe: HubProbe,
    pub overview: Option<HubOverview>,
    pub events: Vec<HubEventView>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NodeSetup {
    pub host_id: HostId,
    /// Full node id (for matching members), when the node is connected.
    pub node_id: Option<String>,
    pub link: Option<LinkView>,
    pub hub: Option<HubView>,
    pub units: Vec<NodeUnit>,
    pub doctor: Vec<DoctorFinding>,
    /// Problems reading parts of this (the rest is still shown).
    pub notes: Vec<String>,
}

async fn link_view(c: &NodeClient, notes: &mut Vec<String>) -> Option<LinkView> {
    let w = c.welcome();
    if w.cap("peers") < 1 {
        return None;
    }
    let latest: NodePeersLatest = match node_call(c, "peers.latest", json!({})).await {
        Ok(v) => v,
        Err(e) => {
            notes.push(e.message);
            return None;
        }
    };
    let list: NodePeerList = node_call(c, "peers.get", json!({})).await.unwrap_or_default();
    let can_join = w.cap("hub.member") >= 1;
    let hub_id = if can_join {
        node_call::<hub::HubMember>(c, "hub.member.get", json!({})).await.ok().and_then(|m| m.hub_id)
    } else {
        None
    };
    Some(LinkView {
        access: latest.access,
        fingerprint: latest.fingerprint,
        listening: latest.listening,
        serve: list.serve,
        peers: list.peers,
        readings: latest.readings,
        hub_id,
        can_join,
    })
}

async fn hub_view(conn: &Connection) -> SysResult<Option<HubView>> {
    let probe = hub_probe(conn).await?;
    if !probe.installed {
        return Ok(None);
    }
    let mut v = HubView { probe: probe.clone(), overview: None, events: vec![], error: None };
    if probe.usable() {
        match hub_call(conn, &probe, "fleet.overview", Value::Null).await {
            Ok(o) => v.overview = serde_json::from_value(o).ok(),
            Err(e) => v.error = Some(e.message),
        }
        if let Ok(page) = hub_call(conn, &probe, "fleet.events.sync", json!({ "after": 0, "limit": 500 })).await {
            let events: Vec<NodeEvent> =
                serde_json::from_value(page.get("events").cloned().unwrap_or_default()).unwrap_or_default();
            v.events = events
                .iter()
                .rev()
                .take(15)
                .map(|e| HubEventView { at: e.at as f64, level: e.level.clone(), title: e.title.clone() })
                .collect();
        }
    } else if probe.socket {
        v.error = Some("this login isn't in the nightgrid-hub group yet: Update the hub to add it".into());
    } else {
        v.error =
            Some(format!("the hub's service is {}", if probe.unit.is_empty() { "not running" } else { &probe.unit }));
    }
    Ok(Some(v))
}

/// Everything about the node and hub on one host: link and hub
/// membership, the hub (if this host runs one), units and `doctor`.
#[tauri::command]
#[specta::specta]
pub async fn node_setup(app: AppHandle, host_id: HostId) -> SysResult<NodeSetup> {
    let conn = connect_posix(&app, host_id, "node").await?;
    let mut notes = Vec::new();
    let client = ensure(&app, host_id).await;
    let link = match &client {
        Some(c) => link_view(c, &mut notes).await,
        None => None,
    };
    let hub = hub_view(&conn).await.unwrap_or_else(|e| {
        notes.push(e.message);
        None
    });
    let mut script = hub::units_script(NodePart::Node);
    if hub.is_some() {
        script.push_str(&hub::units_script(NodePart::Hub));
    }
    let units = exec(&conn, &script).await.map(|(_, o, _)| hub::parse_units(&o)).unwrap_or_default();
    let doctor = match exec(&conn, "command -v nightgrid-node >/dev/null 2>&1 && nightgrid-node doctor --json").await {
        Ok((_, o, _)) => hub::parse_doctor(&o),
        Err(_) => vec![],
    };
    Ok(NodeSetup {
        host_id,
        node_id: client.as_ref().map(|c| c.welcome().node.node_id.clone()),
        link,
        hub,
        units,
        doctor,
        notes,
    })
}

// ---- config and logs ----

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NodeConfigFile {
    pub path: String,
    pub text: String,
    /// The file is there (a missing one means the defaults apply).
    pub exists: bool,
}

async fn read_config(conn: &Connection, part: NodePart) -> SysResult<NodeConfigFile> {
    let path = part.config();
    let (code, out, _) = exec(conn, &format!("cat {path} 2>/dev/null")).await?;
    Ok(NodeConfigFile { path: path.into(), text: if code == 0 { out } else { String::new() }, exists: code == 0 })
}

/// The program's config file (both are world-readable; no sudo).
#[tauri::command]
#[specta::specta]
pub async fn node_config_read(app: AppHandle, host_id: HostId, part: NodePart) -> SysResult<NodeConfigFile> {
    let conn = connect_posix(&app, host_id, "node").await?;
    read_config(&conn, part).await
}

/// Uploads `text` to a private temp folder and returns the folder.
async fn upload(
    app: &AppHandle,
    host_id: HostId,
    conn: &Connection,
    name: &str,
    bytes: &[u8],
    mode: u32,
) -> SysResult<(String, String)> {
    let (code, dir, e) = exec(conn, "mktemp -d /tmp/nightgrid-node.XXXXXX").await?;
    let dir = dir.trim().to_owned();
    if code != 0 || !dir.starts_with("/tmp/nightgrid-node.") {
        return Err(err(format!("couldn't make a temp folder: {}", e.trim())));
    }
    let path = format!("{dir}/{name}");
    let res = async {
        let fs = crate::files_cmd::fs_for(app, Side::Host { id: host_id }).await.map_err(|e| err(e.message))?;
        let mut w = fs.open_write(&path, None).await.map_err(err)?;
        w.write_all(bytes).await.map_err(err)?;
        w.shutdown().await.map_err(err)?;
        drop(w);
        fs.set_mode(&path, mode).await.map_err(err)?;
        Ok::<_, crate::system_cmd::SystemCmdError>(())
    }
    .await;
    if let Err(e) = res {
        let _ = conn.exec(&format!("rm -rf {}", shell_quote(&dir))).await;
        return Err(e);
    }
    Ok((dir, path))
}

async fn save_config(
    app: &AppHandle,
    host_id: HostId,
    conn: &Connection,
    auth: &sudo::SudoAuth,
    part: NodePart,
    text: &str,
    title: &str,
) -> SysResult<ConfigSaveReport> {
    hub::precheck(part, text).map_err(err)?;
    let old = read_config(conn, part).await?;
    if old.exists && old.text == text {
        return Ok(ConfigSaveReport { ok: true, steps: vec!["nothing changed".into()], ..Default::default() });
    }
    let (dir, path) = upload(app, host_id, conn, "config.toml", text.as_bytes(), 0o644).await?;
    let result = async {
        if part == NodePart::Node {
            // The node checks the file itself before it goes anywhere.
            let (_, out, _) =
                exec(conn, &format!("nightgrid-node doctor --config {} --json", shell_quote(&path))).await?;
            if let Some(f) = hub::parse_doctor(&out).into_iter().find(|f| f.check == "config" && f.level == "fail") {
                return Err(err(format!("the node refuses this config: {}", f.detail.replace(&path, part.config()))));
            }
        }
        let reapply = hub::reapply_after(part, &old.text, text);
        let o = sudo::run(conn, auth, &hub::save_script(part, &path, &reapply)).await?;
        Ok(hub::parse_save(&format!("{}\n{}", o.stdout, o.stderr), o.code))
    }
    .await;
    let _ = conn.exec(&format!("rm -rf {}", shell_quote(&dir))).await;
    let r = result?;
    record(
        app,
        Change::new(host_id, ChangeArea::System, title)
            .detail(&[r.steps.join("\n"), r.failure.clone().unwrap_or_default()].join("\n"))
            .ok(r.ok),
    );
    Ok(r)
}

/// Saves the program's config as root: checked first (the node's own
/// `doctor`, TOML and sections for the hub), the old file kept as `.bak`,
/// services restarted, `allow` run again where networks changed, and the
/// old file put back if the main service doesn't stay up.
#[tauri::command]
#[specta::specta]
pub async fn node_config_save(
    app: AppHandle,
    host_id: HostId,
    part: NodePart,
    text: String,
    sudo: Option<SudoInput>,
) -> SysResult<ConfigSaveReport> {
    let conn = connect_posix(&app, host_id, "node").await?;
    let auth = resolve_sudo(&app, host_id, &conn, sudo).await?;
    let title = format!("Edited the {} config", part.binary());
    let r = save_config(&app, host_id, &conn, &auth, part, &text, &title).await?;
    if part == NodePart::Node {
        reconnect(&app, host_id).await;
    }
    Ok(r)
}

/// The last journal lines of the program's units (as root: the journal is
/// usually closed to plain users).
#[tauri::command]
#[specta::specta]
pub async fn node_logs(
    app: AppHandle,
    host_id: HostId,
    part: NodePart,
    lines: u32,
    sudo: Option<SudoInput>,
) -> SysResult<String> {
    let conn = connect_posix(&app, host_id, "node").await?;
    let auth = resolve_sudo(&app, host_id, &conn, sudo).await?;
    let o = sudo::run(&conn, &auth, &hub::logs_command(part, lines)).await?;
    Ok(o.stdout)
}

// ---- the hub ----

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HubStepResult {
    pub report: HubReport,
    pub setup: NodeSetup,
}

fn merge(into: &mut HubReport, r: HubReport) {
    into.steps.extend(r.steps);
    into.warnings.extend(r.warnings);
    into.users.extend(r.users);
    if r.hub_id.is_some() {
        into.hub_id = r.hub_id;
    }
    if r.fingerprint.is_some() {
        into.fingerprint = r.fingerprint;
    }
    if r.listening.is_some() {
        into.listening = r.listening;
    }
    if !r.version.is_empty() {
        into.version = r.version;
    }
}

/// Runs a `--json` command as root and reads its report; fails with its
/// last stderr line.
async fn root_report(conn: &Connection, auth: &sudo::SudoAuth, cmd: &str, what: &str) -> SysResult<HubReport> {
    let o = sudo::run(conn, auth, cmd).await?;
    if o.code != 0 {
        let why = o.stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("failed").trim().to_owned();
        return Err(err(format!("{what}: {why}")));
    }
    Ok(hub::parse_report(&o.stdout))
}

/// Makes `host_id` the hub of its network: installs (or updates)
/// `nightgrid-hub` from the same release as the node (checked against
/// SHA256SUMS, or a file the person picks), turns on this host's own link
/// and the hub's collector. Needs the node running on the host.
#[tauri::command]
#[specta::specta]
pub async fn hub_install(
    app: AppHandle,
    host_id: HostId,
    from_file: bool,
    sudo: Option<SudoInput>,
) -> SysResult<Option<HubStepResult>> {
    let conn = connect_posix(&app, host_id, "node").await?;
    let auth = resolve_sudo(&app, host_id, &conn, sudo).await?;
    let c = ensure(&app, host_id)
        .await
        .ok_or_else(|| err("install Nightgrid Node on this host first: the hub runs next to it"))?;
    if c.welcome().cap("peers") < 1 {
        return Err(err(format!(
            "the hub needs Nightgrid Node {} or newer here: Update it first",
            hub::MEMBER_MIN_VERSION
        )));
    }
    let (_, out, _) = exec(&conn, &install::probe_script()).await?;
    let probe = install::parse_probe(&out);
    let arch = install::arch(&probe.arch).ok_or_else(|| err(format!("there's no hub build for {}", probe.arch)))?;
    let bin = if from_file {
        let Some(path) = pick_binary(&app).await else { return Ok(None) };
        let bin = tokio::fs::read(&path).await.map_err(err)?;
        install::looks_like_binary(&bin).map_err(err)?;
        bin
    } else {
        release_binary(&app, "nightgrid-hub", arch).await.map_err(err)?.1
    };
    let (dir, path) = upload(&app, host_id, &conn, "nightgrid-hub", &bin, 0o700).await?;
    let installed = root_report(&conn, &auth, &hub::install_command(&path, &probe.user), "the hub's install").await;
    let _ = conn.exec(&format!("rm -rf {}", shell_quote(&dir))).await;
    let mut report = installed?;
    // This host is a member of its own hub over its own link.
    let latest: NodePeersLatest = node_call(&c, "peers.latest", json!({})).await.unwrap_or_default();
    if latest.access != "ok" {
        merge(
            &mut report,
            root_report(&conn, &auth, "nightgrid-node allow peers --json", "turning on the link").await?,
        );
    }
    merge(
        &mut report,
        root_report(&conn, &auth, "nightgrid-hub allow collect --json", "starting the collector").await?,
    );
    record(
        &app,
        Change::new(host_id, ChangeArea::System, "Made this host the Nightgrid hub").detail(&report.steps.join("\n")),
    );
    reconnect(&app, host_id).await;
    let setup = node_setup(app, host_id).await?;
    Ok(Some(HubStepResult { report, setup }))
}

/// Removes the hub from `host_id` (its store and config too with `purge`).
/// The node stays. Members should leave first.
#[tauri::command]
#[specta::specta]
pub async fn hub_uninstall(
    app: AppHandle,
    host_id: HostId,
    purge: bool,
    sudo: Option<SudoInput>,
) -> SysResult<HubStepResult> {
    let conn = connect_posix(&app, host_id, "node").await?;
    let auth = resolve_sudo(&app, host_id, &conn, sudo).await?;
    let report = root_report(&conn, &auth, &hub::uninstall_command(purge), "removing the hub").await?;
    record(
        &app,
        Change::new(host_id, ChangeArea::System, "Removed the Nightgrid hub").detail(&report.steps.join("\n")),
    );
    let setup = node_setup(app, host_id).await?;
    Ok(HubStepResult { report, setup })
}

/// Waits for the link to report its certificate (it starts a moment after
/// `allow peers` or a restart).
async fn link_ready(app: &AppHandle, host: HostId) -> SysResult<(Arc<NodeClient>, NodePeersLatest)> {
    let mut last = None;
    for _ in 0..12 {
        if let Some(c) = ensure(app, host).await.filter(|c| !c.is_closed()) {
            if let Ok(l) = node_call::<NodePeersLatest>(&c, "peers.latest", json!({})).await {
                if l.access == "ok" && l.fingerprint.is_some() {
                    return Ok((c, l));
                }
                last = Some(l.access);
            }
        } else {
            reconnect(app, host).await;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err(err(format!(
        "the link on {} didn't start (it says {}); see its logs",
        host_name(app, host),
        last.unwrap_or_else(|| "nothing".into())
    )))
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HubJoinResult {
    pub steps: Vec<String>,
    pub warnings: Vec<String>,
    /// Addresses the hub's `[collect] networks` must allow before it can
    /// reach this member: pass them to `hub_allow_networks` on the hub host.
    pub hub_networks: Vec<String>,
}

/// Makes `member_id`'s node report to the hub on `hub_host` (spec 0020 §2):
/// turns on its link if needed, moves the link to its LAN address when it
/// has no tailnet one, allows the hub's address, pairs it with the hub's
/// collector, names the hub, lets root serve it (`serve add sync`), and
/// adds it to the hub's member list. `sudo` is for the member.
#[tauri::command]
#[specta::specta]
pub async fn hub_join(
    app: AppHandle,
    member_id: HostId,
    hub_host: HostId,
    sudo: Option<SudoInput>,
) -> SysResult<HubJoinResult> {
    let hconn = connect_posix(&app, hub_host, "node").await?;
    let mconn = connect_posix(&app, member_id, "node").await?;
    let auth = resolve_sudo(&app, member_id, &mconn, sudo).await?;
    let mut res = HubJoinResult { steps: vec![], warnings: vec![], hub_networks: vec![] };
    let member_name = host_name(&app, member_id);

    let hprobe = hub_probe(&hconn).await?;
    let ov: HubOverview =
        serde_json::from_value(hub_call(&hconn, &hprobe, "fleet.overview", Value::Null).await?).map_err(err)?;
    let hub_fp = ov
        .fingerprint
        .clone()
        .ok_or_else(|| err("the hub's collector hasn't started yet: open the hub and check its logs"))?;

    let c =
        ensure(&app, member_id).await.ok_or_else(|| err(format!("Nightgrid Node isn't running on {member_name}")))?;
    let w = c.welcome().clone();
    if w.cap("hub.member") < 1 || w.cap("peers") < 1 {
        return Err(err(format!(
            "{member_name} needs Nightgrid Node {} or newer: Update it first",
            hub::MEMBER_MIN_VERSION
        )));
    }
    let latest: NodePeersLatest = node_call(&c, "peers.latest", json!({})).await?;
    let (c, mut latest) = if latest.access == "ok" && latest.fingerprint.is_some() {
        (c, latest)
    } else {
        let r = root_report(&mconn, &auth, "nightgrid-node allow peers --json", "turning on the link").await?;
        res.steps.push("turned on the node-to-node link".into());
        res.warnings.extend(r.warnings);
        // The link may not get far without a tailnet address: read what it
        // says now, fix its address below, then wait for its certificate.
        tokio::time::sleep(Duration::from_secs(3)).await;
        let c = reconnect(&app, member_id).await.ok_or_else(|| err(format!("lost the node on {member_name}")))?;
        let l: NodePeersLatest = node_call(&c, "peers.latest", json!({})).await.unwrap_or_default();
        (c, l)
    };

    // Where the hub reaches the member from, and the member's own addresses.
    let host = app
        .state::<crate::state::AppState>()
        .store()
        .get_host(member_id)
        .ok()
        .flatten()
        .ok_or_else(|| err("that host is gone"))?;
    let target =
        latest.listening.as_deref().and_then(hub::addr_ip).map(|i| i.to_string()).unwrap_or(host.hostname.clone());
    let (seen_ip, hub_src) = hub::parse_route_script(&exec(&hconn, &hub::route_script(&target)).await?.1);
    let member_addrs = hub::parse_addrs(&exec(&mconn, hub::ADDRS_SCRIPT).await?.1);
    let mcfg = read_config(&mconn, NodePart::Node).await?;
    let hcfg = read_config(&hconn, NodePart::Hub).await?;
    let mdoc = hub::parse_config(&mcfg.text).map_err(err)?;
    let hdoc = hub::parse_config(&hcfg.text).map_err(err)?;
    let plan = hub::plan_link(
        latest.listening.as_deref(),
        seen_ip,
        &member_addrs,
        hub_src,
        &hub::get_list(&mdoc, "peers", "networks", hub::TAILNET_NETWORKS),
        &hub::get_list(&hdoc, "collect", "networks", hub::TAILNET_NETWORKS),
    )
    .map_err(|e| {
        err(format!("{member_name}: {e}. Set [peers] listen in its node config to an address the hub host can reach."))
    })?;

    let mut c = c;
    if plan.listen.is_some() || plan.member_network.is_some() {
        let mut doc = mdoc.clone();
        if let Some(l) = &plan.listen {
            hub::set_str(&mut doc, "peers", "listen", l);
            res.steps.push(format!("the link now listens on {l} (no tailnet address on this host)"));
        }
        if let Some(ip) = plan.member_network {
            hub::add_network(&mut doc, "peers", ip);
            res.steps.push(format!("the link accepts the hub host's address {ip}"));
        }
        let r = save_config(
            &app,
            member_id,
            &mconn,
            &auth,
            NodePart::Node,
            &doc.to_string(),
            "Set up the node's link for the hub",
        )
        .await?;
        if !r.ok {
            return Err(err(r.failure.unwrap_or_else(|| "saving the node config failed".into())));
        }
        res.warnings.extend(r.warnings);
        reconnect(&app, member_id).await;
        let (c2, l2) = link_ready(&app, member_id).await?;
        c = c2;
        latest = l2;
    }
    if latest.fingerprint.is_none() || latest.access != "ok" {
        let (c2, l2) = link_ready(&app, member_id).await?;
        c = c2;
        latest = l2;
    }
    let member_addr = latest.listening.clone().unwrap_or(plan.member_addr.clone());
    let member_fp = latest.fingerprint.clone().ok_or_else(|| err("the link has no certificate yet"))?;

    // Pair with the hub's collector and name it as this node's hub.
    // The member pings its hub at the collector's tailnet address, or at
    // the hub host's LAN address when the member is LAN-only.
    let on_tailnet = hub::addr_ip(&member_addr).is_some_and(hub::is_tailnet);
    let hub_addr = match (&ov.listening, hub_src) {
        (Some(l), _) if on_tailnet => l.clone(),
        (_, Some(src)) => hub::join_addr(src, hub::COLLECT_PORT),
        (Some(l), None) => l.clone(),
        (None, None) => {
            return Err(err("the hub's collector has no address yet: check that Tailscale is up on the hub host"))
        }
    };
    let mut list: NodePeerList = node_call(&c, "peers.get", json!({})).await?;
    let hub_name = hub::peer_name(&format!("hub {}", host_name(&app, hub_host)));
    hub::upsert(
        &mut list.peers,
        NodePeer { id: ov.hub_id.clone(), name: hub_name, address: hub_addr, fingerprint: hub_fp, follow_ups: false },
    );
    node_call::<Value>(&c, "peers.set", json!({ "peers": list.peers })).await?;
    node_call::<Value>(&c, "hub.member.set", json!({ "hubId": ov.hub_id })).await?;
    res.steps.push(format!("paired with the hub on {} and named it this node's hub", host_name(&app, hub_host)));
    if !list.serve.iter().any(|s| s == "sync") {
        root_report(&mconn, &auth, "nightgrid-node serve add sync --json", "allowing the hub to read").await?;
        res.steps.push("root now serves the hub read-only (serve add sync)".into());
        reconnect(&app, member_id).await;
    }

    // Add it to the hub's member list.
    let mut members: hub::Members =
        serde_json::from_value(hub_call(&hconn, &hprobe, "fleet.members.get", Value::Null).await?).unwrap_or_default();
    hub::upsert(
        &mut members.members,
        NodePeer {
            id: w.node.node_id.clone(),
            name: hub::peer_name(&member_name),
            address: member_addr,
            fingerprint: member_fp,
            follow_ups: false,
        },
    );
    hub_call(&hconn, &hprobe, "fleet.members.set", json!({ "members": members.members })).await?;
    res.steps.push("added to the hub's members".into());
    if let Some(ip) = plan.hub_network {
        res.hub_networks.push(hub::host_cidr(ip));
    }
    record(
        &app,
        Change::new(member_id, ChangeArea::System, &format!("Joined the hub on {}", host_name(&app, hub_host)))
            .detail(&res.steps.join("\n")),
    );
    Ok(res)
}

/// Takes `member_id` out of the hub on `hub_host`: off the hub's list,
/// unnamed and unpaired on the member, and `serve remove sync`.
#[tauri::command]
#[specta::specta]
pub async fn hub_leave(
    app: AppHandle,
    member_id: HostId,
    hub_host: HostId,
    sudo: Option<SudoInput>,
) -> SysResult<HubJoinResult> {
    let mconn = connect_posix(&app, member_id, "node").await?;
    let auth = resolve_sudo(&app, member_id, &mconn, sudo).await?;
    let mut res = HubJoinResult { steps: vec![], warnings: vec![], hub_networks: vec![] };
    let c = ensure(&app, member_id).await.ok_or_else(|| err("Nightgrid Node isn't running on this host"))?;
    let node_id = c.welcome().node.node_id.clone();
    let mine: hub::HubMember = node_call(&c, "hub.member.get", json!({})).await.unwrap_or_default();
    // The hub's list (if it's still there).
    if let Ok(hconn) = connect_posix(&app, hub_host, "node").await {
        if let Ok(p) = hub_probe(&hconn).await {
            if let Ok(v) = hub_call(&hconn, &p, "fleet.members.get", Value::Null).await {
                let mut m: hub::Members = serde_json::from_value(v).unwrap_or_default();
                let before = m.members.len();
                m.members.retain(|x| x.id != node_id);
                if m.members.len() != before {
                    match hub_call(&hconn, &p, "fleet.members.set", json!({ "members": m.members })).await {
                        Ok(_) => res.steps.push("removed from the hub's members".into()),
                        Err(e) => res.warnings.push(e.message),
                    }
                }
            }
        }
    }
    node_call::<Value>(&c, "hub.member.set", json!({ "hubId": null })).await?;
    if let Some(id) = mine.hub_id {
        let mut list: NodePeerList = node_call(&c, "peers.get", json!({})).await?;
        list.peers.retain(|p| p.id != id);
        node_call::<Value>(&c, "peers.set", json!({ "peers": list.peers })).await?;
        res.steps.push("unpaired from the hub".into());
    }
    let list: NodePeerList = node_call(&c, "peers.get", json!({})).await.unwrap_or_default();
    if list.serve.iter().any(|s| s == "sync") {
        root_report(&mconn, &auth, "nightgrid-node serve remove sync --json", "taking the hub's access away").await?;
        res.steps.push("root no longer serves a hub (serve remove sync)".into());
        reconnect(&app, member_id).await;
    }
    record(&app, Change::new(member_id, ChangeArea::System, "Left the Nightgrid hub").detail(&res.steps.join("\n")));
    Ok(res)
}

/// Lets the hub's collector reach these addresses (`[collect] networks`),
/// then runs `allow collect` again. `sudo` is for the hub host.
#[tauri::command]
#[specta::specta]
pub async fn hub_allow_networks(
    app: AppHandle,
    hub_host: HostId,
    networks: Vec<String>,
    sudo: Option<SudoInput>,
) -> SysResult<ConfigSaveReport> {
    let conn = connect_posix(&app, hub_host, "node").await?;
    let auth = resolve_sudo(&app, hub_host, &conn, sudo).await?;
    let cfg = read_config(&conn, NodePart::Hub).await?;
    let mut doc = hub::parse_config(&cfg.text).map_err(err)?;
    let mut changed = false;
    for n in &networks {
        let ip: IpAddr = n.split('/').next().unwrap_or("").parse().map_err(|_| err(format!("{n} isn't an address")))?;
        changed |= hub::add_network(&mut doc, "collect", ip);
    }
    if !changed {
        return Ok(ConfigSaveReport {
            ok: true,
            steps: vec!["the collector already reaches them".into()],
            ..Default::default()
        });
    }
    save_config(&app, hub_host, &conn, &auth, NodePart::Hub, &doc.to_string(), "Let the hub reach more members").await
}

/// Members of the hub on `hub_host` as the hub sees them (for the Fleet
/// list's role column).
#[tauri::command]
#[specta::specta]
pub async fn hub_members(app: AppHandle, hub_host: HostId) -> SysResult<Vec<HubMemberState>> {
    let conn = connect_posix(&app, hub_host, "node").await?;
    let p = hub_probe(&conn).await?;
    let o: HubOverview =
        serde_json::from_value(hub_call(&conn, &p, "fleet.overview", Value::Null).await?).map_err(err)?;
    Ok(o.members)
}
