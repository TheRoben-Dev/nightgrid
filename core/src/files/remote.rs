//! Remote side of [`super::Fs`]: an SFTP session on the host's shared SSH
//! connection.

use std::collections::HashMap;
use std::io::SeekFrom;
use std::sync::{Arc, Mutex};

use russh_sftp::client::error::Error as SftpError;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{FileAttributes, OpenFlags, StatusCode};
use tokio::io::AsyncSeekExt;

use super::path::{posix_file_name, posix_join, shell_quote};
use super::{Entry, EntryKind, FsError, FsResult, Reader, Writer};
use crate::hosts::HostId;
use crate::ssh::client::Connection;

pub struct RemoteFs {
    sftp: SftpSession,
    conn: Arc<Connection>,
}

impl RemoteFs {
    /// Starts the `sftp` subsystem on `conn`.
    pub async fn open(conn: Arc<Connection>) -> FsResult<Self> {
        let ch = conn.open_subsystem("sftp").await.map_err(|e| FsError::other(format!("SFTP unavailable: {e}")))?;
        let sftp = SftpSession::new(ch.into_stream()).await.map_err(|e| FsError::other(format!("SFTP: {e}")))?;
        sftp.set_timeout(30);
        Ok(Self { sftp, conn })
    }

    pub fn connection(&self) -> &Arc<Connection> {
        &self.conn
    }

    pub fn is_closed(&self) -> bool {
        self.conn.is_closed()
    }

    fn err(&self, path: &str, e: SftpError) -> FsError {
        if self.conn.is_closed() {
            return FsError::Disconnected;
        }
        match &e {
            SftpError::Status(s) => match s.status_code {
                StatusCode::NoSuchFile => FsError::NotFound { path: path.to_owned() },
                StatusCode::PermissionDenied => FsError::PermissionDenied { path: path.to_owned() },
                _ if s.error_message.is_empty() => FsError::Other { message: format!("{path}: {}", s.status_code) },
                _ => FsError::Other { message: format!("{path}: {}", s.error_message) },
            },
            _ => FsError::Other { message: format!("{path}: {e}") },
        }
    }

    pub async fn canonicalize(&self, path: &str) -> FsResult<String> {
        let path = if path == "~" {
            "."
        } else if let Some(rest) = path.strip_prefix("~/") {
            let home = self.sftp.canonicalize(".").await.map_err(|e| self.err(".", e))?;
            return Box::pin(self.canonicalize(&posix_join(&home, rest))).await;
        } else {
            path
        };
        self.sftp.canonicalize(path).await.map_err(|e| self.err(path, e))
    }

    fn entry(name: String, path: String, a: &FileAttributes, target: Option<&FileAttributes>) -> Entry {
        let kind = a.permissions.map(EntryKind::from_mode).unwrap_or(EntryKind::Other);
        let target_kind = (kind == EntryKind::Symlink)
            .then(|| target.and_then(|t| t.permissions.map(EntryKind::from_mode)))
            .flatten();
        let size_src = if kind == EntryKind::Symlink { target.unwrap_or(a) } else { a };
        Entry {
            name,
            path,
            kind,
            target_kind,
            size: size_src.size.unwrap_or(0),
            mtime: a.mtime.map(i64::from),
            mode: a.permissions.map(|p| p & 0o7777),
            uid: a.uid,
            gid: a.gid,
        }
    }

    pub async fn stat(&self, path: &str, follow: bool) -> FsResult<Entry> {
        let a = if follow { self.sftp.metadata(path).await } else { self.sftp.symlink_metadata(path).await }
            .map_err(|e| self.err(path, e))?;
        let target = if !follow && a.permissions.map(EntryKind::from_mode) == Some(EntryKind::Symlink) {
            self.sftp.metadata(path).await.ok()
        } else {
            None
        };
        Ok(Self::entry(posix_file_name(path).to_owned(), path.to_owned(), &a, target.as_ref()))
    }

    pub async fn list(&self, dir: &str) -> FsResult<Vec<Entry>> {
        let rd = self.sftp.read_dir(dir).await.map_err(|e| self.err(dir, e))?;
        let mut out = Vec::new();
        for de in rd {
            let name = de.file_name();
            if name == "." || name == ".." {
                continue;
            }
            let path = posix_join(dir, &name);
            let attrs = de.metadata();
            let target = if attrs.permissions.map(EntryKind::from_mode) == Some(EntryKind::Symlink) {
                self.sftp.metadata(path.clone()).await.ok()
            } else {
                None
            };
            out.push(Self::entry(name, path, &attrs, target.as_ref()));
        }
        Ok(out)
    }

    pub async fn mkdir(&self, path: &str) -> FsResult<()> {
        match self.sftp.create_dir(path).await {
            Ok(()) => Ok(()),
            Err(e) => {
                // OpenSSH answers a generic failure for an existing path.
                if self.sftp.symlink_metadata(path).await.is_ok() {
                    Err(FsError::Exists { path: path.to_owned() })
                } else {
                    Err(self.err(path, e))
                }
            }
        }
    }

    pub async fn rename(&self, from: &str, to: &str) -> FsResult<()> {
        self.sftp.rename(from, to).await.map_err(|e| self.err(from, e))
    }

    pub async fn remove_file(&self, path: &str) -> FsResult<()> {
        self.sftp.remove_file(path).await.map_err(|e| self.err(path, e))
    }

    pub async fn remove_dir(&self, path: &str) -> FsResult<()> {
        self.sftp.remove_dir(path).await.map_err(|e| self.err(path, e))
    }

    pub async fn open_read(&self, path: &str, offset: u64) -> FsResult<Reader> {
        let mut f = self.sftp.open(path).await.map_err(|e| self.err(path, e))?;
        if offset > 0 {
            f.seek(SeekFrom::Start(offset)).await.map_err(|e| FsError::other(format!("{path}: {e}")))?;
        }
        Ok(Box::new(f))
    }

    pub async fn open_write(&self, path: &str, resume_at: Option<u64>) -> FsResult<Writer> {
        self.open_write_with(path, resume_at, None, false).await
    }

    /// `mode` sets the permissions of a newly created file; `exclusive`
    /// fails if the file exists (temp files).
    pub async fn open_write_with(
        &self,
        path: &str,
        resume_at: Option<u64>,
        mode: Option<u32>,
        exclusive: bool,
    ) -> FsResult<Writer> {
        let mut flags = OpenFlags::WRITE | OpenFlags::CREATE;
        if resume_at.is_none() {
            flags |= OpenFlags::TRUNCATE;
        }
        if exclusive {
            flags |= OpenFlags::EXCLUDE;
        }
        let mut attrs = FileAttributes::empty();
        attrs.permissions = mode;
        let mut f =
            self.sftp.open_with_flags_and_attributes(path, flags, attrs).await.map_err(|e| self.err(path, e))?;
        if let Some(off) = resume_at {
            let mut size = FileAttributes::empty();
            size.size = Some(off);
            f.set_metadata(size).await.map_err(|e| self.err(path, e))?;
            f.seek(SeekFrom::Start(off)).await.map_err(|e| FsError::other(format!("{path}: {e}")))?;
        }
        Ok(Box::new(f))
    }

    pub async fn set_mode(&self, path: &str, mode: u32) -> FsResult<()> {
        let mut a = FileAttributes::empty();
        a.permissions = Some(mode & 0o7777);
        self.sftp.set_metadata(path, a).await.map_err(|e| self.err(path, e))
    }

    pub async fn chown(&self, path: &str, uid: u32, gid: u32) -> FsResult<()> {
        let mut a = FileAttributes::empty();
        a.uid = Some(uid);
        a.gid = Some(gid);
        self.sftp.set_metadata(path, a).await.map_err(|e| self.err(path, e))
    }

    /// Atomically replaces `target` with `tmp` (same directory).
    ///
    /// SFTP v3 `RENAME` refuses an existing target on OpenSSH, and russh-sftp
    /// has no `posix-rename@openssh.com`, so this runs `mv -f` (a single
    /// `rename(2)`) on the same connection.
    pub async fn replace(&self, tmp: &str, target: &str) -> FsResult<()> {
        if self.sftp.rename(tmp, target).await.is_ok() {
            return Ok(());
        }
        let cmd = format!("mv -f -- {} {}", shell_quote(tmp), shell_quote(target));
        match self.conn.exec(&cmd).await {
            Ok((0, _, _)) => Ok(()),
            Ok((code, _, err)) => Err(FsError::Other {
                message: format!("{target}: replace failed ({code}): {}", String::from_utf8_lossy(&err).trim()),
            }),
            Err(e) => Err(FsError::other(e)),
        }
    }
}

/// Cache of SFTP sessions, one per host, tied to the host's current pooled
/// connection (a reconnect gets a fresh session).
#[derive(Default)]
pub struct SftpCache {
    sessions: Mutex<HashMap<HostId, Arc<RemoteFs>>>,
    opening: tokio::sync::Mutex<()>,
}

impl SftpCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get(&self, id: HostId, conn: Arc<Connection>) -> FsResult<Arc<RemoteFs>> {
        let cached = |s: &Self| {
            s.sessions
                .lock()
                .expect("poisoned")
                .get(&id)
                .filter(|r| Arc::ptr_eq(&r.conn, &conn) && !r.is_closed())
                .cloned()
        };
        if let Some(r) = cached(self) {
            return Ok(r);
        }
        let _guard = self.opening.lock().await;
        if let Some(r) = cached(self) {
            return Ok(r);
        }
        let r = Arc::new(RemoteFs::open(conn.clone()).await?);
        self.sessions.lock().expect("poisoned").insert(id, r.clone());
        Ok(r)
    }

    pub fn drop_host(&self, id: HostId) {
        self.sessions.lock().expect("poisoned").remove(&id);
    }
}
