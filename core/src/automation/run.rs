//! Runs one step on one host's connection. The app drives hosts in parallel
//! and steps in order; these functions only do the work and report lines.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;

use super::template::quote_value;
use super::{valid_docker_target, DockerAction};
use crate::files::remote::RemoteFs;
use crate::ssh::client::Connection;
use crate::system::sudo::{self, Stream, SudoAuth};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum OutStream {
    Out,
    Err,
    /// Nightgrid's own notes ("uploaded 2.1 KB", "port open after 3 s").
    Info,
}

/// Runs `command` with `sh` (as root when `auth` says so), streaming lines
/// until it exits or `cancel` flips. `None` means it was stopped.
pub async fn command(
    conn: &Connection,
    auth: &SudoAuth,
    command: &str,
    cancel: watch::Receiver<bool>,
    on_line: &mut (dyn FnMut(OutStream, String) + Send),
) -> Result<Option<i32>, String> {
    let mut f = |s: Stream, l: String| {
        on_line(if s == Stream::Out { OutStream::Out } else { OutStream::Err }, l);
        true
    };
    sudo::stream(conn, auth, command, Some(cancel), &mut f).await.map_err(|e| e.to_string())
}

/// `docker <verb> <target>`, refusing anything that isn't a plain name.
pub fn docker_command(action: DockerAction, target: &str) -> Result<String, String> {
    let t = target.trim();
    if !valid_docker_target(action, t) {
        return Err(format!("not a valid {}: {t}", if action == DockerAction::Pull { "image" } else { "container" }));
    }
    Ok(format!("docker {} {}", action.verb(), quote_value(t)))
}

/// Waits until `host:port` accepts a TCP connection as seen from the server
/// (an SSH `direct-tcpip` channel, so nothing needs installing there).
/// Returns how long it took.
pub async fn wait_port(
    conn: &Connection,
    host: &str,
    port: u16,
    timeout: Duration,
    mut cancel: watch::Receiver<bool>,
) -> Result<Option<Duration>, String> {
    let start = Instant::now();
    loop {
        let attempt = tokio::time::timeout(
            Duration::from_secs(5),
            conn.handle.channel_open_direct_tcpip(host, port.into(), "127.0.0.1", 0),
        )
        .await;
        match attempt {
            Ok(Ok(ch)) => {
                let _ = ch.close().await;
                return Ok(Some(start.elapsed()));
            }
            Ok(Err(russh::Error::ChannelOpenFailure(russh::ChannelOpenFailure::AdministrativelyProhibited))) => {
                return Err(
                    "the SSH server doesn't allow TCP forwarding (AllowTcpForwarding), so the port can't be checked"
                        .into(),
                );
            }
            _ => {}
        }
        if start.elapsed() >= timeout {
            return Err(format!("{host}:{port} didn't accept connections within {} s", timeout.as_secs()));
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
            _ = cancel.changed() => return Ok(None),
        }
    }
}

/// Copies a local file to `remote` over SFTP as the login user. Returns the
/// byte count.
pub async fn upload(conn: Arc<Connection>, local: &Path, remote: &str, mode: Option<u32>) -> Result<u64, String> {
    let mut src = tokio::fs::File::open(local).await.map_err(|e| format!("{}: {e}", local.display()))?;
    if !src.metadata().await.map_err(|e| e.to_string())?.is_file() {
        return Err(format!("{} is not a file", local.display()));
    }
    let fs = RemoteFs::open(conn).await.map_err(|e| e.to_string())?;
    let mut dst = fs.open_write_with(remote, None, mode, false).await.map_err(|e| e.to_string())?;
    let n = tokio::io::copy(&mut src, &mut dst).await.map_err(|e| format!("{remote}: {e}"))?;
    dst.shutdown().await.map_err(|e| format!("{remote}: {e}"))?;
    drop(dst);
    // An existing file keeps its old permissions on open; set them explicitly.
    if let Some(m) = mode {
        fs.set_mode(remote, m).await.map_err(|e| e.to_string())?;
    }
    Ok(n)
}

/// "2.1 KB" style sizes for info lines.
pub fn human_bytes(n: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", U[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_and_sizes() {
        assert_eq!(docker_command(DockerAction::Restart, " web-1 ").unwrap(), "docker restart web-1");
        assert_eq!(
            docker_command(DockerAction::Pull, "ghcr.io/org/app:1.2").unwrap(),
            "docker pull ghcr.io/org/app:1.2"
        );
        assert!(docker_command(DockerAction::Stop, "a;b").is_err());
        assert!(docker_command(DockerAction::Pull, "-x").is_err());
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2150), "2.1 KB");
    }
}
