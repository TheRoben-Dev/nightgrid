//! Docker over SSH (agentless): container list grouped by compose project,
//! resource stats, lifecycle actions, logs and inspect.
//!
//! See [`transport`] for how API requests reach the daemon.

pub mod compose;
pub mod gitstack;
pub mod proxy;
pub mod proxy_edit;
pub mod registry;
pub mod snapshot;
pub mod transport;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bollard::container::LogOutput;
use bollard::errors::Error as BollardError;
use bollard::models::{ContainerInspectResponse, ContainerStatsResponse, ContainerSummary};
use bollard::query_parameters::{
    EventsOptions, InspectContainerOptions, KillContainerOptions, ListContainersOptions, LogsOptions,
    RemoveContainerOptions, RestartContainerOptions, StartContainerOptions, StatsOptions, StopContainerOptions,
};
use bollard::Docker;
use futures_util::{stream, StreamExt};
use serde::{Deserialize, Serialize};

use crate::files::path::shell_quote;
use crate::hosts::HostId;
use crate::ssh::client::Connection;
pub use transport::Route;

/// Compose labels set by `docker compose` on every container it creates.
pub const LABEL_PROJECT: &str = "com.docker.compose.project";
pub const LABEL_SERVICE: &str = "com.docker.compose.service";
pub const LABEL_WORKING_DIR: &str = "com.docker.compose.project.working_dir";
pub const LABEL_CONFIG_FILES: &str = "com.docker.compose.project.config_files";

/// Per-request timeout for ordinary API calls (stop waits for the container's
/// grace period, so it gets its own budget below).
const API_TIMEOUT: u64 = 30;
/// How many stats requests run at once (each holds an SSH channel ~1-2 s).
const STATS_PARALLEL: usize = 6;
/// Masked stand-in for env values in raw inspect JSON.
pub const MASK: &str = "••••••";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum DockerError {
    #[error("Docker was not found on this host (no socket and no docker command)")]
    NotInstalled,
    #[error("permission denied on {socket}: add {user} to the docker group, then reconnect")]
    PermissionDenied { socket: String, user: String },
    #[error("no such container: {id}")]
    NotFound { id: String },
    #[error("Docker: {message}")]
    Api { status: u16, message: String },
    #[error("{message}")]
    Other { message: String },
}

impl DockerError {
    pub fn other(e: impl std::fmt::Display) -> Self {
        DockerError::Other { message: e.to_string() }
    }

    fn from_bollard(e: BollardError, id: Option<&str>) -> Self {
        match e {
            BollardError::DockerResponseServerError { status_code: 404, message } => match id {
                Some(id) => DockerError::NotFound { id: id.to_owned() },
                None => DockerError::Api { status: 404, message },
            },
            BollardError::DockerResponseServerError { status_code, message } => {
                DockerError::Api { status: status_code, message }
            }
            BollardError::RequestTimeoutError => DockerError::other("Docker did not answer in time"),
            e => DockerError::other(e),
        }
    }
}

pub type DockerResult<T> = std::result::Result<T, DockerError>;

/// Shell probe run once per connection to find the daemon socket.
/// Prints `<ok|denied|missing> <socket>`, the user name, and `cli|nocli`.
const PROBE: &str = r#"s=""; st=missing
for c in /var/run/docker.sock "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/docker.sock"; do
  [ -S "$c" ] || continue
  if [ -r "$c" ] && [ -w "$c" ]; then s=$c; st=ok; break; fi
  [ -n "$s" ] || { s=$c; st=denied; }
done
echo "$st $s"; id -un; command -v docker >/dev/null 2>&1 && echo cli || echo nocli"#;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Probe {
    status: String,
    socket: String,
    user: String,
    cli: bool,
}

fn parse_probe(out: &str) -> Probe {
    let mut lines = out.lines();
    let (status, socket) = lines.next().unwrap_or("missing").split_once(' ').unwrap_or(("missing", ""));
    Probe {
        status: status.to_owned(),
        socket: socket.trim().to_owned(),
        user: lines.next().unwrap_or("").trim().to_owned(),
        cli: lines.next().map(str::trim) == Some("cli"),
    }
}

/// Engine summary shown in the Docker view header.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub version: String,
    pub api_version: String,
    pub os: String,
    pub route: String,
}

/// A Docker engine on one host, reached over its pooled SSH connection.
pub struct DockerHost {
    docker: Docker,
    pub route: Route,
    pub conn: Arc<Connection>,
}

impl DockerHost {
    /// Finds the daemon and picks a route: socket forwarding first, then
    /// `docker system dial-stdio` if sshd refuses forwarding.
    pub async fn connect(conn: Arc<Connection>) -> DockerResult<Self> {
        // Windows: the engine listens on a named pipe; only the CLI can reach it.
        if conn.is_windows() {
            let o = crate::win::run(&conn, "if (Get-Command docker -ErrorAction SilentlyContinue) { 'cli' }")
                .await
                .map_err(DockerError::other)?;
            if o.stdout.trim() != "cli" {
                return Err(DockerError::NotInstalled);
            }
            return Self::with_route(conn, Route::WindowsCli).await;
        }
        let (_, out, _) = conn.exec(PROBE).await.map_err(DockerError::other)?;
        let probe = parse_probe(&String::from_utf8_lossy(&out));
        match probe.status.as_str() {
            "denied" => Err(DockerError::PermissionDenied { socket: probe.socket, user: probe.user }),
            "ok" => {
                let route = Route::Socket(probe.socket.clone());
                match Self::with_route(conn.clone(), route).await {
                    Ok(d) => return Ok(d),
                    Err(e) if probe.cli => {
                        tracing::info!("docker socket forwarding failed ({e}); trying dial-stdio");
                    }
                    Err(e) => return Err(e),
                }
                Self::with_route(conn, Route::DialStdio(Some(probe.socket))).await
            }
            // No local socket: the CLI may still reach a daemon (contexts,
            // DOCKER_HOST in the login environment).
            _ if probe.cli => Self::with_route(conn, Route::DialStdio(None)).await,
            _ => Err(DockerError::NotInstalled),
        }
    }

    /// Connects over a fixed route and checks the daemon answers.
    pub async fn with_route(conn: Arc<Connection>, route: Route) -> DockerResult<Self> {
        let docker = transport::client(conn.clone(), route.clone(), API_TIMEOUT).map_err(DockerError::other)?;
        docker.ping().await.map_err(|e| DockerError::from_bollard(e, None))?;
        Ok(Self { docker, route, conn })
    }

    pub fn is_closed(&self) -> bool {
        self.conn.is_closed()
    }

    pub async fn info(&self) -> DockerResult<EngineInfo> {
        let v = self.docker.version().await.map_err(|e| DockerError::from_bollard(e, None))?;
        Ok(EngineInfo {
            version: v.version.unwrap_or_default(),
            api_version: v.api_version.unwrap_or_default(),
            os: format!("{}/{}", v.os.unwrap_or_default(), v.arch.unwrap_or_default()),
            route: self.route.describe(),
        })
    }

    /// All containers, running or not.
    pub async fn containers(&self) -> DockerResult<Vec<ContainerView>> {
        let opts = ListContainersOptions { all: true, ..Default::default() };
        let list = self.docker.list_containers(Some(opts)).await.map_err(|e| DockerError::from_bollard(e, None))?;
        let mut out: Vec<ContainerView> = list.into_iter().map(ContainerView::from).collect();
        out.sort_by(|a, b| (a.project.is_none(), &a.project, &a.name).cmp(&(b.project.is_none(), &b.project, &b.name)));
        Ok(out)
    }

    /// CPU and memory for the given (running) containers. Containers that
    /// fail (e.g. stopped meanwhile) are left out.
    pub async fn stats(&self, ids: &[String]) -> Vec<ContainerStats> {
        stream::iter(ids.iter().cloned())
            .map(|id| async move {
                let opts = StatsOptions { stream: false, one_shot: false };
                let mut s = self.docker.stats(&id, Some(opts));
                match s.next().await {
                    Some(Ok(r)) => Some(ContainerStats::from_response(id, &r)),
                    Some(Err(e)) => {
                        tracing::debug!("stats {id}: {e}");
                        None
                    }
                    None => None,
                }
            })
            .buffer_unordered(STATS_PARALLEL)
            .filter_map(|x| async move { x })
            .collect()
            .await
    }

    pub async fn act(&self, id: &str, action: Action) -> DockerResult<()> {
        let r = match action {
            Action::Start => self.docker.start_container(id, None::<StartContainerOptions>).await,
            Action::Stop => {
                // Docker waits the stop grace period (default 10 s) before
                // SIGKILL; give the request room for that.
                let d = transport::client(self.conn.clone(), self.route.clone(), 120).map_err(DockerError::other)?;
                d.stop_container(id, None::<StopContainerOptions>).await
            }
            Action::Restart => {
                let d = transport::client(self.conn.clone(), self.route.clone(), 120).map_err(DockerError::other)?;
                d.restart_container(id, None::<RestartContainerOptions>).await
            }
            Action::Kill => self.docker.kill_container(id, None::<KillContainerOptions>).await,
            Action::Pause => self.docker.pause_container(id).await,
            Action::Unpause => self.docker.unpause_container(id).await,
            Action::Remove { force } => {
                let opts = RemoveContainerOptions { force, ..Default::default() };
                self.docker.remove_container(id, Some(opts)).await
            }
        };
        match r {
            Ok(()) => Ok(()),
            // Start on a running / stop on a stopped container: already there.
            Err(BollardError::DockerResponseServerError { status_code: 304, .. }) => Ok(()),
            Err(e) => Err(DockerError::from_bollard(e, Some(id))),
        }
    }

    /// Container events between `since` and `until` (unix seconds), as far
    /// back as the daemon still remembers them. Used by the host timeline.
    pub async fn events(&self, since: i64, until: i64) -> DockerResult<Vec<crate::timeline::DockerEvent>> {
        let filters = HashMap::from([
            ("type".to_owned(), vec!["container".to_owned()]),
            (
                "event".to_owned(),
                ["die", "start", "restart", "oom", "health_status"].iter().map(|s| (*s).to_owned()).collect(),
            ),
        ]);
        let opts =
            EventsOptions { since: Some(since.to_string()), until: Some(until.to_string()), filters: Some(filters) };
        let mut s = self.docker.events(Some(opts));
        let mut out = Vec::new();
        while let Some(m) = s.next().await {
            let m = m.map_err(|e| DockerError::from_bollard(e, None))?;
            let attrs = m.actor.and_then(|a| a.attributes).unwrap_or_default();
            let Some(name) = attrs.get("name").cloned() else { continue };
            out.push(crate::timeline::DockerEvent {
                at: m.time.unwrap_or(0),
                action: m.action.unwrap_or_default(),
                name,
                exit_code: attrs.get("exitCode").and_then(|c| c.parse().ok()),
            });
            if out.len() >= 5_000 {
                break;
            }
        }
        Ok(out)
    }

    pub async fn inspect(&self, id: &str) -> DockerResult<ContainerDetail> {
        let r = self
            .docker
            .inspect_container(id, None::<InspectContainerOptions>)
            .await
            .map_err(|e| DockerError::from_bollard(e, Some(id)))?;
        Ok(ContainerDetail::from_inspect(r))
    }

    /// Streams log lines in batches to `sink` until the stream ends (or
    /// `follow` is off and the backlog is sent). `sink` returns false to stop.
    pub async fn logs(&self, id: &str, opts: LogQuery, mut sink: impl FnMut(Vec<LogLine>) -> bool) -> DockerResult<()> {
        let o = LogsOptions {
            follow: opts.follow,
            stdout: true,
            stderr: true,
            timestamps: true,
            tail: opts.tail.map(|n| n.to_string()).unwrap_or_else(|| "all".into()),
            since: opts.since.unwrap_or(0) as i32,
            ..Default::default()
        };
        // Following can idle for hours: no request timeout once headers arrive
        // (bollard's timeout only covers the response head).
        let mut s = self.docker.logs(id, Some(o));
        let mut asm = LineAssembler::default();
        let mut batch = Vec::new();
        let mut deadline: Option<tokio::time::Instant> = None;
        loop {
            let next = match deadline {
                Some(d) => match tokio::time::timeout_at(d, s.next()).await {
                    Ok(x) => x,
                    Err(_) => {
                        deadline = None;
                        if !sink(std::mem::take(&mut batch)) {
                            return Ok(());
                        }
                        continue;
                    }
                },
                None => s.next().await,
            };
            match next {
                None => break,
                Some(Err(e)) => {
                    asm.finish(&mut batch);
                    if !batch.is_empty() {
                        sink(batch);
                    }
                    return Err(DockerError::from_bollard(e, Some(id)));
                }
                Some(Ok(out)) => {
                    asm.push(out, &mut batch);
                    if batch.len() >= 1000 {
                        deadline = None;
                        if !sink(std::mem::take(&mut batch)) {
                            return Ok(());
                        }
                    } else if deadline.is_none() && !batch.is_empty() {
                        deadline = Some(tokio::time::Instant::now() + Duration::from_millis(80));
                    }
                }
            }
        }
        asm.finish(&mut batch);
        if !batch.is_empty() {
            sink(batch);
        }
        Ok(())
    }
}

/// Command for an interactive shell in a container (run in a PTY channel).
/// Prefers bash, falls back to sh. The docker CLI handles the TTY.
pub fn exec_shell_command(container: &str, route: &Route) -> String {
    let host = match route {
        Route::Socket(s) | Route::DialStdio(Some(s)) if s != "/var/run/docker.sock" => {
            format!("DOCKER_HOST={} ", shell_quote(&format!("unix://{s}")))
        }
        _ => String::new(),
    };
    if *route == Route::WindowsCli {
        // From cmd or PowerShell: no quoting tricks. Ids are checked by
        // `valid_container_ref` before they get here.
        return format!("docker exec -it {container} sh");
    }
    format!(
        "{host}docker exec -it {} sh -c 'command -v bash >/dev/null 2>&1 && exec bash || exec sh'",
        shell_quote(container)
    )
}

/// Container ids and names only use these characters; refuse anything else
/// before it goes near a shell or URL.
pub fn valid_container_ref(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
        && !s.starts_with(['-', '.'])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Action {
    Start,
    Stop,
    Restart,
    Kill,
    Pause,
    Unpause,
    Remove { force: bool },
}

impl Action {
    /// Needs a confirmation naming the targets (security rule 5).
    pub fn is_destructive(self) -> bool {
        matches!(self, Action::Stop | Action::Restart | Action::Kill | Action::Remove { .. })
    }

    pub fn verb(self) -> &'static str {
        match self {
            Action::Start => "start",
            Action::Stop => "stop",
            Action::Restart => "restart",
            Action::Kill => "kill",
            Action::Pause => "pause",
            Action::Unpause => "unpause",
            Action::Remove { .. } => "remove",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PortView {
    pub ip: Option<String>,
    pub private: u16,
    pub public: Option<u16>,
    pub proto: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ContainerView {
    pub id: String,
    pub name: String,
    pub image: String,
    /// created, running, paused, restarting, exited, removing, dead
    pub state: String,
    /// Docker's own status line, e.g. "Up 3 hours (healthy)".
    pub status: String,
    /// healthy, unhealthy, starting; None when there is no healthcheck.
    pub health: Option<String>,
    /// Unix seconds.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub created: i64,
    pub ports: Vec<PortView>,
    pub project: Option<String>,
    pub service: Option<String>,
    pub compose_dir: Option<String>,
    /// Compose files the project was started with (absolute paths).
    pub compose_files: Vec<String>,
    /// Published ports that look like a web UI, best first (D-147).
    pub web: Vec<crate::webui::WebUiTarget>,
}

fn health_from_status(status: &str) -> Option<String> {
    ["unhealthy", "healthy", "health: starting"]
        .into_iter()
        .find(|h| status.contains(&format!("({h})")))
        .map(|h| h.trim_start_matches("health: ").to_owned())
}

impl From<ContainerSummary> for ContainerView {
    fn from(c: ContainerSummary) -> Self {
        let labels = c.labels.unwrap_or_default();
        let status = c.status.unwrap_or_default();
        let health = c
            .health
            .and_then(|h| h.status)
            .map(|s| s.to_string())
            .filter(|s| matches!(s.as_str(), "healthy" | "unhealthy" | "starting"))
            .or_else(|| health_from_status(&status));
        let mut ports: Vec<PortView> = c
            .ports
            .unwrap_or_default()
            .into_iter()
            .map(|p| PortView {
                ip: p.ip.filter(|s| !s.is_empty()),
                private: p.private_port,
                public: p.public_port,
                proto: p.typ.map(|t| t.to_string()).unwrap_or_else(|| "tcp".into()),
            })
            .collect();
        ports.sort_by(|a, b| (a.private, &a.proto, &a.ip).cmp(&(b.private, &b.proto, &b.ip)));
        // IPv4 and IPv6 bindings of the same port read as one.
        ports.dedup_by(|a, b| a.private == b.private && a.public == b.public && a.proto == b.proto);
        let name = c
            .names
            .and_then(|n| n.into_iter().next())
            .map(|n| n.trim_start_matches('/').to_owned())
            .unwrap_or_default();
        let web = crate::webui::targets(&ports, labels.get(crate::webui::LABEL_WEBUI).map(String::as_str));
        ContainerView {
            web,
            id: c.id.unwrap_or_default(),
            name,
            image: c.image.unwrap_or_default(),
            state: c.state.map(|s| s.to_string()).unwrap_or_default(),
            status,
            health,
            created: c.created.unwrap_or(0),
            ports,
            project: labels.get(LABEL_PROJECT).cloned(),
            service: labels.get(LABEL_SERVICE).cloned(),
            compose_dir: labels.get(LABEL_WORKING_DIR).cloned(),
            compose_files: labels
                .get(LABEL_CONFIG_FILES)
                .map(|f| f.split(',').map(str::trim).filter(|f| !f.is_empty()).map(str::to_owned).collect())
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ContainerStats {
    pub id: String,
    /// CPU in tenths of a percent of one core (1000 = one full core).
    pub cpu_permille: u32,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub mem_used: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub mem_limit: u64,
}

impl ContainerStats {
    pub fn from_response(id: String, r: &ContainerStatsResponse) -> Self {
        let cpu = |s: &Option<bollard::models::ContainerCpuStats>| {
            let s = s.as_ref();
            (
                s.and_then(|s| s.cpu_usage.as_ref()).and_then(|u| u.total_usage).unwrap_or(0),
                s.and_then(|s| s.system_cpu_usage).unwrap_or(0),
                s.and_then(|s| s.online_cpus).unwrap_or(0),
            )
        };
        let (total, system, online) = cpu(&r.cpu_stats);
        let (pre_total, pre_system, _) = cpu(&r.precpu_stats);
        let cpu_delta = total.saturating_sub(pre_total) as f64;
        let sys_delta = system.saturating_sub(pre_system) as f64;
        let cpus = if online > 0 { online as f64 } else { 1.0 };
        let cpu_permille =
            if sys_delta > 0.0 && pre_total > 0 { (cpu_delta / sys_delta * cpus * 1000.0).round() as u32 } else { 0 };
        let mem = r.memory_stats.as_ref();
        let usage = mem.and_then(|m| m.usage).unwrap_or(0);
        // Like `docker stats`: page cache does not count as used.
        let cache = mem
            .and_then(|m| m.stats.as_ref())
            .and_then(|s| s.get("inactive_file").or_else(|| s.get("total_inactive_file")).copied())
            .unwrap_or(0);
        ContainerStats {
            id,
            cpu_permille,
            mem_used: usage.saturating_sub(cache),
            mem_limit: mem.and_then(|m| m.limit).unwrap_or(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct KeyValue {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MountView {
    pub kind: String,
    pub source: String,
    pub destination: String,
    pub rw: bool,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NetworkView {
    pub name: String,
    pub ip: String,
    pub gateway: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ContainerDetail {
    pub id: String,
    pub name: String,
    pub image: String,
    pub created: String,
    pub state: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub exit_code: Option<i32>,
    pub health: Option<String>,
    pub restart_policy: String,
    pub restart_count: u32,
    pub command: String,
    /// Shown masked by default in the UI (security rule 6).
    pub env: Vec<KeyValue>,
    pub mounts: Vec<MountView>,
    pub networks: Vec<NetworkView>,
    pub ports: Vec<PortView>,
    pub labels: Vec<KeyValue>,
    /// Full inspect JSON, pretty printed, with env values masked.
    pub raw: String,
}

fn nonzero_time(t: Option<String>) -> Option<String> {
    t.filter(|t| !t.is_empty() && !t.starts_with("0001-"))
}

impl ContainerDetail {
    pub fn from_inspect(r: ContainerInspectResponse) -> Self {
        let raw = masked_raw(&r);
        let config = r.config.clone().unwrap_or_default();
        let state = r.state.clone().unwrap_or_default();
        let host = r.host_config.clone().unwrap_or_default();
        let env = config
            .env
            .unwrap_or_default()
            .into_iter()
            .map(|e| match e.split_once('=') {
                Some((k, v)) => KeyValue { key: k.to_owned(), value: v.to_owned() },
                None => KeyValue { key: e, value: String::new() },
            })
            .collect();
        let mut labels: Vec<KeyValue> =
            config.labels.unwrap_or_default().into_iter().map(|(key, value)| KeyValue { key, value }).collect();
        labels.sort_by(|a, b| a.key.cmp(&b.key));
        let mounts = r
            .mounts
            .unwrap_or_default()
            .into_iter()
            .map(|m| MountView {
                kind: m.typ.map(|t| t.to_string()).unwrap_or_default(),
                source: m.source.unwrap_or_default(),
                destination: m.destination.unwrap_or_default(),
                rw: m.rw.unwrap_or(true),
                name: m.name.filter(|n| !n.is_empty()),
            })
            .collect();
        let settings = r.network_settings.unwrap_or_default();
        let mut networks: Vec<NetworkView> = settings
            .networks
            .unwrap_or_default()
            .into_iter()
            .map(|(name, n)| NetworkView {
                name,
                ip: n.ip_address.unwrap_or_default(),
                gateway: n.gateway.unwrap_or_default(),
                aliases: n.aliases.unwrap_or_default(),
            })
            .collect();
        networks.sort_by(|a, b| a.name.cmp(&b.name));
        let mut ports = Vec::new();
        for (key, binds) in settings.ports.unwrap_or_default() {
            let (port, proto) = key.split_once('/').unwrap_or((key.as_str(), "tcp"));
            let private = port.parse().unwrap_or(0);
            match binds {
                Some(b) if !b.is_empty() => {
                    for b in b {
                        ports.push(PortView {
                            ip: b.host_ip.filter(|s| !s.is_empty()),
                            private,
                            public: b.host_port.and_then(|p| p.parse().ok()),
                            proto: proto.to_owned(),
                        })
                    }
                }
                _ => ports.push(PortView { ip: None, private, public: None, proto: proto.to_owned() }),
            }
        }
        ports.sort_by(|a, b| (a.private, &a.proto, &a.ip).cmp(&(b.private, &b.proto, &b.ip)));
        let policy = host.restart_policy.unwrap_or_default();
        let mut restart_policy =
            policy.name.map(|n| n.to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| "no".into());
        if restart_policy == "on-failure" {
            if let Some(n) = policy.maximum_retry_count.filter(|n| *n > 0) {
                restart_policy = format!("on-failure:{n}");
            }
        }
        let mut command = vec![r.path.unwrap_or_default()];
        command.extend(r.args.unwrap_or_default());
        ContainerDetail {
            id: r.id.unwrap_or_default(),
            name: r.name.unwrap_or_default().trim_start_matches('/').to_owned(),
            image: config.image.unwrap_or_default(),
            created: r.created.map(|c| c.to_string()).unwrap_or_default(),
            state: state.status.map(|s| s.to_string()).unwrap_or_default(),
            started_at: nonzero_time(state.started_at),
            finished_at: nonzero_time(state.finished_at),
            exit_code: state.exit_code.map(|c| c as i32),
            health: state
                .health
                .and_then(|h| h.status)
                .map(|s| s.to_string())
                .filter(|s| matches!(s.as_str(), "healthy" | "unhealthy" | "starting")),
            restart_policy,
            restart_count: r.restart_count.unwrap_or(0).max(0) as u32,
            command: command.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" "),
            env,
            mounts,
            networks,
            ports,
            labels,
            raw,
        }
    }
}

/// Inspect JSON with every `Config.Env` value replaced by [`MASK`].
fn masked_raw(r: &ContainerInspectResponse) -> String {
    let mut v = serde_json::to_value(r).unwrap_or_default();
    if let Some(env) = v.pointer_mut("/Config/Env").and_then(|e| e.as_array_mut()) {
        for e in env.iter_mut() {
            if let Some(s) = e.as_str() {
                let key = s.split_once('=').map(|(k, _)| k).unwrap_or(s);
                *e = serde_json::Value::String(format!("{key}={MASK}"));
            }
        }
    }
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LogQuery {
    pub follow: bool,
    /// Last N lines; None = everything.
    pub tail: Option<u32>,
    /// Unix seconds.
    pub since: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum LogStream {
    Out,
    Err,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub stream: LogStream,
    /// RFC 3339 timestamp from the daemon.
    pub ts: Option<String>,
    pub text: String,
}

/// Turns daemon log frames into whole lines (frames can split or join lines).
#[derive(Default)]
struct LineAssembler {
    partial: [Vec<u8>; 2],
}

impl LineAssembler {
    fn push(&mut self, out: LogOutput, batch: &mut Vec<LogLine>) {
        let (stream, bytes) = match out {
            LogOutput::StdErr { message } => (LogStream::Err, message),
            LogOutput::StdOut { message } | LogOutput::Console { message } => (LogStream::Out, message),
            LogOutput::StdIn { .. } => return,
        };
        let buf = &mut self.partial[stream as usize];
        buf.extend_from_slice(&bytes);
        while let Some(i) = buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buf.drain(..=i).collect();
            batch.push(parse_line(stream, &line[..line.len() - 1]));
        }
    }

    fn finish(&mut self, batch: &mut Vec<LogLine>) {
        for (i, stream) in [LogStream::Out, LogStream::Err].into_iter().enumerate() {
            let buf = std::mem::take(&mut self.partial[i]);
            if !buf.is_empty() {
                batch.push(parse_line(stream, &buf));
            }
        }
    }
}

fn parse_line(stream: LogStream, raw: &[u8]) -> LogLine {
    let s = String::from_utf8_lossy(raw);
    let s = s.strip_suffix('\r').unwrap_or(&s);
    // With timestamps on, every line starts "2026-11-14T20:07:30.123456789Z ".
    let (ts, text) = match s.split_once(' ') {
        Some((t, rest)) if t.len() >= 20 && t.as_bytes()[4] == b'-' && t.contains('T') => (Some(t.to_owned()), rest),
        _ => (None, s),
    };
    LogLine { stream, ts, text: strip_ansi(text) }
}

/// Removes ANSI escape sequences (colours, cursor moves) from a log line.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\x1b' {
            out.push(c);
            continue;
        }
        match it.next() {
            // CSI: ESC [ params final-byte(@..~)
            Some('[') => {
                for c in it.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            // OSC: ESC ] ... (BEL | ESC \)
            Some(']') => {
                while let Some(c) = it.next() {
                    if c == '\x07' {
                        break;
                    }
                    if c == '\x1b' && it.peek() == Some(&'\\') {
                        it.next();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// One Docker client per host, on that host's pooled SSH connection.
#[derive(Default)]
pub struct DockerCache {
    hosts: Mutex<HashMap<HostId, Arc<DockerHost>>>,
    opening: tokio::sync::Mutex<()>,
}

impl DockerCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get(&self, id: HostId, conn: Arc<Connection>) -> DockerResult<Arc<DockerHost>> {
        let cached = |s: &Self| {
            s.hosts
                .lock()
                .expect("poisoned")
                .get(&id)
                .filter(|d| Arc::ptr_eq(&d.conn, &conn) && !d.is_closed())
                .cloned()
        };
        if let Some(d) = cached(self) {
            return Ok(d);
        }
        let _guard = self.opening.lock().await;
        if let Some(d) = cached(self) {
            return Ok(d);
        }
        let d = Arc::new(DockerHost::connect(conn).await?);
        self.hosts.lock().expect("poisoned").insert(id, d.clone());
        Ok(d)
    }

    pub fn drop_host(&self, id: HostId) {
        self.hosts.lock().expect("poisoned").remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bollard::models::{ContainerCpuStats, ContainerCpuUsage, ContainerMemoryStats, PortSummary};

    #[test]
    fn probe_parsing() {
        let p = parse_probe("ok /var/run/docker.sock\nroben\ncli\n");
        assert_eq!(p.status, "ok");
        assert_eq!(p.socket, "/var/run/docker.sock");
        assert_eq!(p.user, "roben");
        assert!(p.cli);
        let p = parse_probe("missing \nroben\nnocli\n");
        assert_eq!(p.status, "missing");
        assert!(!p.cli);
        assert_eq!(parse_probe("").status, "missing");
    }

    #[test]
    fn summary_maps_compose_labels_ports_and_health() {
        let labels = HashMap::from([
            (LABEL_PROJECT.to_owned(), "media".to_owned()),
            (LABEL_SERVICE.to_owned(), "web".to_owned()),
            (LABEL_WORKING_DIR.to_owned(), "/srv/media".to_owned()),
        ]);
        let port = |ip: &str, public| PortSummary {
            ip: Some(ip.into()),
            private_port: 80,
            public_port: Some(public),
            typ: Some(bollard::models::PortSummaryTypeEnum::TCP),
        };
        let c = ContainerSummary {
            id: Some("abc".into()),
            names: Some(vec!["/media-web-1".into()]),
            image: Some("nginx:1".into()),
            status: Some("Up 3 hours (healthy)".into()),
            state: Some(bollard::models::ContainerSummaryStateEnum::RUNNING),
            labels: Some(labels),
            ports: Some(vec![port("0.0.0.0", 8080), port("::", 8080)]),
            ..Default::default()
        };
        let v = ContainerView::from(c);
        assert_eq!(v.name, "media-web-1");
        assert_eq!(v.state, "running");
        assert_eq!(v.health.as_deref(), Some("healthy"));
        assert_eq!(v.project.as_deref(), Some("media"));
        assert_eq!(v.service.as_deref(), Some("web"));
        assert_eq!(v.compose_dir.as_deref(), Some("/srv/media"));
        assert_eq!(v.ports.len(), 1, "v4/v6 duplicates folded: {:?}", v.ports);
        assert_eq!(v.ports[0].public, Some(8080));
    }

    #[test]
    fn health_from_status_text() {
        assert_eq!(health_from_status("Up 2 minutes (health: starting)").as_deref(), Some("starting"));
        assert_eq!(health_from_status("Up 1 hour (unhealthy)").as_deref(), Some("unhealthy"));
        assert_eq!(health_from_status("Exited (0) 5 minutes ago"), None);
    }

    #[test]
    fn stats_cpu_and_memory_like_docker_stats() {
        let cpu = |total, system| {
            Some(ContainerCpuStats {
                cpu_usage: Some(ContainerCpuUsage { total_usage: Some(total), ..Default::default() }),
                system_cpu_usage: Some(system),
                online_cpus: Some(4),
                ..Default::default()
            })
        };
        let r = ContainerStatsResponse {
            cpu_stats: cpu(1_500, 10_000),
            precpu_stats: cpu(1_000, 8_000),
            memory_stats: Some(ContainerMemoryStats {
                usage: Some(100 << 20),
                limit: Some(1 << 30),
                stats: Some(HashMap::from([("inactive_file".to_owned(), 20 << 20)])),
                ..Default::default()
            }),
            ..Default::default()
        };
        let s = ContainerStats::from_response("x".into(), &r);
        // 500 / 2000 * 4 cores = 100% of one core.
        assert_eq!(s.cpu_permille, 1000);
        assert_eq!(s.mem_used, 80 << 20);
        assert_eq!(s.mem_limit, 1 << 30);
        // No previous sample yet: 0, not garbage.
        let r = ContainerStatsResponse { cpu_stats: cpu(1_500, 10_000), ..Default::default() };
        assert_eq!(ContainerStats::from_response("x".into(), &r).cpu_permille, 0);
    }

    #[test]
    fn log_lines_reassembled_with_timestamps() {
        let mut asm = LineAssembler::default();
        let mut batch = Vec::new();
        let b = |s: &str| bytes::Bytes::from(s.to_owned());
        asm.push(LogOutput::StdOut { message: b("2026-11-14T20:07:30.123456789Z hel") }, &mut batch);
        assert!(batch.is_empty());
        asm.push(
            LogOutput::StdOut { message: b("lo\n2026-11-14T20:07:31.000000000Z \x1b[32mok\x1b[0m\r\n") },
            &mut batch,
        );
        asm.push(LogOutput::StdErr { message: b("no timestamp here\n") }, &mut batch);
        asm.push(LogOutput::StdOut { message: b("2026-11-14T20:07:32Z tail") }, &mut batch);
        asm.finish(&mut batch);
        let texts: Vec<_> = batch.iter().map(|l| (l.stream, l.ts.as_deref(), l.text.as_str())).collect();
        assert_eq!(
            texts,
            vec![
                (LogStream::Out, Some("2026-11-14T20:07:30.123456789Z"), "hello"),
                (LogStream::Out, Some("2026-11-14T20:07:31.000000000Z"), "ok"),
                (LogStream::Err, None, "no timestamp here"),
                (LogStream::Out, Some("2026-11-14T20:07:32Z"), "tail"),
            ]
        );
    }

    #[test]
    fn ansi_is_stripped() {
        assert_eq!(strip_ansi("\x1b[1;31mred\x1b[0m plain"), "red plain");
        assert_eq!(strip_ansi("\x1b]0;title\x07after"), "after");
        assert_eq!(strip_ansi("no escapes"), "no escapes");
    }

    #[test]
    fn container_refs_are_checked() {
        assert!(valid_container_ref("media-web-1"));
        assert!(valid_container_ref("0123abcdef"));
        assert!(!valid_container_ref("-rf"));
        assert!(!valid_container_ref("a b"));
        assert!(!valid_container_ref("x;reboot"));
        assert!(!valid_container_ref(""));
    }

    #[test]
    fn exec_command_quotes_and_picks_socket() {
        let def = Route::Socket("/var/run/docker.sock".into());
        assert_eq!(
            exec_shell_command("web", &def),
            "docker exec -it 'web' sh -c 'command -v bash >/dev/null 2>&1 && exec bash || exec sh'"
        );
        let rootless = Route::Socket("/run/user/1000/docker.sock".into());
        assert!(exec_shell_command("web", &rootless)
            .starts_with("DOCKER_HOST='unix:///run/user/1000/docker.sock' docker exec"));
    }

    #[test]
    fn raw_inspect_masks_env() {
        let r = ContainerInspectResponse {
            config: Some(bollard::models::ContainerConfig {
                env: Some(vec!["API_TOKEN=s3cret".into(), "TZ=UTC".into()]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let d = ContainerDetail::from_inspect(r);
        assert!(!d.raw.contains("s3cret"));
        assert!(d.raw.contains(&format!("API_TOKEN={MASK}")));
        assert_eq!(d.env[0], KeyValue { key: "API_TOKEN".into(), value: "s3cret".into() });
        assert_eq!(d.restart_policy, "no");
    }

    #[test]
    fn destructive_actions() {
        assert!(Action::Stop.is_destructive());
        assert!(Action::Remove { force: false }.is_destructive());
        assert!(!Action::Start.is_destructive());
        assert!(!Action::Pause.is_destructive());
    }
}
