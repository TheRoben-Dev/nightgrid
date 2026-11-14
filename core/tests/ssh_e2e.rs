//! End-to-end SSH tests against an in-process russh server: host key TOFU,
//! changed-key blocking, password / encrypted key auth, ProxyJump and
//! reconnect. Runs on every OS in CI without Docker.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use nightgrid_core::secrets::Secret;
use nightgrid_core::ssh::client::{connect, Auth, ConnectError, ConnectOptions, Connection, Target};
use nightgrid_core::ssh::known_hosts::KnownHosts;
use nightgrid_core::term::{spawn_ssh, Connector, Sink, TermEvent, TermState};
use russh::keys::{Algorithm, PrivateKey, PublicKey};
use russh::server::{self, Auth as SAuth, Msg, Server as _, Session, VerifiedPublicKey};
use russh::{Channel, ChannelId};
use tokio::net::TcpListener;

const USER: &str = "tester";
const PASSWORD: &str = "hunter2";

#[derive(Clone)]
struct TestServer {
    allowed_key: Option<PublicKey>,
    /// Channels that asked for a shell; only those get the echo behaviour.
    shells: Arc<Mutex<std::collections::HashSet<ChannelId>>>,
}

impl server::Server for TestServer {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        Self { allowed_key: self.allowed_key.clone(), shells: Default::default() }
    }
}

impl server::Handler for TestServer {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<SAuth, Self::Error> {
        Ok(if user == USER && password == PASSWORD { SAuth::Accept } else { SAuth::reject() })
    }

    async fn auth_publickey(&mut self, user: &str, key: VerifiedPublicKey<'_>) -> Result<SAuth, Self::Error> {
        let ok = user == USER && self.allowed_key.as_ref().is_some_and(|k| k.key_data() == key.public_key.key_data());
        Ok(if ok { SAuth::Accept } else { SAuth::reject() })
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host: &str,
        port: u32,
        _oa: &str,
        _op: u32,
        reply: server::ChannelOpenHandle,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        let Ok(mut upstream) = tokio::net::TcpStream::connect((host, port as u16)).await else {
            return Ok(()); // dropping the reply rejects the channel
        };
        reply.accept().await;
        tokio::spawn(async move {
            let mut stream = channel.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut upstream).await;
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        _c: u32,
        _r: u32,
        _pw: u32,
        _ph: u32,
        _m: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        Ok(())
    }

    async fn shell_request(&mut self, channel: ChannelId, session: &mut Session) -> Result<(), Self::Error> {
        self.shells.lock().unwrap().insert(channel);
        session.channel_success(channel)?;
        session.data(channel, b"welcome\r\n".to_vec())?;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        session.data(channel, format!("ran:{}", String::from_utf8_lossy(data)).into_bytes())?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }

    /// Shell echo: "exit" ends the shell with status 7.
    async fn data(&mut self, channel: ChannelId, data: &[u8], session: &mut Session) -> Result<(), Self::Error> {
        if !self.shells.lock().unwrap().contains(&channel) {
            return Ok(());
        }
        if data.starts_with(b"exit") {
            session.exit_status_request(channel, 7)?;
            session.eof(channel)?;
            session.close(channel)?;
        } else {
            session.data(channel, [b"echo:", data].concat())?;
        }
        Ok(())
    }
}

struct Running {
    port: u16,
    handle: server::RunningServerHandle,
}

async fn start(host_key: PrivateKey, allowed_key: Option<PublicKey>, port: u16) -> Running {
    let listener = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let config = Arc::new(server::Config {
        keys: vec![host_key],
        auth_rejection_time: Duration::from_millis(10),
        auth_rejection_time_initial: Some(Duration::from_millis(0)),
        ..Default::default()
    });
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let mut srv = TestServer { allowed_key, shells: Default::default() };
        let running = srv.run_on_socket(config, &listener);
        let _ = tx.send(running.handle());
        let _ = running.await;
    });
    Running { port, handle: rx.await.unwrap() }
}

fn new_key() -> PrivateKey {
    PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap()
}

fn opts(dir: &tempfile::TempDir) -> ConnectOptions {
    let mut o = ConnectOptions::new(KnownHosts::new(dir.path().join("known_hosts"), None));
    o.connect_timeout = Duration::from_secs(5);
    o.keepalive_interval = Duration::from_millis(500);
    o.keepalive_max = 2;
    o
}

fn pw_target(port: u16, pw: Option<&str>) -> Target {
    Target {
        host: "127.0.0.1".into(),
        port,
        user: USER.into(),
        auth: Auth::Password { password: pw.map(|p| Arc::new(Secret::new(p))) },
    }
}

/// Connects, trusting the host key on first use like the UI does.
async fn connect_trusting(t: &Target, jumps: &[Target], o: &ConnectOptions) -> Connection {
    for _ in 0..4 {
        match connect(t, jumps, o).await {
            Ok(c) => return c,
            Err(ConnectError::HostKeyUnknown { host, port, key }) => {
                let k = PublicKey::from_openssh(&key.openssh).unwrap();
                o.known_hosts.trust(&host, port, &k, false).unwrap();
            }
            Err(e) => panic!("connect failed: {e:?}"),
        }
    }
    panic!("too many host key prompts")
}

#[tokio::test]
async fn tofu_then_password_and_exec() {
    let dir = tempfile::tempdir().unwrap();
    let o = opts(&dir);
    let srv = start(new_key(), None, 0).await;

    let err = connect(&pw_target(srv.port, Some(PASSWORD)), &[], &o).await.err().unwrap();
    let ConnectError::HostKeyUnknown { key, .. } = err else { panic!("{err:?}") };
    assert!(key.fingerprint.starts_with("SHA256:"));

    let conn = connect_trusting(&pw_target(srv.port, Some(PASSWORD)), &[], &o).await;
    let (code, out, _) = conn.exec("uptime").await.unwrap();
    assert_eq!(code, 0);
    assert_eq!(String::from_utf8_lossy(&out), "ran:uptime");
}

#[tokio::test]
async fn password_prompts() {
    let dir = tempfile::tempdir().unwrap();
    let o = opts(&dir);
    let srv = start(new_key(), None, 0).await;
    connect_trusting(&pw_target(srv.port, Some(PASSWORD)), &[], &o).await;

    let e = connect(&pw_target(srv.port, None), &[], &o).await.err().unwrap();
    assert!(matches!(e, ConnectError::PasswordRequired { retry: false, .. }), "{e:?}");
    let e = connect(&pw_target(srv.port, Some("wrong")), &[], &o).await.err().unwrap();
    assert!(matches!(e, ConnectError::PasswordRequired { retry: true, .. }), "{e:?}");
}

#[tokio::test]
async fn changed_host_key_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let o = opts(&dir);
    let srv = start(new_key(), None, 0).await;
    connect_trusting(&pw_target(srv.port, Some(PASSWORD)), &[], &o).await;
    let port = srv.port;
    srv.handle.shutdown("bye".into());
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Same address, new host key: must be refused, not silently trusted.
    let _srv2 = start(new_key(), None, port).await;
    let e = connect(&pw_target(port, Some(PASSWORD)), &[], &o).await.err().unwrap();
    let ConnectError::HostKeyChanged { previous, key, .. } = e else { panic!("{e:?}") };
    assert_eq!(previous.len(), 1);
    assert_ne!(previous[0].fingerprint, key.fingerprint);
}

#[tokio::test]
async fn encrypted_key_file_needs_passphrase() {
    let dir = tempfile::tempdir().unwrap();
    let o = opts(&dir);
    let client_key = new_key();
    let path = dir.path().join("id_ed25519");
    let encrypted = client_key.encrypt(&mut rand::rng(), "s3cret").unwrap();
    std::fs::write(&path, encrypted.to_openssh(russh::keys::ssh_key::LineEnding::LF).unwrap().as_bytes()).unwrap();
    let srv = start(new_key(), Some(client_key.public_key().clone()), 0).await;
    connect_trusting(&pw_target(srv.port, Some(PASSWORD)), &[], &o).await;

    let key_target = |pass: Option<&str>| Target {
        host: "127.0.0.1".into(),
        port: srv.port,
        user: USER.into(),
        auth: Auth::Key { path: path.clone(), passphrase: pass.map(|p| Arc::new(Secret::new(p))) },
    };
    let e = connect(&key_target(None), &[], &o).await.err().unwrap();
    assert!(matches!(e, ConnectError::PassphraseRequired { retry: false, .. }), "{e:?}");
    let e = connect(&key_target(Some("nope")), &[], &o).await.err().unwrap();
    assert!(matches!(e, ConnectError::PassphraseRequired { retry: true, .. }), "{e:?}");
    let conn = connect(&key_target(Some("s3cret")), &[], &o).await.unwrap();
    assert_eq!(conn.exec("id").await.unwrap().0, 0);
}

#[tokio::test]
async fn proxy_jump_chain() {
    let dir = tempfile::tempdir().unwrap();
    let o = opts(&dir);
    let bastion = start(new_key(), None, 0).await;
    let inner = start(new_key(), None, 0).await;
    let jump = pw_target(bastion.port, Some(PASSWORD));
    let target = pw_target(inner.port, Some(PASSWORD));
    let conn = connect_trusting(&target, std::slice::from_ref(&jump), &o).await;
    let (code, out, _) = conn.exec("hostname").await.unwrap();
    assert_eq!((code, String::from_utf8_lossy(&out).into_owned()), (0, "ran:hostname".into()));
}

#[derive(Default)]
struct Recorder {
    data: Vec<u8>,
    states: Vec<TermState>,
    exit: Option<Option<i32>>,
}

fn recorder() -> (Arc<Mutex<Recorder>>, Sink) {
    let r = Arc::new(Mutex::new(Recorder::default()));
    let r2 = r.clone();
    let sink: Sink = Arc::new(move |ev| {
        let mut r = r2.lock().unwrap();
        match ev {
            TermEvent::Data(d) => r.data.extend(d),
            TermEvent::Status { state, .. } => r.states.push(state),
            TermEvent::Exit { code } => r.exit = Some(code),
        }
    });
    (r, sink)
}

async fn wait_until(r: &Arc<Mutex<Recorder>>, secs: u64, f: impl Fn(&Recorder) -> bool) {
    let start = std::time::Instant::now();
    while !f(&r.lock().unwrap()) {
        assert!(start.elapsed() < Duration::from_secs(secs), "timed out; states={:?}", r.lock().unwrap().states);
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

fn connector_for(target: Target, o: ConnectOptions) -> Connector {
    Arc::new(move |_fresh| {
        let (t, o) = (target.clone(), o.clone());
        Box::pin(async move { connect(&t, &[], &o).await.map(Arc::new) })
    })
}

#[tokio::test]
async fn shell_io_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let o = opts(&dir);
    let srv = start(new_key(), None, 0).await;
    let t = pw_target(srv.port, Some(PASSWORD));
    connect_trusting(&t, &[], &o).await;

    let (rec, sink) = recorder();
    let h = spawn_ssh(connector_for(t, o), None, 80, 24, sink).await.unwrap();
    wait_until(&rec, 5, |r| String::from_utf8_lossy(&r.data).contains("welcome")).await;
    h.write("ls");
    wait_until(&rec, 5, |r| String::from_utf8_lossy(&r.data).contains("echo:ls")).await;
    h.write("exit");
    wait_until(&rec, 5, |r| r.exit.is_some()).await;
    assert_eq!(rec.lock().unwrap().exit, Some(Some(7)));
}

#[tokio::test]
async fn shell_reconnects_after_server_restart() {
    let dir = tempfile::tempdir().unwrap();
    let o = opts(&dir);
    let host_key = new_key();
    let srv = start(host_key.clone(), None, 0).await;
    let port = srv.port;
    let t = pw_target(port, Some(PASSWORD));
    connect_trusting(&t, &[], &o).await;

    let (rec, sink) = recorder();
    let h = spawn_ssh(connector_for(t, o), None, 80, 24, sink).await.unwrap();
    wait_until(&rec, 5, |r| r.states.contains(&TermState::Connected)).await;

    // Network drop: server goes away, comes back on the same port with the same key.
    srv.handle.shutdown("network drop".into());
    wait_until(&rec, 10, |r| r.states.contains(&TermState::Reconnecting)).await;
    let _srv2 = start(host_key, None, port).await;
    h.reconnect(); // skip the backoff wait
    wait_until(&rec, 15, |r| r.states.iter().filter(|s| **s == TermState::Connected).count() >= 2).await;
    h.write("pwd");
    wait_until(&rec, 5, |r| String::from_utf8_lossy(&r.data).contains("echo:pwd")).await;
    assert!(String::from_utf8_lossy(&rec.lock().unwrap().data).contains("reconnected"));
    h.close();
}

/// Agent auth against a real `ssh-agent` (skipped where OpenSSH isn't installed).
#[cfg(unix)]
#[tokio::test]
async fn agent_auth_with_openssh_agent() {
    use std::process::{Command, Stdio};
    if Command::new("ssh-agent").arg("-h").stderr(Stdio::null()).status().is_err() {
        eprintln!("skipping: ssh-agent not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let o = {
        let mut o = opts(&dir);
        o.agent_path = Some(dir.path().join("agent.sock"));
        o
    };
    let sock = o.agent_path.clone().unwrap();
    let mut agent = Command::new("ssh-agent").arg("-D").arg("-a").arg(&sock).stdout(Stdio::null()).spawn().unwrap();
    for _ in 0..100 {
        if sock.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let client_key = new_key();
    let key_path = dir.path().join("id_agent");
    std::fs::write(&key_path, client_key.to_openssh(russh::keys::ssh_key::LineEnding::LF).unwrap().as_bytes()).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let added =
        Command::new("ssh-add").arg(&key_path).env("SSH_AUTH_SOCK", &sock).stderr(Stdio::null()).status().unwrap();
    assert!(added.success());

    let srv = start(new_key(), Some(client_key.public_key().clone()), 0).await;
    let t = Target {
        host: "127.0.0.1".into(),
        port: srv.port,
        user: USER.into(),
        auth: Auth::Agent { identity_file: None, passphrase: None },
    };
    let conn = connect_trusting(&t, &[], &o).await;
    assert_eq!(conn.exec("whoami").await.unwrap().0, 0);
    let _ = agent.kill();
    let _ = agent.wait();
}
