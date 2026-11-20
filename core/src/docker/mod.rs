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
pub use transport::{valid_docker_user, Route, PATH_FIX};

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
    PermissionDenied {
        socket: String,
        user: String,
        /// Synology DSM: no usermod/groupadd there; groups go through
        /// `synogroup`, and the socket starts out root:root.
        synology: bool,
        /// Group that owns the socket ("" if unknown).
        socket_group: String,
        /// Commands that fix it, each to run as root, in order (see
        /// [`access_steps`]). Empty when only a new login is missing.
        steps: Vec<String>,
        /// The group change needs a new SSH login to apply.
        relogin: bool,
    },
    /// The host has a Docker user set, and the SSH login may not run
    /// docker as that user without a password.
    #[error(
        "{login} can't run docker as {user} without a password: add the sudoers rule, or clear the host's Docker user"
    )]
    DockerUserSudo {
        /// The SSH login.
        login: String,
        /// The host's Docker user.
        user: String,
        synology: bool,
        /// Root commands that add the rule (see [`sudoers_step`]).
        steps: Vec<String>,
    },
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

/// Shell probe run once per connection to find the daemon socket. Prints
/// `<ok|denied|missing> <socket>`, the user name, `cli <path>|nocli`,
/// `dsm|linux`, and, when the socket is denied, what [`access_steps`] needs.
const PROBE: &str = r#"PATH="$PATH:/usr/local/bin:/usr/local/sbin"; export PATH
s=""; st=missing
for c in /var/run/docker.sock "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/docker.sock"; do
  [ -S "$c" ] || continue
  if [ -r "$c" ] && [ -w "$c" ]; then s=$c; st=ok; break; fi
  [ -n "$s" ] || { s=$c; st=denied; }
done
u=$(id -un); echo "$st $s"; echo "$u"
d=$(command -v docker 2>/dev/null); if [ -n "$d" ]; then echo "cli $d"; else echo nocli; fi
if [ -f /etc.defaults/synoinfo.conf ] || [ -x /usr/syno/sbin/synogroup ]; then echo dsm; else echo linux; fi
if [ "$st" = denied ]; then
  if grep -q '^docker:' /etc/group 2>/dev/null; then
    grep '^docker:' /etc/group | cut -d: -f4 | tr ',' '\n' | grep -qx "$u" && echo "group member" || echo group
  else echo nogroup; fi
  id -Gn 2>/dev/null | tr ' ' '\n' | grep -qx docker && echo session
  ls -l "$s" 2>/dev/null | awk '{print "sockgroup " $4}'
  [ -n "$d" ] && sudo -n "$d" version >/dev/null 2>&1 && echo sudo
fi"#;

/// Probe for a host with a Docker user (`__USER__` is replaced by a
/// [`valid_docker_user`] name). Prints `nouser`, or the login, the docker
/// CLI, `dsm|linux`, the docker group state for that user, the socket's
/// group, the docker-compose path, and how `sudo -u` went (`rc N` + output).
const USER_PROBE: &str = r#"PATH="$PATH:/usr/local/bin:/usr/local/sbin"; export PATH
u=__USER__
id -u "$u" >/dev/null 2>&1 || { echo nouser; exit 0; }
id -un
d=$(command -v docker 2>/dev/null); if [ -n "$d" ]; then echo "cli $d"; else echo nocli; fi
if [ -f /etc.defaults/synoinfo.conf ] || [ -x /usr/syno/sbin/synogroup ]; then echo dsm; else echo linux; fi
if id -Gn "$u" 2>/dev/null | tr ' ' '\n' | grep -qx docker; then echo "group member"
elif grep -q '^docker:' /etc/group 2>/dev/null; then echo group; else echo nogroup; fi
for c in /var/run/docker.sock /run/docker.sock; do
  [ -S "$c" ] && { echo "sock $c"; ls -lL "$c" 2>/dev/null | awk '{print "sockgroup " $4}'; break; }
done
c=$(command -v docker-compose 2>/dev/null); [ -n "$c" ] && echo "compose $c"
if [ -n "$d" ]; then
  o=$(sudo -n -u "$u" "$d" version --format '{{.Server.Version}}' 2>&1 </dev/null); echo "rc $?"
  printf '%s\n' "$o" | head -n 5 | sed 's/^/out /'
fi"#;

/// What [`USER_PROBE`] found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct UserProbe {
    no_user: bool,
    login: String,
    cli_path: Option<String>,
    synology: bool,
    group_exists: bool,
    in_group: bool,
    socket: String,
    socket_group: String,
    compose_path: Option<String>,
    rc: Option<i32>,
    out: String,
}

fn parse_user_probe(out: &str) -> UserProbe {
    let mut p = UserProbe::default();
    let mut lines = out.lines().map(str::trim_end);
    match lines.next().map(str::trim) {
        Some("nouser") | None => {
            p.no_user = true;
            return p;
        }
        Some(l) => p.login = l.to_owned(),
    }
    let mut out_lines = Vec::new();
    for l in lines {
        if let Some(o) = l.strip_prefix("out ") {
            out_lines.push(o.trim());
            continue;
        }
        let l = l.trim();
        match l {
            "dsm" => p.synology = true,
            "group" => p.group_exists = true,
            "group member" => (p.group_exists, p.in_group) = (true, true),
            _ => {
                if let Some(v) = l.strip_prefix("cli ") {
                    p.cli_path = Some(v.trim().to_owned()).filter(|v| !v.is_empty());
                } else if let Some(v) = l.strip_prefix("sock ") {
                    p.socket = v.trim().to_owned();
                } else if let Some(v) = l.strip_prefix("sockgroup ") {
                    p.socket_group = v.trim().to_owned();
                } else if let Some(v) = l.strip_prefix("compose ") {
                    p.compose_path = Some(v.trim().to_owned()).filter(|v| transport::safe_bin(v));
                } else if let Some(v) = l.strip_prefix("rc ") {
                    p.rc = v.trim().parse().ok();
                }
            }
        }
    }
    p.out = out_lines.join("\n");
    p
}

/// Prefix and file of the sudoers rule [`sudoers_step`] writes.
const SUDOERS_DIR: &str = "/etc/sudoers.d/nightgrid-docker-";
const SUDOERS_HEAD: &str = "sh -c 'umask 337; echo \"";

/// sudoers ignores files whose names contain a dot.
fn sudoers_file(user: &str) -> String {
    let safe: String =
        user.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    format!("{SUDOERS_DIR}{safe}")
}

/// Root command that lets `login` run the docker CLI (and docker-compose)
/// as `user` without a password: one rule in its own sudoers.d file, mode
/// 0440. Runnable as `sudo <step>` by hand; Nightgrid also checks it with
/// `visudo` (see [`access_fix_script`]).
pub fn sudoers_step(login: &str, user: &str, bins: &[String]) -> String {
    format!("{SUDOERS_HEAD}{login} ALL=({user}) NOPASSWD: {}\" > {}'", bins.join(", "), sudoers_file(user))
}

/// The (login, user, bins) of a step [`sudoers_step`] made, if it is one.
fn parse_sudoers_step(step: &str) -> Option<(String, String, Vec<String>)> {
    let rest = step.strip_prefix(SUDOERS_HEAD)?;
    let (rule, _) = rest.split_once('"')?;
    let (login, rest) = rule.split_once(" ALL=(")?;
    let (user, bins) = rest.split_once(") NOPASSWD: ")?;
    let bins: Vec<String> = bins.split(", ").map(str::to_owned).collect();
    let ok = valid_docker_user(login)
        && valid_docker_user(user)
        && !bins.is_empty()
        && bins.len() <= 2
        && bins.iter().all(|b| transport::safe_bin(b))
        && sudoers_step(login, user, &bins) == step;
    ok.then(|| (login.to_owned(), user.to_owned(), bins))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Probe {
    status: String,
    socket: String,
    user: String,
    cli: bool,
    /// Absolute path of the docker CLI, when the probe printed one.
    cli_path: Option<String>,
    synology: bool,
    group_exists: bool,
    in_group: bool,
    session_group: bool,
    socket_group: String,
    sudo: bool,
}

fn parse_probe(out: &str) -> Probe {
    let mut lines = out.lines();
    let (status, socket) = lines.next().unwrap_or("missing").split_once(' ').unwrap_or(("missing", ""));
    let mut p = Probe {
        status: status.to_owned(),
        socket: socket.trim().to_owned(),
        user: lines.next().unwrap_or("").trim().to_owned(),
        ..Default::default()
    };
    if let Some(l) = lines.next().map(str::trim) {
        if l == "cli" || l.starts_with("cli ") {
            p.cli = true;
            p.cli_path = l.strip_prefix("cli ").map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
        }
    }
    for l in lines.map(str::trim) {
        match l {
            "dsm" => p.synology = true,
            "group" => p.group_exists = true,
            "group member" => (p.group_exists, p.in_group) = (true, true),
            "session" => p.session_group = true,
            "sudo" => p.sudo = true,
            _ => {
                if let Some(g) = l.strip_prefix("sockgroup ") {
                    p.socket_group = g.trim().to_owned();
                }
            }
        }
    }
    p
}

/// A user or path printed in a command: plain when it needs no quoting.
fn arg(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "/._-@+".contains(c)) {
        s.to_owned()
    } else {
        shell_quote(s)
    }
}

/// Root commands that give `user` the Docker socket, skipping what's already
/// in place. Synology DSM has no usermod/groupadd (`synogroup` manages
/// groups there) and ships the socket as root:root, so it also needs a chown.
fn access_steps(p: &Probe) -> Vec<String> {
    let (u, sock) = (arg(&p.user), arg(&p.socket));
    let mut steps = Vec::new();
    if p.synology {
        if !p.group_exists {
            steps.push(format!("synogroup --add docker {u}"));
        } else if !p.in_group {
            steps.push(format!("synogroup --memberadd docker {u}"));
        }
    } else {
        if !p.group_exists {
            steps.push("groupadd docker".to_owned());
        }
        if !p.in_group {
            steps.push(format!("usermod -aG docker {u}"));
        }
    }
    // The system socket only: a rootless daemon's socket belongs to its user.
    let system_socket = p.socket == "/var/run/docker.sock" || p.socket == "/run/docker.sock";
    if system_socket && p.socket_group != "docker" {
        steps.push(format!("chown root:docker {sock}"));
    }
    steps
}

/// Why `sudo -u <user> docker` failed, as the error the card explains.
fn user_failure(p: &UserProbe, user: &str, bin: String) -> DockerError {
    let low = p.out.to_lowercase();
    let sudo_refused =
        ["password is required", "terminal is required", "not allowed", "may not run", "not in the sudoers"]
            .iter()
            .any(|m| low.contains(m));
    if sudo_refused {
        let mut bins = vec![bin];
        bins.extend(p.compose_path.clone());
        let steps = if valid_docker_user(&p.login) { vec![sudoers_step(&p.login, user, &bins)] } else { Vec::new() };
        return DockerError::DockerUserSudo {
            login: p.login.clone(),
            user: user.to_owned(),
            synology: p.synology,
            steps,
        };
    }
    if low.contains("permission denied") && !p.socket.is_empty() {
        // The Docker user can't open the socket: same fix as for a login,
        // for that user. sudo starts it with fresh groups, so no relogin.
        let probe = Probe {
            status: "denied".into(),
            socket: p.socket.clone(),
            user: user.to_owned(),
            cli: true,
            cli_path: None,
            synology: p.synology,
            group_exists: p.group_exists,
            in_group: p.in_group,
            session_group: true,
            socket_group: p.socket_group.clone(),
            sudo: false,
        };
        return permission_denied(&probe);
    }
    let why = if p.out.is_empty() { "no output".to_owned() } else { p.out.clone() };
    DockerError::other(format!("docker as {user} failed: {why}"))
}

fn permission_denied(p: &Probe) -> DockerError {
    DockerError::PermissionDenied {
        socket: p.socket.clone(),
        user: p.user.clone(),
        synology: p.synology,
        socket_group: p.socket_group.clone(),
        steps: access_steps(p),
        relogin: !p.session_group,
    }
}

/// The steps from a permission error as one root script (`set -e`: stops
/// at the first failure). A sudoers rule is only written where sudo reads
/// `/etc/sudoers.d`, and is removed again if `visudo` rejects it.
pub fn access_fix_script(steps: &[String]) -> String {
    // sudo may hand over the login user's thin non-interactive PATH, which
    // lacks DSM's synogroup (/usr/syno/sbin) and sometimes the sbin tools.
    let mut s = String::from(
        "set -e\nPATH=\"$PATH:/usr/syno/sbin:/usr/syno/bin:/usr/local/sbin:/usr/sbin:/sbin\"; export PATH\n",
    );
    for step in steps {
        let rule = parse_sudoers_step(step);
        if rule.is_some() {
            s.push_str(
                "grep -Eq '^[#@]includedir[[:space:]]+/etc/sudoers\\.d' /etc/sudoers || \
                 { echo \"this host's /etc/sudoers doesn't read /etc/sudoers.d; add the rule with visudo instead\" >&2; exit 1; }\n\
                 mkdir -p /etc/sudoers.d\n",
            );
        }
        s.push_str(step);
        s.push('\n');
        if let Some((_, user, _)) = rule {
            let f = sudoers_file(&user);
            s.push_str(&format!(
                "if command -v visudo >/dev/null 2>&1 && ! visudo -cf {f} >/dev/null; then rm -f {f}; \
                 echo 'sudo rejected the rule, nothing was changed' >&2; exit 1; fi\n"
            ));
        }
    }
    s
}

/// Checks that steps sent back from the UI are ones [`access_steps`] can
/// produce, so the fix command never runs arbitrary text as root.
pub fn valid_access_step(step: &str) -> bool {
    let w: Vec<&str> = step.split(' ').collect();
    let name_ok = |s: &str| {
        !s.is_empty() && !s.starts_with('-') && s.chars().all(|c| c.is_ascii_alphanumeric() || "._-@+".contains(c))
    };
    match w.as_slice() {
        ["groupadd", "docker"] => true,
        ["usermod", "-aG", "docker", u] | ["synogroup", "--add" | "--memberadd", "docker", u] => name_ok(u),
        ["chown", "root:docker", p] => *p == "/var/run/docker.sock" || *p == "/run/docker.sock",
        _ => parse_sudoers_step(step).is_some(),
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
    /// The host's Docker user setting this engine was opened with.
    pub docker_user: Option<String>,
}

impl DockerHost {
    /// Finds the daemon and picks a route: socket forwarding first, then
    /// `docker system dial-stdio` if sshd refuses forwarding. With a Docker
    /// user (`docker_user`, POSIX hosts), docker always runs as that user
    /// through `sudo -n -u`.
    pub async fn connect(conn: Arc<Connection>, docker_user: Option<&str>) -> DockerResult<Self> {
        if let Some(u) = docker_user.filter(|_| !conn.is_windows()) {
            return Self::connect_as(conn, u).await;
        }
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
            "denied" => {
                // Passwordless sudo can reach the daemon while the group
                // isn't set up; anything else gets the fix-it card.
                if let Some(bin) = probe.cli_path.as_deref().filter(|b| probe.sudo && transport::safe_bin(b)) {
                    if probe.socket == "/var/run/docker.sock" || probe.socket == "/run/docker.sock" {
                        match Self::with_route(conn.clone(), Route::Sudo(bin.to_owned())).await {
                            Ok(d) => return Ok(d),
                            Err(e) => tracing::info!("docker through sudo failed ({e})"),
                        }
                    }
                }
                Err(permission_denied(&probe))
            }
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

    /// Docker as the host's Docker user: `sudo -n -u <user> docker …` over
    /// the login's connection. When that doesn't work, says why: no such
    /// user, no sudoers rule (with the step that adds it), or the user
    /// itself can't open the socket (the usual NO ACCESS steps, for it).
    async fn connect_as(conn: Arc<Connection>, user: &str) -> DockerResult<Self> {
        if !valid_docker_user(user) {
            return Err(DockerError::other(format!("{user:?} isn't a valid Docker user name")));
        }
        let script = USER_PROBE.replace("__USER__", user);
        let (_, out, _) = conn.exec(&script).await.map_err(DockerError::other)?;
        let p = parse_user_probe(&String::from_utf8_lossy(&out));
        if p.no_user {
            return Err(DockerError::other(format!(
                "there's no user {user} on this host: fix the host's Docker user in its settings"
            )));
        }
        let Some(bin) = p.cli_path.clone().filter(|b| transport::safe_bin(b)) else {
            return Err(DockerError::NotInstalled);
        };
        // The login itself: no sudo needed (or wanted).
        if p.login == user {
            return Box::pin(Self::connect(conn, None)).await;
        }
        if p.rc == Some(0) {
            return Self::with_route(conn, Route::AsUser { user: user.to_owned(), bin }).await;
        }
        Err(user_failure(&p, user, bin))
    }

    /// Connects over a fixed route and checks the daemon answers.
    pub async fn with_route(conn: Arc<Connection>, route: Route) -> DockerResult<Self> {
        let docker = transport::client(conn.clone(), route.clone(), API_TIMEOUT).map_err(DockerError::other)?;
        docker.ping().await.map_err(|e| DockerError::from_bollard(e, None))?;
        Ok(Self { docker, route, conn, docker_user: None })
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
    if *route == Route::WindowsCli {
        // From cmd or PowerShell: no quoting tricks. Ids are checked by
        // `valid_container_ref` before they get here.
        return format!("docker exec -it {container} sh");
    }
    format!(
        "{}{} exec -it {} sh -c 'command -v bash >/dev/null 2>&1 && exec bash || exec sh'",
        route.shell_env(),
        route.docker_cli(),
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
    /// The user ignores this container's healthcheck (D-201): Nightgrid
    /// treats it as having none. `health` still says what Docker reports.
    pub health_ignored: bool,
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

impl ContainerView {
    /// The health Nightgrid judges by: `None` when there is no healthcheck
    /// or the user ignores it (D-201).
    pub fn effective_health(&self) -> Option<&str> {
        self.health.as_deref().filter(|_| !self.health_ignored)
    }
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
            health_ignored: false,
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

    /// The host's engine over `conn`, as `docker_user` when set (see
    /// [`DockerHost::connect`]). Reconnects when the user setting changed.
    pub async fn get(
        &self,
        id: HostId,
        conn: Arc<Connection>,
        docker_user: Option<&str>,
    ) -> DockerResult<Arc<DockerHost>> {
        let cached = |s: &Self| {
            s.hosts
                .lock()
                .expect("poisoned")
                .get(&id)
                .filter(|d| Arc::ptr_eq(&d.conn, &conn) && !d.is_closed() && d.docker_user.as_deref() == docker_user)
                .cloned()
        };
        if let Some(d) = cached(self) {
            return Ok(d);
        }
        let _guard = self.opening.lock().await;
        if let Some(d) = cached(self) {
            return Ok(d);
        }
        let mut d = DockerHost::connect(conn, docker_user).await?;
        d.docker_user = docker_user.map(str::to_owned);
        let d = Arc::new(d);
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
        let p = parse_probe("ok /var/run/docker.sock\nadmin\ncli /usr/local/bin/docker\ndsm\n");
        assert_eq!(p.cli_path.as_deref(), Some("/usr/local/bin/docker"));
        assert!(p.synology && p.cli);
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
            format!(
                "{}docker exec -it 'web' sh -c 'command -v bash >/dev/null 2>&1 && exec bash || exec sh'",
                PATH_FIX
            )
        );
        let rootless = Route::Socket("/run/user/1000/docker.sock".into());
        assert!(exec_shell_command("web", &rootless)
            .starts_with(&format!("{PATH_FIX}export DOCKER_HOST='unix:///run/user/1000/docker.sock'; docker exec")));
        let sudo = Route::Sudo("/usr/local/bin/docker".into());
        assert!(
            exec_shell_command("web", &sudo).starts_with(&format!("{PATH_FIX}sudo -n /usr/local/bin/docker exec -it"))
        );
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

    fn denied(out: &str) -> (Probe, Vec<String>) {
        let p = parse_probe(out);
        let steps = access_steps(&p);
        (p, steps)
    }

    #[test]
    fn synology_gets_synogroup_and_chown() {
        // Stock DSM: no docker group, socket root:root.
        let (p, steps) =
            denied("denied /var/run/docker.sock\nnasuser\ncli /usr/local/bin/docker\ndsm\nnogroup\nsockgroup root\n");
        assert!(p.synology && !p.group_exists && !p.session_group);
        assert_eq!(steps, ["synogroup --add docker nasuser", "chown root:docker /var/run/docker.sock"]);
        // Group made earlier by someone else.
        let (_, steps) =
            denied("denied /var/run/docker.sock\nnasuser\ncli /usr/local/bin/docker\ndsm\ngroup\nsockgroup docker\n");
        assert_eq!(steps, ["synogroup --memberadd docker nasuser"]);
        // Container Manager restarted and the socket went back to root.
        let (p, steps) = denied(
            "denied /var/run/docker.sock\nnasuser\ncli /usr/local/bin/docker\ndsm\ngroup member\nsession\nsockgroup root\n",
        );
        assert!(p.in_group && p.session_group);
        assert_eq!(steps, ["chown root:docker /var/run/docker.sock"]);
        match permission_denied(&p) {
            DockerError::PermissionDenied { synology, relogin, .. } => assert!(synology && !relogin),
            e => panic!("{e:?}"),
        }
    }

    #[test]
    fn docker_user_sudoers_rule() {
        let p = parse_user_probe(
            "Pontuzz\ncli /usr/local/bin/docker\ndsm\ngroup member\nsock /var/run/docker.sock\nsockgroup docker\n\
             compose /usr/local/bin/docker-compose\nrc 1\nout sudo: a password is required\n",
        );
        assert_eq!(p.login, "Pontuzz");
        assert!(p.synology && p.in_group && !p.no_user);
        assert_eq!(p.rc, Some(1));
        match user_failure(&p, "dockeruser", "/usr/local/bin/docker".into()) {
            DockerError::DockerUserSudo { login, user, synology, steps } => {
                assert_eq!((login.as_str(), user.as_str(), synology), ("Pontuzz", "dockeruser", true));
                assert_eq!(
                    steps,
                    [
                        r#"sh -c 'umask 337; echo "Pontuzz ALL=(dockeruser) NOPASSWD: /usr/local/bin/docker, /usr/local/bin/docker-compose" > /etc/sudoers.d/nightgrid-docker-dockeruser'"#
                    ]
                );
                assert!(valid_access_step(&steps[0]));
                let s = access_fix_script(&steps);
                assert!(s.contains("includedir"), "{s}");
                assert!(s.contains("visudo -cf /etc/sudoers.d/nightgrid-docker-dockeruser"), "{s}");
            }
            e => panic!("{e:?}"),
        }
        // A dot in the user name: sudoers skips such files, so the file name drops it.
        let step = sudoers_step("sam", "svc.docker", &["/usr/bin/docker".into()]);
        assert!(step.ends_with("/etc/sudoers.d/nightgrid-docker-svc_docker'"), "{step}");
        assert!(valid_access_step(&step));
        // Anything not exactly what sudoers_step makes is refused.
        for bad in [
            r#"sh -c 'umask 337; echo "sam ALL=(root) NOPASSWD: ALL" > /etc/sudoers.d/nightgrid-docker-root'"#,
            r#"sh -c 'umask 337; echo "sam ALL=(x) NOPASSWD: /bin/sh" > /etc/sudoers.d/nightgrid-docker-y'"#,
            r#"sh -c 'umask 337; echo "sam ALL=(x) NOPASSWD: /usr/bin/docker" > /etc/sudoers.d/nightgrid-docker-x'; reboot"#,
            r#"sh -c 'umask 337; echo "%admin ALL=(x) NOPASSWD: /usr/bin/docker" > /etc/sudoers.d/nightgrid-docker-x'"#,
            r#"sh -c 'umask 337; echo "sam ALL=(x) NOPASSWD: /usr/bin/docker, /a, /b" > /etc/sudoers.d/nightgrid-docker-x'"#,
        ] {
            assert!(!valid_access_step(bad), "{bad}");
        }
    }

    #[test]
    fn docker_user_without_socket_access_gets_group_steps() {
        let p = parse_user_probe(
            "admin\ncli /usr/local/bin/docker\ndsm\nnogroup\nsock /var/run/docker.sock\nsockgroup root\nrc 1\n\
             out permission denied while trying to connect to the Docker daemon socket at unix:///var/run/docker.sock\n",
        );
        match user_failure(&p, "dockeruser", "/usr/local/bin/docker".into()) {
            DockerError::PermissionDenied { user, steps, relogin, .. } => {
                assert_eq!(user, "dockeruser");
                assert_eq!(steps, ["synogroup --add docker dockeruser", "chown root:docker /var/run/docker.sock"]);
                assert!(!relogin);
            }
            e => panic!("{e:?}"),
        }
        assert!(parse_user_probe("nouser\n").no_user);
        let other =
            parse_user_probe("sam\ncli /usr/bin/docker\nlinux\ngroup\nrc 1\nout Cannot connect to the Docker daemon\n");
        assert!(matches!(user_failure(&other, "svc", "/usr/bin/docker".into()), DockerError::Other { .. }));
    }

    #[test]
    fn linux_gets_usermod() {
        let (p, steps) =
            denied("denied /var/run/docker.sock\nsam\ncli /usr/bin/docker\nlinux\ngroup\nsockgroup docker\n");
        assert!(!p.synology);
        assert_eq!(steps, ["usermod -aG docker sam"]);
        let (_, steps) = denied("denied /var/run/docker.sock\nsam\nnocli\nlinux\nnogroup\nsockgroup root\n");
        assert_eq!(steps, ["groupadd docker", "usermod -aG docker sam", "chown root:docker /var/run/docker.sock"]);
        // Already a member, this login just predates it.
        let (p, steps) =
            denied("denied /var/run/docker.sock\nsam\ncli /usr/bin/docker\nlinux\ngroup member\nsockgroup docker\n");
        assert!(steps.is_empty());
        assert!(matches!(permission_denied(&p), DockerError::PermissionDenied { relogin: true, .. }));
        // Someone else's rootless socket is never chowned.
        let (_, steps) = denied("denied /run/user/1001/docker.sock\nsam\nnocli\nlinux\nnogroup\nsockgroup ann\n");
        assert_eq!(steps, ["groupadd docker", "usermod -aG docker sam"]);
    }

    #[test]
    fn odd_user_names_are_quoted_and_steps_are_checked() {
        let (_, steps) = denied("denied /var/run/docker.sock\nj doe\nnocli\nlinux\ngroup\nsockgroup docker\n");
        assert_eq!(steps, ["usermod -aG docker 'j doe'"]);
        assert!(!valid_access_step(&steps[0]));
        for ok in [
            "groupadd docker",
            "usermod -aG docker sam",
            "synogroup --add docker nasuser",
            "synogroup --memberadd docker nas.user",
            "chown root:docker /var/run/docker.sock",
        ] {
            assert!(valid_access_step(ok), "{ok}");
        }
        for bad in [
            "chown root:docker /etc/shadow",
            "usermod -aG wheel sam",
            "usermod -aG docker sam;reboot",
            "synogroup --add docker -x",
            "rm -rf /",
            "",
        ] {
            assert!(!valid_access_step(bad), "{bad}");
        }
        let s = access_fix_script(&["groupadd docker".into(), "usermod -aG docker sam".into()]);
        assert!(s.starts_with("set -e\nPATH="), "{s}");
        assert!(s.contains("/usr/syno/sbin"));
        assert!(s.ends_with("export PATH\ngroupadd docker\nusermod -aG docker sam\n"), "{s}");
    }
}
