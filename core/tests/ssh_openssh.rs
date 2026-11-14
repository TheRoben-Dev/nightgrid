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
         AllowTcpForwarding yes\nSubsystem sftp internal-sftp\nLogLevel ERROR\n",
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

#[tokio::test(flavor = "multi_thread")]
async fn openssh_sftp_browse_edit_and_transfer() {
    use nightgrid_core::files::text::{read_text, save_text, Stamp};
    use nightgrid_core::files::transfer::{ConflictPolicy, JobState, NewJob, Resolver, Side, Transfers};
    use nightgrid_core::files::{EntryKind, Fs, FsError, RemoteFs};
    use std::os::unix::fs::PermissionsExt;

    if !enabled() {
        return;
    }
    let sshd = start_sshd("host_ed25519", None, None);
    let o = opts(sshd.dir.path());
    let conn = Arc::new(connect_trusting(&key_target(&sshd), &[], &o).await);
    let remote = Arc::new(RemoteFs::open(conn.clone()).await.unwrap());
    let fs = Fs::Remote(remote.clone());

    // The sshd runs as us, so "remote" paths are on this machine: use a temp dir.
    let work = tempfile::tempdir().unwrap();
    let root = work.path().to_string_lossy().into_owned();
    let home = fs.home().await.unwrap();
    assert!(home.starts_with('/'), "{home}");

    let stack = fs.join(&root, "stack");
    fs.mkdir(&stack).await.unwrap();
    assert!(matches!(fs.mkdir(&stack).await, Err(FsError::Exists { .. })));
    let compose = fs.join(&stack, "compose.yaml");
    save_text(&fs, &compose, "services: {}\n", None).await.unwrap();
    std::fs::set_permissions(&compose, std::fs::Permissions::from_mode(0o640)).unwrap();

    let listed = fs.list(&root).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].kind, EntryKind::Dir);

    // Edit: atomic replace keeps the mode; a stale stamp is a conflict.
    let t = read_text(&fs, &compose).await.unwrap();
    assert_eq!(t.content, "services: {}\n");
    let r = save_text(&fs, &t.path, "services:\n  web: {}\n", Some(Stamp::of(&t.stat))).await.unwrap();
    assert!(r.atomic);
    assert_eq!(std::fs::read_to_string(&compose).unwrap(), "services:\n  web: {}\n");
    assert_eq!(std::fs::metadata(&compose).unwrap().permissions().mode() & 0o777, 0o640);
    assert_eq!(std::fs::read_dir(&stack).unwrap().count(), 1, "temp file left behind");
    std::thread::sleep(Duration::from_millis(1100));
    std::fs::write(&compose, "changed elsewhere\n").unwrap();
    let e = save_text(&fs, &t.path, "mine\n", Some(Stamp::of(&r.stat))).await.unwrap_err();
    assert!(matches!(e, FsError::Changed { .. }), "{e:?}");

    // Transfers: remote → local and local → remote.
    let resolver: Resolver = {
        let remote = remote.clone();
        Arc::new(move |side| {
            let remote = remote.clone();
            Box::pin(async move {
                Ok(match side {
                    Side::Local => Fs::Local,
                    Side::Host { .. } => Fs::Remote(remote),
                })
            })
        })
    };
    let q = Transfers::new(resolver, 2);
    std::fs::write(work.path().join("stack/big.bin"), vec![7u8; 2_500_000]).unwrap();
    let down = tempfile::tempdir().unwrap();
    let id = q.add(vec![NewJob {
        src: Side::Host { id: 1 },
        src_path: stack.clone(),
        dst: Side::Local,
        dst_dir: down.path().to_string_lossy().into_owned(),
        policy: ConflictPolicy::Ask,
    }])[0];
    let up_dir = fs.join(&root, "up");
    fs.mkdir(&up_dir).await.unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let v = q.list().into_iter().find(|v| v.id == id).unwrap();
        if v.state == JobState::Done {
            break;
        }
        assert!(v.state == JobState::Running || v.state == JobState::Queued, "{v:?}");
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(std::fs::metadata(down.path().join("stack/big.bin")).unwrap().len(), 2_500_000);
    let id = q.add(vec![NewJob {
        src: Side::Local,
        src_path: down.path().join("stack").to_string_lossy().into_owned(),
        dst: Side::Host { id: 1 },
        dst_dir: up_dir.clone(),
        policy: ConflictPolicy::Ask,
    }])[0];
    loop {
        let v = q.list().into_iter().find(|v| v.id == id).unwrap();
        if v.state == JobState::Done {
            break;
        }
        assert!(v.state == JobState::Running || v.state == JobState::Queued, "{v:?}");
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(std::fs::read(work.path().join("up/stack/big.bin")).unwrap(), vec![7u8; 2_500_000]);

    // Rename and recursive delete.
    let renamed = fs.join(&root, "stack2");
    fs.rename(&stack, &renamed).await.unwrap();
    assert!(matches!(fs.rename(&renamed, &up_dir).await, Err(FsError::Exists { .. })));
    fs.remove(&renamed).await.unwrap();
    fs.remove(&up_dir).await.unwrap();
    assert!(fs.list(&root).await.unwrap().is_empty());
}
