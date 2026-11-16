//! Editing scheduled jobs (M10v): crontabs and Nightgrid's own systemd
//! timers. Crontab edits are applied here to the text that was read, checked
//! line by line, and written back whole with `crontab -` after a copy of the
//! old one is saved on the host. Timers are a `.service` + `.timer` pair in
//! `/etc/systemd/system/nightgrid-job-*`, with the spec as JSON on the first
//! line so they can be edited again.

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::schedule::{describe, CronSpec};
use crate::files::path::shell_quote;
use crate::{Error, Result};

fn invalid(field: &'static str, reason: impl Into<String>) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

/// Lines with this prefix are jobs turned off in Nightgrid.
pub const OFF: &str = "#nightgrid-off ";

/// A user name `crontab -u` and `User=` accept.
pub fn valid_user(u: &str) -> bool {
    let mut c = u.chars();
    matches!(c.next(), Some(f) if f.is_ascii_lowercase() || f == '_')
        && u.len() <= 32
        && c.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.'))
}

// ---- crontab ----------------------------------------------------------------

/// One job line of a crontab.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CrontabJob {
    /// 1-based line number in the text.
    pub line: u32,
    pub schedule: String,
    pub every: String,
    pub command: String,
    pub enabled: bool,
    /// Next run, unix seconds (none when off, `@reboot` or never).
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub next: Option<i64>,
}

/// A user's crontab as read.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Crontab {
    /// `None`: the login user's.
    pub user: Option<String>,
    pub text: String,
    /// Identifies this text; a write is refused if the crontab changed.
    pub hash: String,
    pub jobs: Vec<CrontabJob>,
    /// `MAILTO`, `PATH`, … settings in order.
    pub env: Vec<String>,
}

/// A change to one crontab.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CronEdit {
    Add { schedule: String, command: String },
    Update { line: u32, schedule: String, command: String },
    Remove { line: u32 },
    SetEnabled { line: u32, enabled: bool },
}

impl CronEdit {
    /// The kind of change, for logs (never the command, which may hold a token).
    pub fn op(&self) -> &'static str {
        match self {
            Self::Add { .. } => "add",
            Self::Update { .. } => "update",
            Self::Remove { .. } => "remove",
            Self::SetEnabled { .. } => "set-enabled",
        }
    }
}

pub fn hash(text: &str) -> String {
    let d = Sha256::digest(text.as_bytes());
    d.iter().take(12).map(|b| format!("{b:02x}")).collect()
}

/// Splits a job line into (schedule, command); `None` for anything else.
fn split_job(line: &str) -> Option<(String, String)> {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return None;
    }
    if let Some((k, _)) = t.split_once('=') {
        let k = k.trim();
        if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return None;
        }
    }
    let toks: Vec<&str> = t.split_whitespace().collect();
    let n = if toks.first()?.starts_with('@') { 1 } else { 5 };
    if toks.len() <= n {
        return None;
    }
    // The command keeps its own spacing.
    let mut rest = t;
    for _ in 0..n {
        rest = rest.trim_start();
        rest = &rest[rest.find(char::is_whitespace)?..];
    }
    Some((toks[..n].join(" "), rest.trim().to_owned()))
}

fn is_env(line: &str) -> bool {
    let t = line.trim();
    !t.starts_with('#')
        && t.split_once('=').is_some_and(|(k, _)| {
            let k = k.trim();
            !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

/// Parses a crontab. `now`/`offset_secs` are the host's clock and UTC
/// offset, for next runs.
pub fn parse_crontab(user: Option<String>, text: &str, now: i64, offset_secs: i64) -> Crontab {
    let mut jobs = Vec::new();
    let mut env = Vec::new();
    for (i, l) in text.lines().enumerate() {
        let (enabled, body) = match l.strip_prefix(OFF) {
            Some(b) => (false, b),
            None => (true, l),
        };
        if enabled && is_env(l) {
            env.push(l.trim().to_owned());
            continue;
        }
        let Some((schedule, command)) = split_job(body) else { continue };
        let next = if enabled {
            CronSpec::parse(&schedule).and_then(|s| s.next_after(now + offset_secs)).map(|t| t - offset_secs)
        } else {
            None
        };
        jobs.push(CrontabJob { line: i as u32 + 1, every: describe(&schedule), schedule, command, enabled, next });
    }
    Crontab { user, hash: hash(text), text: text.to_owned(), jobs, env }
}

/// Checks a schedule and command before they go into a crontab.
pub fn check_job(schedule: &str, command: &str) -> Result<(String, String)> {
    let s = schedule.split_whitespace().collect::<Vec<_>>().join(" ");
    if s != "@reboot" && CronSpec::parse(&s).is_none() {
        return Err(invalid("schedule", "not a cron schedule (five fields such as `*/5 * * * *`, or @daily)"));
    }
    let c = command.trim();
    if c.is_empty() {
        return Err(invalid("command", "empty"));
    }
    if c.contains(['\n', '\r']) {
        return Err(invalid("command", "one line only"));
    }
    if c.len() > 2000 {
        return Err(invalid("command", "longer than 2000 characters"));
    }
    // cron turns an unescaped % into a newline and feeds the rest as stdin.
    let b = c.as_bytes();
    if b.iter().enumerate().any(|(i, ch)| *ch == b'%' && (i == 0 || b[i - 1] != b'\\')) {
        return Err(invalid("command", "cron treats % as a line break: write \\% (for example date +\\%F)"));
    }
    Ok((s, c.to_owned()))
}

/// Applies `edit` to `text`, returning the new crontab text.
pub fn apply(text: &str, edit: &CronEdit) -> Result<String> {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let job_at = |lines: &Vec<String>, line: u32| -> Result<usize> {
        let i = (line as usize)
            .checked_sub(1)
            .filter(|i| *i < lines.len())
            .ok_or_else(|| invalid("line", "no such line"))?;
        let body = lines[i].strip_prefix(OFF).unwrap_or(&lines[i]);
        split_job(body).map(|_| i).ok_or_else(|| invalid("line", "not a job"))
    };
    match edit {
        CronEdit::Add { schedule, command } => {
            let (s, c) = check_job(schedule, command)?;
            lines.push(format!("{s} {c}"));
        }
        CronEdit::Update { line, schedule, command } => {
            let i = job_at(&lines, *line)?;
            let (s, c) = check_job(schedule, command)?;
            let off = lines[i].starts_with(OFF);
            lines[i] = format!("{}{s} {c}", if off { OFF } else { "" });
        }
        CronEdit::Remove { line } => {
            let i = job_at(&lines, *line)?;
            lines.remove(i);
        }
        CronEdit::SetEnabled { line, enabled } => {
            let i = job_at(&lines, *line)?;
            let body = lines[i].strip_prefix(OFF).unwrap_or(&lines[i]).to_owned();
            lines[i] = if *enabled { body } else { format!("{OFF}{body}") };
        }
    }
    let mut out = lines.join("\n");
    // crontab wants a final newline.
    out.push('\n');
    Ok(out)
}

fn crontab_flag(user: Option<&str>) -> Result<String> {
    match user {
        None => Ok(String::new()),
        Some(u) if valid_user(u) => Ok(format!(" -u {u}")),
        Some(_) => Err(invalid("user", "not a user name")),
    }
}

/// Prints the host's clock and offset, then the crontab.
pub fn read_command(user: Option<&str>) -> Result<String> {
    let u = crontab_flag(user)?;
    Ok(format!("date +%s; date +%z; echo @@crontab; crontab{u} -l 2>/dev/null; true"))
}

/// Splits [`read_command`] output into (now, offset seconds, text).
pub fn parse_read(out: &str) -> Option<(i64, i64, String)> {
    let (head, text) = out.split_once("@@crontab\n").or_else(|| out.split_once("@@crontab"))?;
    let mut h = head.lines();
    let now = h.next()?.trim().parse().ok()?;
    let z = h.next().unwrap_or("+0000").trim();
    let sign = if z.starts_with('-') { -1 } else { 1 };
    let d: String = z.chars().filter(char::is_ascii_digit).collect();
    let off = sign
        * (d.get(0..2).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0) * 3600
            + d.get(2..4).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0) * 60);
    Some((now, off, text.to_owned()))
}

/// Saves the current crontab under `~/.nightgrid/cron-backups` (keeping 20),
/// then installs `text`. Prints `written`.
pub fn write_command(user: Option<&str>, text: &str, ts: i64) -> Result<String> {
    let u = crontab_flag(user)?;
    let name = user.unwrap_or("me");
    let b64 = base64::engine::general_purpose::STANDARD.encode(text);
    Ok(format!(
        "umask 077; d=\"$HOME/.nightgrid/cron-backups\"; mkdir -p \"$d\"; b=\"$d/{name}-{ts}\"; \
crontab{u} -l > \"$b\" 2>/dev/null || rm -f \"$b\"; \
(cd \"$d\" && ls -1t | tail -n +21 | while IFS= read -r f; do rm -f -- \"$f\"; done); \
printf '%s' '{b64}' | base64 -d | crontab{u} - && echo written"
    ))
}

// ---- Nightgrid timers -------------------------------------------------------

/// A scheduled job as a systemd timer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TimerJob {
    pub name: String,
    pub command: String,
    /// `OnCalendar=`, such as `*-*-* 03:00:00` or `Mon..Fri 08:00`.
    pub calendar: String,
    /// `User=`; root when empty.
    pub run_as: Option<String>,
    /// Catch up on a run missed while the host was off.
    pub persistent: bool,
}

/// One installed Nightgrid timer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct InstalledJob {
    /// `nightgrid-job-…` (without `.timer`).
    pub unit: String,
    pub job: TimerJob,
}

pub const JOB_PREFIX: &str = "nightgrid-job-";
const DIR: &str = "/etc/systemd/system";

/// `Nightly backup!` → `nightly-backup`.
pub fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c.to_ascii_lowercase());
        } else if !s.ends_with('-') && !s.is_empty() {
            s.push('-');
        }
    }
    s.trim_end_matches('-').chars().take(40).collect::<String>().trim_end_matches('-').to_owned()
}

pub fn valid_job_unit(unit: &str) -> bool {
    unit.strip_prefix(JOB_PREFIX).is_some_and(|r| {
        !r.is_empty() && r.len() <= 40 && r.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    })
}

fn valid_calendar(c: &str) -> bool {
    !c.is_empty() && c.len() <= 100 && c.chars().all(|ch| ch.is_ascii_alphanumeric() || " *:,./~-+".contains(ch))
}

impl TimerJob {
    pub fn normalized(&self) -> Result<Self> {
        let name = self.name.trim().to_owned();
        if name.is_empty() || name.len() > 60 || name.contains(['\n', '\r']) || slug(&name).is_empty() {
            return Err(invalid("name", "1 to 60 characters, with a letter or digit"));
        }
        let command = self.command.trim().to_owned();
        if command.is_empty() || command.contains(['\n', '\r']) || command.len() > 2000 {
            return Err(invalid("command", "one line, up to 2000 characters"));
        }
        let calendar = self.calendar.split_whitespace().collect::<Vec<_>>().join(" ");
        if !valid_calendar(&calendar) {
            return Err(invalid("calendar", "letters, digits, spaces and * : , . / ~ - + only"));
        }
        let run_as = self.run_as.as_deref().map(str::trim).filter(|u| !u.is_empty() && *u != "root").map(str::to_owned);
        if run_as.as_deref().is_some_and(|u| !valid_user(u)) {
            return Err(invalid("runAs", "not a user name"));
        }
        Ok(Self { name, command, calendar, run_as, persistent: self.persistent })
    }

    pub fn unit(&self) -> String {
        format!("{JOB_PREFIX}{}", slug(&self.name))
    }
}

/// The command for `ExecStart=`: systemd expands `%` specifiers and `$VAR`,
/// so both are doubled; the shell gets the command as one quoted argument.
fn exec_start(command: &str) -> String {
    let esc = command.replace('\\', "\\\\").replace('"', "\\\"").replace('%', "%%").replace('$', "$$");
    format!("/bin/sh -c \"{esc}\"")
}

/// The `.service` and `.timer` files for a normalized job.
pub fn unit_files(job: &TimerJob) -> Result<(String, String)> {
    let header = format!("# nightgrid-job {}", serde_json::to_string(job)?);
    let user = job.run_as.as_deref().map(|u| format!("User={u}\n")).unwrap_or_default();
    let service = format!(
        "{header}\n[Unit]\nDescription=Nightgrid job: {}\n\n[Service]\nType=oneshot\n{user}ExecStart={}\n",
        job.name,
        exec_start(&job.command)
    );
    let timer = format!(
        "{header}\n[Unit]\nDescription=Nightgrid job: {}\n\n[Timer]\nOnCalendar={}\nPersistent={}\n\n[Install]\nWantedBy=timers.target\n",
        job.name, job.calendar, job.persistent
    );
    Ok((service, timer))
}

/// Root script that checks the calendar with systemd, writes the units and
/// starts the timer. `replaces` is the unit being edited (removed first when
/// the name changed). Prints `installed`.
pub fn install_command(job: &TimerJob, replaces: Option<&str>) -> Result<String> {
    let job = job.normalized()?;
    let unit = job.unit();
    let (service, timer) = unit_files(&job)?;
    let b64 = |s: &str| base64::engine::general_purpose::STANDARD.encode(s);
    let mut pre = String::new();
    if let Some(old) = replaces.filter(|o| *o != unit) {
        if !valid_job_unit(old) {
            return Err(invalid("unit", "not a Nightgrid job"));
        }
        pre = format!(
            "systemctl disable --now {old}.timer >/dev/null 2>&1; rm -f {DIR}/{old}.timer {DIR}/{old}.service; "
        );
    }
    Ok(format!(
        "set -e; command -v systemctl >/dev/null 2>&1 || {{ echo 'timers need systemd on this host' >&2; exit 3; }}; \
if command -v systemd-analyze >/dev/null 2>&1; then systemd-analyze calendar -- {cal} >/dev/null 2>&1 || {{ echo 'systemd does not understand the schedule {calq}' >&2; exit 4; }}; fi; \
{pre}printf '%s' '{s}' | base64 -d > {DIR}/{unit}.service; \
printf '%s' '{t}' | base64 -d > {DIR}/{unit}.timer; \
systemctl daemon-reload; systemctl enable --now {unit}.timer >/dev/null 2>&1; systemctl restart {unit}.timer; echo installed",
        cal = shell_quote(&job.calendar),
        calq = job.calendar.replace('\'', ""),
        s = b64(&service),
        t = b64(&timer),
    ))
}

/// Root script that stops and deletes a Nightgrid job. Prints `removed`.
pub fn remove_command(unit: &str) -> Result<String> {
    if !valid_job_unit(unit) {
        return Err(invalid("unit", "not a Nightgrid job"));
    }
    Ok(format!(
        "systemctl disable --now {unit}.timer >/dev/null 2>&1; rm -f {DIR}/{unit}.timer {DIR}/{unit}.service; systemctl daemon-reload; echo removed"
    ))
}

/// Lists Nightgrid jobs (no root needed).
pub const LIST_COMMAND: &str = "for f in /etc/systemd/system/nightgrid-job-*.timer; do [ -f \"$f\" ] || continue; \
echo \"@@ $(basename \"$f\" .timer)\"; head -n 1 \"$f\"; done; true";

pub fn parse_list(out: &str) -> Vec<InstalledJob> {
    let mut res = Vec::new();
    let mut unit: Option<String> = None;
    for l in out.lines() {
        if let Some(u) = l.strip_prefix("@@ ") {
            unit = Some(u.trim().to_owned());
        } else if let (Some(u), Some(json)) = (unit.take(), l.strip_prefix("# nightgrid-job ")) {
            if let Ok(job) = serde_json::from_str::<TimerJob>(json) {
                res.push(InstalledJob { unit: u, job });
            }
        }
    }
    res
}

/// What systemd makes of a calendar expression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CalendarPreview {
    pub ok: bool,
    /// The normalized form, or systemd's complaint.
    pub normalized: String,
    /// The next runs as systemd prints them (host time).
    pub next: Vec<String>,
}

pub fn preview_command(calendar: &str) -> Result<String> {
    let c = calendar.split_whitespace().collect::<Vec<_>>().join(" ");
    if !valid_calendar(&c) {
        return Err(invalid("calendar", "letters, digits, spaces and * : , . / ~ - + only"));
    }
    let q = shell_quote(&c);
    Ok(format!(
        "command -v systemd-analyze >/dev/null 2>&1 || {{ echo 'systemd-analyze is not on this host'; exit 0; }}; \
LC_ALL=C systemd-analyze calendar --iterations=3 -- {q} 2>&1 || LC_ALL=C systemd-analyze calendar -- {q} 2>&1; true"
    ))
}

pub fn parse_preview(out: &str) -> CalendarPreview {
    let mut normalized = None;
    let mut next = Vec::new();
    for l in out.lines() {
        let t = l.trim();
        if let Some(v) = t.strip_prefix("Normalized form:") {
            normalized = Some(v.trim().to_owned());
        } else if let Some(v) = t
            .strip_prefix("Next elapse:")
            .or_else(|| t.strip_prefix("Iter. #").and_then(|r| r.split_once(':').map(|(_, v)| v)))
        {
            if next.len() < 3 {
                next.push(v.trim().to_owned());
            }
        }
    }
    match normalized {
        Some(n) => CalendarPreview { ok: true, normalized: n, next },
        None => {
            let why = out.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("systemd didn't answer").to_owned();
            CalendarPreview { ok: false, normalized: why, next: vec![] }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAB: &str = "MAILTO=me@example.com\n# nightly\n0 3 * * * /usr/local/bin/backup.sh  --quiet\n#nightgrid-off */5 * * * * curl -fsS https://example.com/ping\n@reboot sleep 30 && mount -a\n";

    #[test]
    fn parses_a_crontab() {
        let c = parse_crontab(None, TAB, 0, 0);
        assert_eq!(c.env, vec!["MAILTO=me@example.com"]);
        assert_eq!(c.jobs.len(), 3);
        assert_eq!(c.jobs[0].line, 3);
        assert_eq!(c.jobs[0].command, "/usr/local/bin/backup.sh  --quiet");
        assert_eq!(c.jobs[0].every, "daily at 03:00");
        assert_eq!(c.jobs[0].next, Some(3 * 3600));
        assert!(!c.jobs[1].enabled && c.jobs[1].next.is_none());
        assert_eq!(c.jobs[2].schedule, "@reboot");
        assert_eq!(c.hash, hash(TAB));
    }

    #[test]
    fn applies_edits() {
        let t = apply(TAB, &CronEdit::SetEnabled { line: 4, enabled: true }).unwrap();
        assert!(t.contains("\n*/5 * * * * curl"));
        let t = apply(&t, &CronEdit::SetEnabled { line: 3, enabled: false }).unwrap();
        assert!(t.contains("#nightgrid-off 0 3 * * *"));
        let t = apply(&t, &CronEdit::Update { line: 3, schedule: "30  2 * * *".into(), command: "backup.sh".into() })
            .unwrap();
        assert!(t.contains("#nightgrid-off 30 2 * * * backup.sh\n"), "{t}");
        let t = apply(&t, &CronEdit::Remove { line: 5 }).unwrap();
        assert!(!t.contains("@reboot"));
        let t =
            apply(&t, &CronEdit::Add { schedule: "@daily".into(), command: "date +\\%F >> /tmp/d".into() }).unwrap();
        assert!(t.ends_with("@daily date +\\%F >> /tmp/d\n"));
        // Comments and env lines aren't jobs.
        assert!(apply(TAB, &CronEdit::Remove { line: 2 }).is_err());
        assert!(apply(TAB, &CronEdit::Remove { line: 1 }).is_err());
        assert!(apply(TAB, &CronEdit::Remove { line: 99 }).is_err());
        assert_eq!(
            apply("", &CronEdit::Add { schedule: "* * * * *".into(), command: "true".into() }).unwrap(),
            "* * * * * true\n"
        );
    }

    #[test]
    fn checks_jobs() {
        assert!(check_job("61 * * * *", "x").is_err());
        assert!(check_job("@sometimes", "x").is_err());
        assert!(check_job("* * * * *", "").is_err());
        assert!(check_job("* * * * *", "a\nb").is_err());
        assert!(check_job("* * * * *", "date +%F").is_err());
        assert!(check_job("@reboot", "date +\\%F").is_ok());
    }

    #[test]
    fn crontab_commands() {
        assert!(read_command(Some("root; rm")).is_err());
        assert!(read_command(Some("backup-user")).unwrap().contains("crontab -u backup-user -l"));
        let (now, off, text) = parse_read("1700000000\n+0130\n@@crontab\n* * * * * x\n").unwrap();
        assert_eq!((now, off, text.as_str()), (1_700_000_000, 5400, "* * * * * x\n"));
        let w = write_command(None, "* * * * * echo 'hi'\n", 5).unwrap();
        assert!(w.contains("me-5") && w.contains("| crontab - && echo written"));
        assert!(!w.contains("echo 'hi'"), "text only travels base64-encoded");
    }

    fn job() -> TimerJob {
        TimerJob {
            name: "Nightly sync!".into(),
            command: "rsync -a /srv/ nas:/backup/ && echo \"done $HOME 100%\"".into(),
            calendar: " *-*-*  03:00 ".into(),
            run_as: Some("root".into()),
            persistent: true,
        }
    }

    #[test]
    fn timer_units() {
        let j = job().normalized().unwrap();
        assert_eq!(j.unit(), "nightgrid-job-nightly-sync");
        assert_eq!(j.calendar, "*-*-* 03:00");
        assert_eq!(j.run_as, None);
        let (svc, tmr) = unit_files(&j).unwrap();
        assert!(
            svc.contains(r#"ExecStart=/bin/sh -c "rsync -a /srv/ nas:/backup/ && echo \"done $$HOME 100%%\"""#),
            "{svc}"
        );
        assert!(!svc.contains("User="));
        assert!(tmr.contains("OnCalendar=*-*-* 03:00\nPersistent=true"));
        let list = format!("@@ {}\n{}\n", j.unit(), svc.lines().next().unwrap());
        assert_eq!(parse_list(&list), vec![InstalledJob { unit: j.unit(), job: j.clone() }]);
        let cmd = install_command(&job(), Some("nightgrid-job-old")).unwrap();
        assert!(cmd.contains("rm -f /etc/systemd/system/nightgrid-job-old.timer"));
        assert!(cmd.contains("systemd-analyze calendar -- '*-*-* 03:00'"));
        assert!(install_command(&job(), Some("sshd")).is_err());
        assert!(TimerJob { calendar: "daily; reboot".into(), ..job() }.normalized().is_err());
        assert!(TimerJob { run_as: Some("Bad User".into()), ..job() }.normalized().is_err());
        assert!(remove_command("ssh").is_err());
        assert!(remove_command("nightgrid-job-nightly-sync").is_ok());
    }

    #[test]
    fn previews() {
        let ok = "  Original form: daily\nNormalized form: *-*-* 00:00:00\n    Next elapse: Tue 2026-11-17 00:00:00 UTC\n       (in UTC): Tue 2026-11-17 00:00:00 UTC\n       From now: 18h left\n       Iter. #2: Wed 2026-11-18 00:00:00 UTC\n";
        let p = parse_preview(ok);
        assert!(p.ok);
        assert_eq!(p.normalized, "*-*-* 00:00:00");
        assert_eq!(p.next, vec!["Tue 2026-11-17 00:00:00 UTC", "Wed 2026-11-18 00:00:00 UTC"]);
        let bad = parse_preview("Failed to parse calendar specification 'nope': Invalid argument\n");
        assert!(!bad.ok && bad.normalized.starts_with("Failed to parse"));
        assert!(preview_command("x; y").is_err());
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("  Hello, World  "), "hello-world");
        assert_eq!(slug("!!!"), "");
        assert!(valid_job_unit("nightgrid-job-a-1"));
        assert!(!valid_job_unit("nightgrid-job-"));
        assert!(!valid_job_unit("nightgrid-job-A"));
    }
}
