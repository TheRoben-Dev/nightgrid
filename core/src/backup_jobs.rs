//! Backup status board (M10g): restic, borg, rsync and rclone jobs per host,
//! with last run, size, result and a "stale" flag.
//!
//! Nightgrid doesn't run backups or open repositories (that would need the
//! repository password). A job points at something the host already has:
//!
//! - a **systemd unit** (the timer or the service it starts): the last run,
//!   exit status and the run's journal output, where restic, borg, rsync and
//!   rclone print their totals;
//! - a **path** (a restic or borg repository, an rsync target or a marker
//!   file): the newest file's time and the size on disk.
//!
//! One read-only script per host covers every job; no sudo.

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::files::path::shell_quote;
use crate::hosts::HostId;
use crate::redact::line_has_secret;
use crate::ssh::client::Connection;
use crate::store::Store;
use crate::{Error, Result};

pub type BackupJobId = i32;

pub const MAX_NAME: usize = 80;
/// At most 90 days.
pub const MAX_STALE_HOURS: u32 = 24 * 90;
const MARK: &str = "@@NGBK@@";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum BackupTool {
    Restic,
    Borg,
    Rsync,
    Rclone,
    Other,
}

impl BackupTool {
    pub fn as_str(self) -> &'static str {
        match self {
            BackupTool::Restic => "restic",
            BackupTool::Borg => "borg",
            BackupTool::Rsync => "rsync",
            BackupTool::Rclone => "rclone",
            BackupTool::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "restic" => BackupTool::Restic,
            "borg" => BackupTool::Borg,
            "rsync" => BackupTool::Rsync,
            "rclone" => BackupTool::Rclone,
            _ => BackupTool::Other,
        }
    }

    /// The tool a command line runs (borgmatic and autorestic count as their tool).
    pub fn detect(command: &str) -> Option<Self> {
        let words: Vec<&str> = command
            .split(|c: char| c.is_whitespace() || matches!(c, ';' | '&' | '|' | '(' | ')' | '"' | '\''))
            .map(|w| w.rsplit('/').next().unwrap_or(w))
            .collect();
        let has = |names: &[&str]| words.iter().any(|w| names.contains(w));
        if has(&["restic", "autorestic", "resticprofile"]) {
            Some(BackupTool::Restic)
        } else if has(&["borg", "borgmatic", "borgbackup"]) {
            Some(BackupTool::Borg)
        } else if has(&["rclone"]) {
            Some(BackupTool::Rclone)
        } else if has(&["rsync", "rsnapshot"]) {
            Some(BackupTool::Rsync)
        } else if has(&["kopia", "duplicity", "duplicati", "duplicacy", "bupstash", "tar"]) {
            Some(BackupTool::Other)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum BackupSource {
    /// A systemd timer or service.
    Unit,
    /// A repository, target folder or marker file.
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BackupJob {
    pub id: BackupJobId,
    pub host_id: HostId,
    pub name: String,
    pub tool: BackupTool,
    pub source: BackupSource,
    /// Unit name or absolute path.
    pub target: String,
    /// The job counts as stale when its last good run is older than this.
    pub stale_hours: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BackupJobInput {
    pub host_id: HostId,
    pub name: String,
    pub tool: BackupTool,
    pub source: BackupSource,
    pub target: String,
    pub stale_hours: u32,
}

/// "26 hours", "3 days".
pub fn stale_words(hours: u32) -> String {
    if hours % 24 == 0 && hours >= 48 {
        format!("{} days", hours / 24)
    } else if hours == 1 {
        "hour".into()
    } else {
        format!("{hours} hours")
    }
}

pub fn valid_unit(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 200
        && !s.starts_with('-')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '_' | ':' | '-' | '\\'))
}

/// Trims and checks a job before it is stored.
pub fn normalize_job(i: &BackupJobInput) -> Result<BackupJobInput> {
    let name = i.name.trim().to_owned();
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(Error::Invalid { field: "name", reason: format!("must be 1-{MAX_NAME} characters") });
    }
    let target = i.target.trim().to_owned();
    match i.source {
        BackupSource::Unit if !valid_unit(&target) => {
            return Err(Error::Invalid { field: "target", reason: "isn't a systemd unit name".into() })
        }
        BackupSource::Path
            if !target.starts_with('/') || target.len() > 4096 || target.chars().any(char::is_control) =>
        {
            return Err(Error::Invalid { field: "target", reason: "must be an absolute path".into() })
        }
        _ => {}
    }
    if i.stale_hours == 0 || i.stale_hours > MAX_STALE_HOURS {
        return Err(Error::Invalid { field: "staleHours", reason: format!("must be 1-{MAX_STALE_HOURS}") });
    }
    Ok(BackupJobInput { name, target, ..i.clone() })
}

const COLS: &str = "id, host_id, name, tool, source, target, stale_hours";

fn job_row(r: &Row<'_>) -> rusqlite::Result<BackupJob> {
    let tool: String = r.get(3)?;
    let source: String = r.get(4)?;
    Ok(BackupJob {
        id: r.get(0)?,
        host_id: r.get(1)?,
        name: r.get(2)?,
        tool: BackupTool::parse(&tool),
        source: if source == "path" { BackupSource::Path } else { BackupSource::Unit },
        target: r.get(5)?,
        stale_hours: r.get(6)?,
    })
}

fn source_str(s: BackupSource) -> &'static str {
    match s {
        BackupSource::Unit => "unit",
        BackupSource::Path => "path",
    }
}

impl Store {
    pub fn list_backup_jobs(&self) -> Result<Vec<BackupJob>> {
        let mut stmt = self.conn().prepare(&format!("SELECT {COLS} FROM backup_jobs ORDER BY host_id, name, id"))?;
        let rows = stmt.query_map([], job_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn get_backup_job(&self, id: BackupJobId) -> Result<BackupJob> {
        self.conn()
            .query_row(&format!("SELECT {COLS} FROM backup_jobs WHERE id = ?1"), [id], job_row)
            .optional()?
            .ok_or(Error::NotFound("backup job"))
    }

    pub fn create_backup_job(&mut self, input: &BackupJobInput) -> Result<BackupJob> {
        let j = normalize_job(input)?;
        self.conn().execute(
            "INSERT INTO backup_jobs (host_id, name, tool, source, target, stale_hours) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![j.host_id, j.name, j.tool.as_str(), source_str(j.source), j.target, j.stale_hours],
        )?;
        let id = self.conn().last_insert_rowid() as BackupJobId;
        self.get_backup_job(id)
    }

    pub fn update_backup_job(&mut self, id: BackupJobId, input: &BackupJobInput) -> Result<BackupJob> {
        let j = normalize_job(input)?;
        let changed = self.conn().execute(
            "UPDATE backup_jobs SET host_id = ?2, name = ?3, tool = ?4, source = ?5, target = ?6, stale_hours = ?7
             WHERE id = ?1",
            params![id, j.host_id, j.name, j.tool.as_str(), source_str(j.source), j.target, j.stale_hours],
        )?;
        if changed == 0 {
            return Err(Error::NotFound("backup job"));
        }
        self.get_backup_job(id)
    }

    pub fn delete_backup_job(&mut self, id: BackupJobId) -> Result<()> {
        if self.conn().execute("DELETE FROM backup_jobs WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("backup job"));
        }
        Ok(())
    }
}

// ---- status ---------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum BackupResult {
    Ok,
    Failed,
    Running,
    /// Never ran, not found or not readable.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BackupStatus {
    pub job_id: BackupJobId,
    pub result: BackupResult,
    /// When the last run finished, or the newest file's time (unix seconds).
    pub last_run: Option<f64>,
    /// When the last good run finished; drives "stale".
    pub last_ok: Option<f64>,
    pub exit_code: Option<i32>,
    /// Size of the data backed up (or of the path on disk).
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub size: Option<u64>,
    /// What the last run added or transferred.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub added: Option<u64>,
    /// One line about the last run (snapshot id, error).
    pub summary: Option<String>,
    pub stale: bool,
    pub error: Option<String>,
}

impl BackupStatus {
    fn empty(job_id: BackupJobId) -> Self {
        BackupStatus {
            job_id,
            result: BackupResult::Unknown,
            last_run: None,
            last_ok: None,
            exit_code: None,
            size: None,
            added: None,
            summary: None,
            stale: false,
            error: None,
        }
    }
}

/// One script for all of a host's jobs.
pub fn status_command(jobs: &[BackupJob]) -> String {
    let mut s = String::from("exec 2>/dev/null; echo \"@@NGBK@@ now $(date +%s) $(cut -d' ' -f1 /proc/uptime)\"\n");
    for j in jobs {
        let t = shell_quote(&j.target);
        match j.source {
            BackupSource::Unit => s.push_str(&format!(
                "u={t}; case \"$u\" in *.timer) sv=$(systemctl show -p Unit --value \"$u\");; *.service) sv=$u;; \
                 *) sv=\"$u.service\";; esac\n\
                 echo '{MARK} {id} unit'\n\
                 systemctl show \"$sv\" -p Id,LoadState,ActiveState,SubState,Result,ExecMainStatus,\
                 ExecMainStartTimestampMonotonic,ExecMainExitTimestampMonotonic,InvocationID\n\
                 echo '{MARK} log'\n\
                 inv=$(systemctl show -p InvocationID --value \"$sv\")\n\
                 l=''; [ -n \"$inv\" ] && l=$(journalctl _SYSTEMD_INVOCATION_ID=\"$inv\" -o cat --no-pager -q | tail -n 80)\n\
                 [ -z \"$l\" ] && l=$(journalctl -u \"$sv\" -n 60 -o cat --no-pager -q)\n\
                 printf '%s\\n' \"$l\"\n",
                id = j.id
            )),
            BackupSource::Path => s.push_str(&format!(
                "p={t}; echo '{MARK} {id} path'\n\
                 if [ -e \"$p\" ]; then stat -c 'stat %Y %s %F' \"$p\"; \
                 if [ -d \"$p\" ]; then echo \"newest $(timeout 15 find \"$p\" -maxdepth 3 -type f -printf '%T@\\n' | sort -n | tail -n 1)\"; \
                 echo \"du $(timeout 20 du -sk \"$p\" | cut -f1)\"; fi; \
                 elif [ -r \"$(dirname \"$p\")\" ]; then echo missing; else echo denied; fi\n",
                id = j.id
            )),
        }
    }
    s.push_str(&format!("echo '{MARK} end'\n"));
    s
}

/// Reads [`status_command`]'s output. `now` is the local clock, used only
/// when the host's clock line is missing.
pub fn parse_status(out: &str, jobs: &[BackupJob], now: f64) -> Vec<BackupStatus> {
    let mut host_now = now;
    let mut uptime: Option<f64> = None;
    let mut blocks: Vec<(BackupJobId, &str, Vec<&str>, Vec<&str>)> = Vec::new();
    let mut in_log = false;
    for line in out.lines() {
        if let Some(rest) = line.strip_prefix(MARK) {
            let w: Vec<&str> = rest.split_whitespace().collect();
            match w.as_slice() {
                ["now", n, up] => {
                    host_now = n.parse().unwrap_or(now);
                    uptime = up.parse().ok();
                }
                ["log"] => in_log = true,
                [id, kind] => {
                    in_log = false;
                    if let Ok(id) = id.parse() {
                        blocks.push((id, if *kind == "path" { "path" } else { "unit" }, Vec::new(), Vec::new()));
                    }
                }
                _ => in_log = false,
            }
            continue;
        }
        if let Some(b) = blocks.last_mut() {
            if in_log {
                b.3.push(line);
            } else {
                b.2.push(line);
            }
        }
    }
    jobs.iter()
        .map(|j| {
            let mut st = match blocks.iter().find(|b| b.0 == j.id) {
                Some((_, "path", props, _)) => path_status(j.id, props),
                Some((_, _, props, log)) => unit_status(j.id, props, log, host_now, uptime),
                None => BackupStatus {
                    error: Some("No answer from the host for this job.".into()),
                    ..BackupStatus::empty(j.id)
                },
            };
            let limit = f64::from(j.stale_hours) * 3600.0;
            st.stale = st.error.is_none()
                && st.result != BackupResult::Running
                && match (st.last_ok, st.last_run) {
                    (Some(t), _) => host_now - t > limit,
                    // A failed run with no good one since boot.
                    (None, Some(_)) => true,
                    // No run since the host booted: only stale once it has
                    // been up longer than the limit.
                    (None, None) => j.source == BackupSource::Path || uptime.map_or(true, |u| u > limit),
                };
            st
        })
        .collect()
}

fn unit_status(id: BackupJobId, props: &[&str], log: &[&str], now: f64, uptime: Option<f64>) -> BackupStatus {
    let mut st = BackupStatus::empty(id);
    let get = |k: &str| props.iter().find_map(|l| l.strip_prefix(k)?.strip_prefix('=')).unwrap_or("");
    if get("LoadState") == "not-found" || props.is_empty() {
        st.error = Some("The unit wasn't found on this host.".into());
        return st;
    }
    let boot = uptime.map(|u| now - u);
    let mono = |k: &str| get(k).parse::<f64>().ok().filter(|v| *v > 0.0);
    let started = mono("ExecMainStartTimestampMonotonic").zip(boot).map(|(m, b)| b + m / 1e6);
    let exited = mono("ExecMainExitTimestampMonotonic").zip(boot).map(|(m, b)| b + m / 1e6);
    let active = get("ActiveState");
    let code = get("ExecMainStatus").parse::<i32>().ok();
    let result = get("Result");
    let totals = parse_totals(log);
    st.size = totals.size;
    st.added = totals.added;
    st.summary = totals.summary;
    if active == "activating" || (active == "active" && get("SubState") == "running") {
        st.result = BackupResult::Running;
        st.last_run = started;
        st.summary = Some("Running now".into());
        return st;
    }
    let Some(done) = exited.or(started) else {
        // Never ran since boot. The journal may still have an earlier run.
        st.summary = Some("No run since the host booted".into());
        if totals.ok_seen {
            st.result = BackupResult::Unknown;
        }
        return st;
    };
    st.last_run = Some(done);
    st.exit_code = code;
    if result == "success" && code.unwrap_or(0) == 0 {
        st.result = BackupResult::Ok;
        st.last_ok = Some(done);
    } else {
        st.result = BackupResult::Failed;
        st.summary = totals.error.or_else(|| Some(format!("Result: {result}, exit {}", code.unwrap_or(-1))));
    }
    st
}

fn path_status(id: BackupJobId, props: &[&str]) -> BackupStatus {
    let mut st = BackupStatus::empty(id);
    if props.first() == Some(&"missing") {
        st.error = Some("The path doesn't exist on this host.".into());
        return st;
    }
    if props.first() == Some(&"denied") || props.is_empty() {
        st.error = Some("The path can't be read by this login.".into());
        return st;
    }
    let mut mtime = None;
    let mut is_dir = false;
    for l in props {
        let w: Vec<&str> = l.split_whitespace().collect();
        match w.as_slice() {
            ["stat", m, size, kind @ ..] => {
                mtime = m.parse::<f64>().ok();
                is_dir = kind.first() == Some(&"directory");
                if !is_dir {
                    st.size = size.parse().ok();
                }
            }
            ["newest", t] => {
                if let Ok(t) = t.parse::<f64>() {
                    mtime = Some(t.floor());
                }
            }
            ["du", kb] => st.size = kb.parse::<u64>().ok().map(|k| k * 1024),
            _ => {}
        }
    }
    st.last_run = mtime;
    st.last_ok = mtime;
    st.result = if mtime.is_some() { BackupResult::Ok } else { BackupResult::Unknown };
    if is_dir && st.size.is_none() {
        st.summary = Some("Size not read (took too long)".into());
    }
    st
}

#[derive(Debug, Default, PartialEq)]
pub struct Totals {
    pub size: Option<u64>,
    pub added: Option<u64>,
    pub summary: Option<String>,
    pub error: Option<String>,
    /// A line that only a successful run prints was seen.
    pub ok_seen: bool,
}

/// Totals that restic, borg (`--stats`), rsync (`--stats` or `-v`) and
/// rclone print at the end of a run. Later lines win.
pub fn parse_totals(lines: &[&str]) -> Totals {
    let mut t = Totals::default();
    for raw in lines {
        let l = raw.trim();
        let lower = l.to_ascii_lowercase();
        // restic: "processed 1234 files, 5.678 GiB in 1:23"
        if let Some(rest) = l.strip_prefix("processed ") {
            if let Some((_, after)) = rest.split_once(" files, ") {
                if let Some(sz) = size_at(after) {
                    t.size = Some(sz);
                }
            }
        }
        // restic: "Added to the repository: 12.345 MiB (3.4 MiB stored)"
        if let Some(rest) = l.strip_prefix("Added to the repository:") {
            t.added = size_at(rest.trim());
        }
        // restic: "snapshot 1a2b3c4d saved"
        if l.starts_with("snapshot ") && l.ends_with(" saved") {
            t.summary = Some(l.to_owned());
            t.ok_seen = true;
        }
        // borg --stats: "This archive:   1.23 GB   800 MB   12.3 MB"
        if let Some(rest) = l.strip_prefix("This archive:") {
            let sizes = sizes_in(rest);
            if let Some(first) = sizes.first() {
                t.size = Some(*first);
            }
            if sizes.len() >= 3 {
                t.added = Some(sizes[2]);
            }
            t.ok_seen = true;
        }
        if let Some(rest) = l.strip_prefix("Archive name:") {
            t.summary = Some(format!("archive {}", rest.trim()));
        }
        // rsync --stats
        if let Some(rest) = l.strip_prefix("Total file size:") {
            t.size = size_at(rest.trim());
        }
        if let Some(rest) = l.strip_prefix("Total transferred file size:") {
            t.added = size_at(rest.trim());
        }
        // rsync -v: "sent 1,234 bytes  received 56 bytes  …" / "total size is 1,234,567  speedup is 1.00"
        if let Some(rest) = l.strip_prefix("sent ") {
            if rest.contains(" received ") {
                t.added = t.added.or(size_at(rest));
                t.ok_seen = true;
            }
        }
        if let Some(rest) = l.strip_prefix("total size is ") {
            t.size = size_at(rest);
        }
        // rclone: "Transferred:   	  1.234 GiB / 1.234 GiB, 100%, …"
        if let Some(rest) = l.strip_prefix("Transferred:") {
            if rest.contains('/') {
                t.added = size_at(rest.trim());
            }
        }
        if (lower.starts_with("fatal") || lower.contains("error:") || lower.starts_with("error")) && !line_has_secret(l)
        {
            t.error = Some(l.chars().take(200).collect());
        }
    }
    t
}

/// A size at the start of `s`: "5.678 GiB", "12,345 bytes", "800 MB", "1234".
fn size_at(s: &str) -> Option<u64> {
    let mut it = s.split_whitespace();
    let num = it.next()?;
    let unit = it.next().unwrap_or("").trim_end_matches([',', '.', ')']);
    to_bytes(num, unit)
}

fn sizes_in(s: &str) -> Vec<u64> {
    let w: Vec<&str> = s.split_whitespace().collect();
    w.windows(2).filter_map(|p| to_bytes(p[0], p[1]).filter(|_| unit_factor(p[1]).is_some())).collect()
}

fn unit_factor(unit: &str) -> Option<f64> {
    Some(match unit {
        "B" | "bytes" | "byte" => 1.0,
        "kB" | "KB" | "K" => 1e3,
        "MB" | "M" => 1e6,
        "GB" | "G" => 1e9,
        "TB" | "T" => 1e12,
        "KiB" => 1024.0,
        "MiB" => 1024f64.powi(2),
        "GiB" => 1024f64.powi(3),
        "TiB" => 1024f64.powi(4),
        _ => return None,
    })
}

fn to_bytes(num: &str, unit: &str) -> Option<u64> {
    let n: f64 = num.replace(',', "").parse().ok()?;
    if !n.is_finite() || n < 0.0 {
        return None;
    }
    Some((n * unit_factor(unit).unwrap_or(1.0)).round() as u64)
}

/// Reads every job of one host.
pub async fn check(conn: &Connection, jobs: &[BackupJob], now: f64) -> std::result::Result<Vec<BackupStatus>, String> {
    if jobs.is_empty() {
        return Ok(Vec::new());
    }
    let (_, out, _) = conn.exec(&status_command(jobs)).await.map_err(|e| e.to_string())?;
    Ok(parse_status(&String::from_utf8_lossy(&out), jobs, now))
}

// ---- detection ------------------------------------------------------------

/// A job Nightgrid found on the host, for the user to add.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BackupSuggestion {
    pub name: String,
    pub tool: BackupTool,
    pub source: BackupSource,
    /// Empty when the user has to fill it in (a cron job without a local target).
    pub target: String,
    /// Where it was found and what it runs (masked if it looks secret).
    pub detail: String,
}

pub fn detect_command() -> String {
    format!(
        "exec 2>/dev/null\n\
         for t in $(systemctl list-timers --all --no-legend --plain | awk '{{for(i=1;i<=NF;i++) if($i ~ /\\.timer$/) print $i}}'); do\n\
         s=$(systemctl show -p Unit --value \"$t\"); e=$(systemctl show -p ExecStart --value \"$s\" | head -c 600 | tr '\\n' ' ')\n\
         echo \"T\t$t\t$s\t$e\"; done\n\
         crontab -l | sed 's/^/C\tcrontab\t/'\n\
         for f in /etc/crontab /etc/cron.d/*; do [ -r \"$f\" ] && sed \"s|^|C\t$f\t|\" \"$f\"; done\n\
         for f in /etc/cron.daily/* /etc/cron.weekly/*; do [ -r \"$f\" ] && grep -E 'restic|borg|rsync|rclone' \"$f\" | head -n 3 | sed \"s|^|C\t$f\t|\"; done\n\
         echo '{MARK} end'\n"
    )
}

pub fn parse_detect(out: &str) -> Vec<BackupSuggestion> {
    let mut v: Vec<BackupSuggestion> = Vec::new();
    for line in out.lines() {
        let parts: Vec<&str> = line.splitn(4, '\t').collect();
        match parts.as_slice() {
            ["T", timer, _service, exec] => {
                let Some(tool) = BackupTool::detect(exec) else { continue };
                if !valid_unit(timer) {
                    continue;
                }
                v.push(BackupSuggestion {
                    name: timer.trim_end_matches(".timer").to_owned(),
                    tool,
                    source: BackupSource::Unit,
                    target: (*timer).to_owned(),
                    detail: masked(&format!("timer {timer}: {}", exec_path(exec))),
                });
            }
            ["C", file, rest @ ..] => {
                let cmd = rest.join("\t");
                let cmd = cmd.trim();
                if cmd.is_empty() || cmd.starts_with('#') || cmd.contains("=") && !cmd.contains(' ') {
                    continue;
                }
                let Some(tool) = BackupTool::detect(cmd) else { continue };
                let target = local_target(tool, cmd).unwrap_or_default();
                let name = format!("{} ({})", tool.as_str(), file.rsplit('/').next().unwrap_or(file));
                if v.iter().any(|s| s.detail == masked(&format!("{file}: {cmd}"))) {
                    continue;
                }
                v.push(BackupSuggestion {
                    name,
                    tool,
                    source: BackupSource::Path,
                    target,
                    detail: masked(&format!("{file}: {cmd}")),
                });
            }
            _ => {}
        }
    }
    v
}

/// systemd prints ExecStart as `{ path=/usr/bin/restic ; argv[]=/usr/bin/restic backup … ; … }`.
fn exec_path(exec: &str) -> String {
    exec.split("argv[]=")
        .nth(1)
        .map(|a| a.split(" ;").next().unwrap_or(a).trim().to_owned())
        .unwrap_or_else(|| exec.trim().to_owned())
}

fn masked(s: &str) -> String {
    if line_has_secret(s) {
        let head: String = s.split(':').next().unwrap_or("").chars().take(120).collect();
        format!("{head}: (command hidden: it looks like it holds a secret)")
    } else {
        s.chars().take(300).collect()
    }
}

/// A local repository or target folder named on a command line.
pub fn local_target(tool: BackupTool, cmd: &str) -> Option<String> {
    let words: Vec<&str> = cmd.split_whitespace().map(|w| w.trim_matches(['"', '\''])).collect();
    let local = |s: &str| s.starts_with('/').then(|| s.trim_end_matches('/').to_owned()).filter(|s| !s.is_empty());
    match tool {
        BackupTool::Restic => words.iter().enumerate().find_map(|(i, w)| {
            if let Some(v) = w.strip_prefix("RESTIC_REPOSITORY=").or(w.strip_prefix("--repo=")) {
                return local(v);
            }
            (matches!(*w, "-r" | "--repo")).then(|| words.get(i + 1).and_then(|v| local(v))).flatten()
        }),
        BackupTool::Borg => words.iter().find_map(|w| {
            if let Some(v) = w.strip_prefix("BORG_REPO=") {
                return local(v);
            }
            w.split_once("::").and_then(|(repo, _)| local(repo))
        }),
        BackupTool::Rsync => {
            let args: Vec<&str> = words
                .iter()
                .skip_while(|w| !w.ends_with("rsync"))
                .skip(1)
                .copied()
                .filter(|w| !w.starts_with('-') && !matches!(*w, ">" | ">>" | "2>&1" | "&&" | ";" | "|"))
                .take_while(|w| !w.starts_with('>'))
                .collect();
            args.last().and_then(|d| local(d))
        }
        _ => None,
    }
}

pub async fn detect(conn: &Connection) -> std::result::Result<Vec<BackupSuggestion>, String> {
    let (_, out, _) = conn.exec(&detect_command()).await.map_err(|e| e.to_string())?;
    Ok(parse_detect(&String::from_utf8_lossy(&out)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: i32, source: BackupSource, target: &str) -> BackupJob {
        BackupJob {
            id,
            host_id: 1,
            name: "j".into(),
            tool: BackupTool::Restic,
            source,
            target: target.into(),
            stale_hours: 26,
        }
    }

    #[test]
    fn detects_tools() {
        assert_eq!(BackupTool::detect("/usr/bin/restic backup /srv"), Some(BackupTool::Restic));
        assert_eq!(BackupTool::detect("borgmatic --stats"), Some(BackupTool::Borg));
        assert_eq!(BackupTool::detect("rsync -a /a/ /b/"), Some(BackupTool::Rsync));
        assert_eq!(BackupTool::detect("rclone sync /a remote:b"), Some(BackupTool::Rclone));
        assert_eq!(BackupTool::detect("apt-get update"), None);
        assert_eq!(BackupTool::detect("/usr/local/bin/unrestic"), None);
    }

    #[test]
    fn normalizes_jobs() {
        let mut i = BackupJobInput {
            host_id: 1,
            name: " nightly ".into(),
            tool: BackupTool::Restic,
            source: BackupSource::Unit,
            target: "restic-backup.timer".into(),
            stale_hours: 26,
        };
        assert_eq!(normalize_job(&i).unwrap().name, "nightly");
        i.target = "x; rm -rf /".into();
        assert!(normalize_job(&i).is_err());
        i.source = BackupSource::Path;
        i.target = "relative/dir".into();
        assert!(normalize_job(&i).is_err());
        i.target = "/mnt/backup/restic".into();
        assert!(normalize_job(&i).is_ok());
        i.stale_hours = 0;
        assert!(normalize_job(&i).is_err());
    }

    #[test]
    fn store_roundtrip() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&crate::hosts::HostInput {
                name: "nas".into(),
                hostname: "nas.lan".into(),
                ..Default::default()
            })
            .unwrap();
        let j = s
            .create_backup_job(&BackupJobInput {
                host_id: h.id,
                name: "repo".into(),
                tool: BackupTool::Borg,
                source: BackupSource::Path,
                target: "/srv/borg".into(),
                stale_hours: 48,
            })
            .unwrap();
        assert_eq!(s.list_backup_jobs().unwrap(), vec![j.clone()]);
        s.delete_host(h.id).unwrap();
        assert!(s.list_backup_jobs().unwrap().is_empty());
    }

    #[test]
    fn restic_totals() {
        let log = [
            "Files:          12 new,     3 changed,  1200 unmodified",
            "Added to the repository: 12.500 MiB (3.402 MiB stored)",
            "processed 1215 files, 5.250 GiB in 0:42",
            "snapshot 1a2b3c4d saved",
        ];
        let t = parse_totals(&log);
        assert_eq!(t.size, Some((5.25 * 1024f64.powi(3)) as u64));
        assert_eq!(t.added, Some((12.5 * 1024f64.powi(2)) as u64));
        assert_eq!(t.summary.as_deref(), Some("snapshot 1a2b3c4d saved"));
    }

    #[test]
    fn borg_and_rsync_totals() {
        let borg = [
            "Archive name: host-2026-11-15",
            "                       Original size      Compressed size    Deduplicated size",
            "This archive:                1.23 GB            800.00 MB             12.30 MB",
        ];
        let t = parse_totals(&borg);
        assert_eq!(t.size, Some(1_230_000_000));
        assert_eq!(t.added, Some(12_300_000));
        assert_eq!(t.summary.as_deref(), Some("archive host-2026-11-15"));
        let rsync =
            ["sent 1,234 bytes  received 56 bytes  2,580.00 bytes/sec", "total size is 9,876,543  speedup is 7,656.23"];
        let t = parse_totals(&rsync);
        assert_eq!(t.added, Some(1234));
        assert_eq!(t.size, Some(9_876_543));
        let stats = ["Total file size: 2,000 bytes", "Total transferred file size: 500 bytes"];
        let t = parse_totals(&stats);
        assert_eq!((t.size, t.added), (Some(2000), Some(500)));
    }

    #[test]
    fn errors_are_kept_unless_secret() {
        let t = parse_totals(&["Fatal: unable to open config file: Stat: permission denied"]);
        assert!(t.error.unwrap().starts_with("Fatal: unable"));
        let t = parse_totals(&["error: password=hunter2222 rejected"]);
        assert!(t.error.is_none());
    }

    #[test]
    fn unit_status_ok_failed_running() {
        let jobs = [
            job(1, BackupSource::Unit, "a.timer"),
            job(2, BackupSource::Unit, "b.service"),
            job(3, BackupSource::Unit, "c"),
        ];
        // Host booted at 1000; now 100000.
        let out = "@@NGBK@@ now 100000 99000.5\n\
            @@NGBK@@ 1 unit\nId=a.service\nLoadState=loaded\nActiveState=inactive\nSubState=dead\nResult=success\nExecMainStatus=0\n\
            ExecMainStartTimestampMonotonic=90000000000\nExecMainExitTimestampMonotonic=90060000000\nInvocationID=x\n\
            @@NGBK@@ log\nsnapshot deadbeef saved\nprocessed 3 files, 1.000 KiB in 0:01\n\
            @@NGBK@@ 2 unit\nId=b.service\nLoadState=loaded\nActiveState=failed\nSubState=failed\nResult=exit-code\nExecMainStatus=1\n\
            ExecMainStartTimestampMonotonic=98000000000\nExecMainExitTimestampMonotonic=98001000000\n\
            @@NGBK@@ log\nFatal: repository is locked\n\
            @@NGBK@@ 3 unit\nId=c.service\nLoadState=not-found\n@@NGBK@@ log\n@@NGBK@@ end\n";
        let st = parse_status(out, &jobs, 0.0);
        assert_eq!(st[0].result, BackupResult::Ok);
        let done = st[0].last_run.unwrap();
        assert!((done - (999.5 + 90060.0)).abs() < 0.01, "{done}");
        assert_eq!(st[0].size, Some(1024));
        assert!(!st[0].stale);
        assert_eq!(st[1].result, BackupResult::Failed);
        assert_eq!(st[1].exit_code, Some(1));
        assert_eq!(st[1].summary.as_deref(), Some("Fatal: repository is locked"));
        assert!(st[1].stale, "no good run counts as stale");
        assert!(st[2].error.is_some());
        assert!(!st[2].stale);
    }

    #[test]
    fn stale_after_hours() {
        let jobs = [job(1, BackupSource::Path, "/srv/repo")];
        let out = format!(
            "@@NGBK@@ now {} 5000\n@@NGBK@@ 1 path\nstat 100 4096 directory\nnewest 1000.25\ndu 2048\n@@NGBK@@ end\n",
            1000 + 27 * 3600
        );
        let st = parse_status(&out, &jobs, 0.0);
        assert_eq!(st[0].last_run, Some(1000.0));
        assert_eq!(st[0].size, Some(2048 * 1024));
        assert!(st[0].stale);
        let out = "@@NGBK@@ now 2000 5000\n@@NGBK@@ 1 path\nmissing\n@@NGBK@@ end\n";
        assert!(parse_status(out, &jobs, 0.0)[0].error.as_deref().unwrap().contains("doesn't exist"));
    }

    #[test]
    fn detection() {
        let out = "T\trestic-backup.timer\trestic-backup.service\t{ path=/usr/bin/restic ; argv[]=/usr/bin/restic backup /srv ; ignore_errors=no }\n\
                   T\tlogrotate.timer\tlogrotate.service\t{ path=/usr/sbin/logrotate ; argv[]=/usr/sbin/logrotate /etc/logrotate.conf }\n\
                   C\tcrontab\t0 3 * * * borg create /mnt/usb/borg::{now} /home\n\
                   C\t/etc/cron.d/sync\t30 2 * * * root rsync -a --delete /srv/ /mnt/backup/srv/ >/var/log/sync.log 2>&1\n\
                   C\tcrontab\t0 4 * * * RESTIC_PASSWORD=hunter22hunter restic -r /mnt/r backup /etc\n\
                   C\tcrontab\t# 0 1 * * * rsync -a /a /b\n";
        let v = parse_detect(out);
        assert_eq!(v.len(), 4, "{v:?}");
        assert_eq!(v[0].target, "restic-backup.timer");
        assert_eq!(v[0].detail, "timer restic-backup.timer: /usr/bin/restic backup /srv");
        assert_eq!(v[1].target, "/mnt/usb/borg");
        assert_eq!(v[2].target, "/mnt/backup/srv");
        assert!(v[3].detail.contains("command hidden"));
        assert_eq!(v[3].target, "/mnt/r");
    }

    #[test]
    fn local_targets() {
        assert_eq!(local_target(BackupTool::Restic, "restic --repo=/srv/r backup /x"), Some("/srv/r".into()));
        assert_eq!(local_target(BackupTool::Restic, "restic -r sftp:host:/r backup /x"), None);
        assert_eq!(local_target(BackupTool::Rsync, "rsync -a /a/ host:/b/"), None);
    }

    #[test]
    fn scripts_run_under_sh() {
        let jobs = [job(1, BackupSource::Unit, "nightgrid-test.timer"), job(2, BackupSource::Path, "/tmp")];
        let out = std::process::Command::new("sh").arg("-c").arg(status_command(&jobs)).output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(text.contains("@@NGBK@@ end"), "{text}");
        let st = parse_status(&text, &jobs, 0.0);
        assert_eq!(st.len(), 2);
        assert!(st[1].last_run.is_some(), "{text}");
        let out = std::process::Command::new("sh").arg("-c").arg(detect_command()).output().unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).contains("@@NGBK@@ end"));
    }
}
