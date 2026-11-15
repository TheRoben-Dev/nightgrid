//! The forwarding loops over an authenticated [`Connection`].
//!
//! Each `serve_*` runs until the listener fails, the SSH connection closes or
//! the caller drops the future (that's how a tunnel is stopped), and returns
//! why it ended. Per-connection copy tasks are aborted with it.

use std::pin::Pin;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use russh::client::Msg;
use russh::Channel;
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use super::socks;
use crate::ssh::client::Connection;

/// How often a loop checks whether the SSH connection died.
const ALIVE_CHECK: Duration = Duration::from_secs(2);

/// Live traffic counters for one tunnel.
#[derive(Debug, Default)]
pub struct Stats {
    /// Bytes from the far side to the local side.
    pub rx: AtomicU64,
    /// Bytes from the local side to the far side.
    pub tx: AtomicU64,
    pub active: AtomicU32,
    pub total: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
/// u64 counters are typed `u32` for TypeScript (plain `number`); values above
/// 4 GiB still serialize exactly up to 2^53.
pub struct StatsView {
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub rx: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub tx: u64,
    pub active: u32,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub total: u64,
}

impl Stats {
    pub fn view(&self) -> StatsView {
        StatsView {
            rx: self.rx.load(Ordering::Relaxed),
            tx: self.tx.load(Ordering::Relaxed),
            active: self.active.load(Ordering::Relaxed),
            total: self.total.load(Ordering::Relaxed),
        }
    }
}

/// Counts bytes through the local end of a forwarded connection: reads are
/// `tx` (going out), writes are `rx` (coming back).
struct Counted<S> {
    inner: S,
    stats: Arc<Stats>,
}

impl<S: AsyncRead + Unpin> AsyncRead for Counted<S> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let r = Pin::new(&mut self.inner).poll_read(cx, buf);
        let n = buf.filled().len() - before;
        self.stats.tx.fetch_add(n as u64, Ordering::Relaxed);
        r
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Counted<S> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<std::io::Result<usize>> {
        let r = Pin::new(&mut self.inner).poll_write(cx, buf);
        if let Poll::Ready(Ok(n)) = &r {
            self.stats.rx.fetch_add(*n as u64, Ordering::Relaxed);
        }
        r
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Copies both ways until either side closes, keeping `active` accurate.
async fn pipe(local: TcpStream, channel: Channel<Msg>, stats: Arc<Stats>) {
    stats.active.fetch_add(1, Ordering::Relaxed);
    stats.total.fetch_add(1, Ordering::Relaxed);
    let _ = local.set_nodelay(true);
    let mut local = Counted { inner: local, stats: stats.clone() };
    let mut remote = channel.into_stream();
    let _ = tokio::io::copy_bidirectional(&mut local, &mut remote).await;
    stats.active.fetch_sub(1, Ordering::Relaxed);
}

/// Binds a local listener (so a busy port is reported before "running").
pub async fn listen(addr: &str, port: u16) -> Result<TcpListener, String> {
    TcpListener::bind((addr, port)).await.map_err(|e| match e.kind() {
        std::io::ErrorKind::AddrInUse => format!("port {port} on {addr} is already in use"),
        std::io::ErrorKind::PermissionDenied => {
            format!("not allowed to listen on {addr}:{port} (ports below 1024 need admin rights)")
        }
        _ => format!("can't listen on {addr}:{port}: {e}"),
    })
}

/// `-L`: each local connection opens a `direct-tcpip` channel to `dest`.
pub async fn serve_local(
    conn: Arc<Connection>,
    listener: TcpListener,
    dest_host: String,
    dest_port: u16,
    stats: Arc<Stats>,
) -> String {
    let mut tasks = JoinSet::new();
    let mut alive = tokio::time::interval(ALIVE_CHECK);
    loop {
        tokio::select! {
            r = listener.accept() => {
                let (sock, peer) = match r {
                    Ok(x) => x,
                    Err(e) => return format!("listener failed: {e}"),
                };
                let conn = conn.clone();
                let stats = stats.clone();
                let (host, port) = (dest_host.clone(), dest_port);
                tasks.spawn(async move {
                    match conn.handle.channel_open_direct_tcpip(host.clone(), port.into(), peer.ip().to_string(), peer.port().into()).await {
                        Ok(ch) => pipe(sock, ch, stats).await,
                        Err(e) => tracing::debug!(%host, port, "forward refused: {e}"),
                    }
                });
            }
            _ = alive.tick() => {
                if conn.is_closed() {
                    return "SSH connection closed".into();
                }
                while tasks.try_join_next().is_some() {}
            }
        }
    }
}

/// `-D`: a SOCKS5 proxy; each CONNECT opens a `direct-tcpip` channel.
pub async fn serve_dynamic(conn: Arc<Connection>, listener: TcpListener, stats: Arc<Stats>) -> String {
    let mut tasks = JoinSet::new();
    let mut alive = tokio::time::interval(ALIVE_CHECK);
    loop {
        tokio::select! {
            r = listener.accept() => {
                let (mut sock, peer) = match r {
                    Ok(x) => x,
                    Err(e) => return format!("listener failed: {e}"),
                };
                let conn = conn.clone();
                let stats = stats.clone();
                tasks.spawn(async move {
                    let Ok(Ok((host, port))) = tokio::time::timeout(Duration::from_secs(10), socks::accept(&mut sock)).await else {
                        return;
                    };
                    match conn.handle.channel_open_direct_tcpip(host.clone(), port.into(), peer.ip().to_string(), peer.port().into()).await {
                        Ok(ch) => {
                            if socks::reply(&mut sock, socks::REP_OK).await.is_ok() {
                                pipe(sock, ch, stats).await;
                            }
                        }
                        Err(e) => {
                            tracing::debug!(%host, port, "socks connect refused: {e}");
                            let code = if matches!(e, russh::Error::ChannelOpenFailure(_)) { socks::REP_HOST_UNREACHABLE } else { socks::REP_FAILURE };
                            let _ = socks::reply(&mut sock, code).await;
                        }
                    }
                });
            }
            _ = alive.tick() => {
                if conn.is_closed() {
                    return "SSH connection closed".into();
                }
                while tasks.try_join_next().is_some() {}
            }
        }
    }
}

/// A granted remote forward. Dropping it cancels the forward on the server.
pub struct RemoteForward {
    conn: Arc<Connection>,
    bind_addr: String,
    /// The port the server listens on (the one asked for, or the one it picked).
    pub port: u32,
    rx: mpsc::UnboundedReceiver<Channel<Msg>>,
}

impl Drop for RemoteForward {
    fn drop(&mut self) {
        self.conn.forwards.lock().expect("poisoned").remove(&self.port);
        let conn = self.conn.clone();
        let (addr, port) = (self.bind_addr.clone(), self.port);
        if let Ok(rt) = tokio::runtime::Handle::try_current() {
            rt.spawn(async move {
                let _ = conn.handle.cancel_tcpip_forward(addr, port).await;
            });
        }
    }
}

/// `-R`: asks the server to listen on `bind_addr:bind_port` (0 = any port).
pub async fn request_remote(conn: Arc<Connection>, bind_addr: &str, bind_port: u16) -> Result<RemoteForward, String> {
    let granted = conn.handle.tcpip_forward(bind_addr.to_owned(), bind_port.into()).await.map_err(|e| {
        format!("server refused to listen on {bind_addr}:{bind_port}: {e} (check AllowTcpForwarding / GatewayPorts)")
    })?;
    // OpenSSH answers 0 when the requested port was non-zero.
    let port = if bind_port != 0 { u32::from(bind_port) } else { granted };
    let (tx, rx) = mpsc::unbounded_channel();
    conn.forwards.lock().expect("poisoned").insert(port, tx);
    Ok(RemoteForward { conn, bind_addr: bind_addr.to_owned(), port, rx })
}

/// Serves a granted remote forward: each server-side connection is piped to
/// `dest_host:dest_port` as reached from this machine.
pub async fn serve_remote(mut fwd: RemoteForward, dest_host: String, dest_port: u16, stats: Arc<Stats>) -> String {
    let mut tasks = JoinSet::new();
    let mut alive = tokio::time::interval(ALIVE_CHECK);
    loop {
        tokio::select! {
            ch = fwd.rx.recv() => {
                let Some(ch) = ch else { return "forward closed".into() };
                let stats = stats.clone();
                let host = dest_host.clone();
                tasks.spawn(async move {
                    match tokio::time::timeout(Duration::from_secs(10), TcpStream::connect((host.as_str(), dest_port))).await {
                        Ok(Ok(sock)) => pipe(sock, ch, stats).await,
                        Ok(Err(e)) => tracing::debug!(%host, dest_port, "remote forward target unreachable: {e}"),
                        Err(_) => tracing::debug!(%host, dest_port, "remote forward target timed out"),
                    }
                });
            }
            _ = alive.tick() => {
                if fwd.conn.is_closed() {
                    return "SSH connection closed".into();
                }
                while tasks.try_join_next().is_some() {}
            }
        }
    }
}
