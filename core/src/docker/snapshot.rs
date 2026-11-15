//! Stack snapshots and rollback (M10f). A snapshot records which image each
//! service of a compose project runs (image reference, image id and the
//! registry digest it was pulled as). Rolling back re-points each image tag
//! at the recorded image (pulling it by digest if it was pruned) and runs
//! `docker compose up -d`, which recreates only the services that changed.
//! One is taken automatically before "update stack" and before a rollback.
//!
//! The compose files and `.env` are copied too, but on the host, under
//! `~/.local/state/nightgrid/snapshots/<project>/<time>/` (mode 700): they can
//! hold secrets, which never come into the local database (rule 1).

use std::collections::HashMap;

use bollard::query_parameters::{CreateImageOptions, ListContainersOptions, TagImageOptions};
use futures_util::StreamExt;
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::compose::{check_stack, compose_command, ComposeOp, Stack};
use super::registry::{ImageRef, LocalImage};
use super::{DockerError, DockerHost, DockerResult, LABEL_PROJECT, LABEL_SERVICE};
use crate::files::path::shell_quote;
use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

pub type SnapshotId = i32;
/// Snapshots kept per stack; older ones are dropped.
pub const KEEP: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SnapshotService {
    pub service: String,
    /// The image reference the container was created from (`nginx:1.27`).
    pub image: String,
    /// Full image id (`sha256:…`).
    pub image_id: String,
    /// `repo@sha256:…` it was pulled as, if any (to re-pull a pruned image).
    pub repo_digest: Option<String>,
}

/// A stack file copied on the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SnapshotFile {
    /// Where the file lives in the stack.
    pub path: String,
    /// The copy on the host.
    pub saved: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: SnapshotId,
    pub host_id: HostId,
    pub project: String,
    /// `manual`, `before update` or `before rollback`.
    pub reason: String,
    pub services: Vec<SnapshotService>,
    pub files: Vec<SnapshotFile>,
    /// Unix seconds.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub created_at: i64,
}

const COLS: &str = "id, host_id, project, reason, services, files, created_at";

fn row(r: &Row<'_>) -> rusqlite::Result<Snapshot> {
    let services: String = r.get(4)?;
    Ok(Snapshot {
        id: r.get(0)?,
        host_id: r.get(1)?,
        project: r.get(2)?,
        reason: r.get(3)?,
        services: serde_json::from_str(&services).unwrap_or_default(),
        files: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(),
        created_at: r.get(6)?,
    })
}

impl Store {
    /// Stores a snapshot and keeps only the newest [`KEEP`] for the stack.
    pub fn save_snapshot(
        &mut self,
        host_id: HostId,
        project: &str,
        reason: &str,
        services: &[SnapshotService],
        files: &[SnapshotFile],
    ) -> Result<Snapshot> {
        if services.is_empty() && files.is_empty() {
            return Err(Error::Invalid {
                field: "services",
                reason: "the stack has no containers or files to snapshot".into(),
            });
        }
        let json = serde_json::to_string(services).unwrap_or_else(|_| "[]".into());
        let fjson = serde_json::to_string(files).unwrap_or_else(|_| "[]".into());
        let id = self.tx(|tx| {
            tx.execute(
                "INSERT INTO stack_snapshots (host_id, project, reason, services, files) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![host_id, project, reason, json, fjson],
            )?;
            let id = tx.last_insert_rowid() as SnapshotId;
            tx.execute(
                "DELETE FROM stack_snapshots WHERE host_id = ?1 AND project = ?2 AND id NOT IN
                   (SELECT id FROM stack_snapshots WHERE host_id = ?1 AND project = ?2 ORDER BY created_at DESC, id DESC LIMIT ?3)",
                params![host_id, project, KEEP as i64],
            )?;
            Ok(id)
        })?;
        self.get_snapshot(id)
    }

    /// Newest first.
    pub fn list_snapshots(&self, host_id: HostId, project: &str) -> Result<Vec<Snapshot>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {COLS} FROM stack_snapshots WHERE host_id = ?1 AND project = ?2 ORDER BY created_at DESC, id DESC"
        ))?;
        let rows = stmt.query_map(params![host_id, project], row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn get_snapshot(&self, id: SnapshotId) -> Result<Snapshot> {
        self.conn()
            .query_row(&format!("SELECT {COLS} FROM stack_snapshots WHERE id = ?1"), [id], row)
            .optional()?
            .ok_or(Error::NotFound("snapshot"))
    }

    pub fn delete_snapshot(&mut self, id: SnapshotId) -> Result<()> {
        if self.conn().execute("DELETE FROM stack_snapshots WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("snapshot"));
        }
        Ok(())
    }
}

/// The digest entry of `repo_digests` for `image`'s repository, else the first.
fn digest_for(image: &str, repo_digests: &[String]) -> Option<String> {
    let want = ImageRef::parse(image).map(|r| r.repo);
    repo_digests
        .iter()
        .find(|d| {
            let repo = d.split('@').next().and_then(ImageRef::parse).map(|r| r.repo);
            repo.is_some() && repo == want
        })
        .or_else(|| repo_digests.first())
        .cloned()
}

/// Splits `repo[:tag]` for `docker tag` (tag defaults to `latest`).
fn repo_tag(image: &str) -> (String, String) {
    let name = image.split('@').next().unwrap_or(image);
    let slash = name.rfind('/').map_or(0, |i| i + 1);
    match name[slash..].rfind(':') {
        Some(i) => (name[..slash + i].to_owned(), name[slash + i + 1..].to_owned()),
        None => (name.to_owned(), "latest".to_owned()),
    }
}

/// Files worth keeping for a stack: its compose files (or the default names
/// in its folder) and `.env`.
pub fn stack_files(stack: &Stack) -> Vec<String> {
    let dir = stack.dir.trim_end_matches('/');
    let mut v: Vec<String> = if stack.files.is_empty() {
        super::compose::COMPOSE_FILES.iter().map(|f| format!("{dir}/{f}")).collect()
    } else {
        stack.files.clone()
    };
    v.push(format!("{dir}/.env"));
    v
}

/// Copies `files` that exist into a fresh snapshot folder on the host and
/// prunes old folders of the project beyond [`KEEP`]. Prints `F <path>\t<copy>`
/// per copied file and `S <path>` for unreadable ones.
pub fn save_files_command(project: &str, stamp: i64, files: &[String]) -> String {
    let mut s = format!(
        "umask 077; b=\"${{XDG_STATE_HOME:-$HOME/.local/state}}/nightgrid/snapshots/\"{p}; d=\"$b/{stamp}\"; mkdir -p \"$d\" || exit 1; i=0\n",
        p = shell_quote(project)
    );
    for f in files {
        let q = shell_quote(f);
        s.push_str(&format!(
            "if [ -f {q} ]; then if [ -r {q} ]; then i=$((i+1)); n=\"$d/$i-$(basename {q})\"; cp -p {q} \"$n\" && printf 'F %s\\t%s\\n' {q} \"$n\"; else printf 'S %s\\n' {q}; fi; fi\n"
        ));
    }
    s.push_str(&format!(
        "[ \"$i\" -gt 0 ] || rmdir \"$d\" 2>/dev/null; cd \"$b\" 2>/dev/null && ls -1 | grep -E '^[0-9]+$' | sort -rn | tail -n +{} | while read -r old; do rm -rf -- \"./$old\"; done; true\n",
        KEEP + 1
    ));
    s
}

pub fn parse_saved(out: &str) -> (Vec<SnapshotFile>, Vec<String>) {
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    for line in out.lines() {
        if let Some(rest) = line.strip_prefix("F ") {
            if let Some((path, saved)) = rest.split_once('\t') {
                files.push(SnapshotFile { path: path.to_owned(), saved: saved.to_owned() });
            }
        } else if let Some(p) = line.strip_prefix("S ") {
            skipped.push(p.to_owned());
        }
    }
    (files, skipped)
}

/// Copies saved files back over the stack's files.
pub fn restore_files_command(files: &[SnapshotFile]) -> String {
    let mut s = String::from("set -e\n");
    for f in files {
        let (src, dst) = (shell_quote(&f.saved), shell_quote(&f.path));
        s.push_str(&format!(
            "if [ -f {src} ]; then cp -p {src} {dst}; printf 'restored %s\\n' {dst}; else printf 'missing the copy of %s\\n' {dst}; fi\n"
        ));
    }
    s
}

impl DockerHost {
    /// Copies the stack's compose files and `.env` into a snapshot folder on
    /// the host. Returns the copies and the files that couldn't be read.
    pub async fn save_files(&self, stack: &Stack, stamp: i64) -> DockerResult<(Vec<SnapshotFile>, Vec<String>)> {
        check_stack(stack)?;
        let (_, out, _) = self
            .conn
            .exec(&save_files_command(&stack.name, stamp, &stack_files(stack)))
            .await
            .map_err(DockerError::other)?;
        Ok(parse_saved(&String::from_utf8_lossy(&out)))
    }

    async fn project_containers(&self, project: &str) -> DockerResult<Vec<(String, String, String)>> {
        let opts = ListContainersOptions {
            all: true,
            filters: Some(HashMap::from([("label".to_owned(), vec![format!("{LABEL_PROJECT}={project}")])])),
            ..Default::default()
        };
        let list = self.docker.list_containers(Some(opts)).await.map_err(|e| DockerError::from_bollard(e, None))?;
        let mut v: Vec<(String, String, String)> = list
            .into_iter()
            .filter_map(|c| {
                let service = c.labels.as_ref()?.get(LABEL_SERVICE)?.clone();
                Some((service, c.image.unwrap_or_default(), c.image_id.unwrap_or_default()))
            })
            .collect();
        v.sort();
        v.dedup_by(|a, b| a.0 == b.0);
        Ok(v)
    }

    /// What every service of `project` runs right now.
    pub async fn capture(&self, project: &str) -> DockerResult<Vec<SnapshotService>> {
        let mut out = Vec::new();
        for (service, image, image_id) in self.project_containers(project).await? {
            let digests = match self.docker.inspect_image(&image_id).await {
                Ok(i) => i.repo_digests.unwrap_or_default(),
                Err(_) => Vec::new(),
            };
            out.push(SnapshotService { repo_digest: digest_for(&image, &digests), service, image, image_id });
        }
        Ok(out)
    }

    /// The images a stack's services use, for the update check.
    pub async fn local_images(&self, project: &str) -> DockerResult<Vec<LocalImage>> {
        let mut by_image: Vec<LocalImage> = Vec::new();
        for (service, image, image_id) in self.project_containers(project).await? {
            if let Some(l) = by_image.iter_mut().find(|l| l.image == image) {
                l.services.push(service);
                continue;
            }
            let mut l = LocalImage { image: image.clone(), services: vec![service], ..Default::default() };
            if let Ok(i) = self.docker.inspect_image(&image_id).await {
                l.repo_digests = i.repo_digests.unwrap_or_default();
                l.labels = i.config.and_then(|c| c.labels).unwrap_or_default();
                l.os = i.os.unwrap_or_else(|| "linux".into());
                l.arch = i.architecture.unwrap_or_default();
                l.variant = i.variant;
                l.created = i.created;
            }
            by_image.push(l);
        }
        Ok(by_image)
    }

    /// Rolls `stack` back to `snap`: re-tags each changed service's image
    /// (pulling it by digest when it is gone), then `compose up -d`.
    /// Returns compose's exit code. Confirmed in the UI (rule 5).
    pub async fn rollback(
        &self,
        stack: &Stack,
        snap: &Snapshot,
        mut cancel: watch::Receiver<bool>,
        sink: &mut (dyn FnMut(Vec<String>) + Send),
    ) -> DockerResult<i32> {
        check_stack(stack)?;
        let mut changed = 0;
        if !snap.files.is_empty() {
            let (code, out, err) =
                self.conn.exec(&restore_files_command(&snap.files)).await.map_err(DockerError::other)?;
            sink(String::from_utf8_lossy(&out).lines().map(str::to_owned).collect());
            if code != 0 {
                return Err(DockerError::other(format!(
                    "restoring files failed: {}",
                    String::from_utf8_lossy(&err).trim()
                )));
            }
            changed += 1;
        }
        let now: HashMap<String, String> =
            self.project_containers(&stack.name).await?.into_iter().map(|(s, _, id)| (s, id)).collect();
        for s in &snap.services {
            if now.get(&s.service) == Some(&s.image_id) {
                sink(vec![format!("{}: already on {}", s.service, short(&s.image_id))]);
                continue;
            }
            if self.docker.inspect_image(&s.image_id).await.is_err() {
                let Some(d) = &s.repo_digest else {
                    return Err(DockerError::other(format!(
                        "{}: image {} was removed from the host and has no registry digest to pull it back",
                        s.service,
                        short(&s.image_id)
                    )));
                };
                sink(vec![format!("{}: pulling {d}", s.service)]);
                let opts = CreateImageOptions { from_image: Some(d.clone()), ..Default::default() };
                let mut st = self.docker.create_image(Some(opts), None, None);
                while let Some(r) = st.next().await {
                    r.map_err(|e| DockerError::from_bollard(e, None))?;
                    if *cancel.borrow() {
                        return Ok(130);
                    }
                }
            }
            let (repo, tag) = repo_tag(&s.image);
            if s.image.contains("@sha256:") {
                sink(vec![format!("{}: pinned by digest, nothing to re-tag", s.service)]);
            } else {
                self.docker
                    .tag_image(&s.image_id, Some(TagImageOptions { repo: Some(repo), tag: Some(tag) }))
                    .await
                    .map_err(|e| DockerError::from_bollard(e, None))?;
                sink(vec![format!("{}: {} → {}", s.service, s.image, short(&s.image_id))]);
            }
            changed += 1;
        }
        if changed == 0 {
            sink(vec!["Every service already runs the snapshot's image and no files were saved.".into()]);
            return Ok(0);
        }
        sink(vec![String::new(), "── recreating changed services (up -d) ──".into()]);
        self.run_streaming(&compose_command(stack, ComposeOp::Up, &self.route), &mut cancel, sink).await
    }
}

fn short(id: &str) -> String {
    id.trim_start_matches("sha256:").chars().take(12).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::HostInput;

    fn svc(n: &str, id: &str) -> SnapshotService {
        SnapshotService { service: n.into(), image: format!("{n}:latest"), image_id: id.into(), repo_digest: None }
    }

    #[test]
    fn store_keeps_newest() {
        let mut s = Store::open_in_memory().unwrap();
        let h =
            s.create_host(&HostInput { name: "nas".into(), hostname: "nas.lan".into(), ..Default::default() }).unwrap();
        let f = SnapshotFile {
            path: "/opt/media/compose.yaml".into(),
            saved: "/home/u/.local/state/x/1-compose.yaml".into(),
        };
        let a = s.save_snapshot(h.id, "media", "manual", &[svc("web", "sha256:1")], std::slice::from_ref(&f)).unwrap();
        assert_eq!(a.files, vec![f]);
        for i in 2..15 {
            s.save_snapshot(h.id, "media", "manual", &[svc("web", &format!("sha256:{i}"))], &[]).unwrap();
        }
        let list = s.list_snapshots(h.id, "media").unwrap();
        assert_eq!(list.len(), KEEP);
        assert_eq!(list[0].services[0].image_id, "sha256:14");
        assert!(s.save_snapshot(h.id, "media", "manual", &[], &[]).is_err());
        s.delete_snapshot(list[0].id).unwrap();
        s.delete_host(h.id).unwrap();
        assert!(s.list_snapshots(h.id, "media").unwrap().is_empty());
    }

    #[test]
    fn file_scripts_run_under_sh() {
        let dir = std::env::temp_dir().join(format!("ng-snap-{}", std::process::id()));
        let state = dir.join("state");
        std::fs::create_dir_all(dir.join("stack")).unwrap();
        let compose = dir.join("stack/compose.yaml");
        std::fs::write(&compose, "services: {}\n").unwrap();
        let stack = Stack {
            name: "demo".into(),
            dir: dir.join("stack").to_string_lossy().into_owned(),
            files: vec![],
            running: 0,
            total: 0,
            in_folder: true,
        };
        let run = |cmd: String| {
            let o = std::process::Command::new("sh").arg("-c").arg(cmd).env("XDG_STATE_HOME", &state).output().unwrap();
            assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
            String::from_utf8_lossy(&o.stdout).into_owned()
        };
        let (files, skipped) = parse_saved(&run(save_files_command("demo", 100, &stack_files(&stack))));
        assert_eq!(files.len(), 1, "{files:?}");
        assert!(skipped.is_empty());
        assert_eq!(files[0].path, compose.to_string_lossy());
        std::fs::write(&compose, "changed\n").unwrap();
        let out = run(restore_files_command(&files));
        assert!(out.contains("restored"));
        assert_eq!(std::fs::read_to_string(&compose).unwrap(), "services: {}\n");
        for t in 101..115 {
            run(save_files_command("demo", t, &stack_files(&stack)));
        }
        let kept = std::fs::read_dir(state.join("nightgrid/snapshots/demo")).unwrap().count();
        assert_eq!(kept, KEEP);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn digests_and_tags() {
        let d = vec!["ghcr.io/acme/other@sha256:x".to_owned(), "nginx@sha256:y".to_owned()];
        assert_eq!(digest_for("nginx:1.27", &d).as_deref(), Some("nginx@sha256:y"));
        assert_eq!(digest_for("redis", &d).as_deref(), Some("ghcr.io/acme/other@sha256:x"));
        assert_eq!(repo_tag("nginx"), ("nginx".into(), "latest".into()));
        assert_eq!(repo_tag("localhost:5000/app:dev"), ("localhost:5000/app".into(), "dev".into()));
        assert_eq!(repo_tag("ghcr.io/a/b:1.2"), ("ghcr.io/a/b".into(), "1.2".into()));
    }
}
