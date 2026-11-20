//! Compose stacks: discovery (container labels + user stack folders) and
//! running `docker compose` commands on the host with live output.

use std::collections::{BTreeMap, HashMap};

use bollard::query_parameters::ListContainersOptions;
use russh::{ChannelMsg, Sig};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::{strip_ansi, ContainerView, DockerError, DockerHost, DockerResult, Route, LABEL_PROJECT, LABEL_SERVICE};
use crate::files::path::{posix_file_name, posix_parent, shell_quote};

/// Compose file names, in the order `docker compose` looks for them.
pub const COMPOSE_FILES: [&str; 4] = ["compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Stack {
    /// Compose project name.
    pub name: String,
    /// Project (working) directory on the host.
    pub dir: String,
    /// Compose files (absolute paths). Empty = compose's default lookup in `dir`.
    pub files: Vec<String>,
    pub running: u32,
    pub total: u32,
    /// Found in a stack folder (vs. only from running containers' labels).
    pub in_folder: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ComposeOp {
    Up,
    Pull,
    Restart,
    Down,
    /// `pull` then `up -d`, reporting which services got a new image.
    Update,
}

impl ComposeOp {
    pub fn is_destructive(self) -> bool {
        matches!(self, ComposeOp::Down | ComposeOp::Restart | ComposeOp::Update)
    }

    fn args(self) -> &'static str {
        match self {
            ComposeOp::Up => "up -d",
            ComposeOp::Pull => "pull",
            ComposeOp::Restart => "restart",
            ComposeOp::Down => "down",
            ComposeOp::Update => "pull",
        }
    }
}

/// A service whose image changed during "update stack".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ImageChange {
    pub service: String,
    pub image: String,
    /// Short image ids (12 hex chars); `None` before = service was not running.
    pub before: Option<String>,
    pub after: String,
}

/// Compose's project name rule: lowercase letters, digits, `-` and `_`,
/// starting with a letter or digit.
pub fn valid_project(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
        && name.as_bytes()[0].is_ascii_alphanumeric()
}

/// Project name compose derives from a folder name.
pub fn project_from_dir(dir: &str) -> String {
    let base = posix_file_name(dir.trim_end_matches('/')).to_lowercase();
    let mut out: String =
        base.chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-' || *c == '_').collect();
    while out.starts_with(['-', '_']) {
        out.remove(0);
    }
    if out.is_empty() {
        "stack".into()
    } else {
        out
    }
}

/// Quotes a stack folder for the shell, expanding a leading `~`.
pub(crate) fn quote_dir(dir: &str) -> String {
    if dir == "~" {
        "\"$HOME\"".into()
    } else if let Some(rest) = dir.strip_prefix("~/") {
        format!("\"$HOME\"/{}", shell_quote(rest))
    } else {
        shell_quote(dir)
    }
}

/// Shell snippet printing every compose file directly in, or one level
/// below, each folder (first matching name per folder), one per line.
pub fn scan_command(dirs: &[String]) -> String {
    let roots: Vec<String> = dirs.iter().map(|d| quote_dir(d)).collect();
    let names = COMPOSE_FILES.join(" ");
    format!(
        "for r in {}; do for d in \"$r\" \"$r\"/*; do [ -d \"$d\" ] || continue; \
         for f in {names}; do if [ -f \"$d/$f\" ]; then (cd \"$d\" && echo \"$(pwd -P)/$f\"); break; fi; done; done; done",
        roots.join(" ")
    )
}

/// Merges stacks seen on containers with compose files found in folders.
pub fn merge_stacks(containers: &[ContainerView], found: &[String]) -> Vec<Stack> {
    let mut by_name: BTreeMap<String, Stack> = BTreeMap::new();
    for c in containers {
        let Some(project) = &c.project else { continue };
        let s = by_name.entry(project.clone()).or_insert_with(|| Stack {
            name: project.clone(),
            dir: c.compose_dir.clone().unwrap_or_default(),
            files: c.compose_files.clone(),
            running: 0,
            total: 0,
            in_folder: false,
        });
        s.total += 1;
        if c.state == "running" {
            s.running += 1;
        }
    }
    for file in found {
        let Some(dir) = posix_parent(file) else { continue };
        if let Some(s) = by_name.values_mut().find(|s| s.dir == dir) {
            s.in_folder = true;
            if s.files.is_empty() {
                s.files = vec![file.clone()];
            }
            continue;
        }
        let mut name = project_from_dir(&dir);
        // Two folders with the same name: keep both, compose would clash.
        while by_name.contains_key(&name) {
            name.push('_');
        }
        by_name.insert(
            name.clone(),
            Stack { name, dir: dir.to_owned(), files: vec![file.clone()], running: 0, total: 0, in_folder: true },
        );
    }
    by_name.into_values().collect()
}

/// The shell command for one compose operation on a stack. Output of both
/// streams is merged so it reads in order.
pub fn compose_command(stack: &Stack, op: ComposeOp, route: &Route) -> String {
    compose_shell(stack, op.args(), route)
}

/// `docker compose -p NAME [-f …] ARGS` in the stack folder. `args` is a
/// fixed string from this crate, never user input.
pub fn compose_shell(stack: &Stack, args: &str, route: &Route) -> String {
    let env = route.shell_env();
    let docker = route.docker_cli();
    // Through sudo, docker-compose needs its absolute path too.
    let legacy = match route {
        Route::Sudo(_) => "\"sudo -n $(command -v docker-compose)\"",
        _ => "docker-compose",
    };
    let files: String = stack.files.iter().map(|f| format!(" -f {}", shell_quote(f))).collect();
    format!(
        "exec 2>&1; {env}if {docker} compose version >/dev/null 2>&1; then dc=\"{docker} compose\"; \
         elif command -v docker-compose >/dev/null 2>&1; then dc={legacy}; \
         else echo 'neither \"docker compose\" nor docker-compose is installed'; exit 127; fi; \
         cd {} && $dc -p {}{files} {}",
        shell_quote(&stack.dir),
        shell_quote(&stack.name),
        args
    )
}

/// The PowerShell for one compose operation on a Windows host: `docker
/// compose` (or `docker-compose.exe`) in the stack folder, both streams
/// merged so output reads in order.
pub fn compose_ps(stack: &Stack, args: &str) -> String {
    use crate::win::quote;
    let files: String = stack.files.iter().map(|f| format!(" '-f', {},", quote(f))).collect();
    let words: String = args.split_whitespace().map(|a| format!(" {},", quote(a))).collect();
    let argv = format!("@('-p', {},{files}{words})", quote(&stack.name));
    let argv = argv.replace(",)", ")");
    format!(
        r#"$ErrorActionPreference = 'Continue'
$argv = {argv}
$dc = $null
& docker compose version *> $null
if ($LASTEXITCODE -eq 0) {{ $dc = 'docker'; $argv = @('compose') + $argv }}
elseif (Get-Command docker-compose.exe -ErrorAction SilentlyContinue) {{ $dc = 'docker-compose.exe' }}
else {{ [Console]::Out.WriteLine('neither "docker compose" nor docker-compose is installed'); exit 127 }}
Set-Location -LiteralPath {dir}
& $dc @argv 2>&1 | ForEach-Object {{ [Console]::Out.WriteLine([string]$_) }}
exit $LASTEXITCODE
"#,
        dir = quote(&stack.dir),
    )
}

/// Checks a stack reference that came from the UI before it reaches a shell.
pub fn check_stack(stack: &Stack) -> DockerResult<()> {
    let ok_path = |p: &str| {
        crate::backup_jobs::absolute_path(p) && p.len() <= 4096 && !p.chars().any(|c| c.is_control() || c == '`')
    };
    if !valid_project(&stack.name) {
        return Err(DockerError::other(format!("invalid project name {:?}", stack.name)));
    }
    if !ok_path(&stack.dir) || !stack.files.iter().all(|f| ok_path(f)) || stack.files.len() > 16 {
        return Err(DockerError::other("stack paths must be absolute"));
    }
    Ok(())
}

/// A new stack's compose file (and optional `.env`) for the deploy wizard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DeployInput {
    /// Folder the stack's own folder is created in (a stack folder, usually).
    pub parent: String,
    /// Project name, also the folder name.
    pub name: String,
    pub compose: String,
    /// `.env` content (written with mode 600), if any.
    pub env: Option<String>,
    /// Replace an existing compose.yaml / .env there (confirmed in the UI).
    pub overwrite: bool,
}

pub const MAX_DEPLOY_FILE: usize = 256 * 1024;

impl DeployInput {
    /// Checks names and sizes; returns the stack it will create.
    pub fn stack(&self) -> DockerResult<Stack> {
        if !valid_project(&self.name) {
            return Err(DockerError::other(
                "name: lowercase letters, digits, - and _, starting with a letter or digit",
            ));
        }
        if self.compose.trim().is_empty()
            || self.compose.len() > MAX_DEPLOY_FILE
            || self.env.as_ref().is_some_and(|e| e.len() > MAX_DEPLOY_FILE)
        {
            return Err(DockerError::other("the compose file is empty or too large"));
        }
        let parent = self.parent.trim_end_matches('/');
        let stack = Stack {
            name: self.name.clone(),
            dir: format!("{}/{}", if parent.is_empty() { "" } else { parent }, self.name),
            files: Vec::new(),
            running: 0,
            total: 0,
            in_folder: true,
        };
        check_stack(&stack)?;
        Ok(stack)
    }
}

impl DockerHost {
    /// Validates a stack's compose file on the host (`docker compose config -q`).
    /// Returns compose's complaint on failure.
    pub async fn validate_stack(&self, stack: &Stack) -> DockerResult<Result<(), String>> {
        check_stack(stack)?;
        if self.route == Route::WindowsCli {
            let o = crate::win::run(&self.conn, &compose_ps(stack, "config -q")).await.map_err(DockerError::other)?;
            let text = if o.stdout.trim().is_empty() { o.error_text() } else { o.stdout.trim().to_owned() };
            return Ok(if o.code == 0 { Ok(()) } else { Err(text) });
        }
        let (code, out, _) =
            self.conn.exec(&compose_shell(stack, "config -q", &self.route)).await.map_err(DockerError::other)?;
        Ok(if code == 0 { Ok(()) } else { Err(clean(&out)) })
    }

    /// The engine's platform, e.g. ("linux", "arm64").
    pub async fn platform(&self) -> DockerResult<(String, String)> {
        let v = self.docker.version().await.map_err(|e| DockerError::from_bollard(e, None))?;
        Ok((v.os.unwrap_or_else(|| "linux".into()), v.arch.unwrap_or_else(|| "amd64".into())))
    }

    /// Stacks from container labels plus compose files in `dirs`.
    pub async fn stacks(&self, dirs: &[String]) -> DockerResult<Vec<Stack>> {
        let containers = self.containers().await?;
        // Windows: stacks show from their containers' labels; folders aren't scanned.
        let found = if dirs.is_empty() || self.route == Route::WindowsCli {
            Vec::new()
        } else {
            let (_, out, _) = self.conn.exec(&scan_command(dirs)).await.map_err(DockerError::other)?;
            String::from_utf8_lossy(&out).lines().map(str::to_owned).filter(|l| l.starts_with('/')).collect()
        };
        Ok(merge_stacks(&containers, &found))
    }

    /// service → (image, image id) for a project's containers.
    async fn service_images(&self, project: &str) -> DockerResult<HashMap<String, (String, String)>> {
        let opts = ListContainersOptions {
            all: true,
            filters: Some(HashMap::from([("label".to_owned(), vec![format!("{LABEL_PROJECT}={project}")])])),
            ..Default::default()
        };
        let list = self.docker.list_containers(Some(opts)).await.map_err(|e| DockerError::from_bollard(e, None))?;
        Ok(list
            .into_iter()
            .filter_map(|c| {
                let service = c.labels.as_ref()?.get(LABEL_SERVICE)?.clone();
                Some((service, (c.image.unwrap_or_default(), c.image_id.unwrap_or_default())))
            })
            .collect())
    }

    /// Runs one compose operation, sending output lines to `sink`, until it
    /// exits or `cancel` flips to true (then the command gets SIGINT).
    /// Returns the exit code, plus image changes for `Update`.
    pub async fn compose(
        &self,
        stack: &Stack,
        op: ComposeOp,
        cancel: watch::Receiver<bool>,
        sink: &mut (dyn FnMut(Vec<String>) + Send),
    ) -> DockerResult<(i32, Option<Vec<ImageChange>>)> {
        self.compose_holding(stack, op, None, cancel, sink).await
    }

    /// [`Self::compose`], where Pull and Update pull only `pull_only`'s
    /// services when given (the rest are held, D-195; empty = all held).
    /// Held services keep their image: `up -d` only recreates services
    /// whose image changed.
    pub async fn compose_holding(
        &self,
        stack: &Stack,
        op: ComposeOp,
        pull_only: Option<&[String]>,
        mut cancel: watch::Receiver<bool>,
        sink: &mut (dyn FnMut(Vec<String>) + Send),
    ) -> DockerResult<(i32, Option<Vec<ImageChange>>)> {
        check_stack(stack)?;
        let pulls = matches!(op, ComposeOp::Pull | ComposeOp::Update);
        let pull_only = pull_only
            .filter(|_| pulls)
            .map(|l| l.iter().filter(|s| crate::holds::valid_service(s)).cloned().collect::<Vec<String>>());
        if let Some(list) = &pull_only {
            if list.is_empty() {
                sink(vec!["Every service of this stack is held: nothing pulled.".into()]);
                return Ok((0, (op == ComposeOp::Update).then(Vec::new)));
            }
            sink(vec![format!("Held services are left out; pulling {}.", list.join(", "))]);
        }
        let before = if op == ComposeOp::Update { Some(self.service_images(&stack.name).await?) } else { None };
        let code = match &pull_only {
            // Service names are checked (letters, digits, . _ -), so they
            // are safe as words in both shells.
            Some(list) => self.run_args(stack, &format!("pull {}", list.join(" ")), &mut cancel, sink).await?,
            None => self.run_op(stack, op, &mut cancel, sink).await?,
        };
        if op != ComposeOp::Update || code != 0 {
            return Ok((code, None));
        }
        sink(vec![String::new(), "── pull done, recreating changed services (up -d) ──".into()]);
        let code = self.run_op(stack, ComposeOp::Up, &mut cancel, sink).await?;
        let after = self.service_images(&stack.name).await?;
        let before = before.unwrap_or_default();
        let mut changes: Vec<ImageChange> = after
            .into_iter()
            .filter(|(svc, (_, id))| before.get(svc).map(|(_, b)| b) != Some(id))
            .map(|(service, (image, id))| ImageChange {
                before: before.get(&service).map(|(_, b)| short_id(b)),
                after: short_id(&id),
                service,
                image,
            })
            .collect();
        changes.sort_by(|a, b| a.service.cmp(&b.service));
        Ok((code, Some(changes)))
    }

    /// One compose operation through the host's shell (PowerShell on Windows).
    async fn run_op(
        &self,
        stack: &Stack,
        op: ComposeOp,
        cancel: &mut watch::Receiver<bool>,
        sink: &mut (dyn FnMut(Vec<String>) + Send),
    ) -> DockerResult<i32> {
        self.run_args(stack, op.args(), cancel, sink).await
    }

    /// `args` is built in this crate (fixed words and checked service names).
    async fn run_args(
        &self,
        stack: &Stack,
        args: &str,
        cancel: &mut watch::Receiver<bool>,
        sink: &mut (dyn FnMut(Vec<String>) + Send),
    ) -> DockerResult<i32> {
        if self.route != Route::WindowsCli {
            return self.run_streaming(&compose_shell(stack, args, &self.route), cancel, sink).await;
        }
        let mut send = |_: crate::system::sudo::Stream, line: String| {
            sink(vec![strip_ansi(line.rsplit('\r').find(|p| !p.is_empty()).unwrap_or(""))]);
            true
        };
        // Cancelling closes the channel, which ends compose on the host.
        let code = crate::win::stream(&self.conn, &compose_ps(stack, args), Some(cancel.clone()), &mut send)
            .await
            .map_err(DockerError::other)?;
        Ok(code.unwrap_or(130))
    }

    pub(crate) async fn run_streaming(
        &self,
        command: &str,
        cancel: &mut watch::Receiver<bool>,
        sink: &mut (dyn FnMut(Vec<String>) + Send),
    ) -> DockerResult<i32> {
        let mut ch = self.conn.open_exec(command).await.map_err(DockerError::other)?;
        let mut buf = Vec::<u8>::new();
        let mut code = None;
        let mut cancelled = false;
        loop {
            let msg = tokio::select! {
                m = ch.wait() => m,
                _ = cancel.changed(), if !cancelled => {
                    cancelled = true;
                    sink(vec!["^C cancelling…".into()]);
                    let _ = ch.signal(Sig::INT).await;
                    continue;
                }
            };
            match msg {
                Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                    buf.extend_from_slice(&data);
                    let lines = take_lines(&mut buf);
                    if !lines.is_empty() {
                        sink(lines);
                    }
                }
                Some(ChannelMsg::ExitStatus { exit_status }) => code = Some(exit_status as i32),
                Some(ChannelMsg::ExitSignal { .. }) => code = code.or(Some(130)),
                Some(_) => {}
                None => break,
            }
        }
        if !buf.is_empty() {
            sink(vec![clean(&buf)]);
        }
        Ok(code.unwrap_or(if cancelled { 130 } else { -1 }))
    }
}

fn short_id(id: &str) -> String {
    id.trim_start_matches("sha256:").chars().take(12).collect()
}

fn clean(line: &[u8]) -> String {
    let s = String::from_utf8_lossy(line);
    // Progress redraws use \r; keep the last state of the line.
    let s = s.rsplit('\r').find(|p| !p.is_empty()).unwrap_or("");
    strip_ansi(s)
}

fn take_lines(buf: &mut Vec<u8>) -> Vec<String> {
    let mut out = Vec::new();
    while let Some(i) = buf.iter().position(|b| *b == b'\n') {
        let line: Vec<u8> = buf.drain(..=i).collect();
        out.push(clean(&line[..line.len() - 1]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docker::PATH_FIX;

    fn win_stack() -> Stack {
        Stack {
            name: "web".into(),
            dir: "C:\\stacks\\web".into(),
            files: vec!["C:\\stacks\\web\\compose.yaml".into()],
            running: 0,
            total: 0,
            in_folder: false,
        }
    }

    #[test]
    fn windows_compose_script() {
        let st = win_stack();
        check_stack(&st).unwrap();
        let s = compose_ps(&st, "up -d");
        assert!(s.contains("$argv = @('-p', 'web', '-f', 'C:\\stacks\\web\\compose.yaml', 'up', '-d')"), "{s}");
        assert!(s.contains("Set-Location -LiteralPath 'C:\\stacks\\web'"));
        assert!(!s.contains("&&") && !s.contains("??"));
        let mut bad = win_stack();
        bad.dir = "stacks\\web".into();
        assert!(check_stack(&bad).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_compose_script_runs() {
        // Runners may lack compose: the script must still run and say so, or run it.
        let mut st = win_stack();
        st.dir = std::env::temp_dir().to_string_lossy().into_owned();
        st.files.clear();
        let o = crate::win::run_local(&compose_ps(&st, "version"), b"").unwrap();
        assert!(o.code == 0 || o.stdout.contains("neither"), "{} / {}", o.stdout, o.stderr);
    }

    fn view(name: &str, project: &str, dir: &str, state: &str) -> ContainerView {
        ContainerView {
            id: name.into(),
            name: name.into(),
            image: "img".into(),
            state: state.into(),
            status: String::new(),
            health: None,
            created: 0,
            ports: vec![],
            project: Some(project.into()),
            service: Some(name.into()),
            compose_dir: Some(dir.into()),
            compose_files: vec![format!("{dir}/compose.yaml")],
            web: vec![],
        }
    }

    #[test]
    fn project_names() {
        assert!(valid_project("media"));
        assert!(valid_project("my_stack-2"));
        assert!(!valid_project("Media"));
        assert!(!valid_project("-x"));
        assert!(!valid_project("a b"));
        assert_eq!(project_from_dir("/opt/stacks/My.Stack"), "mystack");
        assert_eq!(project_from_dir("/opt/stacks/home-assistant/"), "home-assistant");
        assert_eq!(project_from_dir("/"), "stack");
    }

    #[test]
    fn merges_labels_and_folders() {
        let containers = vec![
            view("plex", "media", "/opt/stacks/media", "running"),
            view("sonarr", "media", "/opt/stacks/media", "exited"),
            view("homepage", "dash", "/srv/dash", "running"),
        ];
        let found =
            vec!["/opt/stacks/media/compose.yaml".to_owned(), "/opt/stacks/vault/docker-compose.yml".to_owned()];
        let stacks = merge_stacks(&containers, &found);
        let names: Vec<_> = stacks.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["dash", "media", "vault"]);
        let media = &stacks[1];
        assert_eq!((media.running, media.total, media.in_folder), (1, 2, true));
        let vault = &stacks[2];
        assert_eq!(vault.dir, "/opt/stacks/vault");
        assert_eq!(vault.files, vec!["/opt/stacks/vault/docker-compose.yml"]);
        assert_eq!((vault.running, vault.total), (0, 0));
        assert!(!stacks[0].in_folder);
    }

    #[test]
    fn commands_are_quoted() {
        let s = Stack {
            name: "media".into(),
            dir: "/opt/my stacks/media".into(),
            files: vec!["/opt/my stacks/media/compose.yaml".into()],
            running: 0,
            total: 0,
            in_folder: true,
        };
        let c = compose_command(&s, ComposeOp::Up, &Route::Socket("/var/run/docker.sock".into()));
        assert!(
            c.contains("cd '/opt/my stacks/media' && $dc -p 'media' -f '/opt/my stacks/media/compose.yaml' up -d"),
            "{c}"
        );
        assert!(c.starts_with(&format!("exec 2>&1; {PATH_FIX}if docker compose version")), "{c}");
        let r = compose_command(&s, ComposeOp::Down, &Route::Socket("/run/user/1000/docker.sock".into()));
        assert!(r.contains("export DOCKER_HOST='unix:///run/user/1000/docker.sock'; "));
        assert!(r.ends_with(" down"));
        let sudo = compose_command(&s, ComposeOp::Up, &Route::Sudo("/usr/local/bin/docker".into()));
        assert!(sudo.contains("if sudo -n /usr/local/bin/docker compose version"), "{sudo}");
        assert!(sudo.contains("dc=\"sudo -n /usr/local/bin/docker compose\""), "{sudo}");
        let bad = Stack { name: "x;reboot".into(), ..s.clone() };
        assert!(check_stack(&bad).is_err());
        let rel = Stack { dir: "media".into(), ..s };
        assert!(check_stack(&rel).is_err());
    }

    #[test]
    fn scan_expands_home_and_quotes() {
        let c = scan_command(&["~/stacks".into(), "/opt/it's".into()]);
        assert!(c.starts_with("for r in \"$HOME\"/'stacks' '/opt/it'\\''s'; do"), "{c}");
    }

    #[cfg(unix)]
    #[test]
    fn scan_finds_compose_files_locally() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::create_dir_all(root.join("b")).unwrap();
        std::fs::create_dir_all(root.join("c")).unwrap();
        std::fs::write(root.join("a/compose.yaml"), "").unwrap();
        std::fs::write(root.join("b/docker-compose.yml"), "").unwrap();
        std::fs::write(root.join("b/compose.yml"), "").unwrap();
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(scan_command(&[root.to_string_lossy().into_owned()]))
            .output()
            .unwrap();
        let found: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_owned).collect();
        let r = root.to_string_lossy();
        assert_eq!(found, vec![format!("{r}/a/compose.yaml"), format!("{r}/b/compose.yml")]);
    }

    #[test]
    fn output_lines_are_cleaned() {
        let mut buf = b"\x1b[32m pulled\x1b[0m\nhalf\rwhole line\npartial".to_vec();
        assert_eq!(take_lines(&mut buf), vec![" pulled", "whole line"]);
        assert_eq!(buf, b"partial");
        assert_eq!(short_id("sha256:0123456789abcdef"), "0123456789ab");
    }
}
