//! Tests against a real OpenSSH `sshd` (run as the current user on a free
//! port), to catch interoperability problems the in-process server can't.
//! Opt-in: set `NIGHTGRID_TEST_OPENSSH=1` (CI does on Linux and macOS).

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nightgrid_core::ssh::client::{connect, Auth, ConnectError, ConnectOptions, Connection, Target};
use nightgrid_core::ssh::known_hosts::KnownHosts;
use nightgrid_core::ssh::resolve::local_username;
use nightgrid_core::term::{spawn_ssh, Connector, Sink, TermEvent};

fn enabled() -> bool {
    std::env::var("NIGHTGRID_TEST_OPENSSH").is_ok_and(|v| v == "1")
}

fn sshd_binary() -> PathBuf {
    ["/usr/sbin/sshd", "/usr/local/sbin/sshd", "/opt/homebrew/sbin/sshd"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
        .expect("sshd not found")
}

fn keygen(path: &Path) {
    let ok =
        Command::new("ssh-keygen").args(["-q", "-t", "ed25519", "-N", "", "-f"]).arg(path).status().unwrap().success();
    assert!(ok, "ssh-keygen failed");
}

struct Sshd {
    child: Child,
    port: u16,
    dir: tempfile::TempDir,
}

impl Drop for Sshd {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Sshd {
    fn client_key(&self) -> PathBuf {
        self.dir.path().join("id_ed25519")
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn start_sshd(host_key_name: &str, dir: Option<tempfile::TempDir>, port: Option<u16>) -> Sshd {
    let dir = dir.unwrap_or_else(|| tempfile::tempdir().unwrap());
    let d = dir.path();
    let host_key = d.join(host_key_name);
    if !host_key.exists() {
        keygen(&host_key);
    }
    if !d.join("id_ed25519").exists() {
        keygen(&d.join("id_ed25519"));
        std::fs::copy(d.join("id_ed25519.pub"), d.join("authorized_keys")).unwrap();
    }
    let port = port.unwrap_or_else(free_port);
    let config = format!(
        "Port {port}\nListenAddress 127.0.0.1\nHostKey {hk}\nAuthorizedKeysFile {ak}\nPidFile {pid}\n\
         PasswordAuthentication no\nKbdInteractiveAuthentication no\nUsePAM no\nStrictModes no\n\
         AllowTcpForwarding yes\nLogLevel ERROR\n",
        hk = host_key.display(),
        ak = d.join("authorized_keys").display(),
        pid = d.join("sshd.pid").display(),
    );
    std::fs::write(d.join("sshd_config"), config).unwrap();
    let child = Command::new(sshd_binary())
        .args(["-D", "-e", "-f"])
        .arg(d.join("sshd_config"))
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn sshd");
    let deadline = Instant::now() + Duration::from_secs(10);
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < deadline, "sshd did not start");
        std::thread::sleep(Duration::from_millis(50));
    }
    Sshd { child, port, dir }
}

fn opts(dir: &Path) -> ConnectOptions {
    let mut o = ConnectOptions::new(KnownHosts::new(dir.join("nightgrid_known_hosts"), None));
    o.connect_timeout = Duration::from_secs(5);
    o
}

fn key_target(s: &Sshd) -> Target {
    Target {
        host: "127.0.0.1".into(),
        port: s.port,
        user: local_username(),
        auth: Auth::Key { path: s.client_key(), passphrase: None },
    }
}

async fn connect_trusting(t: &Target, jumps: &[Target], o: &ConnectOptions) -> Connection {
    for _ in 0..4 {
        match connect(t, jumps, o).await {
            Ok(c) => return c,
            Err(ConnectError::HostKeyUnknown { host, port, key }) => {
                o.known_hosts.trust_openssh(&host, port, &key.openssh, false).unwrap();
            }
            Err(e) => panic!("connect failed: {e:?}"),
        }
    }
    panic!("too many host key prompts")
}

#[tokio::test]
async fn openssh_key_auth_exec_and_jump() {
    if !enabled() {
        return;
    }
    let sshd = start_sshd("host_ed25519", None, None);
    let o = opts(sshd.dir.path());
    let t = key_target(&sshd);

    let conn = connect_trusting(&t, &[], &o).await;
    let (code, out, _) = conn.exec("echo nightgrid-$((40+2))").await.unwrap();
    assert_eq!((code, String::from_utf8_lossy(&out).trim().to_owned()), (0, "nightgrid-42".to_owned()));
    let (code, _, _) = conn.exec("exit 5").await.unwrap();
    assert_eq!(code, 5);

    // ProxyJump through the same sshd (direct-tcpip to itself).
    let conn = connect_trusting(&t, std::slice::from_ref(&t), &o).await;
    let (code, out, _) = conn.exec("echo via-jump").await.unwrap();
    assert_eq!((code, String::from_utf8_lossy(&out).trim().to_owned()), (0, "via-jump".to_owned()));
}

#[tokio::test]
async fn openssh_interactive_shell() {
    if !enabled() {
        return;
    }
    let sshd = start_sshd("host_ed25519", None, None);
    let o = opts(sshd.dir.path());
    let t = key_target(&sshd);
    connect_trusting(&t, &[], &o).await;

    let data = Arc::new(Mutex::new((Vec::<u8>::new(), None::<Option<i32>>)));
    let d2 = data.clone();
    let sink: Sink = Arc::new(move |ev| {
        let mut d = d2.lock().unwrap();
        match ev {
            TermEvent::Data(b) => d.0.extend(b),
            TermEvent::Exit { code } => d.1 = Some(code),
            TermEvent::Status { .. } => {}
        }
    });
    let connector: Connector = Arc::new(move |_| {
        let (t, o) = (t.clone(), o.clone());
        Box::pin(async move { connect(&t, &[], &o).await.map(Arc::new) })
    });
    let h = spawn_ssh(connector, 100, 30, sink).await.unwrap();
    h.write("echo marker-$((1+1)); exit 3\n");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        {
            let d = data.lock().unwrap();
            if d.1.is_some() {
                assert!(String::from_utf8_lossy(&d.0).contains("marker-2"), "{}", String::from_utf8_lossy(&d.0));
                assert_eq!(d.1, Some(Some(3)));
                break;
            }
        }
        assert!(Instant::now() < deadline, "shell did not exit: {}", String::from_utf8_lossy(&data.lock().unwrap().0));
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn openssh_changed_host_key_is_blocked() {
    if !enabled() {
        return;
    }
    let sshd = start_sshd("host_a", None, None);
    let o = opts(sshd.dir.path());
    let t = key_target(&sshd);
    connect_trusting(&t, &[], &o).await;

    // Restart on the same port with a different host key.
    let port = sshd.port;
    let mut sshd = sshd;
    let _ = sshd.child.kill();
    let _ = sshd.child.wait();
    let dir = std::mem::replace(&mut sshd.dir, tempfile::tempdir().unwrap());
    drop(sshd);
    std::thread::sleep(Duration::from_millis(200));
    let sshd = start_sshd("host_b", Some(dir), Some(port));
    let e = connect(&key_target(&sshd), &[], &o).await.err().expect("must not connect");
    assert!(matches!(e, ConnectError::HostKeyChanged { .. }), "{e:?}");
}
