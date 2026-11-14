//! Transfer queue: copies files and folders between any two filesystems
//! (local ↔ remote, remote ↔ remote through the app) with progress, speed,
//! pause/resume/retry and conflict handling.
//!
//! Each job is one top-level item. Folders are copied recursively; symlinks
//! inside folders are followed for files and skipped for directories (no
//! loops). Resuming (after pause, failure or retry) continues each file from
//! the size already written at the destination.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{free_name, Entry, EntryKind, Fs, FsError, FsResult};
use crate::hosts::HostId;

pub type JobId = u32;

/// Which filesystem a path is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Side {
    Local,
    Host { id: HostId },
}

/// What to do when the destination already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ConflictPolicy {
    /// Pause the job and let the user compare and choose.
    #[default]
    Ask,
    /// Replace files (folders are merged, replacing files inside).
    Overwrite,
    Skip,
    /// Keep both: copy as `name (1).ext`.
    Rename,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum JobState {
    Queued,
    Running,
    Paused,
    /// Waiting for the user to resolve a conflict.
    Conflict,
    Done,
    Skipped,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn is_finished(self) -> bool {
        matches!(self, Self::Done | Self::Skipped | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ConflictInfo {
    pub src: Entry,
    pub dst: Entry,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NewJob {
    pub src: Side,
    pub src_path: String,
    pub dst: Side,
    /// Directory the item is copied into.
    pub dst_dir: String,
    #[serde(default)]
    pub policy: ConflictPolicy,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: JobId,
    pub src: Side,
    pub src_path: String,
    pub dst: Side,
    pub dst_dir: String,
    /// Name at the destination (may differ after a rename).
    pub name: String,
    pub is_dir: bool,
    pub state: JobState,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub bytes_done: u64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub bytes_total: u64,
    pub files_done: u32,
    pub files_total: u32,
    /// Bytes per second (smoothed) while running.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub speed: u64,
    pub error: Option<String>,
    pub conflict: Option<ConflictInfo>,
    /// Destination path of the file being copied.
    pub current: Option<String>,
}

pub type ResolveFuture = Pin<Box<dyn Future<Output = FsResult<Fs>> + Send>>;
/// Opens the filesystem for a side (connecting to the host if needed).
pub type Resolver = Arc<dyn Fn(Side) -> ResolveFuture + Send + Sync>;
/// Receives the full job list whenever it changes (throttled).
pub type Notify = Arc<dyn Fn(Vec<JobView>) + Send + Sync>;

const RUN: u8 = 0;
const PAUSE: u8 = 1;
const CANCEL: u8 = 2;
const CHUNK: usize = 256 * 1024;
const TICK: Duration = Duration::from_millis(250);

struct Control {
    signal: AtomicU8,
    bytes: AtomicU64,
}

struct Slot {
    view: JobView,
    policy: ConflictPolicy,
    resume: bool,
    /// Got past the conflict check (so a resume continues, never re-asks).
    started: bool,
    control: Arc<Control>,
    last_bytes: u64,
}

struct Inner {
    slots: Mutex<Vec<Slot>>,
    resolver: Resolver,
    notify: Mutex<Option<Notify>>,
    max_parallel: usize,
    next_id: AtomicU64,
    dirty: AtomicBool,
    ticker: AtomicBool,
}

#[derive(Clone)]
pub struct Transfers {
    inner: Arc<Inner>,
}

impl Transfers {
    pub fn new(resolver: Resolver, max_parallel: usize) -> Self {
        Self {
            inner: Arc::new(Inner {
                slots: Mutex::default(),
                resolver,
                notify: Mutex::default(),
                max_parallel: max_parallel.max(1),
                next_id: AtomicU64::new(1),
                dirty: AtomicBool::new(false),
                ticker: AtomicBool::new(false),
            }),
        }
    }

    /// Sets the listener for job list updates (and sends the current list).
    pub fn subscribe(&self, notify: Notify) {
        *self.inner.notify.lock().expect("poisoned") = Some(notify);
        self.inner.dirty.store(true, Ordering::Relaxed);
        self.inner.flush();
    }

    pub fn list(&self) -> Vec<JobView> {
        self.inner.snapshot()
    }

    /// Queues jobs. Must be called inside a Tokio runtime.
    pub fn add(&self, jobs: Vec<NewJob>) -> Vec<JobId> {
        let mut ids = Vec::new();
        {
            let mut slots = self.inner.slots.lock().expect("poisoned");
            for j in jobs {
                let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed) as JobId;
                let name = match j.src {
                    Side::Local => super::local::file_name(&j.src_path).to_owned(),
                    Side::Host { .. } => super::path::posix_file_name(&j.src_path).to_owned(),
                };
                slots.push(Slot {
                    view: JobView {
                        id,
                        src: j.src,
                        src_path: j.src_path,
                        dst: j.dst,
                        dst_dir: j.dst_dir,
                        name,
                        is_dir: false,
                        state: JobState::Queued,
                        bytes_done: 0,
                        bytes_total: 0,
                        files_done: 0,
                        files_total: 0,
                        speed: 0,
                        error: None,
                        conflict: None,
                        current: None,
                    },
                    policy: j.policy,
                    resume: false,
                    started: false,
                    control: Arc::new(Control { signal: AtomicU8::new(RUN), bytes: AtomicU64::new(0) }),
                    last_bytes: 0,
                });
                ids.push(id);
            }
        }
        self.inner.dirty.store(true, Ordering::Relaxed);
        Inner::start_ticker(&self.inner);
        Inner::pump(&self.inner);
        ids
    }

    fn update(&self, id: JobId, f: impl FnOnce(&mut Slot)) {
        if let Some(s) = self.inner.slots.lock().expect("poisoned").iter_mut().find(|s| s.view.id == id) {
            f(s);
        }
        self.inner.dirty.store(true, Ordering::Relaxed);
        Inner::pump(&self.inner);
        self.inner.flush();
    }

    pub fn pause(&self, id: JobId) {
        self.update(id, |s| match s.view.state {
            JobState::Running => s.control.signal.store(PAUSE, Ordering::Relaxed),
            JobState::Queued => s.view.state = JobState::Paused,
            _ => {}
        });
    }

    /// Resumes a paused job, or retries a failed one (continuing partial files).
    pub fn resume(&self, id: JobId) {
        self.update(id, |s| {
            if matches!(s.view.state, JobState::Paused | JobState::Failed) {
                s.view.state = JobState::Queued;
                s.view.error = None;
                s.resume = s.started;
            }
        });
    }

    pub fn cancel(&self, id: JobId) {
        self.update(id, |s| match s.view.state {
            JobState::Running => s.control.signal.store(CANCEL, Ordering::Relaxed),
            JobState::Queued | JobState::Paused | JobState::Conflict | JobState::Failed => {
                s.view.state = JobState::Cancelled;
                s.view.conflict = None;
            }
            _ => {}
        });
    }

    /// Answers a conflict. `all` applies the choice to every other job that is
    /// waiting on (or would hit) a conflict.
    pub fn resolve(&self, id: JobId, policy: ConflictPolicy, all: bool) {
        {
            let mut slots = self.inner.slots.lock().expect("poisoned");
            for s in slots.iter_mut() {
                let target = s.view.id == id;
                let waiting = s.view.state == JobState::Conflict;
                if target || (all && (waiting || (s.view.state == JobState::Queued && s.policy == ConflictPolicy::Ask)))
                {
                    s.policy = policy;
                    if waiting {
                        s.view.state = JobState::Queued;
                        s.view.conflict = None;
                    }
                }
            }
        }
        self.inner.dirty.store(true, Ordering::Relaxed);
        Inner::pump(&self.inner);
        self.inner.flush();
    }

    /// Removes finished (done, skipped, cancelled) jobs from the list.
    pub fn clear_finished(&self) {
        self.inner.slots.lock().expect("poisoned").retain(|s| !s.view.state.is_finished());
        self.inner.dirty.store(true, Ordering::Relaxed);
        self.inner.flush();
    }

    /// Active (queued or running) job count, for the status strip.
    pub fn active(&self) -> usize {
        self.inner
            .slots
            .lock()
            .expect("poisoned")
            .iter()
            .filter(|s| matches!(s.view.state, JobState::Queued | JobState::Running))
            .count()
    }
}

impl Inner {
    fn snapshot(&self) -> Vec<JobView> {
        self.slots
            .lock()
            .expect("poisoned")
            .iter()
            .map(|s| {
                let mut v = s.view.clone();
                if v.state == JobState::Running {
                    v.bytes_done = s.control.bytes.load(Ordering::Relaxed);
                }
                v
            })
            .collect()
    }

    fn flush(&self) {
        if !self.dirty.swap(false, Ordering::Relaxed) {
            return;
        }
        let notify = self.notify.lock().expect("poisoned").clone();
        if let Some(n) = notify {
            n(self.snapshot());
        }
    }

    fn start_ticker(this: &Arc<Self>) {
        if this.ticker.swap(true, Ordering::Relaxed) {
            return;
        }
        let weak: Weak<Self> = Arc::downgrade(this);
        tokio::spawn(async move {
            let mut last = Instant::now();
            loop {
                tokio::time::sleep(TICK).await;
                let Some(inner) = weak.upgrade() else { return };
                let dt = last.elapsed().as_secs_f64().max(0.001);
                last = Instant::now();
                {
                    let mut slots = inner.slots.lock().expect("poisoned");
                    for s in slots.iter_mut().filter(|s| s.view.state == JobState::Running) {
                        let now = s.control.bytes.load(Ordering::Relaxed);
                        let rate = now.saturating_sub(s.last_bytes) as f64 / dt;
                        let prev = s.view.speed as f64;
                        s.view.speed = if prev == 0.0 { rate } else { prev * 0.7 + rate * 0.3 } as u64;
                        s.last_bytes = now;
                        s.view.bytes_done = now;
                        inner.dirty.store(true, Ordering::Relaxed);
                    }
                }
                inner.flush();
            }
        });
    }

    /// Starts queued jobs up to the parallel limit.
    fn pump(this: &Arc<Self>) {
        let mut to_start = Vec::new();
        {
            let mut slots = this.slots.lock().expect("poisoned");
            let running = slots.iter().filter(|s| s.view.state == JobState::Running).count();
            let free = this.max_parallel.saturating_sub(running);
            for s in slots.iter_mut().filter(|s| s.view.state == JobState::Queued).take(free) {
                s.view.state = JobState::Running;
                s.view.speed = 0;
                s.control.signal.store(RUN, Ordering::Relaxed);
                to_start.push(s.view.id);
            }
        }
        for id in to_start {
            let inner = this.clone();
            tokio::spawn(async move {
                let outcome = run_job(&inner, id).await;
                inner.finish(id, outcome);
                Inner::pump(&inner);
                inner.flush();
            });
        }
    }

    fn finish(&self, id: JobId, outcome: FsResult<Outcome>) {
        let mut slots = self.slots.lock().expect("poisoned");
        let Some(s) = slots.iter_mut().find(|s| s.view.id == id) else { return };
        s.view.bytes_done = s.control.bytes.load(Ordering::Relaxed);
        s.view.speed = 0;
        s.view.current = None;
        match outcome {
            Ok(Outcome::Done) => s.view.state = JobState::Done,
            Ok(Outcome::Skipped) => s.view.state = JobState::Skipped,
            Ok(Outcome::Conflict(c)) => {
                s.view.state = JobState::Conflict;
                s.view.conflict = Some(*c);
            }
            Err(FsError::Cancelled) if s.control.signal.load(Ordering::Relaxed) == PAUSE => {
                s.view.state = JobState::Paused;
            }
            Err(FsError::Cancelled) => s.view.state = JobState::Cancelled,
            Err(e) => {
                s.view.state = JobState::Failed;
                s.view.error = Some(e.to_string());
            }
        }
        self.dirty.store(true, Ordering::Relaxed);
    }

    fn with_slot<T>(&self, id: JobId, f: impl FnOnce(&mut Slot) -> T) -> Option<T> {
        let mut slots = self.slots.lock().expect("poisoned");
        let r = slots.iter_mut().find(|s| s.view.id == id).map(f);
        self.dirty.store(true, Ordering::Relaxed);
        r
    }
}

enum Outcome {
    Done,
    Skipped,
    Conflict(Box<ConflictInfo>),
}

/// One file or directory to create, relative to the job's destination root.
struct Item {
    src: String,
    rel: Vec<String>,
    dir: bool,
    size: u64,
    mode: Option<u32>,
}

async fn walk(fs: &Fs, root_path: &str, out: &mut Vec<Item>, control: &Control) -> FsResult<()> {
    let mut stack = vec![(root_path.to_owned(), Vec::<String>::new())];
    while let Some((dir, rel)) = stack.pop() {
        for e in fs.list(&dir).await? {
            if control.signal.load(Ordering::Relaxed) != RUN {
                return Err(FsError::Cancelled);
            }
            let mut r = rel.clone();
            r.push(e.name.clone());
            match (e.kind, e.target_kind) {
                (EntryKind::Dir, _) => {
                    out.push(Item { src: e.path.clone(), rel: r.clone(), dir: true, size: 0, mode: e.mode });
                    stack.push((e.path, r));
                }
                (EntryKind::File, _) | (EntryKind::Symlink, Some(EntryKind::File)) => {
                    out.push(Item { src: e.path, rel: r, dir: false, size: e.size, mode: e.mode });
                }
                // Device files, sockets, dangling links and links to directories.
                _ => tracing::debug!(path = %e.path, "transfer: skipping non-regular entry"),
            }
        }
    }
    Ok(())
}

fn join_rel(fs: &Fs, root: &str, rel: &[String]) -> String {
    rel.iter().fold(root.to_owned(), |acc, part| fs.join(&acc, part))
}

async fn run_job(inner: &Arc<Inner>, id: JobId) -> FsResult<Outcome> {
    let Some((view, policy, resume, control)) =
        inner.with_slot(id, |s| (s.view.clone(), s.policy, s.resume, s.control.clone()))
    else {
        return Err(FsError::Cancelled);
    };
    let src_fs = (inner.resolver)(view.src).await?;
    let dst_fs = if view.dst == view.src { src_fs.clone() } else { (inner.resolver)(view.dst).await? };

    let src = src_fs.stat(&view.src_path).await?.ok_or_else(|| FsError::NotFound { path: view.src_path.clone() })?;
    let is_dir = src.kind == EntryKind::Dir;
    if !is_dir && src.kind != EntryKind::File {
        return Err(FsError::other(format!("{}: not a regular file or folder", view.src_path)));
    }

    let mut name = view.name.clone();
    let mut dst_root = dst_fs.join(&view.dst_dir, &name);
    if !resume {
        let same_place = view.src == view.dst && dst_root == view.src_path;
        if let Some(existing) = dst_fs.lstat(&dst_root).await? {
            let policy = if same_place { ConflictPolicy::Rename } else { policy };
            match policy {
                ConflictPolicy::Ask => return Ok(Outcome::Conflict(Box::new(ConflictInfo { src, dst: existing }))),
                ConflictPolicy::Skip => return Ok(Outcome::Skipped),
                ConflictPolicy::Rename => {
                    name = free_name(&dst_fs, &view.dst_dir, &name).await?;
                    dst_root = dst_fs.join(&view.dst_dir, &name);
                }
                ConflictPolicy::Overwrite => {
                    if (existing.kind == EntryKind::Dir) != is_dir {
                        return Err(FsError::Other {
                            message: format!(
                                "{dst_root}: cannot replace a {} with a {}",
                                kind_word(existing.kind),
                                kind_word(src.kind)
                            ),
                        });
                    }
                }
            }
        }
    }

    // Plan.
    let mut items = Vec::new();
    if is_dir {
        walk(&src_fs, &view.src_path, &mut items, &control).await?;
    } else {
        items.push(Item { src: view.src_path.clone(), rel: vec![], dir: false, size: src.size, mode: src.mode });
    }
    let total: u64 = items.iter().map(|i| i.size).sum();
    let files_total = items.iter().filter(|i| !i.dir).count() as u32;
    control.bytes.store(0, Ordering::Relaxed);
    inner.with_slot(id, |s| {
        s.view.name = name.clone();
        s.view.is_dir = is_dir;
        s.view.bytes_total = total;
        s.view.files_total = files_total;
        s.view.files_done = 0;
        s.last_bytes = 0;
        s.started = true;
    });
    inner.flush();

    if is_dir {
        dst_fs.mkdir_all(&dst_root).await?;
        if let Some(m) = src.mode {
            let _ = dst_fs.set_mode(&dst_root, m).await;
        }
    }
    let mut files_done = 0u32;
    for item in &items {
        let dst_path = join_rel(&dst_fs, &dst_root, &item.rel);
        if item.dir {
            dst_fs.mkdir_all(&dst_path).await?;
            if let Some(m) = item.mode {
                let _ = dst_fs.set_mode(&dst_path, m).await;
            }
            continue;
        }
        inner.with_slot(id, |s| s.view.current = Some(dst_path.clone()));
        copy_file(&src_fs, &item.src, &dst_fs, &dst_path, item.size, item.mode, resume, &control).await?;
        files_done += 1;
        inner.with_slot(id, |s| s.view.files_done = files_done);
    }
    Ok(Outcome::Done)
}

fn kind_word(k: EntryKind) -> &'static str {
    match k {
        EntryKind::Dir => "folder",
        EntryKind::File => "file",
        EntryKind::Symlink => "link",
        EntryKind::Other => "special file",
    }
}

#[allow(clippy::too_many_arguments)]
async fn copy_file(
    src_fs: &Fs,
    src: &str,
    dst_fs: &Fs,
    dst: &str,
    size: u64,
    mode: Option<u32>,
    resume: bool,
    control: &Control,
) -> FsResult<()> {
    let mut offset = 0;
    if resume {
        if let Some(existing) = dst_fs.stat(dst).await? {
            if existing.kind == EntryKind::File && existing.size <= size {
                offset = existing.size;
            }
        }
    }
    control.bytes.fetch_add(offset, Ordering::Relaxed);
    if offset == size && resume && size > 0 {
        return Ok(());
    }
    let mut r = src_fs.open_read(src, offset).await?;
    let mut w = dst_fs.open_write(dst, (offset > 0).then_some(offset)).await?;
    let mut buf = vec![0u8; CHUNK];
    loop {
        if control.signal.load(Ordering::Relaxed) != RUN {
            let _ = w.shutdown().await;
            if control.signal.load(Ordering::Relaxed) == CANCEL {
                let _ = dst_fs.remove(dst).await;
            }
            return Err(FsError::Cancelled);
        }
        let n = r.read(&mut buf).await.map_err(|e| io_err(src_fs, src, e))?;
        if n == 0 {
            break;
        }
        w.write_all(&buf[..n]).await.map_err(|e| io_err(dst_fs, dst, e))?;
        control.bytes.fetch_add(n as u64, Ordering::Relaxed);
    }
    w.shutdown().await.map_err(|e| io_err(dst_fs, dst, e))?;
    if let Some(m) = mode {
        let _ = dst_fs.set_mode(dst, m & 0o777).await;
    }
    Ok(())
}

fn io_err(fs: &Fs, path: &str, e: std::io::Error) -> FsError {
    if let Fs::Remote(r) = fs {
        if r.is_closed() {
            return FsError::Disconnected;
        }
    }
    FsError::from_io(path, e)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_transfers() -> Transfers {
        let resolver: Resolver = Arc::new(|_side| Box::pin(async { Ok(Fs::Local) }));
        Transfers::new(resolver, 2)
    }

    async fn wait_for(t: &Transfers, id: JobId, state: JobState) -> JobView {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let v = t.list().into_iter().find(|v| v.id == id).unwrap();
            if v.state == state {
                return v;
            }
            assert!(Instant::now() < deadline, "job {id} stuck in {:?} ({:?})", v.state, v.error);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    fn s(p: &std::path::Path) -> String {
        p.to_string_lossy().into_owned()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn copies_files_and_folders_and_handles_conflicts() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(src.path().join("stack/conf")).unwrap();
        std::fs::write(src.path().join("stack/compose.yaml"), "services: {}\n").unwrap();
        std::fs::write(src.path().join("stack/conf/app.env"), "A=1\n").unwrap();
        std::fs::write(src.path().join("notes.txt"), vec![b'x'; 700_000]).unwrap();

        let t = local_transfers();
        let job = |p: &str| NewJob {
            src: Side::Local,
            src_path: s(&src.path().join(p)),
            dst: Side::Local,
            dst_dir: s(dst.path()),
            policy: ConflictPolicy::Ask,
        };
        let ids = t.add(vec![job("stack"), job("notes.txt")]);
        let v = wait_for(&t, ids[0], JobState::Done).await;
        assert!(v.is_dir);
        assert_eq!((v.files_done, v.files_total), (2, 2));
        let v = wait_for(&t, ids[1], JobState::Done).await;
        assert_eq!(v.bytes_done, 700_000);
        assert_eq!(std::fs::read_to_string(dst.path().join("stack/conf/app.env")).unwrap(), "A=1\n");
        assert_eq!(std::fs::metadata(dst.path().join("notes.txt")).unwrap().len(), 700_000);

        // Same again: conflict → ask, then "keep both" for all.
        let ids = t.add(vec![job("notes.txt"), job("stack")]);
        let v = wait_for(&t, ids[0], JobState::Conflict).await;
        assert_eq!(v.conflict.as_ref().unwrap().dst.size, 700_000);
        wait_for(&t, ids[1], JobState::Conflict).await;
        t.resolve(ids[0], ConflictPolicy::Rename, true);
        assert_eq!(wait_for(&t, ids[0], JobState::Done).await.name, "notes (1).txt");
        assert_eq!(wait_for(&t, ids[1], JobState::Done).await.name, "stack (1)");
        assert!(dst.path().join("stack (1)/conf/app.env").exists());

        // Skip policy.
        let mut j = job("notes.txt");
        j.policy = ConflictPolicy::Skip;
        let id = t.add(vec![j])[0];
        wait_for(&t, id, JobState::Skipped).await;

        // Overwrite a folder: merges.
        std::fs::write(src.path().join("stack/compose.yaml"), "services:\n  web: {}\n").unwrap();
        let mut j = job("stack");
        j.policy = ConflictPolicy::Overwrite;
        let id = t.add(vec![j])[0];
        wait_for(&t, id, JobState::Done).await;
        assert_eq!(std::fs::read_to_string(dst.path().join("stack/compose.yaml")).unwrap(), "services:\n  web: {}\n");

        // Copying onto itself makes a numbered copy.
        let id = t.add(vec![NewJob {
            src: Side::Local,
            src_path: s(&dst.path().join("notes.txt")),
            dst: Side::Local,
            dst_dir: s(dst.path()),
            policy: ConflictPolicy::Overwrite,
        }])[0];
        assert_eq!(wait_for(&t, id, JobState::Done).await.name, "notes (2).txt");

        t.clear_finished();
        assert!(t.list().is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn resume_continues_partial_file_and_cancel_cleans_up() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        let data: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(src.path().join("big.bin"), &data).unwrap();
        // Pretend a previous attempt wrote the first megabyte.
        std::fs::write(dst.path().join("big.bin"), &data[..1_000_000]).unwrap();

        let t = local_transfers();
        let id = t.add(vec![NewJob {
            src: Side::Local,
            src_path: s(&src.path().join("big.bin")),
            dst: Side::Local,
            dst_dir: s(dst.path()),
            policy: ConflictPolicy::Ask,
        }])[0];
        wait_for(&t, id, JobState::Conflict).await;
        // Simulate "failed earlier" → retry resumes instead of conflicting.
        t.inner.with_slot(id, |s| {
            s.view.state = JobState::Failed;
            s.started = true;
        });
        t.resume(id);
        wait_for(&t, id, JobState::Done).await;
        assert_eq!(std::fs::read(dst.path().join("big.bin")).unwrap(), data);

        // Cancel before start removes nothing and ends cancelled.
        let id = t.add(vec![NewJob {
            src: Side::Local,
            src_path: s(&src.path().join("missing")),
            dst: Side::Local,
            dst_dir: s(dst.path()),
            policy: ConflictPolicy::Ask,
        }])[0];
        let v = wait_for(&t, id, JobState::Failed).await;
        assert!(v.error.unwrap().contains("no such file"));
        t.cancel(id);
        wait_for(&t, id, JobState::Cancelled).await;
    }
}
