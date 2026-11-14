//! Text files for the editor: size/encoding checks on open, and atomic saves
//! (temp file + rename) that keep permissions and owner, with a conflict
//! check against what was opened.

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{temp_name, Entry, EntryKind, Fs, FsError, FsResult};

/// Largest file the editor opens.
pub const MAX_EDIT_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TextFile {
    /// Resolved path (symlinks followed), which is what gets saved.
    pub path: String,
    pub content: String,
    pub stat: Entry,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub stat: Entry,
    /// `false` when the file had to be rewritten in place (directory not
    /// writable, or the owner could not be preserved through a new file).
    pub atomic: bool,
}

/// What the editor saw when it opened the file; a save fails with
/// [`FsError::Changed`] if the file no longer matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Stamp {
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub size: u64,
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub mtime: Option<i64>,
}

impl Stamp {
    pub fn of(e: &Entry) -> Self {
        Self { size: e.size, mtime: e.mtime }
    }
}

/// Valid UTF-8 without NUL bytes.
pub fn decode_text(bytes: Vec<u8>) -> Option<String> {
    if bytes.contains(&0) {
        return None;
    }
    String::from_utf8(bytes).ok()
}

pub async fn read_text(fs: &Fs, path: &str) -> FsResult<TextFile> {
    let real = fs.canonicalize(path).await?;
    let stat = fs.stat(&real).await?.ok_or_else(|| FsError::NotFound { path: path.to_owned() })?;
    if stat.kind == EntryKind::Dir {
        return Err(FsError::IsDir { path: path.to_owned() });
    }
    if stat.size > MAX_EDIT_BYTES {
        return Err(FsError::TooLarge { path: path.to_owned(), size: stat.size, limit: MAX_EDIT_BYTES });
    }
    let mut r = fs.open_read(&real, 0).await?;
    let mut buf = Vec::with_capacity(stat.size as usize);
    (&mut r).take(MAX_EDIT_BYTES + 1).read_to_end(&mut buf).await.map_err(|e| FsError::from_io(path, e))?;
    if buf.len() as u64 > MAX_EDIT_BYTES {
        return Err(FsError::TooLarge { path: path.to_owned(), size: buf.len() as u64, limit: MAX_EDIT_BYTES });
    }
    let content = decode_text(buf).ok_or_else(|| FsError::NotText { path: path.to_owned() })?;
    Ok(TextFile { path: real, content, stat })
}

async fn write_all(mut w: super::Writer, path: &str, data: &[u8]) -> FsResult<()> {
    w.write_all(data).await.map_err(|e| FsError::from_io(path, e))?;
    w.shutdown().await.map_err(|e| FsError::from_io(path, e))?;
    Ok(())
}

async fn write_in_place(fs: &Fs, path: &str, data: &[u8]) -> FsResult<()> {
    let w = fs.open_write(path, None).await?;
    write_all(w, path, data).await
}

/// Saves `content` to `path` (a resolved path from [`read_text`], or a new
/// file). With `expect`, refuses to overwrite a file that changed since.
pub async fn save_text(fs: &Fs, path: &str, content: &str, expect: Option<Stamp>) -> FsResult<SaveResult> {
    let data = content.as_bytes();
    let current = fs.stat(path).await?;
    if let (Some(exp), Some(cur)) = (expect, &current) {
        if Stamp::of(cur) != exp {
            return Err(FsError::Changed { path: path.to_owned(), current: Box::new(cur.clone()) });
        }
    }
    let Some(cur) = current else {
        // New file.
        write_in_place(fs, path, data).await?;
        let stat = fs.stat(path).await?.ok_or_else(|| FsError::NotFound { path: path.to_owned() })?;
        return Ok(SaveResult { stat, atomic: true });
    };
    if cur.kind == EntryKind::Dir {
        return Err(FsError::IsDir { path: path.to_owned() });
    }

    let dir = fs.parent(path).unwrap_or_else(|| ".".into());
    let tmp = fs.join(&dir, &temp_name(fs.file_name(path)));
    let atomic = match write_temp(fs, &tmp, &cur, data).await {
        Ok(true) => match replace(fs, &tmp, path).await {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(path, "atomic replace failed ({e}); writing in place");
                let _ = fs.remove(&tmp).await;
                write_in_place(fs, path, data).await?;
                false
            }
        },
        Ok(false) => {
            let _ = fs.remove(&tmp).await;
            write_in_place(fs, path, data).await?;
            false
        }
        Err(FsError::PermissionDenied { .. }) => {
            // Directory not writable, but the file may be.
            let _ = fs.remove(&tmp).await;
            write_in_place(fs, path, data).await?;
            false
        }
        Err(e) => {
            let _ = fs.remove(&tmp).await;
            return Err(e);
        }
    };
    let stat = fs.stat(path).await?.ok_or_else(|| FsError::NotFound { path: path.to_owned() })?;
    Ok(SaveResult { stat, atomic })
}

/// Writes the temp file with the original's mode and owner. `Ok(false)` if
/// the owner could not be kept (then an atomic swap would change it).
async fn write_temp(fs: &Fs, tmp: &str, orig: &Entry, data: &[u8]) -> FsResult<bool> {
    let w = match fs {
        Fs::Local => fs.open_write(tmp, None).await?,
        Fs::Remote(r) => r.open_write_with(tmp, None, orig.mode, true).await?,
    };
    write_all(w, tmp, data).await?;
    if let Some(mode) = orig.mode {
        fs.set_mode(tmp, mode).await?;
    }
    let mine = fs.stat(tmp).await?.ok_or_else(|| FsError::NotFound { path: tmp.to_owned() })?;
    if let (Some(uid), Some(gid)) = (orig.uid, orig.gid) {
        if mine.uid != Some(uid) || mine.gid != Some(gid) {
            let ok = match fs {
                Fs::Local => super::local::chown(tmp, Some(uid), Some(gid)).is_ok(),
                Fs::Remote(r) => r.chown(tmp, uid, gid).await.is_ok(),
            };
            if !ok {
                return Ok(false);
            }
            // chown may clear setuid/setgid bits; restore the mode.
            if let Some(mode) = orig.mode {
                fs.set_mode(tmp, mode).await?;
            }
        }
    }
    Ok(true)
}

async fn replace(fs: &Fs, tmp: &str, target: &str) -> FsResult<()> {
    match fs {
        // std/tokio rename replaces atomically on Unix and Windows (MoveFileEx).
        Fs::Local => tokio::fs::rename(tmp, target).await.map_err(|e| FsError::from_io(target, e)),
        Fs::Remote(r) => r.replace(tmp, target).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(dir: &tempfile::TempDir, name: &str) -> String {
        dir.path().join(name).to_string_lossy().into_owned()
    }

    #[test]
    fn decoding() {
        assert_eq!(decode_text(b"a: 1\n".to_vec()).as_deref(), Some("a: 1\n"));
        assert_eq!(decode_text(vec![0x7f, 0x45, 0x4c, 0x46, 0]), None);
        assert_eq!(decode_text(vec![0xff, 0xfe]), None);
    }

    #[tokio::test]
    async fn local_round_trip_keeps_mode_and_detects_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let f = p(&dir, "compose.yaml");
        std::fs::write(&f, "services: {}\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o640)).unwrap();
        }
        let fs = Fs::Local;
        let t = read_text(&fs, &f).await.unwrap();
        assert_eq!(t.content, "services: {}\n");
        let r = save_text(&fs, &t.path, "services:\n  web: {}\n", Some(Stamp::of(&t.stat))).await.unwrap();
        assert!(r.atomic);
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "services:\n  web: {}\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o640);
        }
        // No temp files left behind.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);

        // Stale stamp → conflict.
        let e = save_text(&fs, &t.path, "x", Some(Stamp::of(&t.stat))).await.unwrap_err();
        assert!(matches!(e, FsError::Changed { .. }), "{e:?}");
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "services:\n  web: {}\n");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn local_save_through_symlink_keeps_link() {
        let dir = tempfile::tempdir().unwrap();
        let real = p(&dir, "real.env");
        let link = p(&dir, "link.env");
        std::fs::write(&real, "A=1\n").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let fs = Fs::Local;
        let t = read_text(&fs, &link).await.unwrap();
        save_text(&fs, &t.path, "A=2\n", Some(Stamp::of(&t.stat))).await.unwrap();
        assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(&real).unwrap(), "A=2\n");
    }

    #[tokio::test]
    async fn refuses_binary_and_large_files_and_creates_new() {
        let dir = tempfile::tempdir().unwrap();
        let bin = p(&dir, "a.bin");
        std::fs::write(&bin, [0u8, 1, 2]).unwrap();
        assert!(matches!(read_text(&Fs::Local, &bin).await, Err(FsError::NotText { .. })));
        let big = p(&dir, "big.log");
        std::fs::File::create(&big).unwrap().set_len(MAX_EDIT_BYTES + 1).unwrap();
        assert!(matches!(read_text(&Fs::Local, &big).await, Err(FsError::TooLarge { .. })));
        let new = p(&dir, "new.txt");
        save_text(&Fs::Local, &new, "hi", None).await.unwrap();
        assert_eq!(std::fs::read_to_string(&new).unwrap(), "hi");
    }
}
