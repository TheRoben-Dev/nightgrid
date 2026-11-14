//! Docker Engine API over the host's SSH connection.
//!
//! Every API request gets its own SSH channel: a `direct-streamlocal` channel
//! to the daemon's Unix socket (like `ssh -L x:/var/run/docker.sock`), or,
//! when sshd refuses socket forwarding, an exec channel running
//! `docker system dial-stdio`. HTTP/1.1 is spoken over the channel with
//! hyper, and bollard does the API (de)serialisation on top.

use std::sync::Arc;

use bollard::{BollardRequest, Docker, API_DEFAULT_VERSION};
use hyper::body::Incoming;
use hyper::header::{HeaderValue, HOST};
use hyper::{Response, Uri};
use hyper_util::rt::TokioIo;

use crate::files::path::shell_quote;
use crate::ssh::client::Connection;

/// How API requests reach the daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// `direct-streamlocal` channel to this socket path.
    Socket(String),
    /// Exec channel running `docker system dial-stdio`; `Some(socket)` sets
    /// `DOCKER_HOST` for a non-default (e.g. rootless) socket.
    DialStdio(Option<String>),
}

impl Route {
    pub fn describe(&self) -> String {
        match self {
            Route::Socket(p) => format!("socket {p} over SSH"),
            Route::DialStdio(_) => "docker system dial-stdio over SSH".into(),
        }
    }
}

/// Command that starts the stdio bridge for `Route::DialStdio`.
pub(crate) fn dial_stdio_command(socket: Option<&str>) -> String {
    match socket {
        Some(s) if s != "/var/run/docker.sock" => {
            format!("DOCKER_HOST={} exec docker system dial-stdio", shell_quote(&format!("unix://{s}")))
        }
        _ => "exec docker system dial-stdio".into(),
    }
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
        Route::DialStdio(sock) => conn.open_exec(&dial_stdio_command(sock.as_deref())).await,
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
        assert_eq!(dial_stdio_command(None), "exec docker system dial-stdio");
        assert_eq!(dial_stdio_command(Some("/var/run/docker.sock")), "exec docker system dial-stdio");
        assert_eq!(
            dial_stdio_command(Some("/run/user/1000/docker.sock")),
            "DOCKER_HOST='unix:///run/user/1000/docker.sock' exec docker system dial-stdio"
        );
    }
}
