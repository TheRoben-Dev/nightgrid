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

/// Tests here start sshd on ports picked by `free_port` and some restart
/// sshd on the same port, so running them in parallel can hand one test's
/// port to another. Each test holds this for its whole run.
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
    start_sshd_with(host_key_name, dir, port, "")
}

fn start_sshd_with(host_key_name: &str, dir: Option<tempfile::TempDir>, port: Option<u16>, extra: &str) -> Sshd {
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
         AllowTcpForwarding yes\nSubsystem sftp internal-sftp\nLogLevel ERROR\n{extra}",
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
    let _serial = SERIAL.lock().await;
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
    let _serial = SERIAL.lock().await;
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
    let h = spawn_ssh(connector, None, 100, 30, sink).await.unwrap();
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
    let _serial = SERIAL.lock().await;
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
    let _serial = SERIAL.lock().await;
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

/// Docker through a real sshd: socket forwarding, the dial-stdio fallback when
/// sshd forbids it, list/stats/logs/inspect/actions. Needs a local Docker the
/// test user can reach; opt-in with `NIGHTGRID_TEST_DOCKER=1` (CI on Linux).
#[tokio::test(flavor = "multi_thread")]
async fn openssh_docker_over_ssh() {
    use nightgrid_core::docker::{Action, DockerHost, LogQuery, Route, MASK};

    if !enabled() || std::env::var("NIGHTGRID_TEST_DOCKER").map_or(true, |v| v != "1") {
        return;
    }
    let _serial = SERIAL.lock().await;
    let s = start_sshd("ssh_host_ed25519_key", None, None);
    let conn = Arc::new(connect_trusting(&key_target(&s), &[], &opts(s.dir.path())).await);
    let name = format!("ng-test-{}", std::process::id());
    let run = format!(
        "docker rm -f {name} >/dev/null 2>&1; docker run -d --name {name} --network none \
         --label com.docker.compose.project=ngtest --label com.docker.compose.service=web \
         -e API_TOKEN=s3cr3t-value alpine:3.20 \
         sh -c 'i=0; while true; do echo tick $i; echo oops $i >&2; i=$((i+1)); sleep 0.2; done'"
    );
    let (code, _, err) = conn.exec(&run).await.unwrap();
    assert_eq!(code, 0, "docker run: {}", String::from_utf8_lossy(&err));

    let d = DockerHost::connect(conn.clone()).await.expect("docker connect");
    assert!(matches!(d.route, Route::Socket(_)), "{:?}", d.route);
    let info = d.info().await.unwrap();
    assert!(!info.version.is_empty());

    let list = d.containers().await.unwrap();
    let c = list.iter().find(|c| c.name == name).expect("test container listed");
    assert_eq!(c.state, "running");
    assert_eq!(c.project.as_deref(), Some("ngtest"));
    assert_eq!(c.service.as_deref(), Some("web"));

    let stats = d.stats(std::slice::from_ref(&c.id)).await;
    assert_eq!(stats.len(), 1);
    assert!(stats[0].mem_used > 0);

    let detail = d.inspect(&name).await.unwrap();
    assert!(detail.env.iter().any(|e| e.key == "API_TOKEN" && e.value == "s3cr3t-value"));
    assert!(!detail.raw.contains("s3cr3t-value") && detail.raw.contains(MASK));

    // Backlog: both streams, timestamps parsed.
    tokio::time::sleep(Duration::from_millis(700)).await;
    let mut lines = Vec::new();
    d.logs(&name, LogQuery { follow: false, tail: Some(4), since: None }, |b| {
        lines.extend(b);
        true
    })
    .await
    .unwrap();
    assert_eq!(lines.len(), 4, "{lines:?}");
    assert!(lines.iter().all(|l| l.ts.is_some()));
    assert!(lines.iter().any(|l| l.text.starts_with("oops")));
    // Follow: new lines arrive; returning false stops the stream.
    let mut got = 0;
    let follow = d.logs(&name, LogQuery { follow: true, tail: Some(0), since: None }, |b| {
        got += b.len();
        got < 6
    });
    tokio::time::timeout(Duration::from_secs(15), follow).await.expect("follow delivers").unwrap();
    assert!(got >= 6);

    // Same daemon via `docker system dial-stdio` when sshd refuses forwarding.
    let s2 = start_sshd_with("ssh_host_ed25519_key", None, None, "AllowStreamLocalForwarding no\n");
    let conn2 = Arc::new(connect_trusting(&key_target(&s2), &[], &opts(s2.dir.path())).await);
    let d2 = DockerHost::connect(conn2).await.expect("dial-stdio fallback");
    assert!(matches!(d2.route, Route::DialStdio(_)), "{:?}", d2.route);
    assert!(d2.containers().await.unwrap().iter().any(|c| c.name == name));

    // Lifecycle.
    d.act(&name, Action::Pause).await.unwrap();
    d.act(&name, Action::Unpause).await.unwrap();
    d.act(&name, Action::Kill).await.unwrap();
    let state = |l: Vec<nightgrid_core::docker::ContainerView>| l.into_iter().find(|c| c.name == name).map(|c| c.state);
    // `kill` returns once the signal is sent; the daemon flips the state a moment later.
    let deadline = Instant::now() + Duration::from_secs(10);
    while state(d.containers().await.unwrap()).as_deref() != Some("exited") {
        assert!(Instant::now() < deadline, "container did not exit after kill");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    d.act(&name, Action::Start).await.unwrap();
    d.act(&name, Action::Start).await.expect("start when running is a no-op");
    d.act(&name, Action::Remove { force: false }).await.expect_err("running container needs force");
    d.act(&name, Action::Remove { force: true }).await.unwrap();
    assert_eq!(state(d.containers().await.unwrap()), None);
    let err = d.inspect(&name).await.unwrap_err();
    assert!(matches!(err, nightgrid_core::docker::DockerError::NotFound { .. }), "{err:?}");
}

/// Compose stacks: found in a stack folder, then up / update / down with
/// streamed output, against the runner's dockerd.
#[tokio::test(flavor = "multi_thread")]
async fn openssh_compose_stacks() {
    use nightgrid_core::docker::compose::{project_from_dir, ComposeOp};
    use nightgrid_core::docker::DockerHost;

    if !enabled() || std::env::var("NIGHTGRID_TEST_DOCKER").map_or(true, |v| v != "1") {
        return;
    }
    let _serial = SERIAL.lock().await;
    let s = start_sshd("ssh_host_ed25519_key", None, None);
    let conn = Arc::new(connect_trusting(&key_target(&s), &[], &opts(s.dir.path())).await);
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join(format!("ngstack{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(
        dir.join("compose.yaml"),
        "services:\n  web:\n    image: alpine:3.20\n    network_mode: none\n    command: [\"sleep\", \"600\"]\n",
    )
    .unwrap();
    let root_s = root.path().to_str().unwrap().to_owned();
    let project = project_from_dir(dir.to_str().unwrap());

    let d = DockerHost::connect(conn.clone()).await.expect("docker connect");
    let find = |list: Vec<nightgrid_core::docker::compose::Stack>| list.into_iter().find(|st| st.name == project);
    let stack = find(d.stacks(std::slice::from_ref(&root_s)).await.unwrap()).expect("stack found in folder");
    assert!(stack.in_folder);
    assert_eq!((stack.running, stack.total), (0, 0));
    assert!(stack.files[0].ends_with("/compose.yaml"), "{:?}", stack.files);

    let (_tx, rx) = tokio::sync::watch::channel(false);
    let mut out = Vec::<String>::new();
    let mut sink = |l: Vec<String>| out.extend(l);
    let (code, _) = d.compose(&stack, ComposeOp::Up, rx.clone(), &mut sink).await.unwrap();
    assert_eq!(code, 0, "up: {out:?}");
    let up = find(d.stacks(std::slice::from_ref(&root_s)).await.unwrap()).unwrap();
    assert_eq!((up.running, up.total), (1, 1));

    // Same image: update pulls, recreates nothing and reports no changes.
    let (code, changes) = d.compose(&up, ComposeOp::Update, rx.clone(), &mut sink).await.unwrap();
    assert_eq!(code, 0, "update: {out:?}");
    assert_eq!(changes, Some(vec![]));

    let (code, _) = d.compose(&up, ComposeOp::Down, rx.clone(), &mut sink).await.unwrap();
    assert_eq!(code, 0, "down: {out:?}");
    let down = find(d.stacks(std::slice::from_ref(&root_s)).await.unwrap()).unwrap();
    assert_eq!(down.total, 0);
    assert!(out.iter().any(|l| l.contains("Removed")), "{out:?}");
}

/// "Edit in Nightgrid" hook: `nano FILE` and `$EDITOR` emit the edit request
/// with the session token and wait until the app answers, and the temp folder
/// is gone after the shell exits.
#[tokio::test(flavor = "multi_thread")]
async fn openssh_edit_hook() {
    use base64::Engine;
    use nightgrid_core::edit_hook::{bootstrap_command, parse_request};

    if !enabled() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let token = "00112233445566778899aabbccddeeff";
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
    let h = spawn_ssh(connector, Some(bootstrap_command(token)), 120, 30, sink).await.unwrap();
    let wait_for = |needle: String| {
        let data = data.clone();
        async move {
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                let out = String::from_utf8_lossy(&data.lock().unwrap().0).into_owned();
                if out.contains(&needle) {
                    return out;
                }
                assert!(Instant::now() < deadline, "no {needle:?} in: {out}");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    };
    h.write("cd /tmp && echo dir=$NIGHTGRID_EDIT && \"$EDITOR\" notes.txt && echo editor-returned-$?\n");
    let out = wait_for(format!("\x1b]7766;{token};")).await;
    let start = out.find("\x1b]7766;").unwrap() + 2 + 5;
    let payload = &out[start..start + out[start..].find('\x07').unwrap()];
    assert_eq!(parse_request(payload, token).as_deref(), Some("/tmp/notes.txt"));
    let dir = out
        .split("dir=/")
        .nth(1)
        .map(|s| format!("/{s}"))
        .unwrap()
        .split(['\r', '\n'])
        .next()
        .unwrap()
        .trim_end_matches("/edit")
        .to_owned();
    assert!(std::path::Path::new(&dir).exists(), "{dir}");
    // Still waiting: nothing after the prompt until the app answers.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!String::from_utf8_lossy(&data.lock().unwrap().0).contains("editor-returned-0"));
    h.write(format!("{token}\r"));
    wait_for("editor-returned-0".into()).await;
    // The nano function goes through the same helper; Enter alone stops waiting.
    h.write("nano -w /etc/hostname; echo nano-done\n");
    let b = base64::engine::general_purpose::STANDARD.encode("/etc/hostname");
    wait_for(format!("{token};{b}")).await;
    h.write("\r");
    wait_for("nano-done".into()).await;
    h.write("exit 4\n");
    let deadline = Instant::now() + Duration::from_secs(10);
    while data.lock().unwrap().1.is_none() {
        assert!(Instant::now() < deadline, "shell did not exit");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(data.lock().unwrap().1, Some(Some(4)));
    assert!(!std::path::Path::new(&dir).exists(), "temp folder left behind: {dir}");
}

/// Metrics: the collection script runs over a real exec channel and two
/// samples produce rates, memory and at least one mount.
#[tokio::test(flavor = "multi_thread")]
async fn openssh_metrics_sample() {
    use nightgrid_core::metrics::{parse, Collector, COLLECT};

    // The script reads /proc; macOS hosts are not a metrics target.
    if !enabled() || !cfg!(target_os = "linux") {
        return;
    }
    let _serial = SERIAL.lock().await;
    let s = start_sshd("ssh_host_ed25519_key", None, None);
    let conn = connect_trusting(&key_target(&s), &[], &opts(s.dir.path())).await;
    let mut c = Collector::default();
    let mut last = None;
    for i in 0..2u32 {
        let (code, out, err) = conn.exec(COLLECT).await.expect("exec");
        assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
        let raw = parse(&String::from_utf8_lossy(&out));
        last = Some(c.sample(raw, f64::from(i) * 1000.0));
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
    let sample = last.unwrap();
    assert!(sample.cpu.is_some_and(|v| (0.0..=100.0).contains(&v)), "{:?}", sample.cpu);
    assert!(sample.mem_total > 0 && sample.mem_used <= sample.mem_total);
    assert!(!sample.cores.is_empty());
    assert!(!sample.mounts.is_empty(), "no mounts parsed");
    assert!(!sample.kernel.is_empty() && !sample.hostname.is_empty());
}

/// Echo server on 127.0.0.1; returns its port.
async fn echo_server() -> u16 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = l.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                while let Ok(n) = s.read(&mut buf).await {
                    if n == 0 || s.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    port
}

async fn roundtrip(s: &mut tokio::net::TcpStream, msg: &[u8]) -> Vec<u8> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    s.write_all(msg).await.unwrap();
    let mut out = vec![0u8; msg.len()];
    tokio::time::timeout(Duration::from_secs(5), s.read_exact(&mut out)).await.expect("echo timed out").unwrap();
    out
}

#[tokio::test]
async fn openssh_tunnels_local_dynamic_remote() {
    use nightgrid_core::tunnels::forward::{listen, request_remote, serve_dynamic, serve_local, serve_remote, Stats};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    if !enabled() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let sshd = start_sshd("host_ed25519", None, None);
    let o = opts(sshd.dir.path());
    let conn = Arc::new(connect_trusting(&key_target(&sshd), &[], &o).await);
    let echo = echo_server().await;

    // -L
    let stats = Arc::new(Stats::default());
    let l = listen("127.0.0.1", 0).await.unwrap();
    let lport = l.local_addr().unwrap().port();
    let local = tokio::spawn(serve_local(conn.clone(), l, "127.0.0.1".into(), echo, stats.clone()));
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", lport)).await.unwrap();
    assert_eq!(roundtrip(&mut s, b"over -L").await, b"over -L");
    let v = stats.view();
    assert_eq!((v.tx, v.rx, v.total), (7, 7, 1));
    assert!(listen("127.0.0.1", lport).await.unwrap_err().contains("in use"));
    drop(s);
    local.abort();

    // -D (SOCKS5, domain name target)
    let stats = Arc::new(Stats::default());
    let l = listen("127.0.0.1", 0).await.unwrap();
    let dport = l.local_addr().unwrap().port();
    let dynamic = tokio::spawn(serve_dynamic(conn.clone(), l, stats.clone()));
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", dport)).await.unwrap();
    s.write_all(&[5, 1, 0]).await.unwrap();
    let mut greet = [0u8; 2];
    s.read_exact(&mut greet).await.unwrap();
    assert_eq!(greet, [5, 0]);
    let mut req = vec![5, 1, 0, 3, 9];
    req.extend_from_slice(b"localhost");
    req.extend_from_slice(&echo.to_be_bytes());
    s.write_all(&req).await.unwrap();
    let mut rep = [0u8; 10];
    s.read_exact(&mut rep).await.unwrap();
    assert_eq!(rep[1], 0, "socks reply {rep:?}");
    assert_eq!(roundtrip(&mut s, b"over -D").await, b"over -D");
    drop(s);
    dynamic.abort();

    // -R (server picks the port)
    let stats = Arc::new(Stats::default());
    let fwd = request_remote(conn.clone(), "127.0.0.1", 0).await.unwrap();
    let rport = u16::try_from(fwd.port).unwrap();
    assert!(rport > 0);
    let remote = tokio::spawn(serve_remote(fwd, "127.0.0.1".into(), echo, stats.clone()));
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", rport)).await.unwrap();
    assert_eq!(roundtrip(&mut s, b"over -R").await, b"over -R");
    assert_eq!(stats.view().total, 1);
    drop(s);
    remote.abort();
    // Stopping cancels the server-side listener.
    let deadline = Instant::now() + Duration::from_secs(5);
    while tokio::net::TcpStream::connect(("127.0.0.1", rport)).await.is_ok() {
        assert!(Instant::now() < deadline, "remote listener still open after stop");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// A stand-in `sudo` (password "hunter2") and `id`, put first on the session
/// PATH, to exercise the sudo protocol without real root.
fn fake_sudo_bin(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("fakebin");
    std::fs::create_dir_all(&bin).unwrap();
    let sudo = r#"#!/bin/sh
mode=""; prompt=""
while [ $# -gt 0 ]; do
  case "$1" in
    -n) mode=n; shift ;;
    -S) mode=S; shift ;;
    -k) shift ;;
    -p) prompt="$2"; shift 2 ;;
    --) shift; break ;;
    *) break ;;
  esac
done
if [ "$mode" = n ]; then echo "sudo: a password is required" >&2; exit 1; fi
printf '%s' "$prompt" >&2
IFS= read -r pw || pw=""
if [ "$pw" != "hunter2" ]; then
  echo "Sorry, try again." >&2
  printf '%s' "$prompt" >&2
  echo "sudo: no password was provided" >&2
  echo "sudo: 1 incorrect password attempt" >&2
  exit 1
fi
FAKE_ROOT=1 exec "$@"
"#;
    let id = "#!/bin/sh\nif [ \"$FAKE_ROOT\" = 1 ]; then echo 0; else echo 1000; fi\n";
    for (name, body) in [("sudo", sudo), ("id", id)] {
        let p = bin.join(name);
        std::fs::write(&p, body).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    bin
}

#[tokio::test]
async fn openssh_sudo_password_on_stdin() {
    use nightgrid_core::secrets::Secret;
    use nightgrid_core::system::sudo::{self, Stream, SudoAuth, SudoState};
    use nightgrid_core::system::SystemError;
    if !enabled() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let bin = fake_sudo_bin(dir.path());
    let extra = format!("SetEnv PATH={}:/usr/bin:/bin:/usr/sbin:/sbin\n", bin.display());
    let sshd = start_sshd_with("host_ed25519", Some(dir), None, &extra);
    let o = opts(sshd.dir.path());
    let conn = connect_trusting(&key_target(&sshd), &[], &o).await;

    assert_eq!(sudo::probe(&conn).await.unwrap(), SudoState::Password);
    assert_eq!(
        sudo::run(&conn, &SudoAuth::NoPassword, "true").await.unwrap_err(),
        SystemError::SudoRequired { retry: false }
    );
    let wrong = SudoAuth::Password(Arc::new(Secret::new("nope")));
    assert_eq!(sudo::validate(&conn, &wrong).await.unwrap_err(), SystemError::SudoRequired { retry: true });

    // The password line goes to sudo only: `cat` sees an empty stdin.
    let good = SudoAuth::Password(Arc::new(Secret::new("hunter2")));
    sudo::validate(&conn, &good).await.unwrap();
    let out = sudo::run(&conn, &good, "id -u; cat; echo done >&2").await.unwrap();
    assert_eq!((out.code, out.stdout.as_str(), out.stderr.as_str()), (0, "0\n", "done"));

    let mut lines = Vec::new();
    let code = sudo::stream(&conn, &good, "echo a; echo b >&2; printf tail; exit 3", None, &mut |s, l| {
        lines.push((s, l));
        true
    })
    .await
    .unwrap();
    assert_eq!(code, Some(3));
    // stdout and stderr travel separately, so only compare within a stream.
    let of = |s: Stream| lines.iter().filter(|(x, _)| *x == s).map(|(_, l)| l.as_str()).collect::<Vec<_>>();
    assert_eq!((of(Stream::Out), of(Stream::Err)), (vec!["a", "tail"], vec!["b"]));

    let err = sudo::stream(&conn, &wrong, "echo never", None, &mut |_, _| true).await.unwrap_err();
    assert_eq!(err, SystemError::SudoRequired { retry: true });
}

#[tokio::test(flavor = "multi_thread")]
async fn openssh_automation_steps() {
    use nightgrid_core::automation::run::{self, OutStream};
    use nightgrid_core::system::sudo::SudoAuth;
    if !enabled() {
        return;
    }
    let _serial = SERIAL.lock().await;
    let sshd = start_sshd("host_ed25519", None, None);
    let o = opts(sshd.dir.path());
    let conn = Arc::new(connect_trusting(&key_target(&sshd), &[], &o).await);
    let (_tx, rx) = tokio::sync::watch::channel(false);

    // Command: lines from both streams and the exit code.
    let mut lines = Vec::new();
    let code = run::command(&conn, &SudoAuth::User, "echo out; echo err >&2; exit 3", rx.clone(), &mut |s, l| {
        lines.push((s, l))
    })
    .await
    .unwrap();
    assert_eq!(code, Some(3));
    assert!(lines.contains(&(OutStream::Out, "out".into())) && lines.contains(&(OutStream::Err, "err".into())));

    // Stop: a long command ends with None.
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let c2 = conn.clone();
    let h = tokio::spawn(async move { run::command(&c2, &SudoAuth::User, "sleep 30", stop_rx, &mut |_, _| {}).await });
    tokio::time::sleep(Duration::from_millis(300)).await;
    stop_tx.send(true).unwrap();
    assert_eq!(tokio::time::timeout(Duration::from_secs(5), h).await.unwrap().unwrap().unwrap(), None);

    // Upload with permissions.
    let local = sshd.dir.path().join("payload.txt");
    std::fs::write(&local, b"hello automation\n").unwrap();
    let remote = sshd.dir.path().join("uploaded.txt");
    let n = run::upload(conn.clone(), &local, remote.to_str().unwrap(), Some(0o600)).await.unwrap();
    assert_eq!(n, 17);
    assert_eq!(std::fs::read(&remote).unwrap(), b"hello automation\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&remote).unwrap().permissions().mode() & 0o777, 0o600);
    }

    // Wait for port: open at once, and a closed one times out.
    let port = echo_server().await;
    assert!(run::wait_port(&conn, "127.0.0.1", port, Duration::from_secs(5), rx.clone()).await.unwrap().is_some());
    let closed = free_port();
    let err = run::wait_port(&conn, "127.0.0.1", closed, Duration::from_secs(1), rx.clone()).await.unwrap_err();
    assert!(err.contains("didn't accept connections"), "{err}");
}
