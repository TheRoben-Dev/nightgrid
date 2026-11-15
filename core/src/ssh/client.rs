//! SSH client connections over russh: host key verification, authentication
//! (agent, key file, password / keyboard-interactive) and ProxyJump chains.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::client::{self, Handle, KeyboardInteractiveAuthResponse};
use russh::keys::agent::client::AgentClient;
use russh::keys::agent::AgentIdentity;
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate};
use russh::{ChannelMsg, MethodKind, MethodSet};
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncWrite};

use super::known_hosts::{HostKeyInfo, HostKeyStatus, KnownHosts};
use crate::secrets::Secret;

/// How to authenticate to one hop.
#[derive(Debug, Clone)]
pub enum Auth {
    /// SSH agent. `identity_file` (if set) is tried first: its `.pub` picks the
    /// agent identity, and the private key is the fallback if the agent lacks it.
    Agent {
        identity_file: Option<PathBuf>,
        passphrase: Option<Arc<Secret>>,
    },
    Key {
        path: PathBuf,
        passphrase: Option<Arc<Secret>>,
    },
    Password {
        password: Option<Arc<Secret>>,
    },
}

/// One SSH hop.
#[derive(Debug, Clone)]
pub struct Target {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: Auth,
}

impl Target {
    pub fn label(&self) -> String {
        if self.port == 22 {
            format!("{}@{}", self.user, self.host)
        } else {
            format!("{}@{}:{}", self.user, self.host, self.port)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectOptions {
    pub known_hosts: KnownHosts,
    pub connect_timeout: Duration,
    pub keepalive_interval: Duration,
    pub keepalive_max: usize,
    /// Agent socket / pipe override. `None` uses `SSH_AUTH_SOCK` (or the
    /// OpenSSH pipe / Pageant on Windows).
    pub agent_path: Option<PathBuf>,
}

impl ConnectOptions {
    pub fn new(known_hosts: KnownHosts) -> Self {
        Self {
            known_hosts,
            connect_timeout: Duration::from_secs(10),
            keepalive_interval: Duration::from_secs(10),
            keepalive_max: 3,
            agent_path: None,
        }
    }
}

/// Why a connection failed, in a shape the UI can act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ConnectError {
    #[error("unknown host key for {host}:{port}")]
    HostKeyUnknown { host: String, port: u16, key: HostKeyInfo },
    #[error("HOST KEY CHANGED for {host}:{port}")]
    HostKeyChanged { host: String, port: u16, key: HostKeyInfo, previous: Vec<HostKeyInfo> },
    /// `target` is the hop label (`user@host[:port]`) the secret is for.
    #[error("passphrase required for {path}")]
    PassphraseRequired { target: String, host: String, path: String, retry: bool },
    #[error("password required for {target}")]
    PasswordRequired { target: String, host: String, user: String, retry: bool },
    #[error("authentication failed for {user}@{host}: {message}")]
    AuthFailed { host: String, user: String, message: String },
    #[error("cannot reach {host}: {message}")]
    Network { host: String, message: String },
    #[error("{message}")]
    Other { message: String },
}

impl ConnectError {
    fn other(e: impl std::fmt::Display) -> Self {
        Self::Other { message: e.to_string() }
    }

    /// Errors that need the user before trying again (no auto-retry).
    pub fn needs_user(&self) -> bool {
        !matches!(self, Self::Network { .. })
    }
}

/// Remote (`-R`) forwards on one connection: bound server port → where the
/// server's `forwarded-tcpip` channels for it go.
pub type RemoteForwards = Arc<Mutex<HashMap<u32, tokio::sync::mpsc::UnboundedSender<russh::Channel<client::Msg>>>>>;

/// russh client handler: verifies the server key against known_hosts and
/// routes remote-forward channels to their tunnel.
pub struct Verifier {
    host: String,
    port: u16,
    known_hosts: KnownHosts,
    issue: Arc<Mutex<Option<ConnectError>>>,
    forwards: RemoteForwards,
}

impl client::Handler for Verifier {
    type Error = russh::Error;

    async fn check_server_key(&mut self, server_key: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        let key: PublicKey = match server_key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.clone(),
            // Host certificates: verify the embedded key with TOFU like a plain key.
            PublicKeyOrCertificate::Certificate(cert) => PublicKey::from(cert.public_key().clone()),
        };
        let status = self.known_hosts.check(&self.host, self.port, &key);
        let issue = match status {
            Ok(HostKeyStatus::Trusted) => return Ok(true),
            Ok(HostKeyStatus::Unknown) => ConnectError::HostKeyUnknown {
                host: self.host.clone(),
                port: self.port,
                key: HostKeyInfo::from_key(&key),
            },
            Ok(HostKeyStatus::Changed { previous }) => {
                tracing::warn!(host = %self.host, port = self.port, "host key changed; blocking connection");
                ConnectError::HostKeyChanged {
                    host: self.host.clone(),
                    port: self.port,
                    key: HostKeyInfo::from_key(&key),
                    previous,
                }
            }
            Err(e) => ConnectError::other(format!("known_hosts: {e}")),
        };
        *self.issue.lock().expect("poisoned") = Some(issue);
        Ok(false)
    }

    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: russh::Channel<client::Msg>,
        _connected_address: &str,
        connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        let target = self.forwards.lock().expect("poisoned").get(&connected_port).cloned();
        match target {
            Some(tx) => {
                reply.accept().await;
                let _ = tx.send(channel);
            }
            // Nothing asked for this port (or the tunnel just stopped).
            None => reply.reject(russh::ChannelOpenFailure::AdministrativelyProhibited).await,
        }
        Ok(())
    }
}

pub type SshHandle = Handle<Verifier>;

/// An authenticated connection, plus the jump-host connections it rides on.
pub struct Connection {
    pub handle: SshHandle,
    pub label: String,
    /// Active remote forwards (see [`RemoteForwards`]).
    pub forwards: RemoteForwards,
    _jumps: Vec<Arc<SshHandle>>,
}

impl Connection {
    pub fn is_closed(&self) -> bool {
        self.handle.is_closed() || self._jumps.iter().any(|j| j.is_closed())
    }

    /// Opens an interactive shell channel with a PTY.
    pub async fn open_shell(&self, cols: u16, rows: u16) -> Result<russh::Channel<client::Msg>, ConnectError> {
        let ch = self.handle.channel_open_session().await.map_err(ConnectError::other)?;
        ch.request_pty(true, "xterm-256color", cols.into(), rows.into(), 0, 0, &[])
            .await
            .map_err(ConnectError::other)?;
        ch.request_shell(true).await.map_err(ConnectError::other)?;
        Ok(ch)
    }

    /// Opens a session channel running a subsystem (e.g. `sftp`).
    pub async fn open_subsystem(&self, name: &str) -> Result<russh::Channel<client::Msg>, ConnectError> {
        let ch = self.handle.channel_open_session().await.map_err(ConnectError::other)?;
        ch.request_subsystem(true, name).await.map_err(ConnectError::other)?;
        Ok(ch)
    }

    /// Opens a channel to a Unix socket on the server (`direct-streamlocal`,
    /// like `ssh -L local:/remote.sock`). Fails if the server forbids it.
    pub async fn open_streamlocal(&self, path: &str) -> Result<russh::Channel<client::Msg>, ConnectError> {
        self.handle.channel_open_direct_streamlocal(path).await.map_err(ConnectError::other)
    }

    /// Starts `command` without a PTY and returns its channel (raw stdio stream).
    pub async fn open_exec(&self, command: &str) -> Result<russh::Channel<client::Msg>, ConnectError> {
        let ch = self.handle.channel_open_session().await.map_err(ConnectError::other)?;
        ch.exec(true, command).await.map_err(ConnectError::other)?;
        Ok(ch)
    }

    /// Opens a PTY channel running `command` (e.g. `docker exec -it ...`).
    pub async fn open_pty_exec(
        &self,
        command: &str,
        cols: u16,
        rows: u16,
    ) -> Result<russh::Channel<client::Msg>, ConnectError> {
        let ch = self.handle.channel_open_session().await.map_err(ConnectError::other)?;
        ch.request_pty(true, "xterm-256color", cols.into(), rows.into(), 0, 0, &[])
            .await
            .map_err(ConnectError::other)?;
        ch.exec(true, command).await.map_err(ConnectError::other)?;
        Ok(ch)
    }

    /// Runs a command and collects stdout/stderr/exit code (for tests and quick probes).
    pub async fn exec(&self, command: &str) -> Result<(i32, Vec<u8>, Vec<u8>), ConnectError> {
        let mut ch = self.handle.channel_open_session().await.map_err(ConnectError::other)?;
        ch.exec(true, command).await.map_err(ConnectError::other)?;
        let (mut out, mut err, mut code) = (Vec::new(), Vec::new(), -1);
        while let Some(msg) = ch.wait().await {
            match msg {
                ChannelMsg::Data { data } => out.extend_from_slice(&data),
                ChannelMsg::ExtendedData { data, .. } => err.extend_from_slice(&data),
                ChannelMsg::ExitStatus { exit_status } => code = exit_status as i32,
                _ => {}
            }
        }
        Ok((code, out, err))
    }

    pub async fn disconnect(&self) {
        let _ = self.handle.disconnect(russh::Disconnect::ByApplication, "", "en").await;
    }
}

fn client_config(opts: &ConnectOptions) -> Arc<client::Config> {
    Arc::new(client::Config {
        keepalive_interval: Some(opts.keepalive_interval),
        keepalive_max: opts.keepalive_max,
        nodelay: true,
        ..Default::default()
    })
}

async fn handshake<S>(
    stream: S,
    t: &Target,
    opts: &ConnectOptions,
    forwards: RemoteForwards,
) -> Result<SshHandle, ConnectError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let issue = Arc::new(Mutex::new(None));
    let verifier = Verifier {
        host: t.host.clone(),
        port: t.port,
        known_hosts: opts.known_hosts.clone(),
        issue: issue.clone(),
        forwards,
    };
    let res = tokio::time::timeout(opts.connect_timeout, client::connect_stream(client_config(opts), stream, verifier))
        .await
        .map_err(|_| ConnectError::Network { host: t.host.clone(), message: "SSH handshake timed out".into() })?;
    match res {
        Ok(h) => Ok(h),
        Err(e) => {
            if let Some(issue) = issue.lock().expect("poisoned").take() {
                return Err(issue);
            }
            Err(ConnectError::Network { host: t.host.clone(), message: e.to_string() })
        }
    }
}

fn wants_password(methods: &MethodSet) -> bool {
    methods.contains(&MethodKind::Password) || methods.contains(&MethodKind::KeyboardInteractive)
}

async fn rsa_hash(h: &SshHandle, key: &PublicKey) -> Option<HashAlg> {
    if key.algorithm().is_rsa() {
        h.best_supported_rsa_hash().await.ok().flatten().flatten()
    } else {
        None
    }
}

type DynAgent = AgentClient<Box<dyn russh::keys::agent::client::AgentStream + Send + Unpin + 'static>>;

/// Connects to the platform's SSH agent: `SSH_AUTH_SOCK` (OpenSSH, 1Password,
/// Secretive...) on Unix; the OpenSSH named pipe (also used by 1Password) and
/// then Pageant on Windows.
pub async fn connect_agent(path: Option<&std::path::Path>) -> Result<DynAgent, String> {
    #[cfg(unix)]
    {
        let res = match path {
            Some(p) => AgentClient::connect_uds(p).await,
            None => AgentClient::connect_env().await,
        };
        res.map(|a| a.dynamic()).map_err(|e| format!("SSH agent unavailable: {e}"))
    }
    #[cfg(windows)]
    {
        let pipe = path
            .map(|p| p.as_os_str().to_owned())
            .or_else(|| std::env::var_os("SSH_AUTH_SOCK"))
            .unwrap_or_else(|| r"\\.\pipe\openssh-ssh-agent".into());
        if let Ok(a) = AgentClient::connect_named_pipe(&pipe).await {
            return Ok(a.dynamic());
        }
        AgentClient::connect_pageant()
            .await
            .map(|a| a.dynamic())
            .map_err(|e| format!("no SSH agent (OpenSSH pipe or Pageant): {e}"))
    }
}

fn read_pubkey(identity: &std::path::Path) -> Option<PublicKey> {
    let pub_path = PathBuf::from(format!("{}.pub", identity.display()));
    std::fs::read_to_string(pub_path).ok().and_then(|s| PublicKey::from_openssh(s.trim()).ok())
}

enum Step {
    Done,
    Failed(MethodSet),
}

async fn try_agent(
    h: &mut SshHandle,
    t: &Target,
    preferred: Option<&PublicKey>,
    agent_path: Option<&std::path::Path>,
) -> Result<Step, String> {
    let mut agent = connect_agent(agent_path).await?;
    let mut ids = agent.request_identities().await.map_err(|e| format!("SSH agent: {e}"))?;
    if let Some(p) = preferred {
        ids.sort_by_key(|id| (id.public_key().key_data() != p.key_data()) as u8);
    }
    let mut remaining = MethodSet::empty();
    for id in ids {
        let key = id.public_key().into_owned();
        let hash = rsa_hash(h, &key).await;
        let res = match &id {
            AgentIdentity::PublicKey { .. } => h.authenticate_publickey_with(&t.user, key, hash, &mut agent).await,
            AgentIdentity::Certificate { certificate, .. } => {
                h.authenticate_certificate_with(&t.user, certificate.clone(), hash, &mut agent).await
            }
        };
        match res {
            Ok(r) if r.success() => return Ok(Step::Done),
            Ok(russh::client::AuthResult::Failure { remaining_methods, .. }) => remaining = remaining_methods,
            Ok(_) => {}
            Err(e) => tracing::debug!("agent identity rejected: {e}"),
        }
    }
    Ok(Step::Failed(remaining))
}

async fn try_key(
    h: &mut SshHandle,
    t: &Target,
    path: &PathBuf,
    passphrase: Option<&Secret>,
) -> Result<Step, ConnectError> {
    let key = match russh::keys::load_secret_key(path, passphrase.map(Secret::expose)) {
        Ok(k) => k,
        Err(russh::keys::Error::KeyIsEncrypted) => {
            return Err(ConnectError::PassphraseRequired {
                target: t.label(),
                host: t.host.clone(),
                path: path.display().to_string(),
                retry: false,
            })
        }
        Err(e) if passphrase.is_some() => {
            tracing::debug!("key decode failed: {e}");
            return Err(ConnectError::PassphraseRequired {
                target: t.label(),
                host: t.host.clone(),
                path: path.display().to_string(),
                retry: true,
            });
        }
        Err(e) => return Err(ConnectError::other(format!("cannot read key {}: {e}", path.display()))),
    };
    let hash = rsa_hash(h, key.public_key()).await;
    let res = h
        .authenticate_publickey(&t.user, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
        .await
        .map_err(ConnectError::other)?;
    Ok(match res {
        r if r.success() => Step::Done,
        russh::client::AuthResult::Failure { remaining_methods, .. } => Step::Failed(remaining_methods),
        _ => Step::Failed(MethodSet::empty()),
    })
}

async fn try_password(
    h: &mut SshHandle,
    t: &Target,
    password: &Secret,
    methods: &MethodSet,
) -> Result<bool, ConnectError> {
    if methods.is_empty() || methods.contains(&MethodKind::Password) {
        let r = h.authenticate_password(&t.user, password.expose()).await.map_err(ConnectError::other)?;
        if r.success() {
            return Ok(true);
        }
    }
    // Keyboard-interactive, answering every hidden prompt with the password.
    if methods.is_empty() || methods.contains(&MethodKind::KeyboardInteractive) {
        let mut resp = h.authenticate_keyboard_interactive_start(&t.user, None).await.map_err(ConnectError::other)?;
        for _ in 0..5 {
            match resp {
                KeyboardInteractiveAuthResponse::Success => return Ok(true),
                KeyboardInteractiveAuthResponse::Failure { .. } => return Ok(false),
                KeyboardInteractiveAuthResponse::InfoRequest { prompts, .. } => {
                    let answers = prompts
                        .iter()
                        .map(|p| if p.echo { String::new() } else { password.expose().to_owned() })
                        .collect();
                    resp = h.authenticate_keyboard_interactive_respond(answers).await.map_err(ConnectError::other)?;
                }
            }
        }
    }
    Ok(false)
}

async fn authenticate(h: &mut SshHandle, t: &Target, opts: &ConnectOptions) -> Result<(), ConnectError> {
    let user_err =
        |retry| ConnectError::PasswordRequired { target: t.label(), host: t.host.clone(), user: t.user.clone(), retry };
    match &t.auth {
        Auth::Agent { identity_file, passphrase } => {
            let preferred = identity_file.as_deref().and_then(read_pubkey);
            let agent_res = try_agent(h, t, preferred.as_ref(), opts.agent_path.as_deref()).await;
            let mut methods = match agent_res {
                Ok(Step::Done) => return Ok(()),
                Ok(Step::Failed(m)) => m,
                Err(msg) => {
                    tracing::info!("{msg}");
                    if identity_file.is_none() {
                        return Err(ConnectError::AuthFailed {
                            host: t.host.clone(),
                            user: t.user.clone(),
                            message: msg,
                        });
                    }
                    MethodSet::empty()
                }
            };
            if let Some(path) = identity_file {
                if path.exists() {
                    match try_key(h, t, path, passphrase.as_deref()).await? {
                        Step::Done => return Ok(()),
                        Step::Failed(m) => methods = m,
                    }
                }
            }
            if wants_password(&methods) {
                return Err(user_err(false));
            }
            Err(ConnectError::AuthFailed {
                host: t.host.clone(),
                user: t.user.clone(),
                message: "no agent identity was accepted".into(),
            })
        }
        Auth::Key { path, passphrase } => match try_key(h, t, path, passphrase.as_deref()).await? {
            Step::Done => Ok(()),
            Step::Failed(m) if wants_password(&m) => Err(user_err(false)),
            Step::Failed(_) => Err(ConnectError::AuthFailed {
                host: t.host.clone(),
                user: t.user.clone(),
                message: "key was rejected".into(),
            }),
        },
        Auth::Password { password } => {
            let Some(pw) = password else { return Err(user_err(false)) };
            // Probe allowed methods with "none" first.
            let methods = match h.authenticate_none(&t.user).await.map_err(ConnectError::other)? {
                russh::client::AuthResult::Success => return Ok(()),
                russh::client::AuthResult::Failure { remaining_methods, .. } => remaining_methods,
            };
            if try_password(h, t, pw, &methods).await? {
                Ok(())
            } else {
                Err(user_err(true))
            }
        }
    }
}

async fn tcp_connect(t: &Target, opts: &ConnectOptions) -> Result<tokio::net::TcpStream, ConnectError> {
    // Through the built-in tailnet when that's the way there (M10j).
    let s = crate::tailnet::dial(&t.host, t.port, opts.connect_timeout)
        .await
        .map_err(|e| ConnectError::Network { host: t.host.clone(), message: e.to_string() })?;
    let _ = s.set_nodelay(true);
    Ok(s)
}

/// Connects to `target`, hopping through `jumps` in order (ProxyJump).
pub async fn connect(target: &Target, jumps: &[Target], opts: &ConnectOptions) -> Result<Connection, ConnectError> {
    let mut chain: Vec<Arc<SshHandle>> = Vec::new();
    // Only the final hop's forwards are used; jump hosts get their own, unused map.
    let forwards = RemoteForwards::default();
    let hops: Vec<&Target> = jumps.iter().chain(std::iter::once(target)).collect();
    let fwd_for = |i: usize| if i + 1 == hops.len() { forwards.clone() } else { RemoteForwards::default() };
    for (i, hop) in hops.iter().enumerate() {
        let mut handle = if i == 0 {
            handshake(tcp_connect(hop, opts).await?, hop, opts, fwd_for(i)).await?
        } else {
            let via = chain.last().expect("previous hop");
            let ch = via.channel_open_direct_tcpip(hop.host.clone(), hop.port.into(), "127.0.0.1", 0).await.map_err(
                |e| ConnectError::Network { host: hop.host.clone(), message: format!("via jump host: {e}") },
            )?;
            handshake(ch.into_stream(), hop, opts, fwd_for(i)).await?
        };
        authenticate(&mut handle, hop, opts).await?;
        tracing::info!(hop = %hop.label(), "authenticated");
        chain.push(Arc::new(handle));
    }
    let last = chain.pop().expect("at least one hop");
    let handle = Arc::try_unwrap(last).unwrap_or_else(|_| unreachable!("no other references"));
    Ok(Connection { handle, label: target.label(), forwards, _jumps: chain })
}

/// Lightweight reachability check: TCP connect to host:port, returns latency.
pub async fn tcp_ping(host: &str, port: u16, timeout: Duration) -> Option<Duration> {
    let start = std::time::Instant::now();
    crate::tailnet::dial(host, port, timeout).await.ok().map(|_| start.elapsed())
}
