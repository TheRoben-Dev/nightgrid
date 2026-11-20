//! Docker Engine API over the host's SSH connection.
//!
//! Every API request gets its own SSH channel: a `direct-streamlocal` channel
//! to the daemon's Unix socket (like `ssh -L x:/var/run/docker.sock`), or,
//! when sshd refuses socket forwarding, an exec channel running
//! `docker system dial-stdio` (as the login user, or through passwordless
//! sudo when only root may use the socket). HTTP/1.1 is spoken over the channel with
//! hyper, and bollard does the API (de)serialisation on top.

use std::sync::Arc;

use bollard::{BollardRequest, Docker, API_DEFAULT_VERSION};
use hyper::body::Incoming;
use hyper::header::{HeaderValue, HOST};
use hyper::{Response, Uri};
use hyper_util::rt::TokioIo;

use crate::files::path::shell_quote;
use crate::ssh::client::Connection;

/// Shell prefix for every docker CLI call: some systems (Synology DSM) keep
/// `docker` in /usr/local/bin, which non-interactive SSH sessions leave out of
/// `PATH`.
pub const PATH_FIX: &str = r#"PATH="$PATH:/usr/local/bin:/usr/local/sbin"; export PATH; "#;

const DEFAULT_SOCKET: &str = "/var/run/docker.sock";

/// How API requests reach the daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// `direct-streamlocal` channel to this socket path.
    Socket(String),
    /// Exec channel running `docker system dial-stdio`; `Some(socket)` sets
    /// `DOCKER_HOST` for a non-default (e.g. rootless) socket.
    DialStdio(Option<String>),
    /// `sudo -n <docker> system dial-stdio`: the user may not open the socket
    /// but has passwordless sudo. Holds the docker binary's absolute path
    /// (sudo's secure_path may not include it); see [`safe_bin`].
    Sudo(String),
}

impl Route {
    pub fn describe(&self) -> String {
        match self {
            Route::Socket(p) => format!("socket {p} over SSH"),
            Route::DialStdio(_) => "docker system dial-stdio over SSH".into(),
            Route::Sudo(_) => "sudo docker system dial-stdio over SSH".into(),
        }
    }

    /// Shell prefix that lets the docker CLI find the binary and this daemon.
    pub fn shell_env(&self) -> String {
        match self {
            Route::Socket(s) | Route::DialStdio(Some(s)) if s != DEFAULT_SOCKET => {
                format!("{PATH_FIX}export DOCKER_HOST={}; ", shell_quote(&format!("unix://{s}")))
            }
            _ => PATH_FIX.to_owned(),
        }
    }

    /// The docker CLI for this route (`docker`, or `sudo -n /path/docker`).
    pub fn docker_cli(&self) -> String {
        match self {
            // `safe_bin` paths need no quoting, so the result also survives
            // word splitting in `$dc` (compose).
            Route::Sudo(bin) => format!("sudo -n {bin}"),
            _ => "docker".into(),
        }
    }
}

/// An absolute binary path made only of characters that need no shell
/// quoting (what `command -v docker` prints on real systems).
pub fn safe_bin(p: &str) -> bool {
    p.starts_with('/') && p.len() < 256 && p.chars().all(|c| c.is_ascii_alphanumeric() || "/._-+".contains(c))
}

/// Command that starts the stdio bridge for the exec routes.
pub(crate) fn dial_stdio_command(route: &Route) -> String {
    format!("{}exec {} system dial-stdio", route.shell_env(), route.docker_cli())
}

fn io_err(msg: impl std::fmt::Display) -> bollard::errors::Error {
    bollard::errors::Error::IOError { err: std::io::Error::other(msg.to_string()) }
}

async fn send(
    conn: Arc<Connection>,
    route: Route,
    mut req: BollardRequest,
) -> Result<Response<Incoming>, bollard::errors::Error> {
    let channel = match &route {
        Route::Socket(path) => conn.open_streamlocal(path).await,
        Route::DialStdio(_) | Route::Sudo(_) => conn.open_exec(&dial_stdio_command(&route)).await,
    }
    .map_err(|e| io_err(format!("could not open Docker channel ({}): {e}", route.describe())))?;

    let io = TokioIo::new(channel.into_stream());
    let (mut sender, connection) = hyper::client::conn::http1::handshake(io).await?;
    tokio::spawn(async move {
        // Ends once the response body has been read (or dropped).
        if let Err(e) = connection.await {
            tracing::debug!("docker connection ended: {e}");
        }
    });

    // bollard builds absolute URIs (`http://docker/v1.x/...`); send origin
    // form with a Host header like the docker CLI does.
    let path = req.uri().path_and_query().map(|p| p.as_str().to_owned()).unwrap_or_else(|| "/".into());
    *req.uri_mut() = path.parse::<Uri>().map_err(io_err)?;
    req.headers_mut().insert(HOST, HeaderValue::from_static("docker"));
    Ok(sender.send_request(req).await?)
}

/// A bollard client whose requests travel over `conn` via `route`.
pub fn client(conn: Arc<Connection>, route: Route, timeout_secs: u64) -> Result<Docker, bollard::errors::Error> {
    let transport = move |req: BollardRequest| send(conn.clone(), route.clone(), req);
    Docker::connect_with_custom_transport(transport, Some("http://docker"), timeout_secs, API_DEFAULT_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dial_stdio_sets_docker_host_only_for_other_sockets() {
        assert_eq!(dial_stdio_command(&Route::DialStdio(None)), format!("{PATH_FIX}exec docker system dial-stdio"));
        assert_eq!(
            dial_stdio_command(&Route::DialStdio(Some("/var/run/docker.sock".into()))),
            format!("{PATH_FIX}exec docker system dial-stdio")
        );
        assert_eq!(
            dial_stdio_command(&Route::DialStdio(Some("/run/user/1000/docker.sock".into()))),
            format!("{PATH_FIX}export DOCKER_HOST='unix:///run/user/1000/docker.sock'; exec docker system dial-stdio")
        );
    }

    #[test]
    fn sudo_route_uses_the_absolute_binary() {
        let r = Route::Sudo("/usr/local/bin/docker".into());
        assert_eq!(dial_stdio_command(&r), format!("{PATH_FIX}exec sudo -n /usr/local/bin/docker system dial-stdio"));
        assert_eq!(r.docker_cli(), "sudo -n /usr/local/bin/docker");
        assert!(safe_bin("/usr/local/bin/docker"));
        assert!(!safe_bin("docker"));
        assert!(!safe_bin("/opt/my bin/docker"));
        assert!(!safe_bin("/x;reboot"));
    }

    #[test]
    fn path_fix_adds_usr_local() {
        assert!(PATH_FIX.contains("/usr/local/bin"));
        assert!(PATH_FIX.ends_with("export PATH; "));
    }
}
