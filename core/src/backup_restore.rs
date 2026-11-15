//! Test restore (M10m): proves a backup can be read back by restoring one
//! file from the latest snapshot into a temporary folder on the host and
//! checking it, then deleting the folder.
//!
//! It runs as root on the host, with the backup unit's own environment
//! (`EnvironmentFile=`, `Environment=`, `User=`), so the repository location
//! and password stay where they already are: nothing secret is read into
//! Nightgrid or put on a command line (rule 1).

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::backup_jobs::{BackupJob, BackupJobId, BackupSource, BackupTool};
use crate::files::path::shell_quote;
use crate::redact::line_has_secret;
use crate::store::Store;
use crate::Result;

/// Files above this size aren't picked (keeps the test quick).
pub const MAX_FILE: u64 = 20_000_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RestoreTest {
    pub job_id: BackupJobId,
    pub ok: bool,
    /// The file that was restored (its path inside the backup).
    pub file: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub size: Option<u64>,
    pub sha256: Option<String>,
    /// Snapshot or archive id, and its time as the tool prints it.
    pub snapshot: Option<String>,
    pub snapshot_time: Option<String>,
    /// What happened, step by step.
    pub steps: Vec<String>,
    pub error: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub at: i64,
}

/// The last test of a job, as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LastTest {
    pub job_id: BackupJobId,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub at: i64,
    pub ok: bool,
    pub note: String,
}

/// Whether a job can be tested, and if not, why.
pub fn unsupported(job: &BackupJob) -> Option<&'static str> {
    match (job.tool, job.source) {
        (BackupTool::Restic | BackupTool::Borg, _) => None,
        (BackupTool::Rsync | BackupTool::Other, BackupSource::Path) => None,
        (BackupTool::Rsync | BackupTool::Other, BackupSource::Unit) => {
            Some("this job points at a unit, which doesn't say where the copy is: add a path job for the target folder to test it")
        }
        (BackupTool::Rclone, _) => Some("rclone remotes can't be test-restored yet"),
    }
}

const PRELUDE: &str = r#"exec 2>&1
say() { echo "@step $*"; }
fail() { echo "@fail $*"; exit 0; }
last() { tail -n 3 "$1" | tr '\n' ' ' | cut -c1-300; }
tmp=$(mktemp -d /tmp/nightgrid-restore.XXXXXX) || fail "couldn't make a temporary folder"
trap 'rm -rf "$tmp"' EXIT INT TERM
mkdir "$tmp/r"
RUNAS=root
as_user() { if [ "$RUNAS" = root ]; then "$@"; else runuser -u "$RUNAS" -- "$@"; fi; }
lim() { if command -v timeout >/dev/null 2>&1; then timeout "$@"; else shift; "$@"; fi; }
fsize() { stat -c %s "$1" 2>/dev/null || stat -f %z "$1" 2>/dev/null || wc -c <"$1" | tr -d ' '; }
sha() { if command -v sha256sum >/dev/null 2>&1; then sha256sum; else shasum -a 256; fi | cut -d' ' -f1; }
pick() { awk -v max="$MAX" 'BEGIN { srand() } $1 > 0 && $1 < max { n++; if (rand() * n < 1) l = $0 } END { print l }'; }
"#;

const UNIT_ENV: &str = r#"case "$TARGET" in *.timer) sv=$(systemctl show -p Unit --value "$TARGET");; *.service) sv=$TARGET;; *) sv="$TARGET.service";; esac
[ -n "$sv" ] || fail "couldn't find the service behind $TARGET"
say "using the environment of $sv"
for f in $(systemctl show -p EnvironmentFiles --value "$sv" | awk '{ print $1 }'); do
  if [ -r "$f" ]; then set -a; . "$f"; set +a; fi
done
e=$(systemctl show -p Environment --value "$sv"); [ -n "$e" ] && eval "export $e"
u=$(systemctl show -p User --value "$sv"); [ -n "$u" ] && RUNAS=$u
[ "$RUNAS" = root ] || { chown -R "$RUNAS" "$tmp"; say "running as $RUNAS"; }
"#;

const RESTIC: &str = r#"command -v restic >/dev/null 2>&1 || fail "restic isn't installed"
[ "$SRC" = path ] && export RESTIC_REPOSITORY="$TARGET"
[ -n "$RESTIC_REPOSITORY$RESTIC_REPOSITORY_FILE" ] || fail "no RESTIC_REPOSITORY in the unit's environment"
[ -n "$RESTIC_PASSWORD$RESTIC_PASSWORD_FILE$RESTIC_PASSWORD_COMMAND" ] || fail "no repository password in the unit's environment (RESTIC_PASSWORD_FILE or RESTIC_PASSWORD_COMMAND); for a path job, test the unit job instead"
say "reading the latest snapshot"
as_user lim 300 restic snapshots --latest 1 --json --no-lock --no-cache </dev/null >"$tmp/snap" 2>"$tmp/err" || fail "couldn't read snapshots: $(last "$tmp/err")"
snap=$(grep -o '"short_id":"[^"]*"' "$tmp/snap" | tail -n 1 | cut -d'"' -f4)
when=$(grep -o '"time":"[^"]*"' "$tmp/snap" | tail -n 1 | cut -d'"' -f4)
[ -n "$snap" ] || fail "the repository has no snapshots"
echo "@snap $snap $when"
say "listing snapshot $snap"
as_user lim 300 restic ls --json --no-lock --no-cache "$snap" </dev/null >"$tmp/ls" 2>"$tmp/err" || fail "couldn't list the snapshot: $(last "$tmp/err")"
p=$(grep '"type":"file"' "$tmp/ls" | grep -v '\\' | sed -n 's/.*"path":"\([^"]*\)".*"size":\([0-9]*\).*/\2 \1/p' | pick)
[ -n "$p" ] || fail "no small regular file found in the snapshot"
size=${p%% *}; path=${p#* }
echo "@file $size $path"
say "restoring it into a temporary folder"
as_user lim 600 restic restore "$snap" --target "$tmp/r" --include "$path" --no-lock --no-cache </dev/null >"$tmp/out" 2>&1 || fail "restore failed: $(last "$tmp/out")"
got="$tmp/r$path"
"#;

const BORG: &str = r#"command -v borg >/dev/null 2>&1 || fail "borg isn't installed"
[ "$SRC" = path ] && export BORG_REPO="$TARGET"
[ -n "$BORG_REPO" ] || fail "no BORG_REPO in the unit's environment (borgmatic jobs can't be tested yet)"
export BORG_RELOCATED_REPO_ACCESS_IS_OK=yes BORG_UNKNOWN_UNENCRYPTED_REPO_ACCESS_IS_OK=yes
say "reading the latest archive"
as_user lim 300 borg list --last 1 --format '{archive} {time}{NL}' </dev/null >"$tmp/a" 2>"$tmp/err" || fail "couldn't read archives: $(last "$tmp/err")"
line=$(tail -n 1 "$tmp/a"); snap=${line%% *}
[ -n "$snap" ] || fail "the repository has no archives"
echo "@snap $line"
say "listing archive $snap"
as_user lim 300 borg list --format '{type} {size} {path}{NL}' "::$snap" </dev/null >"$tmp/ls" 2>"$tmp/err" || fail "couldn't list the archive: $(last "$tmp/err")"
p=$(awk '$1 == "-" { sub(/^- /, ""); print }' "$tmp/ls" | pick)
[ -n "$p" ] || fail "no small regular file found in the archive"
size=${p%% *}; path=${p#* }
echo "@file $size $path"
say "extracting it into a temporary folder"
(cd "$tmp/r" && as_user lim 600 borg extract "::$snap" "$path" </dev/null >"$tmp/out" 2>&1) || fail "extract failed: $(last "$tmp/out")"
got="$tmp/r/$path"
"#;

const COPY: &str = r#"[ -d "$TARGET" ] || fail "the target isn't a folder"
say "picking a file in $TARGET"
p=$(lim 60 find "$TARGET" -type f -size +0 2>/dev/null | head -n 5000 | while IFS= read -r f; do echo "$(fsize "$f" || echo 0) $f"; done | pick)
[ -n "$p" ] || fail "no small file found in the target"
size=${p%% *}; path=${p#* }
echo "@file $size $path"
say "copying it into a temporary folder"
cp -p "$path" "$tmp/r/f" 2>"$tmp/err" || fail "couldn't read it: $(last "$tmp/err")"
got="$tmp/r/f"
want=$(sha <"$path")
"#;

const VERIFY: &str = r#"[ -f "$got" ] || fail "the restored file isn't there"
have=$(fsize "$got")
[ "$have" = "$size" ] || fail "restored $have bytes, the backup lists $size"
sum=$(sha <"$got")
[ -z "$want" ] || [ "$want" = "$sum" ] || fail "the copy's checksum differs from the file in the target"
echo "@ok $sum"
"#;

/// The root script for `job`. Its values go in quoted; the unit's
/// environment is read on the host and never leaves it.
pub fn test_command(job: &BackupJob) -> Option<String> {
    if unsupported(job).is_some() {
        return None;
    }
    let src = match job.source {
        BackupSource::Unit => "unit",
        BackupSource::Path => "path",
    };
    let mut s = format!("TARGET={}; SRC={src}; MAX={MAX_FILE}; want=''\n{PRELUDE}", shell_quote(&job.target));
    if job.source == BackupSource::Unit {
        s.push_str(UNIT_ENV);
    }
    s.push_str(match job.tool {
        BackupTool::Restic => RESTIC,
        BackupTool::Borg => BORG,
        _ => COPY,
    });
    s.push_str(VERIFY);
    Some(s)
}

fn clean(s: &str) -> String {
    let t: String = s.chars().filter(|c| !c.is_control()).take(300).collect();
    if line_has_secret(&t) {
        "(hidden: looked like a secret)".into()
    } else {
        t
    }
}

pub fn parse_test(job_id: BackupJobId, out: &str, at: i64) -> RestoreTest {
    let mut t = RestoreTest {
        job_id,
        ok: false,
        file: None,
        size: None,
        sha256: None,
        snapshot: None,
        snapshot_time: None,
        steps: Vec::new(),
        error: None,
        at,
    };
    for l in out.lines() {
        if let Some(s) = l.strip_prefix("@step ") {
            t.steps.push(clean(s));
        } else if let Some(s) = l.strip_prefix("@snap ") {
            let (id, when) = s.split_once(' ').unwrap_or((s, ""));
            t.snapshot = Some(clean(id));
            t.snapshot_time = Some(clean(when.trim())).filter(|w| !w.is_empty());
        } else if let Some(s) = l.strip_prefix("@file ") {
            let (size, path) = s.split_once(' ').unwrap_or(("", s));
            t.size = size.parse().ok();
            t.file = Some(clean(path));
        } else if let Some(s) = l.strip_prefix("@ok ") {
            t.ok = true;
            t.sha256 = Some(s.trim().chars().take(64).collect());
        } else if let Some(s) = l.strip_prefix("@fail ") {
            t.error = Some(clean(s));
        }
    }
    if !t.ok && t.error.is_none() {
        let tail = out.lines().rev().find(|l| !l.trim().is_empty() && !l.starts_with('@')).unwrap_or("no output");
        t.error = Some(format!("the test stopped early: {}", clean(tail)));
    }
    t
}

/// A one-line note for the board.
pub fn note(t: &RestoreTest) -> String {
    if t.ok {
        format!(
            "restored {}{}",
            t.file.as_deref().unwrap_or("a file"),
            t.snapshot.as_deref().map(|s| format!(" from {s}")).unwrap_or_default()
        )
    } else {
        t.error.clone().unwrap_or_else(|| "failed".into())
    }
}

impl Store {
    pub fn backup_test_save(&self, t: &RestoreTest) -> Result<()> {
        self.conn().execute(
            "UPDATE backup_jobs SET tested_at = ?2, test_ok = ?3, test_note = ?4 WHERE id = ?1",
            params![t.job_id, t.at, t.ok, note(t)],
        )?;
        Ok(())
    }

    pub fn backup_tests(&self) -> Result<Vec<LastTest>> {
        let mut st = self.conn().prepare(
            "SELECT id, tested_at, test_ok, COALESCE(test_note, '') FROM backup_jobs WHERE tested_at IS NOT NULL ORDER BY id",
        )?;
        let rows =
            st.query_map([], |r| Ok(LastTest { job_id: r.get(0)?, at: r.get(1)?, ok: r.get(2)?, note: r.get(3)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(tool: BackupTool, source: BackupSource, target: &str) -> BackupJob {
        BackupJob { id: 3, host_id: 1, name: "nightly".into(), tool, source, target: target.into(), stale_hours: 26 }
    }

    #[test]
    fn commands() {
        let c = test_command(&job(BackupTool::Restic, BackupSource::Unit, "restic-backup.timer")).unwrap();
        assert!(c.starts_with("TARGET='restic-backup.timer'; SRC=unit;"));
        assert!(c.contains("EnvironmentFiles") && c.contains("restic restore"));
        assert!(!c.contains("borg extract"));
        let c = test_command(&job(BackupTool::Borg, BackupSource::Path, "/srv/borg/it's")).unwrap();
        assert!(c.starts_with(r"TARGET='/srv/borg/it'\''s'; SRC=path;"));
        assert!(!c.contains("EnvironmentFiles"));
        assert!(test_command(&job(BackupTool::Rsync, BackupSource::Path, "/mnt/backup")).unwrap().contains("cp -p"));
        assert!(test_command(&job(BackupTool::Rsync, BackupSource::Unit, "rsync.timer")).is_none());
        assert!(unsupported(&job(BackupTool::Rclone, BackupSource::Path, "/x")).is_some());
    }

    #[test]
    fn parses_results() {
        let t = parse_test(
            3,
            "@step using the environment of restic-backup.service\n@snap 4f2a9c1e 2026-11-15T03:00:12.5+01:00\n@step listing snapshot 4f2a9c1e\n\
             @file 1234 /etc/nginx/nginx.conf\n@step restoring it into a temporary folder\n@ok 9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08\n",
            100,
        );
        assert!(t.ok);
        assert_eq!(t.file.as_deref(), Some("/etc/nginx/nginx.conf"));
        assert_eq!((t.size, t.snapshot.as_deref()), (Some(1234), Some("4f2a9c1e")));
        assert_eq!(t.snapshot_time.as_deref(), Some("2026-11-15T03:00:12.5+01:00"));
        assert_eq!(t.steps.len(), 3);
        assert_eq!(note(&t), "restored /etc/nginx/nginx.conf from 4f2a9c1e");
        let t = parse_test(
            3,
            "@step reading the latest snapshot\n@fail couldn't read snapshots: Fatal: wrong password or no key found\n",
            5,
        );
        assert!(!t.ok);
        assert_eq!(note(&t), "couldn't read snapshots: Fatal: wrong password or no key found");
        let t = parse_test(3, "@step x\nsh: 3: Syntax error\n", 5);
        assert_eq!(t.error.as_deref(), Some("the test stopped early: sh: 3: Syntax error"));
        let t = parse_test(3, "@fail export RESTIC_PASSWORD=hunter2hunter2\n", 5);
        assert_eq!(t.error.as_deref(), Some("(hidden: looked like a secret)"));
    }

    #[test]
    fn saves_last_test() {
        let s = Store::open_in_memory().unwrap();
        s.conn().execute("INSERT INTO hosts (name, hostname) VALUES ('a', 'a.example.com')", []).unwrap();
        s.conn()
            .execute(
                "INSERT INTO backup_jobs (host_id, name, tool, source, target, stale_hours) VALUES (1, 'n', 'restic', 'unit', 'r.timer', 26)",
                [],
            )
            .unwrap();
        let id = s.conn().last_insert_rowid() as BackupJobId;
        assert!(s.backup_tests().unwrap().is_empty());
        let t = parse_test(id, "@file 5 /a\n@ok abc\n", 77);
        s.backup_test_save(&t).unwrap();
        assert_eq!(s.backup_tests().unwrap(), [LastTest { job_id: id, at: 77, ok: true, note: "restored /a".into() }]);
    }
}

#[cfg(all(test, unix))]
mod shell {
    use super::*;

    /// The copy test end to end against a temp folder (no root needed).
    #[test]
    fn copy_test_runs() {
        let dir = std::env::temp_dir().join(format!("ng-restore-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/a.txt"), b"hello backup").unwrap();
        let j = BackupJob {
            id: 1,
            host_id: 1,
            name: "copy".into(),
            tool: BackupTool::Rsync,
            source: BackupSource::Path,
            target: dir.to_string_lossy().into(),
            stale_hours: 26,
        };
        let out = std::process::Command::new("sh").arg("-c").arg(test_command(&j).unwrap()).output().unwrap();
        let t = parse_test(1, &String::from_utf8_lossy(&out.stdout), 0);
        std::fs::remove_dir_all(&dir).ok();
        assert!(t.ok, "{t:?}");
        assert_eq!(t.size, Some(12));
        assert_eq!(t.sha256.as_deref().map(str::len), Some(64));
        assert!(t.file.unwrap().ends_with("sub/a.txt"));
    }

    /// The restic path with a stand-in `restic` on PATH.
    #[test]
    fn restic_test_runs_with_a_fake_tool() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("ng-restic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake = r#"#!/bin/sh
case "$1" in
  snapshots) echo '[{"time":"2026-11-15T03:00:00Z","short_id":"4f2a9c1e","id":"x"}]' ;;
  ls) echo '{"time":"x","short_id":"4f2a9c1e","struct_type":"snapshot"}'
      echo '{"name":"etc","type":"dir","path":"/etc","uid":0,"gid":0,"size":0,"mode":1,"struct_type":"node"}'
      echo '{"name":"hosts","type":"file","path":"/etc/hosts","uid":0,"gid":0,"size":5,"mode":420,"struct_type":"node"}' ;;
  restore) t=""; while [ $# -gt 0 ]; do [ "$1" = "--target" ] && t=$2; shift; done
      mkdir -p "$t/etc"; printf 'hello' > "$t/etc/hosts" ;;
esac
"#;
        std::fs::write(dir.join("restic"), fake).unwrap();
        std::fs::set_permissions(dir.join("restic"), std::fs::Permissions::from_mode(0o755)).unwrap();
        let j = BackupJob {
            id: 2,
            host_id: 1,
            name: "r".into(),
            tool: BackupTool::Restic,
            source: BackupSource::Path,
            target: "/srv/restic".into(),
            stale_hours: 26,
        };
        let path = format!("{}:{}", dir.display(), std::env::var("PATH").unwrap_or_default());
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(test_command(&j).unwrap())
            .env("PATH", path)
            .env("RESTIC_PASSWORD_FILE", "/dev/null")
            .output()
            .unwrap();
        let t = parse_test(2, &String::from_utf8_lossy(&out.stdout), 0);
        std::fs::remove_dir_all(&dir).ok();
        assert!(t.ok, "{t:?}");
        assert_eq!((t.file.as_deref(), t.size, t.snapshot.as_deref()), (Some("/etc/hosts"), Some(5), Some("4f2a9c1e")));
        assert_eq!(t.snapshot_time.as_deref(), Some("2026-11-15T03:00:00Z"));
    }
}
