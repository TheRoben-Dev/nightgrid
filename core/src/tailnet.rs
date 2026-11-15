//! Built-in Tailscale (M10j).
//!
//! Nightgrid can join the user's tailnet itself, without the system
//! Tailscale app: a small sidecar (`nightgrid-tailnet`, Go + tsnet, see
//! `/tailnet`) runs as its own tailnet node in userspace and exposes a SOCKS5
//! proxy on loopback with per-run credentials. This module starts and talks
//! to that sidecar and decides, per connection, whether to dial directly or
//! through the tailnet ([`dial`]).
//!
//! Routing: tailnet addresses (100.64.0.0/10, fd7a:115c:a1e0::/48), MagicDNS
//! names and peer names go through the tailnet whenever the built-in client
//! is running. Addresses inside an accepted subnet route are tried directly
//! first (you may be at home) and then through the tailnet, so a host saved
//! by its LAN address works on and off the LAN. With the built-in client off,
//! everything is dialled directly, which uses a system Tailscale if one is
//! connected.

use std::collections::{HashMap, HashSet};
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::{Child, ChildStdin, Command};

/// A node of the tailnet (this one or a peer).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct TailnetPeer {
    /// Short MagicDNS name (`lumen`).
    pub name: String,
    /// Full MagicDNS name (`lumen.tail1234.ts.net`).
    pub dns_name: String,
    pub ips: Vec<String>,
    pub os: String,
    pub online: bool,
    /// Subnet routes this peer serves (`192.168.1.0/24`). The sidecar always sends a list.
    pub routes: Vec<String>,
}

/// What the app shows about the built-in client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TailnetStatus {
    /// `off`, `unavailable` (no sidecar in this build), `starting`,
    /// `needsLogin`, `needsMachineAuth`, `running`, `stopped`, `failed`.
    pub state: String,
    /// Open this in the browser to log in (when `needsLogin`).
    pub login_url: Option<String>,
    pub tailnet: Option<String>,
    pub magic_dns_suffix: Option<String>,
    pub self_node: Option<TailnetPeer>,
    pub peers: Vec<TailnetPeer>,
    pub accept_routes: bool,
    /// The last error the client reported, in words.
    pub error: Option<String>,
}

impl TailnetStatus {
    pub fn off(state: &str) -> Self {
        Self {
            state: state.into(),
            login_url: None,
            tailnet: None,
            magic_dns_suffix: None,
            self_node: None,
            peers: Vec::new(),
            accept_routes: false,
            error: None,
        }
    }
}

/// One line the sidecar printed.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "ev", rename_all = "camelCase")]
pub enum Event {
    Ready {
        socks: String,
        user: String,
        pass: String,
    },
    Login {
        url: String,
    },
    State {
        state: String,
    },
    #[serde(rename_all = "camelCase")]
    Status {
        state: String,
        #[serde(default)]
        auth_url: String,
        #[serde(default)]
        tailnet: Option<String>,
        #[serde(default)]
        magic_dns_suffix: String,
        #[serde(default, rename = "self")]
        self_node: Option<TailnetPeer>,
        #[serde(default, deserialize_with = "null_peers")]
        peers: Vec<TailnetPeer>,
    },
    Error {
        message: String,
    },
}

fn null_peers<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<TailnetPeer>, D::Error> {
    Ok(Option::<Vec<TailnetPeer>>::deserialize(d)?.unwrap_or_default())
}

pub fn parse_event(line: &str) -> Option<Event> {
    serde_json::from_str(line.trim()).ok()
}

/// `NeedsLogin` → `needsLogin`.
fn state_word(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_ascii_lowercase().to_string() + c.as_str(),
        None => String::new(),
    }
}

#[derive(Debug, Clone)]
struct Proxy {
    addr: SocketAddr,
    user: String,
    pass: String,
}

/// What the dialer needs to know, shared process-wide.
#[derive(Debug, Default)]
struct Shared {
    proxy: Option<Proxy>,
    running: bool,
    accept_routes: bool,
    /// Lower-case peer names and full MagicDNS names.
    names: HashSet<String>,
    suffix: Option<String>,
    routes: Vec<(IpAddr, u8)>,
}

static SHARED: LazyLock<RwLock<Shared>> = LazyLock::new(Default::default);

fn parse_cidr(s: &str) -> Option<(IpAddr, u8)> {
    let (a, p) = s.split_once('/')?;
    Some((a.parse().ok()?, p.parse().ok()?))
}

fn in_prefix(ip: IpAddr, (net, bits): (IpAddr, u8)) -> bool {
    match (ip, net) {
        (IpAddr::V4(a), IpAddr::V4(n)) if bits <= 32 => {
            let mask = if bits == 0 { 0 } else { u32::MAX << (32 - bits) };
            u32::from(a) & mask == u32::from(n) & mask
        }
        (IpAddr::V6(a), IpAddr::V6(n)) if bits <= 128 => {
            let mask = if bits == 0 { 0 } else { u128::MAX << (128 - bits) };
            u128::from(a) & mask == u128::from(n) & mask
        }
        _ => false,
    }
}

/// Tailscale's own address ranges.
pub fn is_tailnet_ip(ip: IpAddr) -> bool {
    in_prefix(ip, ("100.64.0.0".parse().expect("ip"), 10))
        || in_prefix(ip, ("fd7a:115c:a1e0::".parse().expect("ip"), 48))
}

/// How to reach `host`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Direct,
    /// Only the tailnet can reach it.
    Tailnet,
    /// Inside an accepted subnet route: try directly, then the tailnet.
    DirectThenTailnet,
}

fn route_for(sh: &Shared, host: &str) -> Route {
    if !sh.running || sh.proxy.is_none() {
        return Route::Direct;
    }
    let h = host.trim_end_matches('.').to_ascii_lowercase();
    if let Ok(ip) = h.trim_matches(['[', ']']).parse::<IpAddr>() {
        if is_tailnet_ip(ip) {
            return Route::Tailnet;
        }
        if sh.accept_routes && sh.routes.iter().any(|r| in_prefix(ip, *r)) {
            return Route::DirectThenTailnet;
        }
        return Route::Direct;
    }
    if h.ends_with(".ts.net") || sh.suffix.as_deref().is_some_and(|s| !s.is_empty() && h.ends_with(&format!(".{s}"))) {
        return Route::Tailnet;
    }
    if sh.names.contains(&h) {
        // A peer's short name: the LAN may know it too (`lumen`), so try that first.
        return Route::DirectThenTailnet;
    }
    Route::Direct
}

/// The route [`dial`] would take for `host` right now.
pub fn route(host: &str) -> Route {
    route_for(&SHARED.read().expect("poisoned"), host)
}

/// How long to try directly before falling back to the tailnet.
const DIRECT_FIRST: Duration = Duration::from_millis(2500);

/// Per-host second addresses: lower-case primary → (fallback, last working
/// one was the fallback).
static FALLBACKS: LazyLock<RwLock<HashMap<String, (String, bool)>>> = LazyLock::new(Default::default);

/// Replaces the per-host fallback addresses (primary address → tailnet
/// address). Called whenever the host list changes.
pub fn set_fallbacks(pairs: impl IntoIterator<Item = (String, String)>) {
    let mut f = FALLBACKS.write().expect("poisoned");
    let old = std::mem::take(&mut *f);
    for (primary, alt) in pairs {
        let key = primary.trim().to_ascii_lowercase();
        if key.is_empty() || alt.trim().is_empty() || key == alt.trim().to_ascii_lowercase() {
            continue;
        }
        let was = old.get(&key).is_some_and(|(a, used)| *used && a == alt.trim());
        f.insert(key, (alt.trim().to_owned(), was));
    }
}

/// The address `dial` last reached `host` on, when it has a fallback.
pub fn active_address(host: &str) -> Option<String> {
    let f = FALLBACKS.read().expect("poisoned");
    f.get(&host.to_ascii_lowercase()).map(|(alt, used)| if *used { alt.clone() } else { host.to_owned() })
}

/// Connects to `host:port`. A host with a fallback address is tried on the
/// address that worked last time first (for at most a few seconds), then on
/// the other one, so the same host works on the LAN and away from it.
pub async fn dial(host: &str, port: u16, timeout: Duration) -> io::Result<TcpStream> {
    let fb = FALLBACKS.read().expect("poisoned").get(&host.to_ascii_lowercase()).cloned();
    let Some((alt, used_alt)) = fb else { return dial_one(host, port, timeout).await };
    let (first, second) = if used_alt { (alt.as_str(), host) } else { (host, alt.as_str()) };
    match dial_one(first, port, timeout.min(DIRECT_FIRST + Duration::from_millis(500))).await {
        Ok(s) => Ok(s),
        Err(e) => {
            tracing::debug!(%host, port, "{first} failed ({e}); trying {second}");
            let s = dial_one(second, port, timeout).await?;
            if let Some(v) = FALLBACKS.write().expect("poisoned").get_mut(&host.to_ascii_lowercase()) {
                v.1 = !used_alt;
            }
            Ok(s)
        }
    }
}

/// Connects to `host:port`, through the built-in tailnet when that's the way
/// there (see the module docs).
async fn dial_one(host: &str, port: u16, timeout: Duration) -> io::Result<TcpStream> {
    let (route, proxy) = {
        let sh = SHARED.read().expect("poisoned");
        (route_for(&sh, host), sh.proxy.clone())
    };
    match (route, proxy) {
        (Route::Tailnet, Some(p)) => within(timeout, socks_connect(&p, host, port)).await,
        (Route::DirectThenTailnet, Some(p)) => match within(timeout.min(DIRECT_FIRST), direct(host, port)).await {
            Ok(s) => Ok(s),
            Err(e) => {
                tracing::debug!(%host, port, "direct failed ({e}); trying the tailnet");
                within(timeout, socks_connect(&p, host, port)).await
            }
        },
        _ => within(timeout, direct(host, port)).await,
    }
}

async fn within<T>(t: Duration, f: impl std::future::Future<Output = io::Result<T>>) -> io::Result<T> {
    tokio::time::timeout(t, f).await.map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "connection timed out"))?
}

async fn direct(host: &str, port: u16) -> io::Result<TcpStream> {
    TcpStream::connect((host.trim_matches(['[', ']']), port)).await
}

/// SOCKS5 CONNECT with username/password auth (RFC 1928, RFC 1929).
async fn socks_connect(p: &Proxy, host: &str, port: u16) -> io::Result<TcpStream> {
    let bad = |m: &str| io::Error::other(format!("tailnet: {m}"));
    let mut s = TcpStream::connect(p.addr).await?;
    s.write_all(&[5, 1, 2]).await?;
    let mut r = [0u8; 2];
    s.read_exact(&mut r).await?;
    if r != [5, 2] {
        return Err(bad("proxy refused the login method"));
    }
    let (u, w) = (p.user.as_bytes(), p.pass.as_bytes());
    let mut auth = vec![1, u.len() as u8];
    auth.extend_from_slice(u);
    auth.push(w.len() as u8);
    auth.extend_from_slice(w);
    s.write_all(&auth).await?;
    s.read_exact(&mut r).await?;
    if r[1] != 0 {
        return Err(bad("proxy login failed"));
    }
    let h = host.trim_matches(['[', ']']);
    let mut req = vec![5, 1, 0];
    match h.parse::<IpAddr>() {
        Ok(IpAddr::V4(a)) => {
            req.push(1);
            req.extend_from_slice(&a.octets());
        }
        Ok(IpAddr::V6(a)) => {
            req.push(4);
            req.extend_from_slice(&a.octets());
        }
        Err(_) => {
            if h.len() > 255 {
                return Err(bad("host name too long"));
            }
            req.push(3);
            req.push(h.len() as u8);
            req.extend_from_slice(h.as_bytes());
        }
    }
    req.extend_from_slice(&port.to_be_bytes());
    s.write_all(&req).await?;
    let mut head = [0u8; 4];
    s.read_exact(&mut head).await?;
    if head[1] != 0 {
        let why = match head[1] {
            2 => "not allowed by the tailnet's access rules",
            3 => "network unreachable over the tailnet",
            4 => "host unreachable over the tailnet",
            5 => "connection refused",
            6 => "timed out over the tailnet",
            _ => "the tailnet couldn't connect",
        };
        return Err(io::Error::new(
            if head[1] == 5 { io::ErrorKind::ConnectionRefused } else { io::ErrorKind::Other },
            why,
        ));
    }
    let rest = match head[3] {
        1 => 4 + 2,
        4 => 16 + 2,
        3 => {
            let mut l = [0u8; 1];
            s.read_exact(&mut l).await?;
            usize::from(l[0]) + 2
        }
        _ => return Err(bad("bad proxy reply")),
    };
    let mut skip = vec![0u8; rest];
    s.read_exact(&mut skip).await?;
    let _ = s.set_nodelay(true);
    Ok(s)
}

/// A fresh key for the node state file: base64 of 32 random bytes.
pub fn new_state_key() -> String {
    use base64::Engine as _;
    use ring::rand::SecureRandom as _;
    let mut b = [0u8; 32];
    ring::rand::SystemRandom::new().fill(&mut b).expect("system random");
    base64::engine::general_purpose::STANDARD.encode(b)
}

/// How to start the sidecar.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarConfig {
    /// Base64 of 32 random bytes from the keychain: encrypts the node state.
    pub key: String,
    pub dir: PathBuf,
    pub hostname: String,
    pub accept_routes: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub control_url: String,
}

/// The sidecar binary next to the app's executable (where Tauri bundles
/// external binaries), or `NIGHTGRID_TAILNET_BIN` in development.
pub fn sidecar_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("NIGHTGRID_TAILNET_BIN").map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let exe = std::env::current_exe().ok()?;
    let name = if cfg!(windows) { "nightgrid-tailnet.exe" } else { "nightgrid-tailnet" };
    let p = exe.parent()?.join(name);
    p.is_file().then_some(p)
}

/// A running sidecar. Dropping it (or [`Sidecar::stop`]) ends the process.
pub struct Sidecar {
    child: Child,
    stdin: ChildStdin,
    status: Arc<RwLock<TailnetStatus>>,
}

impl Sidecar {
    /// Starts the sidecar. `on_change` gets every status update.
    pub async fn start(
        bin: &Path,
        cfg: &SidecarConfig,
        on_change: impl Fn(TailnetStatus) + Send + Sync + 'static,
    ) -> io::Result<Self> {
        std::fs::create_dir_all(&cfg.dir)?;
        let mut cmd = Command::new(bin);
        cmd.env("TS_NO_LOGS_NO_SUPPORT", "true")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let mut child = cmd.spawn()?;
        let mut stdin = child.stdin.take().ok_or_else(|| io::Error::other("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| io::Error::other("no stdout"))?;
        let mut line = serde_json::to_string(cfg).map_err(io::Error::other)?;
        line.push('\n');
        stdin.write_all(line.as_bytes()).await?;

        let mut first = TailnetStatus::off("starting");
        first.accept_routes = cfg.accept_routes;
        let status = Arc::new(RwLock::new(first.clone()));
        on_change(first);
        {
            let mut sh = SHARED.write().expect("poisoned");
            *sh = Shared { accept_routes: cfg.accept_routes, ..Default::default() };
        }
        let st = status.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                let Some(ev) = parse_event(&l) else { continue };
                let snapshot = {
                    let mut s = st.write().expect("poisoned");
                    apply(&mut s, ev);
                    s.clone()
                };
                on_change(snapshot);
            }
            // The process ended.
            let snapshot = {
                let mut s = st.write().expect("poisoned");
                if s.state != "off" {
                    s.state = "stopped".into();
                }
                s.login_url = None;
                s.clone()
            };
            *SHARED.write().expect("poisoned") = Shared::default();
            on_change(snapshot);
        });
        Ok(Self { child, stdin, status })
    }

    pub fn status(&self) -> TailnetStatus {
        self.status.read().expect("poisoned").clone()
    }

    async fn send(&mut self, cmd: serde_json::Value) -> io::Result<()> {
        let mut line = cmd.to_string();
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).await
    }

    /// Asks the control server for a login link (arrives as `loginUrl`).
    pub async fn login(&mut self) -> io::Result<()> {
        self.send(serde_json::json!({ "cmd": "login" })).await
    }

    /// Logs this node out of the tailnet (it needs a new login afterwards).
    pub async fn logout(&mut self) -> io::Result<()> {
        self.send(serde_json::json!({ "cmd": "logout" })).await
    }

    pub async fn accept_routes(&mut self, on: bool) -> io::Result<()> {
        SHARED.write().expect("poisoned").accept_routes = on;
        self.status.write().expect("poisoned").accept_routes = on;
        self.send(serde_json::json!({ "cmd": "acceptRoutes", "on": on })).await
    }

    pub async fn stop(mut self) {
        let _ = self.send(serde_json::json!({ "cmd": "quit" })).await;
        if tokio::time::timeout(Duration::from_secs(3), self.child.wait()).await.is_err() {
            let _ = self.child.kill().await;
        }
        self.status.write().expect("poisoned").state = "off".into();
        *SHARED.write().expect("poisoned") = Shared::default();
    }
}

/// Folds one sidecar event into the status and the dialer's view.
fn apply(s: &mut TailnetStatus, ev: Event) {
    match ev {
        Event::Ready { socks, user, pass } => {
            if let Ok(addr) = socks.parse() {
                SHARED.write().expect("poisoned").proxy = Some(Proxy { addr, user, pass });
            }
        }
        Event::Login { url } => {
            if url.starts_with("https://") {
                s.login_url = Some(url);
            }
        }
        Event::State { state } => set_state(s, &state),
        Event::Status { state, auth_url, tailnet, magic_dns_suffix, self_node, peers } => {
            set_state(s, &state);
            if auth_url.starts_with("https://") {
                s.login_url = Some(auth_url);
            }
            s.tailnet = tailnet.filter(|t| !t.is_empty());
            s.magic_dns_suffix = Some(magic_dns_suffix).filter(|m| !m.is_empty());
            s.self_node = self_node;
            s.peers = peers;
            let mut sh = SHARED.write().expect("poisoned");
            sh.suffix = s.magic_dns_suffix.clone();
            sh.names = s
                .peers
                .iter()
                .flat_map(|p| [p.name.to_ascii_lowercase(), p.dns_name.to_ascii_lowercase()])
                .filter(|n| !n.is_empty())
                .collect();
            sh.routes = s.peers.iter().flat_map(|p| p.routes.iter().filter_map(|r| parse_cidr(r))).collect();
        }
        Event::Error { message } => s.error = Some(message),
    }
}

fn set_state(s: &mut TailnetStatus, state: &str) {
    s.state = state_word(state);
    let running = s.state == "running";
    if running {
        s.login_url = None;
        s.error = None;
    }
    SHARED.write().expect("poisoned").running = running;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn falls_back_to_the_second_address_and_remembers_it() {
        let ln = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = ln.local_addr().unwrap().port();
        tokio::spawn(async move { while let Ok((_s, _)) = ln.accept().await {} });
        let primary = "nightgrid-fallback-test.invalid";
        set_fallbacks([(primary.to_string(), "127.0.0.1".to_string())]);
        assert_eq!(active_address(primary).as_deref(), Some(primary));
        dial(primary, port, Duration::from_secs(5)).await.unwrap();
        assert_eq!(active_address(primary).as_deref(), Some("127.0.0.1"));
        // A refresh with the same pair keeps what worked.
        set_fallbacks([(primary.to_string(), "127.0.0.1".to_string())]);
        assert_eq!(active_address(primary).as_deref(), Some("127.0.0.1"));
        dial(primary, port, Duration::from_secs(5)).await.unwrap();
        set_fallbacks([]);
        assert_eq!(active_address(primary), None);
    }

    fn shared(running: bool) -> Shared {
        Shared {
            proxy: Some(Proxy { addr: "127.0.0.1:1".parse().unwrap(), user: "u".into(), pass: "p".into() }),
            running,
            accept_routes: true,
            names: ["lumen".to_string(), "lumen.tail1234.ts.net".to_string()].into_iter().collect(),
            suffix: Some("tail1234.ts.net".into()),
            routes: vec![parse_cidr("192.168.1.0/24").unwrap()],
        }
    }

    #[test]
    fn routes() {
        let sh = shared(true);
        assert_eq!(route_for(&sh, "100.101.7.12"), Route::Tailnet);
        assert_eq!(route_for(&sh, "[fd7a:115c:a1e0::5]"), Route::Tailnet);
        assert_eq!(route_for(&sh, "lumen.tail1234.ts.net."), Route::Tailnet);
        assert_eq!(route_for(&sh, "other.ts.net"), Route::Tailnet);
        assert_eq!(route_for(&sh, "Lumen"), Route::DirectThenTailnet);
        assert_eq!(route_for(&sh, "192.168.1.20"), Route::DirectThenTailnet);
        assert_eq!(route_for(&sh, "192.168.2.20"), Route::Direct);
        assert_eq!(route_for(&sh, "nas.lan"), Route::Direct);
        assert_eq!(route_for(&sh, "100.63.0.1"), Route::Direct);
        // Not running (or off): everything is direct, so a system Tailscale still works.
        assert_eq!(route_for(&shared(false), "100.101.7.12"), Route::Direct);
        assert_eq!(route_for(&Shared::default(), "lumen.tail1234.ts.net"), Route::Direct);
        // Subnet routes only count when accepted.
        let off = Shared { accept_routes: false, ..shared(true) };
        assert_eq!(route_for(&off, "192.168.1.20"), Route::Direct);
    }

    #[test]
    fn prefixes() {
        assert!(is_tailnet_ip("100.127.255.255".parse().unwrap()));
        assert!(!is_tailnet_ip("100.128.0.1".parse().unwrap()));
        assert!(in_prefix("10.0.0.9".parse().unwrap(), parse_cidr("0.0.0.0/0").unwrap()));
        assert!(!in_prefix("10.0.0.9".parse().unwrap(), parse_cidr("fd00::/8").unwrap()));
    }

    #[test]
    fn events() {
        assert_eq!(
            parse_event(r#"{"ev":"ready","socks":"127.0.0.1:4100","user":"a","pass":"b"}"#),
            Some(Event::Ready { socks: "127.0.0.1:4100".into(), user: "a".into(), pass: "b".into() })
        );
        let line = r#"{"ev":"status","state":"Running","authUrl":"","tailnet":"me@example.com","magicDnsSuffix":"tail1234.ts.net","self":{"name":"nightgrid","dnsName":"nightgrid.tail1234.ts.net","ips":["100.70.1.2"],"os":"macOS","online":true,"routes":[]},"peers":[{"name":"lumen","dnsName":"lumen.tail1234.ts.net","ips":["100.101.7.12"],"os":"linux","online":true,"routes":["192.168.1.0/24"]}]}"#;
        let mut s = TailnetStatus::off("starting");
        apply(&mut s, parse_event(line).unwrap());
        assert_eq!(s.state, "running");
        assert_eq!(s.tailnet.as_deref(), Some("me@example.com"));
        assert_eq!(s.peers[0].routes, vec!["192.168.1.0/24"]);
        assert_eq!(s.self_node.as_ref().unwrap().routes, Vec::<String>::new());
        apply(&mut s, parse_event(r#"{"ev":"login","url":"javascript:alert(1)"}"#).unwrap());
        assert_eq!(s.login_url, None);
        apply(&mut s, parse_event(r#"{"ev":"state","state":"NeedsLogin"}"#).unwrap());
        assert_eq!(s.state, "needsLogin");
        assert!(parse_event("not json").is_none());
        *SHARED.write().unwrap() = Shared::default();
    }

    /// A tiny SOCKS5 server: checks the login, then answers with a fixed byte.
    #[tokio::test]
    async fn socks_client_talks_rfc1928() {
        let ln = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = ln.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut s, _) = ln.accept().await.unwrap();
            let mut b = [0u8; 3];
            s.read_exact(&mut b).await.unwrap();
            assert_eq!(b, [5, 1, 2]);
            s.write_all(&[5, 2]).await.unwrap();
            let mut v = [0u8; 2];
            s.read_exact(&mut v).await.unwrap();
            let mut u = vec![0u8; v[1] as usize];
            s.read_exact(&mut u).await.unwrap();
            let mut pl = [0u8; 1];
            s.read_exact(&mut pl).await.unwrap();
            let mut p = vec![0u8; pl[0] as usize];
            s.read_exact(&mut p).await.unwrap();
            assert_eq!((u.as_slice(), p.as_slice()), (&b"user"[..], &b"secret"[..]));
            s.write_all(&[1, 0]).await.unwrap();
            let mut h = [0u8; 5];
            s.read_exact(&mut h).await.unwrap();
            assert_eq!(&h[..4], &[5, 1, 0, 3]);
            let mut name = vec![0u8; h[4] as usize + 2];
            s.read_exact(&mut name).await.unwrap();
            assert_eq!(&name[..h[4] as usize], b"lumen.tail1234.ts.net");
            assert_eq!(u16::from_be_bytes([name[name.len() - 2], name[name.len() - 1]]), 22);
            s.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await.unwrap();
            s.write_all(b"!").await.unwrap();
        });
        let p = Proxy { addr, user: "user".into(), pass: "secret".into() };
        let mut s = socks_connect(&p, "lumen.tail1234.ts.net", 22).await.unwrap();
        let mut one = [0u8; 1];
        s.read_exact(&mut one).await.unwrap();
        assert_eq!(&one, b"!");
        server.await.unwrap();
    }
}
