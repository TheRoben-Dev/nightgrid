//! Stacks from git (M10w): a compose stack whose files come from a git
//! repository checked out on the host. Git runs on the host with the host's
//! own credentials (deploy key or credential helper), so Nightgrid stores no
//! token. Checking fetches and lists the incoming commits; deploying moves the
//! checkout to one reviewed commit with `git reset --keep` (refused while
//! tracked files have local changes) and the stack is then brought up.

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::compose::{quote_dir, Stack, COMPOSE_FILES};
use crate::files::path::shell_quote;
use crate::hosts::HostId;
use crate::stack_dirs::valid_stack_dir;
use crate::store::Store;
use crate::{Error, Result};

pub type GitStackId = i32;

/// Commits listed per check, newest first.
pub const MAX_COMMITS: usize = 50;
/// Diff text returned to the UI, in bytes.
pub const MAX_DIFF: usize = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GitStack {
    pub id: GitStackId,
    pub host_id: HostId,
    /// Checkout folder on the host (absolute or `~/…`).
    pub dir: String,
    pub url: String,
    pub branch: String,
    /// The stack's folder inside the repository, "" for the root.
    pub subdir: String,
    /// Commit of the last deploy from Nightgrid.
    pub deployed: Option<String>,
    /// Commit before that, for "Roll back".
    pub previous: Option<String>,
    /// Unix seconds.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub deployed_at: Option<i64>,
}

impl GitStack {
    /// Folder holding the compose file, as typed (may start with `~/`).
    pub fn stack_dir(&self) -> String {
        if self.subdir.is_empty() {
            self.dir.clone()
        } else {
            format!("{}/{}", self.dir.trim_end_matches('/'), self.subdir)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GitStackInput {
    pub url: String,
    pub branch: String,
    pub dir: String,
    #[serde(default)]
    pub subdir: String,
}

fn inv(field: &'static str, reason: impl Into<String>) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

/// `https://`, `http://`, `ssh://`, `git://`, `file://`, `user@host:path` or an
/// absolute path on the host. No spaces, control characters or leading `-`.
pub fn valid_url(u: &str) -> bool {
    if u.is_empty() || u.len() > 2048 || u.starts_with('-') || u.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return false;
    }
    let scheme = ["https://", "http://", "ssh://", "git://", "file://"].iter().any(|s| u.starts_with(s));
    let scp = {
        // user@host:path (no scheme, a colon before any slash)
        let colon = u.find(':');
        let slash = u.find('/');
        colon.is_some_and(|c| c > 0 && slash.map_or(true, |s| c < s)) && !u.contains("://")
    };
    scheme || scp || u.starts_with('/')
}

/// A branch name git accepts (a conservative subset of check-ref-format).
pub fn valid_branch(b: &str) -> bool {
    !b.is_empty()
        && b.len() <= 200
        && !b.starts_with(['-', '/', '.'])
        && !b.ends_with(['/', '.'])
        && !b.ends_with(".lock")
        && !b.contains("..")
        && !b.contains("//")
        && !b.contains("@{")
        && b.chars().all(|c| c.is_ascii_alphanumeric() || "._-/+".contains(c))
}

/// Relative folder inside the repo: no `..`, no leading `/`.
pub fn valid_subdir(s: &str) -> bool {
    s.is_empty()
        || (s.len() <= 512
            && !s.starts_with('/')
            && !s.ends_with('/')
            && s.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
            && !s.chars().any(|c| c.is_control()))
}

/// A full commit id.
pub fn valid_commit(c: &str) -> bool {
    (c.len() == 40 || c.len() == 64) && c.chars().all(|x| x.is_ascii_hexdigit() && !x.is_ascii_uppercase())
}

/// Trims and checks the input.
pub fn normalize(i: &GitStackInput) -> Result<GitStackInput> {
    let url = i.url.trim().to_owned();
    let branch = i.branch.trim().to_owned();
    let dir = i.dir.trim();
    let dir = if dir.len() > 1 { dir.trim_end_matches('/') } else { dir }.to_owned();
    let subdir = i.subdir.trim().trim_matches('/').to_owned();
    if !valid_url(&url) {
        return Err(inv("url", "use an https://, ssh:// or user@host:path address"));
    }
    if !valid_branch(&branch) {
        return Err(inv("branch", format!("{branch:?} isn't a branch name")));
    }
    if !valid_stack_dir(&dir) || dir == "/" || dir == "~" {
        return Err(inv("dir", "use a folder on the host: an absolute path or one starting with ~/"));
    }
    if !valid_subdir(&subdir) {
        return Err(inv("subdir", "use a folder inside the repository, like stacks/media"));
    }
    Ok(GitStackInput { url, branch, dir, subdir })
}

// ---- host scripts -------------------------------------------------------------

/// Git never prompts (no terminal) and SSH fails instead of asking.
const PRELUDE: &str = "export GIT_TERMINAL_PROMPT=0 GIT_SSH_COMMAND='ssh -o BatchMode=yes' LC_ALL=C; \
                       command -v git >/dev/null 2>&1 || { echo @@nogit; exit 0; }";

fn pathspec(subdir: &str) -> String {
    if subdir.is_empty() {
        ".".into()
    } else {
        shell_quote(subdir)
    }
}

fn remote_ref(branch: &str) -> String {
    format!("refs/remotes/origin/{branch}")
}

/// Prints the stack folder and its compose file, from inside the checkout.
fn locate(subdir: &str) -> String {
    let names = COMPOSE_FILES.join(" ");
    format!(
        "if cd -- {sub} 2>/dev/null; then echo \"@@dir $(pwd -P)\"; for f in {names}; do \
         if [ -f \"$f\" ]; then echo \"@@compose $(pwd -P)/$f\"; break; fi; done; fi",
        sub = if subdir.is_empty() { ".".into() } else { shell_quote(subdir) },
    )
}

/// Clones `url` into `dir`, or adopts an existing checkout of the same URL.
pub fn clone_script(i: &GitStackInput) -> String {
    let url = shell_quote(&i.url);
    format!(
        "{PRELUDE}\nd={dir}\n\
         if [ -d \"$d/.git\" ]; then u=$(git -C \"$d\" remote get-url origin 2>/dev/null); \
         if [ \"$u\" = {url} ]; then echo @@adopted; else echo \"@@other $u\"; exit 0; fi; \
         elif [ -e \"$d\" ] && [ -n \"$(ls -A \"$d\" 2>/dev/null)\" ]; then echo @@notempty; exit 0; \
         else mkdir -p -- \"$(dirname -- \"$d\")\" || exit 0; \
         if out=$(git clone --quiet --branch {branch} --single-branch -- {url} \"$d\" 2>&1); then echo @@cloned; \
         else printf '%s\\n' \"$out\" | sed 's/^/@@err /'; exit 0; fi; fi\n\
         cd -- \"$d\" || exit 0\n{locate}\n",
        dir = quote_dir(&i.dir),
        branch = shell_quote(&i.branch),
        locate = locate(&i.subdir),
    )
}

/// Fetches the branch and reports where the checkout stands.
pub fn status_script(g: &GitStack) -> String {
    let rref = remote_ref(&g.branch);
    let spec = shell_quote(&format!("+refs/heads/{}:{rref}", g.branch));
    let ps = pathspec(&g.subdir);
    format!(
        "{PRELUDE}\ncd -- {dir} 2>/dev/null || {{ echo @@missing; exit 0; }}\n\
         git rev-parse --git-dir >/dev/null 2>&1 || {{ echo @@notrepo; exit 0; }}\n\
         echo \"@@branch $(git rev-parse --abbrev-ref HEAD)\"\n\
         echo \"@@head $(git rev-parse HEAD)\"\n\
         if ! out=$(git fetch --quiet origin {spec} 2>&1); then printf '%s\\n' \"$out\" | sed 's/^/@@err /'; fi\n\
         r=$(git rev-parse -q --verify {rref_q})\n\
         if [ -n \"$r\" ]; then echo \"@@remote $r\"; \
         echo \"@@ahead $(git rev-list --count \"$r\"..HEAD)\"; \
         echo \"@@behind $(git rev-list --count HEAD..\"$r\" -- {ps})\"; \
         git log -n {MAX_COMMITS} --format='@@commit %H%x09%at%x09%an%x09%s' HEAD..\"$r\" -- {ps}; \
         git diff --numstat HEAD \"$r\" -- {ps} | head -n 300 | sed 's/^/@@stat /'; fi\n\
         git log -n 1 --format='@@current %H%x09%at%x09%an%x09%s' HEAD\n\
         git status --porcelain --untracked-files=no | head -n 50 | sed 's/^/@@dirty /'\n\
         {locate}\n",
        dir = quote_dir(&g.dir),
        rref_q = shell_quote(&rref),
        locate = locate(&g.subdir),
    )
}

/// The diff from the checkout to `target`, within the stack's folder.
pub fn diff_script(g: &GitStack, target: &str) -> String {
    format!(
        "{PRELUDE}\ncd -- {dir} || exit 0\n\
         git cat-file -e {t}^{{commit}} 2>/dev/null || {{ echo @@unknown; exit 0; }}\n\
         git -c core.quotepath=off diff --no-color --no-ext-diff HEAD {t} -- {ps} | head -c {MAX_DIFF}\n",
        dir = quote_dir(&g.dir),
        t = shell_quote(target),
        ps = pathspec(&g.subdir),
    )
}

/// Prints the stack folder and compose file without touching the checkout.
pub fn locate_script(g: &GitStack) -> String {
    format!("cd -- {} 2>/dev/null || {{ echo @@missing; exit 0; }}\n{}\n", quote_dir(&g.dir), locate(&g.subdir))
}

/// The stack found by [`locate_script`], if the folder has a compose file.
pub fn parse_locate(out: &str) -> Option<Stack> {
    let mut dir = None;
    let mut file = None;
    for line in out.lines() {
        if let Some(d) = line.strip_prefix("@@dir ") {
            dir = Some(d.to_owned());
        } else if let Some(f) = line.strip_prefix("@@compose ") {
            file = Some(f.to_owned());
        }
    }
    file.as_ref()?;
    dir.map(|d| stack_for(&d, file.as_deref()))
}

/// Moves the checkout to `target` (forward to a fetched commit, or back to an
/// earlier one). Refused while tracked files have local changes.
pub fn deploy_script(g: &GitStack, target: &str) -> String {
    let t = shell_quote(target);
    format!(
        "{PRELUDE}\ncd -- {dir} 2>/dev/null || {{ echo @@missing; exit 0; }}\n\
         git cat-file -e {t}^{{commit}} 2>/dev/null || {{ echo @@unknown; exit 0; }}\n\
         r=$(git rev-parse -q --verify {rref})\n\
         git merge-base --is-ancestor {t} HEAD 2>/dev/null || {{ [ -n \"$r\" ] && git merge-base --is-ancestor {t} \"$r\"; }} \
         || {{ echo @@notonbranch; exit 0; }}\n\
         d=$(git status --porcelain --untracked-files=no)\n\
         if [ -n \"$d\" ]; then printf '%s\\n' \"$d\" | head -n 20 | sed 's/^/@@dirty /'; exit 0; fi\n\
         echo \"@@from $(git rev-parse HEAD)\"\n\
         if out=$(git reset --quiet --keep {t} 2>&1); then echo \"@@to $(git rev-parse HEAD)\"; \
         else printf '%s\\n' \"$out\" | sed 's/^/@@err /'; exit 0; fi\n\
         {locate}\n",
        dir = quote_dir(&g.dir),
        rref = shell_quote(&remote_ref(&g.branch)),
        locate = locate(&g.subdir),
    )
}

// ---- parsing ------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GitCommit {
    pub id: String,
    /// Unix seconds.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub at: i64,
    pub author: String,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GitFileStat {
    pub path: String,
    /// `None` for binary files.
    pub added: Option<u32>,
    pub removed: Option<u32>,
}

/// Where a checkout stands after a fetch.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    /// Checked-out branch (`HEAD` when detached).
    pub branch: String,
    pub head: String,
    pub current: Option<GitCommit>,
    /// Tip of the tracked branch on the remote, after the fetch.
    pub remote: Option<String>,
    /// Commits on the remote the checkout doesn't have, touching the stack's folder.
    pub behind: u32,
    pub ahead: u32,
    pub incoming: Vec<GitCommit>,
    pub files: Vec<GitFileStat>,
    /// Tracked files changed on the host (`git status --porcelain` lines).
    pub dirty: Vec<String>,
    /// Folder with the compose file (resolved on the host).
    pub stack_dir: Option<String>,
    pub compose_file: Option<String>,
    /// Fetch failure, with a hint.
    pub error: Option<String>,
}

/// Turns git's error text into one message with a hint where one helps.
pub fn explain(lines: &[String]) -> String {
    let text = lines.join("\n");
    let t = text.to_lowercase();
    let hint = if t.contains("permission denied (publickey)") {
        Some("The host has no SSH key this repository accepts. Add the host's public key (for example ~/.ssh/id_ed25519.pub on the host) as a deploy key in the repository's settings.")
    } else if t.contains("host key verification failed") {
        Some("The host doesn't know the git server's SSH host key yet. Connect once from the host (ssh -T git@<server>), check the fingerprint, and accept it.")
    } else if t.contains("could not read username")
        || t.contains("authentication failed")
        || t.contains("terminal prompts disabled")
    {
        Some("This repository needs a login. Nightgrid uses the host's own git credentials: an SSH deploy key, or a credential helper set up on the host.")
    } else if t.contains("remote branch") && t.contains("not found") || t.contains("couldn't find remote ref") {
        Some("That branch doesn't exist in the repository.")
    } else if t.contains("could not resolve host") {
        Some("The host can't resolve the git server's name.")
    } else {
        None
    };
    let text = text.trim().to_owned();
    match hint {
        Some(h) if text.is_empty() => h.to_owned(),
        Some(h) => format!("{text}\n{h}"),
        None if text.is_empty() => "git failed without saying why".into(),
        None => text,
    }
}

fn parse_commit(rest: &str) -> Option<GitCommit> {
    let mut p = rest.splitn(4, '\t');
    let id = p.next()?.to_owned();
    let at = p.next()?.parse().ok()?;
    let author = p.next()?.to_owned();
    let subject = p.next().unwrap_or("").to_owned();
    valid_commit(&id).then_some(GitCommit { id, at, author, subject })
}

/// Common failures every script reports the same way.
fn common_failure(out: &str) -> Option<String> {
    let first = out.lines().map(str::trim).find(|l| l.starts_with("@@"))?;
    match first {
        "@@nogit" => Some("git isn't installed on the host".into()),
        "@@missing" => Some("the checkout folder is gone from the host".into()),
        "@@notrepo" => Some("the folder on the host is no longer a git checkout".into()),
        _ => None,
    }
}

pub fn parse_status(out: &str) -> std::result::Result<GitStatus, String> {
    if let Some(e) = common_failure(out) {
        return Err(e);
    }
    let mut s = GitStatus::default();
    let mut errs = Vec::new();
    for line in out.lines() {
        let Some((tag, rest)) = line.split_once(' ').or(Some((line, ""))) else { continue };
        match tag {
            "@@branch" => s.branch = rest.trim().to_owned(),
            "@@head" => s.head = rest.trim().to_owned(),
            "@@remote" => s.remote = Some(rest.trim().to_owned()).filter(|r| valid_commit(r)),
            "@@ahead" => s.ahead = rest.trim().parse().unwrap_or(0),
            "@@behind" => s.behind = rest.trim().parse().unwrap_or(0),
            "@@commit" => s.incoming.extend(parse_commit(rest)),
            "@@current" => s.current = parse_commit(rest),
            "@@stat" => {
                let mut p = rest.splitn(3, '\t');
                let (a, r, path) = (p.next(), p.next(), p.next());
                if let Some(path) = path {
                    s.files.push(GitFileStat {
                        path: path.to_owned(),
                        added: a.and_then(|x| x.parse().ok()),
                        removed: r.and_then(|x| x.parse().ok()),
                    });
                }
            }
            "@@dirty" => s.dirty.push(rest.to_owned()),
            "@@dir" => s.stack_dir = Some(rest.to_owned()),
            "@@compose" => s.compose_file = Some(rest.to_owned()),
            "@@err" => errs.push(rest.to_owned()),
            _ => {}
        }
    }
    if !valid_commit(&s.head) {
        return Err("couldn't read the checkout's commit".into());
    }
    if !errs.is_empty() {
        s.error = Some(explain(&errs));
    }
    Ok(s)
}

/// What a clone or deploy left on the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    pub adopted: bool,
    pub from: Option<String>,
    pub to: Option<String>,
    pub stack_dir: Option<String>,
    pub compose_file: Option<String>,
}

pub fn parse_clone(out: &str) -> std::result::Result<Moved, String> {
    if let Some(e) = common_failure(out) {
        return Err(e);
    }
    let mut m = Moved { adopted: false, from: None, to: None, stack_dir: None, compose_file: None };
    let mut errs = Vec::new();
    let mut done = false;
    for line in out.lines() {
        let (tag, rest) = line.split_once(' ').unwrap_or((line, ""));
        match tag {
            "@@cloned" => done = true,
            "@@adopted" => {
                done = true;
                m.adopted = true;
            }
            "@@other" => {
                return Err(format!(
                    "the folder already holds a checkout of another repository ({})",
                    if rest.is_empty() { "no origin" } else { rest }
                ))
            }
            "@@notempty" => return Err("the folder already exists and isn't empty".into()),
            "@@err" => errs.push(rest.to_owned()),
            "@@dir" => m.stack_dir = Some(rest.to_owned()),
            "@@compose" => m.compose_file = Some(rest.to_owned()),
            _ => {}
        }
    }
    if !errs.is_empty() {
        return Err(explain(&errs));
    }
    if !done {
        return Err("git clone didn't finish".into());
    }
    Ok(m)
}

pub fn parse_deploy(out: &str) -> std::result::Result<Moved, String> {
    if let Some(e) = common_failure(out) {
        return Err(e);
    }
    let mut m = Moved { adopted: false, from: None, to: None, stack_dir: None, compose_file: None };
    let mut errs = Vec::new();
    let mut dirty = Vec::new();
    for line in out.lines() {
        let (tag, rest) = line.split_once(' ').unwrap_or((line, ""));
        match tag {
            "@@unknown" => return Err("that commit isn't on the host; check for changes again".into()),
            "@@notonbranch" => return Err("that commit isn't on the tracked branch".into()),
            "@@dirty" => dirty.push(rest.trim().to_owned()),
            "@@from" => m.from = Some(rest.trim().to_owned()),
            "@@to" => m.to = Some(rest.trim().to_owned()),
            "@@err" => errs.push(rest.to_owned()),
            "@@dir" => m.stack_dir = Some(rest.to_owned()),
            "@@compose" => m.compose_file = Some(rest.to_owned()),
            _ => {}
        }
    }
    if !dirty.is_empty() {
        return Err(format!(
            "files in the checkout were changed on the host, so nothing was moved: {}. Commit or undo them there first.",
            dirty.join(", ")
        ));
    }
    if !errs.is_empty() {
        return Err(explain(&errs));
    }
    if m.to.is_none() {
        return Err("git didn't report the new commit".into());
    }
    Ok(m)
}

/// The compose stack for a resolved folder and file (project name from the folder).
pub fn stack_for(stack_dir: &str, compose_file: Option<&str>) -> Stack {
    Stack {
        name: super::compose::project_from_dir(stack_dir),
        dir: stack_dir.to_owned(),
        files: compose_file.map(|f| vec![f.to_owned()]).unwrap_or_default(),
        running: 0,
        total: 0,
        in_folder: true,
    }
}

// ---- store --------------------------------------------------------------------

const COLS: &str = "id, host_id, dir, url, branch, subdir, deployed, previous, deployed_at";

fn row(r: &Row<'_>) -> rusqlite::Result<GitStack> {
    Ok(GitStack {
        id: r.get(0)?,
        host_id: r.get(1)?,
        dir: r.get(2)?,
        url: r.get(3)?,
        branch: r.get(4)?,
        subdir: r.get(5)?,
        deployed: r.get(6)?,
        previous: r.get(7)?,
        deployed_at: r.get(8)?,
    })
}

impl Store {
    pub fn git_stacks(&self, host_id: HostId) -> Result<Vec<GitStack>> {
        let mut stmt =
            self.conn().prepare(&format!("SELECT {COLS} FROM git_stacks WHERE host_id = ?1 ORDER BY dir, subdir"))?;
        let rows = stmt.query_map([host_id], row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn git_stack(&self, id: GitStackId) -> Result<GitStack> {
        self.conn()
            .query_row(&format!("SELECT {COLS} FROM git_stacks WHERE id = ?1"), [id], row)
            .optional()?
            .ok_or(Error::NotFound("git stack"))
    }

    /// Records a checkout (after the clone succeeded). Input must be normalized.
    pub fn add_git_stack(&mut self, host_id: HostId, i: &GitStackInput) -> Result<GitStack> {
        let exists: bool = self.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM git_stacks WHERE host_id = ?1 AND dir = ?2 AND subdir = ?3)",
            params![host_id, i.dir, i.subdir],
            |r| r.get(0),
        )?;
        if exists {
            return Err(inv("dir", "that folder is already tracked"));
        }
        self.conn().execute(
            "INSERT INTO git_stacks (host_id, dir, url, branch, subdir) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![host_id, i.dir, i.url, i.branch, i.subdir],
        )?;
        self.git_stack(self.conn().last_insert_rowid() as GitStackId)
    }

    /// Notes a deploy: `to` becomes the deployed commit, the old one `previous`.
    pub fn mark_git_deployed(&mut self, id: GitStackId, to: &str) -> Result<GitStack> {
        let n = self.conn().execute(
            "UPDATE git_stacks SET previous = CASE WHEN deployed IS NOT NULL AND deployed != ?2 THEN deployed ELSE previous END,
                                   deployed = ?2, deployed_at = unixepoch() WHERE id = ?1",
            params![id, to],
        )?;
        if n == 0 {
            return Err(Error::NotFound("git stack"));
        }
        self.git_stack(id)
    }

    /// Stops tracking; the checkout stays on the host.
    pub fn delete_git_stack(&mut self, id: GitStackId) -> Result<()> {
        self.conn().execute("DELETE FROM git_stacks WHERE id = ?1", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(dir: &str) -> GitStackInput {
        GitStackInput {
            url: "https://git.example.com/me/stacks.git".into(),
            branch: "main".into(),
            dir: dir.into(),
            subdir: String::new(),
        }
    }

    #[test]
    fn validates_urls_branches_and_folders() {
        for ok in [
            "https://github.com/a/b.git",
            "git@github.com:a/b.git",
            "ssh://git@host:2222/a/b",
            "/srv/git/b.git",
            "file:///srv/b",
        ] {
            assert!(valid_url(ok), "{ok}");
        }
        for bad in ["", "-oProxyCommand=x", "https://a b", "ftp//x", "relative/path", "a\nb"] {
            assert!(!valid_url(bad), "{bad:?}");
        }
        for ok in ["main", "release/1.2", "feature_x-y"] {
            assert!(valid_branch(ok), "{ok}");
        }
        for bad in ["", "-x", "a..b", "a/", ".hidden", "x.lock", "a b", "a@{1}", "a;b"] {
            assert!(!valid_branch(bad), "{bad:?}");
        }
        assert!(valid_subdir("") && valid_subdir("stacks/media"));
        for bad in ["/abs", "a/../b", "a//b", "./a", "a/"] {
            assert!(!valid_subdir(bad), "{bad:?}");
        }
        let n =
            normalize(&GitStackInput { subdir: "/stacks/media/".into(), dir: "~/stacks/repo/".into(), ..input("") })
                .unwrap();
        assert_eq!((n.dir.as_str(), n.subdir.as_str()), ("~/stacks/repo", "stacks/media"));
        assert!(normalize(&input("/")).is_err());
        assert!(normalize(&input("relative")).is_err());
    }

    #[test]
    fn parses_status_output() {
        let h = "a".repeat(40);
        let r = "b".repeat(40);
        let out = format!(
            "@@branch main\n@@head {h}\n@@remote {r}\n@@ahead 0\n@@behind 2\n\
             @@commit {r}\t1794800000\tRoben\tBump jellyfin to 10.10\n\
             @@commit {}\t1794790000\tRoben\tAdd healthcheck\n\
             @@stat 3\t1\tmedia/compose.yaml\n@@stat -\t-\tlogo.png\n\
             @@current {h}\t1794700000\tRoben\tInitial\n@@dirty  M media/.env.example\n\
             @@dir /srv/stacks/media\n@@compose /srv/stacks/media/compose.yaml\n",
            "c".repeat(40)
        );
        let s = parse_status(&out).unwrap();
        assert_eq!(s.behind, 2);
        assert_eq!(s.incoming.len(), 2);
        assert_eq!(s.incoming[0].subject, "Bump jellyfin to 10.10");
        assert_eq!(s.files[0], GitFileStat { path: "media/compose.yaml".into(), added: Some(3), removed: Some(1) });
        assert_eq!(s.files[1].added, None);
        assert_eq!(s.dirty, vec![" M media/.env.example"]);
        assert_eq!(s.compose_file.as_deref(), Some("/srv/stacks/media/compose.yaml"));
        assert_eq!(s.current.unwrap().subject, "Initial");
        assert!(s.error.is_none());

        let failed = format!("@@branch main\n@@head {h}\n@@err git@github.com: Permission denied (publickey).\n@@err fatal: Could not read from remote repository.\n");
        let s = parse_status(&failed).unwrap();
        assert!(s.error.unwrap().contains("deploy key"));
        assert_eq!(parse_status("@@nogit\n").unwrap_err(), "git isn't installed on the host");
    }

    #[test]
    fn parses_clone_and_deploy_failures() {
        assert!(parse_clone("@@notempty\n").unwrap_err().contains("isn't empty"));
        assert!(parse_clone("@@other https://elsewhere/x.git\n").unwrap_err().contains("elsewhere"));
        let e = parse_clone("@@err fatal: Remote branch nope not found in upstream origin\n").unwrap_err();
        assert!(e.contains("doesn't exist"), "{e}");
        assert!(parse_deploy("@@dirty  M compose.yaml\n").unwrap_err().contains("compose.yaml"));
        assert!(parse_deploy("@@notonbranch\n").is_err());
    }

    #[test]
    fn store_tracks_deploys() {
        let mut s = Store::open_in_memory().unwrap();
        s.conn().execute("INSERT INTO hosts (id, name, hostname) VALUES (1, 'h', 'h')", []).unwrap();
        let g = s.add_git_stack(1, &normalize(&input("/srv/repo")).unwrap()).unwrap();
        assert!(s.add_git_stack(1, &normalize(&input("/srv/repo")).unwrap()).is_err());
        let a = "a".repeat(40);
        let b = "b".repeat(40);
        let g1 = s.mark_git_deployed(g.id, &a).unwrap();
        assert_eq!((g1.deployed.as_deref(), g1.previous.as_deref()), (Some(a.as_str()), None));
        let g2 = s.mark_git_deployed(g.id, &b).unwrap();
        assert_eq!((g2.deployed.as_deref(), g2.previous.as_deref()), (Some(b.as_str()), Some(a.as_str())));
        let g3 = s.mark_git_deployed(g.id, &b).unwrap();
        assert_eq!(g3.previous.as_deref(), Some(a.as_str()));
        assert_eq!(s.git_stacks(1).unwrap().len(), 1);
        s.delete_git_stack(g.id).unwrap();
        assert!(s.git_stacks(1).unwrap().is_empty());
    }

    /// Runs the real scripts against a local bare repository.
    #[cfg(unix)]
    #[test]
    fn scripts_clone_check_and_deploy_with_real_git() {
        use std::process::Command;
        let t = tempfile::tempdir().unwrap();
        let root = t.path().canonicalize().unwrap();
        let sh = |script: &str| {
            let o = Command::new("sh").arg("-c").arg(script).env("HOME", &root).output().unwrap();
            String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr)
        };
        let git = |dir: &std::path::Path, args: &[&str]| {
            let o = Command::new("git")
                .current_dir(dir)
                .args(["-c", "user.name=T", "-c", "user.email=t@example.com", "-c", "init.defaultBranch=main"])
                .args(args)
                .env("HOME", &root)
                .output()
                .unwrap();
            assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
        };
        // Upstream: a bare repo fed from a work tree.
        let bare = root.join("up.git");
        let work = root.join("work");
        std::fs::create_dir_all(&work).unwrap();
        git(&root, &["init", "-q", "--bare", bare.to_str().unwrap()]);
        git(&work, &["init", "-q"]);
        std::fs::create_dir_all(work.join("media")).unwrap();
        std::fs::write(work.join("media/compose.yaml"), "services:\n  a:\n    image: nginx:1\n").unwrap();
        git(&work, &["add", "-A"]);
        git(&work, &["commit", "-qm", "first"]);
        git(&work, &["remote", "add", "origin", bare.to_str().unwrap()]);
        git(&work, &["push", "-q", "origin", "HEAD:main"]);

        let i = normalize(&GitStackInput {
            url: bare.to_string_lossy().into_owned(),
            branch: "main".into(),
            dir: "~/stacks/repo".into(),
            subdir: "media".into(),
        })
        .unwrap();
        let m = parse_clone(&sh(&clone_script(&i))).unwrap();
        assert!(!m.adopted);
        let media = root.join("stacks/repo/media");
        assert_eq!(m.stack_dir.as_deref(), Some(media.to_str().unwrap()));
        assert_eq!(m.compose_file, Some(format!("{}/compose.yaml", media.display())));
        // Cloning again adopts the same checkout.
        assert!(parse_clone(&sh(&clone_script(&i))).unwrap().adopted);

        let g = GitStack {
            id: 1,
            host_id: 1,
            dir: i.dir.clone(),
            url: i.url.clone(),
            branch: "main".into(),
            subdir: "media".into(),
            deployed: None,
            previous: None,
            deployed_at: None,
        };
        let s = parse_status(&sh(&status_script(&g))).unwrap();
        assert_eq!((s.behind, s.ahead, s.error.as_deref()), (0, 0, None));
        let first = s.head.clone();

        // A new upstream commit shows as incoming, with the file it changes.
        std::fs::write(work.join("media/compose.yaml"), "services:\n  a:\n    image: nginx:2\n").unwrap();
        std::fs::write(work.join("README.md"), "outside the stack folder\n").unwrap();
        git(&work, &["add", "-A"]);
        git(&work, &["commit", "-qm", "bump nginx"]);
        git(&work, &["push", "-q", "origin", "HEAD:main"]);
        let s = parse_status(&sh(&status_script(&g))).unwrap();
        assert_eq!(s.behind, 1);
        assert_eq!(s.incoming[0].subject, "bump nginx");
        assert_eq!(s.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), vec!["media/compose.yaml"]);
        let remote = s.remote.clone().unwrap();
        let diff = sh(&diff_script(&g, &remote));
        assert!(diff.contains("-    image: nginx:1") && diff.contains("+    image: nginx:2"), "{diff}");
        assert!(!diff.contains("README"));

        // Local edits block the deploy; undoing them lets it through.
        let compose = media.join("compose.yaml");
        std::fs::write(&compose, "edited on the host\n").unwrap();
        assert!(parse_deploy(&sh(&deploy_script(&g, &remote))).unwrap_err().contains("compose.yaml"));
        git(&media, &["checkout", "--", "compose.yaml"]);
        let m = parse_deploy(&sh(&deploy_script(&g, &remote))).unwrap();
        assert_eq!((m.from.as_deref(), m.to.as_deref()), (Some(first.as_str()), Some(remote.as_str())));
        assert!(std::fs::read_to_string(&compose).unwrap().contains("nginx:2"));
        // Rolling back to the first commit works too; a made-up commit doesn't.
        let m = parse_deploy(&sh(&deploy_script(&g, &first))).unwrap();
        assert_eq!(m.to.as_deref(), Some(first.as_str()));
        assert!(std::fs::read_to_string(&compose).unwrap().contains("nginx:1"));
        assert!(parse_deploy(&sh(&deploy_script(&g, &"d".repeat(40)))).is_err());
        let st = parse_locate(&sh(&locate_script(&g))).unwrap();
        assert_eq!((st.name.as_str(), st.dir.as_str()), ("media", media.to_str().unwrap()));
    }
}
