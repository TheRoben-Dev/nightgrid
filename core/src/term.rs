//! Terminal sessions: local PTYs (portable-pty) and SSH shells with automatic
//! reconnect. UI-agnostic: output goes to a [`Sink`] callback, input arrives
//! through a [`TermHandle`].

use std::future::Future;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use russh::ChannelMsg;
use serde::Serialize;
use tokio::sync::mpsc;

use crate::ssh::client::{ConnectError, Connection};
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum TermState {
    Connecting,
    Connected,
    Reconnecting,
    /// Gave up or needs the user (e.g. host key changed). Can be retried.
    Disconnected,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TermEvent {
    Data(Vec<u8>),
    Status { state: TermState, attempt: u32, message: Option<String> },
    Exit { code: Option<i32> },
}

pub type Sink = Arc<dyn Fn(TermEvent) + Send + Sync>;

#[derive(Debug)]
enum Cmd {
    Write(Vec<u8>),
    Resize(u16, u16),
    Reconnect,
    Close,
}

/// Input side of a running terminal. Dropping it closes the session.
#[derive(Debug, Clone)]
pub struct TermHandle {
    tx: mpsc::UnboundedSender<Cmd>,
}

impl TermHandle {
    pub fn write(&self, data: impl Into<Vec<u8>>) {
        let _ = self.tx.send(Cmd::Write(data.into()));
    }
    pub fn resize(&self, cols: u16, rows: u16) {
        let _ = self.tx.send(Cmd::Resize(cols.max(1), rows.max(1)));
    }
    /// Retry now (SSH sessions in `Reconnecting`/`Disconnected`).
    pub fn reconnect(&self) {
        let _ = self.tx.send(Cmd::Reconnect);
    }
    pub fn close(&self) {
        let _ = self.tx.send(Cmd::Close);
    }
}

// ───────────────────────────── local shells ─────────────────────────────

/// The user's default interactive shell for this OS.
pub fn default_shell() -> (String, Vec<String>) {
    #[cfg(windows)]
    {
        let pwsh = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).any(|d| d.join("pwsh.exe").is_file()))
            .unwrap_or(false);
        (if pwsh { "pwsh.exe" } else { "powershell.exe" }.into(), vec!["-NoLogo".into()])
    }
    #[cfg(target_os = "macos")]
    {
        (std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into()), vec!["-l".into()])
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        (std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into()), vec![])
    }
}

fn pty_size(cols: u16, rows: u16) -> PtySize {
    PtySize { rows: rows.max(1), cols: cols.max(1), pixel_width: 0, pixel_height: 0 }
}

/// Spawns a local shell in a PTY.
pub fn spawn_local(
    program: Option<(String, Vec<String>)>,
    cwd: Option<PathBuf>,
    cols: u16,
    rows: u16,
    sink: Sink,
) -> Result<TermHandle> {
    let pty_err = |e: anyhow_like::Error| Error::Io(std::io::Error::other(e.0));
    let pair = native_pty_system().openpty(pty_size(cols, rows)).map_err(|e| pty_err(e.into()))?;
    let (prog, args) = program.unwrap_or_else(default_shell);
    let mut cmd = CommandBuilder::new(&prog);
    cmd.args(&args);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("TERM_PROGRAM", "nightgrid");
    if let Some(dir) = cwd.or_else(std::env::home_dir) {
        cmd.cwd(dir);
    }
    let mut child = pair.slave.spawn_command(cmd).map_err(|e| pty_err(e.into()))?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().map_err(|e| pty_err(e.into()))?;
    let mut writer = pair.master.take_writer().map_err(|e| pty_err(e.into()))?;
    let mut killer = child.clone_killer();
    let master = pair.master;

    let (tx, mut rx) = mpsc::unbounded_channel::<Cmd>();
    sink(TermEvent::Status { state: TermState::Connected, attempt: 0, message: Some(prog.clone()) });

    // Output: blocking reads on a dedicated thread.
    let out_sink = sink.clone();
    std::thread::Builder::new().name("pty-reader".into()).spawn(move || {
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => out_sink(TermEvent::Data(buf[..n].to_vec())),
            }
        }
        let code = child.wait().ok().map(|s| s.exit_code() as i32);
        out_sink(TermEvent::Exit { code });
    })?;

    // Input and control.
    std::thread::Builder::new().name("pty-writer".into()).spawn(move || {
        while let Some(cmd) = rx.blocking_recv() {
            match cmd {
                Cmd::Write(b) => {
                    if writer.write_all(&b).and_then(|_| writer.flush()).is_err() {
                        break;
                    }
                }
                Cmd::Resize(c, r) => {
                    let _ = master.resize(pty_size(c, r));
                }
                Cmd::Reconnect => {}
                Cmd::Close => break,
            }
        }
        let _ = killer.kill();
    })?;

    Ok(TermHandle { tx })
}

/// Small adapter so we don't depend on `anyhow` directly.
mod anyhow_like {
    pub struct Error(pub String);
    impl<E: std::fmt::Display> From<E> for Error {
        fn from(e: E) -> Self {
            Self(e.to_string())
        }
    }
}

// ───────────────────────────── SSH shells ─────────────────────────────

pub type ConnectFuture = Pin<Box<dyn Future<Output = std::result::Result<Arc<Connection>, ConnectError>> + Send>>;
/// Produces a (possibly shared) live connection. `true` means "the previous
/// one died, make a fresh one".
pub type Connector = Arc<dyn Fn(bool) -> ConnectFuture + Send + Sync>;

const MAX_BACKOFF: Duration = Duration::from_secs(30);

fn backoff(attempt: u32) -> Duration {
    Duration::from_secs(1u64 << attempt.min(5).saturating_sub(1)).min(MAX_BACKOFF)
}

/// Connects (errors are returned so the UI can prompt), then runs the shell in
/// a background task that reconnects with backoff when the connection drops.
pub async fn spawn_ssh(
    connector: Connector,
    cols: u16,
    rows: u16,
    sink: Sink,
) -> std::result::Result<TermHandle, ConnectError> {
    sink(TermEvent::Status { state: TermState::Connecting, attempt: 0, message: None });
    let conn = connector(false).await?;
    let channel = conn.open_shell(cols, rows).await?;
    sink(TermEvent::Status { state: TermState::Connected, attempt: 0, message: Some(conn.label.clone()) });
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(run_ssh(connector, conn, channel, cols, rows, rx, sink));
    Ok(TermHandle { tx })
}

enum Outcome {
    /// User closed the tab.
    Closed,
    /// Remote shell exited normally.
    Exited(Option<i32>),
    /// Transport died; reconnect.
    Lost,
}

async fn pump(
    channel: &mut russh::Channel<russh::client::Msg>,
    conn: &Connection,
    rx: &mut mpsc::UnboundedReceiver<Cmd>,
    size: &mut (u16, u16),
    sink: &Sink,
) -> Outcome {
    let mut exit_code: Option<i32> = None;
    loop {
        tokio::select! {
            msg = channel.wait() => match msg {
                Some(ChannelMsg::Data { data }) => sink(TermEvent::Data(data.to_vec())),
                Some(ChannelMsg::ExtendedData { data, .. }) => sink(TermEvent::Data(data.to_vec())),
                Some(ChannelMsg::ExitStatus { exit_status }) => exit_code = Some(exit_status as i32),
                Some(ChannelMsg::Eof) => {}
                Some(ChannelMsg::Close) | None => {
                    return if exit_code.is_some() || !conn.is_closed() { Outcome::Exited(exit_code) } else { Outcome::Lost };
                }
                Some(_) => {}
            },
            cmd = rx.recv() => match cmd {
                Some(Cmd::Write(b)) => {
                    if channel.data(&b[..]).await.is_err() && conn.is_closed() {
                        return Outcome::Lost;
                    }
                }
                Some(Cmd::Resize(c, r)) => {
                    *size = (c, r);
                    let _ = channel.window_change(c.into(), r.into(), 0, 0).await;
                }
                Some(Cmd::Reconnect) => {}
                Some(Cmd::Close) | None => {
                    let _ = channel.close().await;
                    return Outcome::Closed;
                }
            }
        }
    }
}

async fn run_ssh(
    connector: Connector,
    mut conn: Arc<Connection>,
    mut channel: russh::Channel<russh::client::Msg>,
    cols: u16,
    rows: u16,
    mut rx: mpsc::UnboundedReceiver<Cmd>,
    sink: Sink,
) {
    let mut size = (cols, rows);
    'session: loop {
        match pump(&mut channel, &conn, &mut rx, &mut size, &sink).await {
            Outcome::Closed => return,
            Outcome::Exited(code) => {
                sink(TermEvent::Exit { code });
                return;
            }
            Outcome::Lost => {}
        }
        // Reconnect loop.
        let mut attempt = 0u32;
        let mut wait_for_user = false;
        loop {
            if wait_for_user {
                // Wait for an explicit retry or close.
                loop {
                    match rx.recv().await {
                        Some(Cmd::Reconnect) => break,
                        Some(Cmd::Close) | None => return,
                        _ => {}
                    }
                }
                wait_for_user = false;
                attempt = 0;
            } else {
                attempt += 1;
                let delay = backoff(attempt);
                sink(TermEvent::Status {
                    state: TermState::Reconnecting,
                    attempt,
                    message: Some(format!("connection lost, retrying in {}s", delay.as_secs())),
                });
                let sleep = tokio::time::sleep(delay);
                tokio::pin!(sleep);
                loop {
                    tokio::select! {
                        _ = &mut sleep => break,
                        cmd = rx.recv() => match cmd {
                            Some(Cmd::Reconnect) => break,
                            Some(Cmd::Close) | None => return,
                            _ => {} // input while offline is dropped
                        }
                    }
                }
            }
            let result = async {
                let c = connector(true).await?;
                let ch = c.open_shell(size.0, size.1).await?;
                Ok::<_, ConnectError>((c, ch))
            }
            .await;
            match result {
                Ok((c, ch)) => {
                    conn = c;
                    channel = ch;
                    sink(TermEvent::Data(b"\r\n\x1b[2m[nightgrid] reconnected\x1b[0m\r\n".to_vec()));
                    sink(TermEvent::Status { state: TermState::Connected, attempt, message: Some(conn.label.clone()) });
                    continue 'session;
                }
                Err(e) if e.needs_user() => {
                    sink(TermEvent::Status { state: TermState::Disconnected, attempt, message: Some(e.to_string()) });
                    wait_for_user = true;
                }
                Err(e) => tracing::info!(attempt, "reconnect failed: {e}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(backoff(1), Duration::from_secs(1));
        assert_eq!(backoff(2), Duration::from_secs(2));
        assert_eq!(backoff(4), Duration::from_secs(8));
        assert_eq!(backoff(10), Duration::from_secs(16));
    }

    #[cfg(unix)]
    #[test]
    fn local_pty_roundtrip() {
        let out = Arc::new(Mutex::new(Vec::<u8>::new()));
        let exited = Arc::new(Mutex::new(None));
        let (o, e) = (out.clone(), exited.clone());
        let sink: Sink = Arc::new(move |ev| match ev {
            TermEvent::Data(d) => o.lock().unwrap().extend(d),
            TermEvent::Exit { code } => *e.lock().unwrap() = Some(code),
            _ => {}
        });
        let h = spawn_local(Some(("/bin/sh".into(), vec![])), None, 80, 24, sink).unwrap();
        h.resize(100, 30);
        h.write("echo nightgrid-$((40+2)); stty size; exit 3\n");
        let start = std::time::Instant::now();
        while exited.lock().unwrap().is_none() && start.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(20));
        }
        let text = String::from_utf8_lossy(&out.lock().unwrap()).into_owned();
        assert!(text.contains("nightgrid-42"), "{text}");
        assert!(text.contains("30 100"), "resize applied: {text}");
        assert_eq!(*exited.lock().unwrap(), Some(Some(3)));
    }
}
