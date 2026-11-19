//! SSH client connections over russh: host key verification, authentication
//! (agent, key file, password / keyboard-interactive) and ProxyJump chains.
//!
//! Every hop asks for "none" auth first, like OpenSSH. Servers that decide
//! who you are without a key (Tailscale SSH) accept it there; in Tailscale's
//! "check" mode they first send a banner with a login URL and hold the
//! answer until it's approved, so banners are passed to `on_notice` and the
//! wait is bounded by `auth_wait`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
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
    ///
    /// With no `identity_file`, the `default_keys` (OpenSSH's `~/.ssh/id_*`)
    /// are tried after the agent. When nothing key-based works and the server
    /// takes passwords, `password` is used (or asked for), like OpenSSH does.
    Agent {
        identity_file: Option<PathBuf>,
        passphrase: Option<Arc<Secret>>,
        default_keys: Vec<PathBuf>,
        password: Option<Arc<Secret>>,
    },
    /// Key file; `password` is the fallback when the key is refused and the
    /// server takes passwords.
    Key {
        path: PathBuf,
        passphrase: Option<Arc<Secret>>,
        password: Option<Arc<Secret>>,
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
    /// How long one authentication step may wait for the server. Tailscale
    /// SSH's check mode holds the answer until the login is approved.
    pub auth_wait: Duration,
    /// Shown to the user while connecting: banners and keyboard-interactive
    /// instructions (Tailscale SSH sends its approval URL this way).
    pub on_notice: Option<NoticeSink>,
}

/// Something the server said during authentication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AuthNotice {
    /// Hop label (`user@host[:port]`).
    pub target: String,
    pub text: String,
    /// The first https link in the text (an approval page), if any.
    pub url: Option<String>,
    /// The server identifies as Tailscale SSH.
    pub tailscale: bool,
}

/// Where [`AuthNotice`]s go (the app emits them to the UI).
#[derive(Clone)]
pub struct NoticeSink(pub Arc<dyn Fn(AuthNotice) + Send + Sync>);

impl NoticeSink {
    fn send(&self, n: AuthNotice) {
        (self.0)(n)
    }
}

impl std::fmt::Debug for NoticeSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NoticeSink")
    }
}

/// The first `https://` link in `text`, without trailing punctuation.
pub fn first_url(text: &str) -> Option<String> {
    // e.g. "# Tailscale SSH requires an additional check.\n# To authenticate, visit: https://login.tailscale.com/a/abc123\n"
    let start = text.find("https://")?;
    let url: String =
        text[start..].chars().take_while(|c| !c.is_whitespace() && !matches!(c, '"' | '\'' | '<' | '>')).collect();
    let url = url.trim_end_matches(['.', ',', ')', ';', ':']).to_owned();
    (url.len() > "https://".len()).then_some(url)
}

impl ConnectOptions {
    pub fn new(known_hosts: KnownHosts) -> Self {
        Self {
            known_hosts,
            connect_timeout: Duration::from_secs(10),
            keepalive_interval: Duration::from_secs(10),
            keepalive_max: 3,
            agent_path: None,
            auth_wait: Duration::from_secs(300),
            on_notice: None,
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
    label: String,
    on_notice: Option<NoticeSink>,
    /// Set once a banner came from a Tailscale SSH server.
    tailscale: Arc<AtomicBool>,
}

/// Wraps the transport to note the server's first bytes, which hold its
/// version line (`SSH-2.0-OpenSSH_for_Windows_9.5`). russh keeps that line
/// to itself until a banner arrives, and most servers send none.
struct SniffId<S> {
    inner: S,
    seen: Arc<Mutex<Vec<u8>>>,
}

/// Enough for the version line (255 bytes max) plus any lines before it.
const SNIFF_MAX: usize = 1024;

impl<S: AsyncRead + Unpin> AsyncRead for SniffId<S> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let res = std::pin::Pin::new(&mut self.inner).poll_read(cx, buf);
        if res.is_ready() {
            let mut seen = self.seen.lock().expect("poisoned");
            if seen.len() < SNIFF_MAX {
                let new = &buf.filled()[before..];
                let take = new.len().min(SNIFF_MAX - seen.len());
                seen.extend_from_slice(&new[..take]);
            }
        }
        res
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for SniffId<S> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// The `SSH-...` version line among the server's first bytes.
fn version_line(seen: &[u8]) -> String {
    String::from_utf8_lossy(seen)
        .lines()
        .find(|l| l.starts_with("SSH-"))
        .map(|l| l.trim().to_owned())
        .unwrap_or_default()
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

    async fn auth_banner(&mut self, banner: &str, session: &mut client::Session) -> Result<(), Self::Error> {
        let ts = String::from_utf8_lossy(session.remote_sshid()).contains("Tailscale") || banner.contains("Tailscale");
        if ts {
            self.tailscale.store(true, Ordering::Relaxed);
        }
        tracing::info!(hop = %self.label, "server banner: {}", banner.trim());
        if let Some(sink) = &self.on_notice {
            sink.send(AuthNotice {
                target: self.label.clone(),
                text: banner.trim().to_owned(),
                url: first_url(banner),
                tailscale: ts,
            });
        }
        Ok(())
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
    /// The server's SSH version string (`SSH-2.0-...`).
    pub server_id: String,
    _jumps: Vec<Arc<SshHandle>>,
}

/// Whether an SSH version string is OpenSSH for Windows (Win32-OpenSSH,
/// the one Windows Server and Windows 10/11 ship). Its default shell is cmd
/// or PowerShell, so POSIX shell snippets must not be sent to it.
pub fn is_windows_server_id(id: &str) -> bool {
    id.to_ascii_lowercase().contains("windows")
}

impl Connection {
    /// The server is OpenSSH for Windows (cmd / PowerShell, no POSIX shell).
    pub fn is_windows(&self) -> bool {
        is_windows_server_id(&self.server_id)
    }

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
) -> Result<(SshHandle, Arc<AtomicBool>, String), ConnectError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let issue = Arc::new(Mutex::new(None));
    let tailscale = Arc::new(AtomicBool::new(false));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let stream = SniffId { inner: stream, seen: seen.clone() };
    let verifier = Verifier {
        host: t.host.clone(),
        port: t.port,
        known_hosts: opts.known_hosts.clone(),
        issue: issue.clone(),
        forwards,
        label: t.label(),
        on_notice: opts.on_notice.clone(),
        tailscale: tailscale.clone(),
    };
    let res = tokio::time::timeout(opts.connect_timeout, client::connect_stream(client_config(opts), stream, verifier))
        .await
        .map_err(|_| ConnectError::Network { host: t.host.clone(), message: "SSH handshake timed out".into() })?;
    match res {
        Ok(h) => {
            let id = version_line(&seen.lock().expect("poisoned"));
            Ok((h, tailscale, id))
        }
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

/// The password step after keys failed: uses the known password, or asks.
async fn password_fallback(
    h: &mut SshHandle,
    t: &Target,
    password: Option<&Secret>,
    methods: &MethodSet,
) -> Result<(), ConnectError> {
    let ask =
        |retry| ConnectError::PasswordRequired { target: t.label(), host: t.host.clone(), user: t.user.clone(), retry };
    match password {
        None => Err(ask(false)),
        Some(pw) if try_password(h, t, pw, methods).await? => Ok(()),
        Some(_) => Err(ask(true)),
    }
}

/// One of OpenSSH's default keys (`~/.ssh/id_*`), tried quietly: a key that
/// can't be read or needs a passphrase we don't have is skipped (`None`).
async fn try_default_key(
    h: &mut SshHandle,
    t: &Target,
    path: &std::path::Path,
    passphrase: Option<&Secret>,
) -> Result<Option<Step>, ConnectError> {
    let key = match russh::keys::load_secret_key(path, passphrase.map(Secret::expose)) {
        Ok(k) => k,
        Err(e) => {
            tracing::debug!(path = %path.display(), "default key skipped: {e}");
            return Ok(None);
        }
    };
    let hash = rsa_hash(h, key.public_key()).await;
    let res = h
        .authenticate_publickey(&t.user, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
        .await
        .map_err(ConnectError::other)?;
    Ok(Some(match res {
        r if r.success() => Step::Done,
        russh::client::AuthResult::Failure { remaining_methods, .. } => Step::Failed(remaining_methods),
        _ => Step::Failed(MethodSet::empty()),
    }))
}

/// OpenSSH's default identity files under `home`, in its order, that exist.
pub fn default_identity_files(home: &std::path::Path) -> Vec<PathBuf> {
    ["id_ed25519", "id_ecdsa", "id_rsa"].iter().map(|n| home.join(".ssh").join(n)).filter(|p| p.is_file()).collect()
}

fn mins(d: Duration) -> String {
    match d.as_secs() {
        s if s >= 120 => format!("{} minutes", s / 60),
        s => format!("{s} seconds"),
    }
}

/// "none" auth: accepted by servers that know who you are without a key
/// (Tailscale SSH). Bounded, since Tailscale's check mode waits for approval.
async fn try_none(h: &mut SshHandle, t: &Target, opts: &ConnectOptions, ts: &AtomicBool) -> Result<Step, ConnectError> {
    match tokio::time::timeout(opts.auth_wait, h.authenticate_none(&t.user)).await {
        Err(_) => Err(waited_too_long(t, opts, ts)),
        Ok(r) => Ok(match r.map_err(ConnectError::other)? {
            russh::client::AuthResult::Success => Step::Done,
            russh::client::AuthResult::Failure { remaining_methods, .. } => Step::Failed(remaining_methods),
        }),
    }
}

fn waited_too_long(t: &Target, opts: &ConnectOptions, ts: &AtomicBool) -> ConnectError {
    let message = if ts.load(Ordering::Relaxed) {
        format!(
            "the Tailscale SSH check wasn't approved within {}. Open the approval link and connect again",
            mins(opts.auth_wait)
        )
    } else {
        format!("the server didn't finish logging in within {}", mins(opts.auth_wait))
    };
    ConnectError::AuthFailed { host: t.host.clone(), user: t.user.clone(), message }
}

async fn bounded<F: std::future::Future>(d: Duration, f: F) -> Option<F::Output> {
    tokio::time::timeout(d, f).await.ok()
}

/// Keyboard-interactive with nothing to type: passes the server's
/// instructions on (Tailscale SSH can send its approval link this way) and
/// answers prompt-less rounds. Gives up at the first real prompt.
async fn try_ki_notice(
    h: &mut SshHandle,
    t: &Target,
    opts: &ConnectOptions,
    ts: &AtomicBool,
) -> Result<Step, ConnectError> {
    let mut resp = bounded(opts.auth_wait, h.authenticate_keyboard_interactive_start(&t.user, None))
        .await
        .ok_or_else(|| waited_too_long(t, opts, ts))?
        .map_err(ConnectError::other)?;
    for _ in 0..5 {
        match resp {
            KeyboardInteractiveAuthResponse::Success => return Ok(Step::Done),
            KeyboardInteractiveAuthResponse::Failure { remaining_methods, .. } => {
                return Ok(Step::Failed(remaining_methods))
            }
            KeyboardInteractiveAuthResponse::InfoRequest { name, instructions, prompts } => {
                let text = [name.trim(), instructions.trim()]
                    .into_iter()
                    .filter(|x| !x.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");
                if !text.is_empty() {
                    if let Some(sink) = &opts.on_notice {
                        sink.send(AuthNotice {
                            target: t.label(),
                            url: first_url(&text),
                            text,
                            tailscale: ts.load(Ordering::Relaxed),
                        });
                    }
                }
                if !prompts.is_empty() {
                    return Ok(Step::Failed(MethodSet::empty()));
                }
                resp = bounded(opts.auth_wait, h.authenticate_keyboard_interactive_respond(Vec::new()))
                    .await
                    .ok_or_else(|| waited_too_long(t, opts, ts))?
                    .map_err(ConnectError::other)?;
            }
        }
    }
    Ok(Step::Failed(MethodSet::empty()))
}

/// Why a Tailscale SSH server still said no.
fn tailscale_refused(t: &Target) -> ConnectError {
    ConnectError::AuthFailed {
        host: t.host.clone(),
        user: t.user.clone(),
        message: format!(
            "Tailscale SSH didn't let this device in as {}. Check that the tailnet policy's \"ssh\" rules allow it, and that Tailscale SSH is on for the host (tailscale set --ssh)",
            t.user
        ),
    }
}

async fn authenticate(
    h: &mut SshHandle,
    t: &Target,
    opts: &ConnectOptions,
    ts: &AtomicBool,
) -> Result<(), ConnectError> {
    let user_err =
        |retry| ConnectError::PasswordRequired { target: t.label(), host: t.host.clone(), user: t.user.clone(), retry };
    // Ask with "none" first: Tailscale SSH (and servers with no auth) accept it.
    let mut methods = match try_none(h, t, opts, ts).await? {
        Step::Done => return Ok(()),
        Step::Failed(m) => m,
    };
    // A Tailscale SSH server, or one that only offers keyboard-interactive
    // without key or password: see whether it wants nothing typed.
    let ki_only = methods.contains(&MethodKind::KeyboardInteractive)
        && !methods.contains(&MethodKind::PublicKey)
        && !methods.contains(&MethodKind::Password);
    let is_ts = || ts.load(Ordering::Relaxed);
    if is_ts() || (ki_only && !matches!(t.auth, Auth::Password { password: Some(_) })) {
        match try_ki_notice(h, t, opts, ts).await? {
            Step::Done => return Ok(()),
            Step::Failed(m) if !m.is_empty() => methods = m,
            Step::Failed(_) => {}
        }
    }
    let res = match &t.auth {
        Auth::Agent { identity_file, passphrase, default_keys, password } => {
            let preferred = identity_file.as_deref().and_then(read_pubkey);
            let mut agent_err = None;
            match try_agent(h, t, preferred.as_ref(), opts.agent_path.as_deref()).await {
                Ok(Step::Done) => return Ok(()),
                // An empty agent says nothing about what the server takes.
                Ok(Step::Failed(m)) if !m.is_empty() => methods = m,
                Ok(Step::Failed(_)) => {}
                Err(msg) => {
                    // No agent is normal (Windows ships with it off): go on to
                    // key files and passwords like OpenSSH does.
                    tracing::debug!("{msg}");
                    agent_err = Some(msg);
                }
            };
            if let Some(path) = identity_file {
                if path.exists() {
                    match try_key(h, t, path, passphrase.as_deref()).await? {
                        Step::Done => return Ok(()),
                        Step::Failed(m) if !m.is_empty() => methods = m,
                        Step::Failed(_) => {}
                    }
                }
            } else {
                for path in default_keys {
                    match try_default_key(h, t, path, passphrase.as_deref()).await? {
                        Some(Step::Done) => return Ok(()),
                        Some(Step::Failed(m)) if !m.is_empty() => methods = m,
                        _ => {}
                    }
                }
            }
            if wants_password(&methods) && !is_ts() {
                return password_fallback(h, t, password.as_deref(), &methods).await;
            }
            let message = match agent_err {
                Some(e) if identity_file.is_none() && default_keys.is_empty() => {
                    format!("{e}, and no key file is set for this host")
                }
                _ => "no key was accepted".into(),
            };
            Err(ConnectError::AuthFailed { host: t.host.clone(), user: t.user.clone(), message })
        }
        Auth::Key { path, passphrase, password } => match try_key(h, t, path, passphrase.as_deref()).await? {
            Step::Done => return Ok(()),
            Step::Failed(m) if !is_ts() && (wants_password(&m) || (m.is_empty() && wants_password(&methods))) => {
                let m = if m.is_empty() { methods } else { m };
                return password_fallback(h, t, password.as_deref(), &m).await;
            }
            Step::Failed(_) => Err(ConnectError::AuthFailed {
                host: t.host.clone(),
                user: t.user.clone(),
                message: "key was rejected".into(),
            }),
        },
        Auth::Password { password } => {
            let Some(pw) = password else { return Err(user_err(false)) };
            if try_password(h, t, pw, &methods).await? {
                return Ok(());
            }
            Err(user_err(true))
        }
    };
    if is_ts() {
        return Err(tailscale_refused(t));
    }
    res
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
    let mut last_id = String::new();
    for (i, hop) in hops.iter().enumerate() {
        let (mut handle, ts, server_id) = if i == 0 {
            handshake(tcp_connect(hop, opts).await?, hop, opts, fwd_for(i)).await?
        } else {
            let via = chain.last().expect("previous hop");
            let ch = via.channel_open_direct_tcpip(hop.host.clone(), hop.port.into(), "127.0.0.1", 0).await.map_err(
                |e| ConnectError::Network { host: hop.host.clone(), message: format!("via jump host: {e}") },
            )?;
            handshake(ch.into_stream(), hop, opts, fwd_for(i)).await?
        };
        // Bounded overall too, so a server that never answers can't hang the
        // connection (each step already has its own limit).
        tokio::time::timeout(opts.auth_wait + opts.connect_timeout * 3, authenticate(&mut handle, hop, opts, &ts))
            .await
            .map_err(|_| waited_too_long(hop, opts, &ts))??;
        tracing::info!(hop = %hop.label(), server = %server_id, "authenticated");
        last_id = server_id;
        chain.push(Arc::new(handle));
    }
    let last = chain.pop().expect("at least one hop");
    let handle = Arc::try_unwrap(last).unwrap_or_else(|_| unreachable!("no other references"));
    Ok(Connection { handle, label: target.label(), forwards, server_id: last_id, _jumps: chain })
}

/// Lightweight reachability check: TCP connect to host:port, returns latency.
pub async fn tcp_ping(host: &str, port: u16, timeout: Duration) -> Option<Duration> {
    let start = std::time::Instant::now();
    crate::tailnet::dial(host, port, timeout).await.ok().map(|_| start.elapsed())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_links_in_banners() {
        let banner = "# Tailscale SSH requires an additional check.\n# To authenticate, visit: https://login.tailscale.com/a/abc123.\n";
        assert_eq!(first_url(banner).as_deref(), Some("https://login.tailscale.com/a/abc123"));
        assert_eq!(first_url("see (https://example.com/x), thanks").as_deref(), Some("https://example.com/x"));
        assert_eq!(first_url("no link here, http://plain.example only"), None);
        assert_eq!(first_url("https://"), None);
    }

    #[test]
    fn windows_servers_by_version_line() {
        let seen = b"SSH-2.0-OpenSSH_for_Windows_9.5\r\n\0\0\x01junk";
        assert_eq!(version_line(seen), "SSH-2.0-OpenSSH_for_Windows_9.5");
        assert!(is_windows_server_id(&version_line(seen)));
        assert_eq!(version_line(b"hello\r\nSSH-2.0-OpenSSH_9.6p1 Ubuntu-3\r\n"), "SSH-2.0-OpenSSH_9.6p1 Ubuntu-3");
        assert!(!is_windows_server_id("SSH-2.0-OpenSSH_9.6p1 Ubuntu-3"));
        assert!(!is_windows_server_id("SSH-2.0-Tailscale"));
        assert_eq!(version_line(b"no version"), "");
    }

    #[test]
    fn default_identities_in_openssh_order() {
        let dir = tempfile::tempdir().unwrap();
        assert!(default_identity_files(dir.path()).is_empty());
        std::fs::create_dir(dir.path().join(".ssh")).unwrap();
        for n in ["id_rsa", "id_ed25519", "id_dsa"] {
            std::fs::write(dir.path().join(".ssh").join(n), "x").unwrap();
        }
        let names: Vec<_> = default_identity_files(dir.path())
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["id_ed25519", "id_rsa"]);
    }

    #[test]
    fn wait_is_said_in_minutes() {
        assert_eq!(mins(Duration::from_secs(300)), "5 minutes");
        assert_eq!(mins(Duration::from_secs(30)), "30 seconds");
    }
}
