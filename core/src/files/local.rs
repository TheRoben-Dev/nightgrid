//! Local filesystem side of [`super::Fs`].

use std::io::SeekFrom;
use std::path::{Path, PathBuf};

use tokio::io::AsyncSeekExt;

use super::{Entry, EntryKind, FsError, FsResult, Reader, Writer};

pub fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        return name.to_owned();
    }
    Path::new(dir).join(name).to_string_lossy().into_owned()
}

pub fn file_name(path: &str) -> &str {
    let t = path.trim_end_matches(['/', '\\']);
    let name = t.rsplit(['/', '\\']).next().unwrap_or(t);
    if name.is_empty() {
        path
    } else {
        name
    }
}

pub fn parent(path: &str) -> Option<String> {
    let p = Path::new(path);
    p.parent().filter(|q| !q.as_os_str().is_empty()).map(|q| q.to_string_lossy().into_owned())
}

pub fn home() -> FsResult<String> {
    std::env::home_dir().map(|h| h.to_string_lossy().into_owned()).ok_or_else(|| FsError::other("no home directory"))
}

fn expand(path: &str) -> FsResult<PathBuf> {
    if path == "~" {
        return home().map(PathBuf::from);
    }
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        return Ok(PathBuf::from(home()?).join(rest));
    }
    Ok(PathBuf::from(path))
}

/// Drops Windows' verbatim prefix (`\\?\C:\...` → `C:\...`) for display.
fn simplify(p: PathBuf) -> String {
    let s = p.to_string_lossy().into_owned();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => rest.to_owned(),
        _ => s,
    }
}

pub async fn canonicalize(path: &str) -> FsResult<String> {
    let p = expand(path)?;
    tokio::fs::canonicalize(&p).await.map(simplify).map_err(|e| FsError::from_io(path, e))
}

fn kind_of(ft: std::fs::FileType) -> EntryKind {
    if ft.is_symlink() {
        EntryKind::Symlink
    } else if ft.is_dir() {
        EntryKind::Dir
    } else if ft.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    }
}

fn entry_from(name: String, path: String, md: &std::fs::Metadata, target: Option<&std::fs::Metadata>) -> Entry {
    let kind = kind_of(md.file_type());
    let mtime =
        md.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64);
    #[cfg(unix)]
    let (mode, uid, gid) = {
        use std::os::unix::fs::MetadataExt;
        (Some(md.mode() & 0o7777), Some(md.uid()), Some(md.gid()))
    };
    #[cfg(not(unix))]
    let (mode, uid, gid) = (None, None, None);
    Entry {
        name,
        path,
        kind,
        target_kind: (kind == EntryKind::Symlink).then(|| target.map(|t| kind_of(t.file_type()))).flatten(),
        size: target.filter(|_| kind == EntryKind::Symlink).unwrap_or(md).len(),
        mtime,
        mode,
        uid,
        gid,
    }
}

pub async fn stat(path: &str, follow: bool) -> FsResult<Entry> {
    let p = expand(path)?;
    let md = if follow { tokio::fs::metadata(&p).await } else { tokio::fs::symlink_metadata(&p).await }
        .map_err(|e| FsError::from_io(path, e))?;
    let target = if md.file_type().is_symlink() { tokio::fs::metadata(&p).await.ok() } else { None };
    Ok(entry_from(file_name(path).to_owned(), simplify(p), &md, target.as_ref()))
}

pub async fn list(dir: &str) -> FsResult<Vec<Entry>> {
    let p = expand(dir)?;
    let mut rd = tokio::fs::read_dir(&p).await.map_err(|e| FsError::from_io(dir, e))?;
    let mut out = Vec::new();
    while let Some(de) = rd.next_entry().await.map_err(|e| FsError::from_io(dir, e))? {
        let Ok(md) = de.metadata().await else { continue };
        let target = if md.file_type().is_symlink() { tokio::fs::metadata(de.path()).await.ok() } else { None };
        out.push(entry_from(de.file_name().to_string_lossy().into_owned(), simplify(de.path()), &md, target.as_ref()));
    }
    Ok(out)
}

pub async fn mkdir(path: &str) -> FsResult<()> {
    tokio::fs::create_dir(expand(path)?).await.map_err(|e| FsError::from_io(path, e))
}

pub async fn rename(from: &str, to: &str) -> FsResult<()> {
    tokio::fs::rename(expand(from)?, expand(to)?).await.map_err(|e| FsError::from_io(from, e))
}

pub async fn remove_file(path: &str) -> FsResult<()> {
    tokio::fs::remove_file(expand(path)?).await.map_err(|e| FsError::from_io(path, e))
}

pub async fn remove_dir(path: &str) -> FsResult<()> {
    tokio::fs::remove_dir(expand(path)?).await.map_err(|e| FsError::from_io(path, e))
}

pub async fn open_read(path: &str, offset: u64) -> FsResult<Reader> {
    let mut f = tokio::fs::File::open(expand(path)?).await.map_err(|e| FsError::from_io(path, e))?;
    if offset > 0 {
        f.seek(SeekFrom::Start(offset)).await.map_err(|e| FsError::from_io(path, e))?;
    }
    Ok(Box::new(f))
}

pub async fn open_write(path: &str, resume_at: Option<u64>) -> FsResult<Writer> {
    let p = expand(path)?;
    let mut opts = tokio::fs::OpenOptions::new();
    opts.write(true).create(true);
    if resume_at.is_none() {
        opts.truncate(true);
    }
    let mut f = opts.open(&p).await.map_err(|e| FsError::from_io(path, e))?;
    if let Some(off) = resume_at {
        f.set_len(off).await.map_err(|e| FsError::from_io(path, e))?;
        f.seek(SeekFrom::Start(off)).await.map_err(|e| FsError::from_io(path, e))?;
    }
    Ok(Box::new(f))
}

pub async fn set_mode(path: &str, mode: u32) -> FsResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(expand(path)?, std::fs::Permissions::from_mode(mode & 0o7777))
            .await
            .map_err(|e| FsError::from_io(path, e))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        Ok(())
    }
}

/// Changes owner/group (Unix; no-op elsewhere).
pub fn chown(path: &str, uid: Option<u32>, gid: Option<u32>) -> FsResult<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::chown(expand(path)?, uid, gid).map_err(|e| FsError::from_io(path, e))
    }
    #[cfg(not(unix))]
    {
        let _ = (path, uid, gid);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(file_name("/a/b/"), "b");
        assert_eq!(file_name(r"C:\Users\me"), "me");
        assert_eq!(parent("/a/b"), Some("/a".into()));
        assert_eq!(parent("/"), None);
    }
}
