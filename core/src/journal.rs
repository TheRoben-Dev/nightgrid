//! The homelab journal: a searchable changelog of everything changed from
//! Nightgrid, across all hosts, plus entries the user writes by hand.
//!
//! The per-host timeline (M10l) answers "what happened on this host"; the
//! journal answers "what did I change, where, and when", fleet-wide and for
//! two years, with a line diff for text edits (crontabs, Caddyfiles, files
//! saved in the editor). DRIFT reads it to answer "what changed before this
//! broke?".
//!
//! Nothing that looks like a secret is stored: titles and details go through
//! the same check as the timeline, and diff lines that look like they carry
//! one are replaced (rule 1). `.env` vault changes are recorded by name only.

use rusqlite::{params, params_from_iter, types::Value};
use serde::{Deserialize, Serialize};

use crate::confdiff::{self, Tag};
use crate::hosts::HostId;
use crate::store::Store;
use crate::Result;

/// Entries older than this are dropped.
pub const KEEP_SECS: i64 = 730 * 86_400;
/// At most this many entries are kept.
pub const KEEP_ROWS: i64 = 50_000;
/// At most this many entries come back from one query.
pub const LIST_MAX: u32 = 1_000;
/// A stored diff is cut at this many bytes.
pub const DIFF_MAX: usize = 32 * 1024;
/// Texts longer than this aren't diffed (only "changed" is recorded).
const DIFF_INPUT_MAX: usize = 512 * 1024;
const HIDDEN: &str = "(hidden: looked like it contained a secret)";

/// What part of the lab an entry is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ChangeArea {
    /// Compose runs, deploys, snapshots, rollbacks, stacks from git.
    Stack,
    /// Container start, stop, restart, remove.
    Container,
    /// Files saved in the editor.
    File,
    /// Config baselines and other host configuration.
    Config,
    Firewall,
    /// Crontabs and systemd timers.
    Jobs,
    /// Reverse-proxy routes (Caddy, Traefik).
    Proxy,
    /// `.env` vault values (names only).
    Secrets,
    /// Package upgrades and systemd unit actions.
    System,
    /// Reboot, shutdown, wake, sleep.
    Power,
    /// Proxmox guests, snapshots, backups, network and firewall.
    Proxmox,
    /// Snippet and playbook runs.
    Automation,
    /// SSH keys pushed or removed.
    Keys,
    /// Integrations added, changed or removed.
    Integrations,
    /// Blueprints exported or applied.
    Blueprint,
    /// Written by hand.
    Note,
}

impl ChangeArea {
    pub const ALL: [ChangeArea; 16] = [
        ChangeArea::Stack,
        ChangeArea::Container,
        ChangeArea::File,
        ChangeArea::Config,
        ChangeArea::Firewall,
        ChangeArea::Jobs,
        ChangeArea::Proxy,
        ChangeArea::Secrets,
        ChangeArea::System,
        ChangeArea::Power,
        ChangeArea::Proxmox,
        ChangeArea::Automation,
        ChangeArea::Keys,
        ChangeArea::Integrations,
        ChangeArea::Blueprint,
        ChangeArea::Note,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ChangeArea::Stack => "stack",
            ChangeArea::Container => "container",
            ChangeArea::File => "file",
            ChangeArea::Config => "config",
            ChangeArea::Firewall => "firewall",
            ChangeArea::Jobs => "jobs",
            ChangeArea::Proxy => "proxy",
            ChangeArea::Secrets => "secrets",
            ChangeArea::System => "system",
            ChangeArea::Power => "power",
            ChangeArea::Proxmox => "proxmox",
            ChangeArea::Automation => "automation",
            ChangeArea::Keys => "keys",
            ChangeArea::Integrations => "integrations",
            ChangeArea::Blueprint => "blueprint",
            ChangeArea::Note => "note",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        ChangeArea::ALL.into_iter().find(|a| a.as_str() == s)
    }

    /// The word shown in lists and exports.
    pub fn label(self) -> &'static str {
        match self {
            ChangeArea::Stack => "Stack",
            ChangeArea::Container => "Container",
            ChangeArea::File => "File",
            ChangeArea::Config => "Config",
            ChangeArea::Firewall => "Firewall",
            ChangeArea::Jobs => "Cron & timers",
            ChangeArea::Proxy => "Proxy",
            ChangeArea::Secrets => ".env vault",
            ChangeArea::System => "System",
            ChangeArea::Power => "Power",
            ChangeArea::Proxmox => "Proxmox",
            ChangeArea::Automation => "Automation",
            ChangeArea::Keys => "SSH keys",
            ChangeArea::Integrations => "Integrations",
            ChangeArea::Blueprint => "Blueprint",
            ChangeArea::Note => "Note",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ChangeEntry {
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub id: i64,
    /// Unix seconds.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub at: i64,
    pub host_id: Option<HostId>,
    /// The host's name when the entry was written ("" for none).
    pub host: String,
    pub area: ChangeArea,
    pub title: String,
    pub detail: String,
    /// A unified line diff (`+`/`-`/` ` prefixes, `@@ n unchanged` gaps).
    pub diff: Option<String>,
    /// False when the change failed.
    pub ok: bool,
    /// Written by hand (can be edited and deleted; recorded ones can't).
    pub manual: bool,
}

/// A change to record.
#[derive(Debug, Clone, Default)]
pub struct NewEntry {
    pub at: i64,
    pub host_id: Option<HostId>,
    pub host: String,
    pub area: Option<ChangeArea>,
    pub title: String,
    pub detail: String,
    pub diff: Option<String>,
    pub ok: bool,
    pub manual: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct ChangeQuery {
    /// Words that must all appear in the title, detail, host or diff.
    pub text: String,
    pub host_id: Option<HostId>,
    /// Empty: every area.
    pub areas: Vec<ChangeArea>,
    /// Unix seconds, inclusive.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub since: Option<i64>,
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub until: Option<i64>,
    /// Only failed changes.
    pub failed_only: bool,
    /// 0: [`LIST_MAX`].
    pub limit: u32,
}

/// Trims, cuts at `max` characters and hides a secret-looking line.
pub fn tidy(s: &str, max: usize) -> String {
    let s = s.trim();
    if crate::redact::line_has_secret(s) {
        return HIDDEN.into();
    }
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut t: String = s.chars().take(max).collect();
    t.push('…');
    t
}

/// A compact unified diff of `old` → `new` for the journal, with secret-
/// looking lines hidden. `None` when nothing changed. Too-large inputs get
/// a one-line summary instead of a diff.
pub fn text_diff(old: &str, new: &str) -> Option<String> {
    if old == new {
        return None;
    }
    if old.len() > DIFF_INPUT_MAX || new.len() > DIFF_INPUT_MAX {
        return Some(format!("@@ too large to diff ({} → {} bytes)", old.len(), new.len()));
    }
    let d = confdiff::line_diff("", old.as_bytes(), new.as_bytes(), 2);
    if d.binary {
        return Some("@@ binary content changed".into());
    }
    if d.too_large {
        return Some(format!("@@ too different to diff (+{} −{})", d.added, d.removed));
    }
    let mut out = String::new();
    for l in &d.lines {
        let (mark, text) = match l.tag {
            Tag::Add => ('+', l.text.as_str()),
            Tag::Del => ('-', l.text.as_str()),
            Tag::Same => (' ', l.text.as_str()),
            Tag::Skip => {
                out.push_str(&format!("@@ {} unchanged\n", l.text));
                continue;
            }
        };
        let text = if crate::redact::line_has_secret(text) { HIDDEN } else { text };
        let line = format!("{mark}{text}\n");
        if out.len() + line.len() > DIFF_MAX {
            out.push_str("@@ diff cut here\n");
            break;
        }
        out.push_str(&line);
    }
    Some(out.trim_end_matches('\n').to_owned())
}

/// `(added, removed)` lines in a stored diff.
pub fn diff_counts(diff: &str) -> (u32, u32) {
    diff.lines().fold((0, 0), |(a, r), l| match l.as_bytes().first() {
        Some(b'+') => (a + 1, r),
        Some(b'-') => (a, r + 1),
        _ => (a, r),
    })
}

fn when(at: i64) -> String {
    // YYYY-MM-DD HH:MM UTC from Unix seconds, without a date crate.
    let days = at.div_euclid(86_400);
    let secs = at.rem_euclid(86_400);
    let (y, m, d) = civil(days);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", secs / 3600, (secs % 3600) / 60)
}

/// Days since 1970-01-01 → (year, month, day) (Howard Hinnant's algorithm).
fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// The entries as a Markdown changelog (newest first, grouped by day, UTC).
pub fn to_markdown(entries: &[ChangeEntry]) -> String {
    let mut out = String::from("# Nightgrid journal\n");
    let mut day = String::new();
    for e in entries {
        let w = when(e.at);
        if w[..10] != day {
            day = w[..10].to_owned();
            out.push_str(&format!("\n## {day}\n\n"));
        }
        let host = if e.host.is_empty() { String::new() } else { format!(" · {}", e.host) };
        let failed = if e.ok { "" } else { " · **failed**" };
        out.push_str(&format!("- **{}** {} · {}{host}{failed}", &w[11..], e.area.label(), e.title));
        if !e.detail.is_empty() {
            out.push_str(&format!(" — {}", e.detail));
        }
        out.push('\n');
        if let Some(d) = &e.diff {
            out.push_str("\n  ```diff\n");
            for l in d.lines() {
                out.push_str("  ");
                out.push_str(l);
                out.push('\n');
            }
            out.push_str("  ```\n");
        }
    }
    out
}

/// A short plain-text list for DRIFT: what changed, newest first.
pub fn for_drift(entries: &[ChangeEntry], max: usize) -> String {
    entries
        .iter()
        .take(max)
        .map(|e| {
            let (a, r) = e.diff.as_deref().map(diff_counts).unwrap_or((0, 0));
            let lines = if a + r > 0 { format!(" (+{a} −{r} lines)") } else { String::new() };
            let host = if e.host.is_empty() { String::new() } else { format!(" on {}", e.host) };
            let failed = if e.ok { "" } else { " [FAILED]" };
            let detail = if e.detail.is_empty() { String::new() } else { format!(": {}", e.detail) };
            format!("- [{} UTC] {}{host}: {}{detail}{lines}{failed}", when(e.at), e.area.label(), e.title)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl Store {
    /// Records one entry and trims old ones. Returns its id.
    pub fn journal_add(&self, e: &NewEntry) -> Result<i64> {
        let c = self.conn();
        let area = e.area.unwrap_or(ChangeArea::Note);
        let diff = e.diff.as_deref().map(|d| d.chars().take(DIFF_MAX).collect::<String>());
        c.execute(
            "INSERT INTO journal (at, host_id, host, area, title, detail, diff, ok, manual)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                e.at,
                e.host_id,
                tidy(&e.host, 80),
                area.as_str(),
                tidy(&e.title, 200),
                if e.manual { e.detail.chars().take(4000).collect() } else { tidy(&e.detail, 600) },
                diff,
                e.ok,
                e.manual
            ],
        )?;
        let id = c.last_insert_rowid();
        c.execute(
            "DELETE FROM journal WHERE at < ?1 OR id NOT IN (SELECT id FROM journal ORDER BY at DESC, id DESC LIMIT ?2)",
            params![e.at - KEEP_SECS, KEEP_ROWS],
        )?;
        Ok(id)
    }

    /// Entries matching `q`, newest first.
    pub fn journal_query(&self, q: &ChangeQuery) -> Result<Vec<ChangeEntry>> {
        let mut sql =
            String::from("SELECT id, at, host_id, host, area, title, detail, diff, ok, manual FROM journal WHERE 1=1");
        let mut args: Vec<Value> = Vec::new();
        if let Some(h) = q.host_id {
            sql.push_str(" AND host_id = ?");
            args.push(Value::Integer(i64::from(h)));
        }
        if let Some(s) = q.since {
            sql.push_str(" AND at >= ?");
            args.push(Value::Integer(s));
        }
        if let Some(u) = q.until {
            sql.push_str(" AND at <= ?");
            args.push(Value::Integer(u));
        }
        if q.failed_only {
            sql.push_str(" AND ok = 0");
        }
        if !q.areas.is_empty() {
            sql.push_str(" AND area IN (");
            sql.push_str(&vec!["?"; q.areas.len()].join(","));
            sql.push(')');
            args.extend(q.areas.iter().map(|a| Value::Text(a.as_str().to_owned())));
        }
        for w in q.text.split_whitespace().take(8) {
            sql.push_str(
                " AND (instr(lower(title), ?) OR instr(lower(detail), ?) OR instr(lower(host), ?) OR instr(lower(coalesce(diff, '')), ?))",
            );
            let w = w.to_lowercase();
            for _ in 0..4 {
                args.push(Value::Text(w.clone()));
            }
        }
        let limit = if q.limit == 0 { LIST_MAX } else { q.limit.min(LIST_MAX) };
        sql.push_str(&format!(" ORDER BY at DESC, id DESC LIMIT {limit}"));
        let c = self.conn();
        let mut st = c.prepare(&sql)?;
        let rows = st.query_map(params_from_iter(args), |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<HostId>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, bool>(8)?,
                r.get::<_, bool>(9)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, at, host_id, host, area, title, detail, diff, ok, manual) = row?;
            let Some(area) = ChangeArea::parse(&area) else { continue };
            out.push(ChangeEntry { id, at, host_id, host, area, title, detail, diff, ok, manual });
        }
        Ok(out)
    }

    /// Changes a hand-written entry. Recorded entries can't be edited.
    pub fn journal_edit(&self, id: i64, title: &str, detail: &str) -> Result<bool> {
        let n = self.conn().execute(
            "UPDATE journal SET title = ?2, detail = ?3 WHERE id = ?1 AND manual = 1",
            params![id, tidy(title, 200), detail.chars().take(4000).collect::<String>()],
        )?;
        Ok(n > 0)
    }

    /// Deletes a hand-written entry. Recorded entries can't be deleted one
    /// by one (the whole journal can be cleared).
    pub fn journal_delete(&self, id: i64) -> Result<bool> {
        Ok(self.conn().execute("DELETE FROM journal WHERE id = ?1 AND manual = 1", [id])? > 0)
    }

    /// Forgets every entry.
    pub fn journal_clear(&self) -> Result<u32> {
        Ok(self.conn().execute("DELETE FROM journal", [])? as u32)
    }

    /// How many entries there are, and the oldest one's time.
    pub fn journal_stats(&self) -> Result<(u32, Option<i64>)> {
        Ok(self.conn().query_row("SELECT count(*), min(at) FROM journal", [], |r| Ok((r.get(0)?, r.get(1)?)))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(s: &Store) -> HostId {
        s.conn().execute("INSERT INTO hosts (name, hostname) VALUES ('server01', 'server01.example.com')", []).unwrap();
        s.conn().last_insert_rowid() as HostId
    }

    fn entry(at: i64, host_id: Option<HostId>, area: ChangeArea, title: &str) -> NewEntry {
        NewEntry {
            at,
            host_id,
            host: "server01".into(),
            area: Some(area),
            title: title.into(),
            ok: true,
            ..Default::default()
        }
    }

    #[test]
    fn records_and_queries() {
        let s = Store::open_in_memory().unwrap();
        let h = host(&s);
        s.journal_add(&entry(100, Some(h), ChangeArea::Stack, "Deployed media")).unwrap();
        s.journal_add(&entry(200, Some(h), ChangeArea::Firewall, "Allowed 443/tcp")).unwrap();
        let mut n = entry(300, None, ChangeArea::Note, "Swapped the UPS battery");
        n.manual = true;
        n.detail = "APC, 2 years old".into();
        let id = s.journal_add(&n).unwrap();

        let all = s.journal_query(&ChangeQuery::default()).unwrap();
        assert_eq!(all.iter().map(|e| e.at).collect::<Vec<_>>(), vec![300, 200, 100]);
        let fw = s.journal_query(&ChangeQuery { areas: vec![ChangeArea::Firewall], ..Default::default() }).unwrap();
        assert_eq!(fw.len(), 1);
        let words = s.journal_query(&ChangeQuery { text: "ups BATTERY".into(), ..Default::default() }).unwrap();
        assert_eq!(words.len(), 1);
        let range = s.journal_query(&ChangeQuery { since: Some(150), until: Some(250), ..Default::default() }).unwrap();
        assert_eq!(range[0].title, "Allowed 443/tcp");
        let host = s.journal_query(&ChangeQuery { host_id: Some(h), ..Default::default() }).unwrap();
        assert_eq!(host.len(), 2);

        // Only hand-written entries can be edited or deleted.
        assert!(s.journal_edit(id, "Swapped the UPS battery pack", "").unwrap());
        assert!(!s.journal_delete(all[1].id).unwrap());
        assert!(s.journal_delete(id).unwrap());
        assert_eq!(s.journal_stats().unwrap().0, 2);
        assert_eq!(s.journal_clear().unwrap(), 2);
    }

    #[test]
    fn hides_secrets_and_keeps_host_names() {
        let s = Store::open_in_memory().unwrap();
        let h = host(&s);
        let mut e = entry(10, Some(h), ChangeArea::File, "Saved /srv/app/.env");
        e.detail = "PASSWORD=hunter2".into();
        e.diff = text_diff("A=1\nTOKEN=abcdef123456\n", "A=2\nTOKEN=zyxw98765432\n");
        s.journal_add(&e).unwrap();
        s.conn().execute("DELETE FROM hosts WHERE id = ?1", [h]).unwrap();
        let got = &s.journal_query(&ChangeQuery::default()).unwrap()[0];
        assert_eq!(got.host_id, None);
        assert_eq!(got.host, "server01");
        assert!(got.detail.starts_with("(hidden"));
        let d = got.diff.as_deref().unwrap();
        assert!(!d.contains("abcdef") && !d.contains("zyxw"), "{d}");
        assert!(d.contains("-A=1") && d.contains("+A=2"));
    }

    #[test]
    fn trims_old_entries() {
        let s = Store::open_in_memory().unwrap();
        s.journal_add(&entry(1, None, ChangeArea::Note, "old")).unwrap();
        s.journal_add(&entry(1 + KEEP_SECS + 10, None, ChangeArea::Note, "new")).unwrap();
        let all = s.journal_query(&ChangeQuery::default()).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].title, "new");
    }

    #[test]
    fn diffs_text() {
        assert_eq!(text_diff("a\n", "a\n"), None);
        let old: String = (1..=20).map(|i| format!("line {i}\n")).collect();
        let new = old.replace("line 10\n", "line ten\n");
        let d = text_diff(&old, &new).unwrap();
        assert!(d.contains("-line 10\n+line ten"), "{d}");
        assert!(d.starts_with("@@ 7 unchanged"), "{d}");
        assert_eq!(diff_counts(&d), (1, 1));
        let big = "x\n".repeat(DIFF_INPUT_MAX);
        assert!(text_diff(&big, "y").unwrap().starts_with("@@ too large"));
    }

    #[test]
    fn exports_markdown_and_drift_text() {
        let e = ChangeEntry {
            id: 1,
            at: 1_800_000_000,
            host_id: Some(1),
            host: "server01".into(),
            area: ChangeArea::Jobs,
            title: "Edited root's crontab".into(),
            detail: "added a job".into(),
            diff: Some("+0 3 * * * backup".into()),
            ok: false,
            manual: false,
        };
        let md = to_markdown(std::slice::from_ref(&e));
        assert!(md.contains("## 2027-01-15"), "{md}");
        assert!(md.contains("Cron & timers · Edited root's crontab · server01 · **failed** — added a job"), "{md}");
        assert!(md.contains("```diff"));
        let t = for_drift(&[e], 5);
        assert_eq!(t, "- [2027-01-15 08:00 UTC] Cron & timers on server01: Edited root's crontab: added a job (+1 −0 lines) [FAILED]");
    }

    #[test]
    fn areas_round_trip() {
        for a in ChangeArea::ALL {
            assert_eq!(ChangeArea::parse(a.as_str()), Some(a));
        }
    }
}
