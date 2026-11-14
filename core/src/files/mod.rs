//! File access for the browser, editor and transfer queue: one [`Fs`] type
//! over the local filesystem and remote hosts (SFTP on the shared SSH
//! connection), so every operation works the same in both panes and
//! transfers can go local ↔ remote or remote ↔ remote.

pub mod local;
pub mod path;
pub mod remote;
pub mod text;
pub mod transfer;

use std::sync::Arc;

use serde::Serialize;
use tokio::io::{AsyncRead, AsyncWrite};

pub use remote::RemoteFs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Dir,
    File,
    Symlink,
    Other,
}

impl EntryKind {
    /// From a POSIX `st_mode`.
    pub fn from_mode(mode: u32) -> Self {
        match mode & 0o170000 {
            0o040000 => Self::Dir,
            0o100000 => Self::File,
            0o120000 => Self::Symlink,
            _ => Self::Other,
        }
    }
}

/// One directory entry (or a `stat` result).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    /// For symlinks: what the link points at (`None` if dangling).
    pub target_kind: Option<EntryKind>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub size: u64,
    /// Seconds since the Unix epoch.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub mtime: Option<i64>,
    /// Permission bits (`0o7777`), when known.
    pub mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
}

impl Entry {
    /// Directory, or a symlink to one (navigable).
    pub fn is_dir_like(&self) -> bool {
        self.kind == EntryKind::Dir || self.target_kind == Some(EntryKind::Dir)
    }
}

/// File errors in a shape the UI can act on. Messages never contain secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum FsError {
    #[error("{path}: no such file or directory")]
    NotFound { path: String },
    #[error("{path}: permission denied")]
    PermissionDenied { path: String },
    #[error("{path}: already exists")]
    Exists { path: String },
    #[error("{path}: is a directory")]
    IsDir { path: String },
    #[error("{path} is {size} bytes; the editor opens files up to {limit} bytes")]
    TooLarge {
        path: String,
        #[cfg_attr(feature = "specta", specta(type = u32))]
        size: u64,
        #[cfg_attr(feature = "specta", specta(type = u32))]
        limit: u64,
    },
    #[error("{path} looks binary (or is not UTF-8) and can't be edited as text")]
    NotText { path: String },
    /// The file changed on disk since it was opened (save conflict).
    #[error("{path} changed since it was opened")]
    Changed { path: String, current: Box<Entry> },
    #[error("connection lost")]
    Disconnected,
    #[error("cancelled")]
    Cancelled,
    #[error("{message}")]
    Other { message: String },
}

impl FsError {
    pub fn other(e: impl std::fmt::Display) -> Self {
        Self::Other { message: e.to_string() }
    }

    pub(crate) fn from_io(path: &str, e: std::io::Error) -> Self {
        use std::io::ErrorKind as K;
        match e.kind() {
            K::NotFound => Self::NotFound { path: path.to_owned() },
            K::PermissionDenied => Self::PermissionDenied { path: path.to_owned() },
            K::AlreadyExists => Self::Exists { path: path.to_owned() },
            K::IsADirectory => Self::IsDir { path: path.to_owned() },
            _ => Self::Other { message: format!("{path}: {e}") },
        }
    }
}

pub type FsResult<T> = std::result::Result<T, FsError>;

pub type Reader = Box<dyn AsyncRead + Send + Unpin>;
pub type Writer = Box<dyn AsyncWrite + Send + Unpin>;

/// A filesystem: this machine or a remote host.
#[derive(Clone)]
pub enum Fs {
    Local,
    Remote(Arc<RemoteFs>),
}

impl Fs {
    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Remote(_))
    }

    /// Joins a child name onto a directory path in this filesystem's syntax.
    pub fn join(&self, dir: &str, name: &str) -> String {
        match self {
            Self::Local => local::join(dir, name),
            Self::Remote(_) => path::posix_join(dir, name),
        }
    }

    pub fn file_name<'a>(&self, path: &'a str) -> &'a str {
        match self {
            Self::Local => local::file_name(path),
            Self::Remote(_) => path::posix_file_name(path),
        }
    }

    pub fn parent(&self, path: &str) -> Option<String> {
        match self {
            Self::Local => local::parent(path),
            Self::Remote(_) => path::posix_parent(path),
        }
    }

    /// The user's home directory (the start location of a pane).
    pub async fn home(&self) -> FsResult<String> {
        match self {
            Self::Local => local::home(),
            Self::Remote(r) => r.canonicalize(".").await,
        }
    }

    /// Absolute path with `.`/`..`/symlinks resolved; `~` expands to home.
    pub async fn canonicalize(&self, path: &str) -> FsResult<String> {
        match self {
            Self::Local => local::canonicalize(path).await,
            Self::Remote(r) => r.canonicalize(path).await,
        }
    }

    /// Directory listing, sorted directories first then by name; `.`/`..` skipped.
    pub async fn list(&self, dir: &str) -> FsResult<Vec<Entry>> {
        let mut v = match self {
            Self::Local => local::list(dir).await?,
            Self::Remote(r) => r.list(dir).await?,
        };
        sort_entries(&mut v);
        Ok(v)
    }

    /// `stat` (follows symlinks). `Ok(None)` if it does not exist.
    pub async fn stat(&self, path: &str) -> FsResult<Option<Entry>> {
        let r = match self {
            Self::Local => local::stat(path, true).await,
            Self::Remote(r) => r.stat(path, true).await,
        };
        match r {
            Ok(e) => Ok(Some(e)),
            Err(FsError::NotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// `lstat` (does not follow symlinks). `Ok(None)` if it does not exist.
    pub async fn lstat(&self, path: &str) -> FsResult<Option<Entry>> {
        let r = match self {
            Self::Local => local::stat(path, false).await,
            Self::Remote(r) => r.stat(path, false).await,
        };
        match r {
            Ok(e) => Ok(Some(e)),
            Err(FsError::NotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn mkdir(&self, path: &str) -> FsResult<()> {
        match self {
            Self::Local => local::mkdir(path).await,
            Self::Remote(r) => r.mkdir(path).await,
        }
    }

    /// Creates `path` and any missing parents.
    pub async fn mkdir_all(&self, path: &str) -> FsResult<()> {
        if let Some(e) = self.stat(path).await? {
            return if e.kind == EntryKind::Dir { Ok(()) } else { Err(FsError::Exists { path: path.to_owned() }) };
        }
        if let Some(parent) = self.parent(path) {
            if parent != path {
                Box::pin(self.mkdir_all(&parent)).await?;
            }
        }
        match self.mkdir(path).await {
            Err(FsError::Exists { .. }) => Ok(()),
            r => r,
        }
    }

    /// Renames within this filesystem. Fails if `to` exists.
    pub async fn rename(&self, from: &str, to: &str) -> FsResult<()> {
        if self.lstat(to).await?.is_some() {
            return Err(FsError::Exists { path: to.to_owned() });
        }
        match self {
            Self::Local => local::rename(from, to).await,
            Self::Remote(r) => r.rename(from, to).await,
        }
    }

    /// Deletes a file, symlink or (recursively) a directory. Symlinks are
    /// removed themselves, never followed.
    pub async fn remove(&self, path: &str) -> FsResult<()> {
        let Some(e) = self.lstat(path).await? else { return Ok(()) };
        if e.kind == EntryKind::Dir {
            for child in self.list(path).await? {
                Box::pin(self.remove(&child.path)).await?;
            }
            match self {
                Self::Local => local::remove_dir(path).await,
                Self::Remote(r) => r.remove_dir(path).await,
            }
        } else {
            match self {
                Self::Local => local::remove_file(path).await,
                Self::Remote(r) => r.remove_file(path).await,
            }
        }
    }

    /// Opens a file for reading at `offset`.
    pub async fn open_read(&self, path: &str, offset: u64) -> FsResult<Reader> {
        match self {
            Self::Local => local::open_read(path, offset).await,
            Self::Remote(r) => r.open_read(path, offset).await,
        }
    }

    /// Opens a file for writing: `None` creates/truncates, `Some(offset)`
    /// keeps the content and continues at `offset` (resume).
    pub async fn open_write(&self, path: &str, resume_at: Option<u64>) -> FsResult<Writer> {
        match self {
            Self::Local => local::open_write(path, resume_at).await,
            Self::Remote(r) => r.open_write(path, resume_at).await,
        }
    }

    /// Sets permission bits (best effort on Windows).
    pub async fn set_mode(&self, path: &str, mode: u32) -> FsResult<()> {
        match self {
            Self::Local => local::set_mode(path, mode).await,
            Self::Remote(r) => r.set_mode(path, mode).await,
        }
    }
}

/// Directories (and links to them) first, then case-insensitive name.
pub fn sort_entries(v: &mut [Entry]) {
    v.sort_by(|a, b| {
        b.is_dir_like().cmp(&a.is_dir_like()).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

/// A name for a copy that does not collide: `name (1).ext`, `name (2).ext`...
pub fn numbered_name(name: &str, n: u32) -> String {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    format!("{stem} ({n}){ext}")
}

/// First `numbered_name` in `dir` that does not exist.
pub async fn free_name(fs: &Fs, dir: &str, name: &str) -> FsResult<String> {
    for n in 1..10_000 {
        let candidate = numbered_name(name, n);
        if fs.lstat(&fs.join(dir, &candidate)).await?.is_none() {
            return Ok(candidate);
        }
    }
    Err(FsError::other("no free name"))
}

/// Unique temp-file name next to `name` (for atomic saves).
pub(crate) fn temp_name(name: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    format!(".{name}.nightgrid-{:x}{:x}.tmp", std::process::id(), nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_kinds() {
        assert_eq!(EntryKind::from_mode(0o040755), EntryKind::Dir);
        assert_eq!(EntryKind::from_mode(0o100644), EntryKind::File);
        assert_eq!(EntryKind::from_mode(0o120777), EntryKind::Symlink);
        assert_eq!(EntryKind::from_mode(0o020666), EntryKind::Other);
    }

    #[test]
    fn numbered_names() {
        assert_eq!(numbered_name("compose.yaml", 1), "compose (1).yaml");
        assert_eq!(numbered_name(".env", 2), ".env (2)");
        assert_eq!(numbered_name("Makefile", 3), "Makefile (3)");
        assert_eq!(numbered_name("a.tar.gz", 1), "a.tar (1).gz");
    }

    #[test]
    fn sorting() {
        let e = |name: &str, kind| Entry {
            name: name.into(),
            path: name.into(),
            kind,
            target_kind: None,
            size: 0,
            mtime: None,
            mode: None,
            uid: None,
            gid: None,
        };
        let mut v = vec![
            e("b.txt", EntryKind::File),
            e("Zeta", EntryKind::Dir),
            e("a.txt", EntryKind::File),
            e("alpha", EntryKind::Dir),
        ];
        sort_entries(&mut v);
        let names: Vec<_> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "Zeta", "a.txt", "b.txt"]);
    }
}
