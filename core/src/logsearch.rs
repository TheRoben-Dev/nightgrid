//! Cross-host log search (M10u): one query over the journal and container
//! logs of several hosts, merged newest first, and saved searches that can
//! raise an alert when they match.
//!
//! Matching happens here (plain text or a regular expression, compiled with
//! a size limit), so the result is the same for journal and container lines.
//! For plain, case-insensitive searches the journal is also narrowed on the
//! host with `grep -F` to keep the transfer small. Lines that look like they
//! carry a secret are replaced by a notice (rule 1).

use regex::{Regex, RegexBuilder};
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::files::path::shell_quote;
use crate::hosts::HostId;
use crate::redact::line_has_secret;
use crate::store::Store;
use crate::system::systemd::valid_unit;
use crate::{Error, Result};

fn invalid(field: &'static str, reason: &str) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

/// Most hits one search returns.
pub const MAX_HITS: usize = 2000;
/// Journal lines read per host before matching.
pub const JOURNAL_READ: u32 = 20_000;
/// Container log lines read per container before matching.
pub const CONTAINER_READ: u32 = 5_000;
/// Longest look-back.
pub const MAX_SINCE_MINS: u32 = 7 * 24 * 60;
pub const MAX_SAVED: usize = 50;
pub const HIDDEN: &str = "[line hidden: it looks like it holds a password or token]";
const LINE_MAX: usize = 2000;

pub type LogSearchId = i32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct LogSearchQuery {
    /// Text to find; empty matches every line (useful with a unit filter).
    pub text: String,
    pub regex: bool,
    pub case_sensitive: bool,
    /// Hosts to search; empty means every host.
    pub hosts: Vec<HostId>,
    pub journal: bool,
    pub containers: bool,
    /// Only this systemd unit (journal).
    pub unit: Option<String>,
    /// Only containers whose name contains this.
    pub container: Option<String>,
    /// Most verbose journal priority, 0 (emerg) to 7 (debug).
    pub priority: Option<u8>,
    /// How far back, in minutes.
    pub since_mins: u32,
}

impl Default for LogSearchQuery {
    fn default() -> Self {
        Self {
            text: String::new(),
            regex: false,
            case_sensitive: false,
            hosts: vec![],
            journal: true,
            containers: true,
            unit: None,
            container: None,
            priority: None,
            since_mins: 60,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LogHit {
    pub host_id: HostId,
    /// `journal/sshd` or `container/web`.
    pub source: String,
    /// Unix seconds.
    pub ts: f64,
    pub text: String,
}

/// A compiled query.
#[derive(Debug, Clone)]
pub enum Matcher {
    All,
    Plain { needle: String, case_sensitive: bool },
    Regex(Regex),
}

impl Matcher {
    pub fn new(q: &LogSearchQuery) -> Result<Self> {
        let t = q.text.trim();
        if t.is_empty() {
            return Ok(Self::All);
        }
        if t.len() > 500 {
            return Err(invalid("text", "at most 500 characters"));
        }
        if q.regex {
            let re = RegexBuilder::new(t)
                .case_insensitive(!q.case_sensitive)
                .size_limit(1 << 20)
                .dfa_size_limit(1 << 20)
                .build()
                .map_err(|e| {
                    invalid("text", &format!("not a valid regular expression: {}", first_line(&e.to_string())))
                })?;
            return Ok(Self::Regex(re));
        }
        Ok(Self::Plain {
            needle: if q.case_sensitive { t.to_owned() } else { t.to_lowercase() },
            case_sensitive: q.case_sensitive,
        })
    }

    pub fn is_match(&self, line: &str) -> bool {
        match self {
            Self::All => true,
            Self::Plain { needle, case_sensitive: true } => line.contains(needle.as_str()),
            Self::Plain { needle, case_sensitive: false } => line.to_lowercase().contains(needle.as_str()),
            Self::Regex(re) => re.is_match(line),
        }
    }

    /// Byte ranges of the matches in `line`, for highlighting.
    pub fn ranges(&self, line: &str) -> Vec<(usize, usize)> {
        match self {
            Self::All => vec![],
            Self::Regex(re) => {
                re.find_iter(line).filter(|m| !m.is_empty()).map(|m| (m.start(), m.end())).take(20).collect()
            }
            Self::Plain { needle, case_sensitive } => {
                let hay = if *case_sensitive { line.to_owned() } else { line.to_lowercase() };
                // Lowercasing can change byte lengths outside ASCII; only highlight when it didn't.
                if hay.len() != line.len() || needle.is_empty() {
                    return vec![];
                }
                hay.match_indices(needle.as_str()).map(|(i, m)| (i, i + m.len())).take(20).collect()
            }
        }
    }
}

fn first_line(s: &str) -> &str {
    s.lines().rfind(|l| !l.trim().is_empty()).unwrap_or(s).trim()
}

/// Checks a query before it runs anywhere.
pub fn validate(q: &LogSearchQuery) -> Result<()> {
    if !q.journal && !q.containers {
        return Err(invalid("sources", "pick the journal, containers or both"));
    }
    if let Some(u) = q.unit.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
        if !valid_unit(u) {
            return Err(invalid("unit", "not a unit name"));
        }
    }
    if q.priority.is_some_and(|p| p > 7) {
        return Err(invalid("priority", "0 to 7"));
    }
    if q.since_mins == 0 || q.since_mins > MAX_SINCE_MINS {
        return Err(invalid("sinceMins", "between 1 minute and 7 days"));
    }
    Matcher::new(q).map(|_| ())
}

/// The journal command for one host: lines since `since` (unix seconds) as
/// `short-unix`, narrowed with `grep -F` for plain case-insensitive text.
pub fn journal_command(q: &LogSearchQuery, since: i64) -> String {
    let mut cmd = format!("journalctl --since @{since} -o short-unix --no-pager -q");
    if let Some(u) = q.unit.as_deref().map(str::trim).filter(|u| !u.is_empty() && valid_unit(u)) {
        cmd.push_str(&format!(" -u {}", shell_quote(u)));
    }
    if let Some(p) = q.priority.filter(|p| *p <= 7) {
        cmd.push_str(&format!(" -p 0..{p}"));
    }
    cmd.push_str(" 2>/dev/null");
    let t = q.text.trim();
    if !q.regex && !q.case_sensitive && !t.is_empty() && t.is_ascii() {
        cmd.push_str(&format!(" | grep -i -F -e {}", shell_quote(t)));
    }
    cmd.push_str(&format!(" | tail -n {JOURNAL_READ}"));
    cmd
}

/// `1731664800.123456 host sshd[123]: message` → (ts, identifier, message).
pub fn parse_journal_line(l: &str) -> Option<(f64, String, String)> {
    let mut it = l.splitn(3, ' ');
    let ts = it.next()?.parse::<f64>().ok()?;
    let _host = it.next()?;
    let rest = it.next()?;
    let (ident, msg) = rest.split_once(": ").unwrap_or(("journal", rest));
    let ident = ident.split('[').next().unwrap_or(ident);
    Some((ts, ident.to_owned(), msg.to_owned()))
}

/// Clips a line and hides it if it looks like it carries a secret.
pub fn tidy(text: &str) -> String {
    if line_has_secret(text) {
        return HIDDEN.to_owned();
    }
    let t = text.trim_end();
    if t.len() <= LINE_MAX {
        return t.to_owned();
    }
    let mut end = LINE_MAX;
    while !t.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &t[..end])
}

/// Matching journal hits from one host's command output.
pub fn journal_hits(host_id: HostId, out: &str, m: &Matcher) -> Vec<LogHit> {
    out.lines()
        .filter_map(parse_journal_line)
        .filter(|(_, _, msg)| m.is_match(msg))
        .map(|(ts, ident, msg)| LogHit { host_id, source: format!("journal/{ident}"), ts, text: tidy(&msg) })
        .collect()
}

/// A container log line with its RFC 3339 timestamp, if it matches.
pub fn container_hit(host_id: HostId, name: &str, ts: Option<&str>, text: &str, m: &Matcher) -> Option<LogHit> {
    if !m.is_match(text) {
        return None;
    }
    let ts = ts.and_then(crate::timeline::iso_offset).unwrap_or(0) as f64;
    Some(LogHit { host_id, source: format!("container/{name}"), ts, text: tidy(text) })
}

/// Newest first, at most [`MAX_HITS`]; `true` if some were cut.
pub fn merge(mut hits: Vec<LogHit>) -> (Vec<LogHit>, bool) {
    hits.sort_by(|a, b| b.ts.total_cmp(&a.ts));
    let cut = hits.len() > MAX_HITS;
    hits.truncate(MAX_HITS);
    (hits, cut)
}

// ---- saved searches ----

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SavedSearch {
    pub id: LogSearchId,
    pub name: String,
    pub query: LogSearchQuery,
    /// Raise an alert when new lines match (checked every few minutes on
    /// connected hosts).
    pub alert: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SavedSearchInput {
    pub id: Option<LogSearchId>,
    pub name: String,
    pub query: LogSearchQuery,
    pub alert: bool,
}

fn saved_row(r: &Row<'_>) -> rusqlite::Result<SavedSearch> {
    let q: String = r.get(2)?;
    Ok(SavedSearch {
        id: r.get(0)?,
        name: r.get(1)?,
        query: serde_json::from_str(&q).unwrap_or_default(),
        alert: r.get(3)?,
    })
}

impl Store {
    pub fn log_searches(&self) -> Result<Vec<SavedSearch>> {
        let mut st =
            self.conn().prepare("SELECT id, name, query, alert FROM log_searches ORDER BY name COLLATE NOCASE")?;
        let rows = st.query_map([], saved_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn log_search_save(&self, i: &SavedSearchInput) -> Result<SavedSearch> {
        let name = i.name.trim();
        if name.is_empty() || name.chars().count() > 60 {
            return Err(invalid("name", "must be 1-60 characters"));
        }
        validate(&i.query)?;
        if i.alert && i.query.text.trim().is_empty() {
            return Err(invalid("alert", "an alert needs text to look for"));
        }
        let q = serde_json::to_string(&i.query).map_err(|e| invalid("query", &e.to_string()))?;
        let c = self.conn();
        let id = match i.id {
            Some(id) => {
                let n = c.execute(
                    "UPDATE log_searches SET name = ?2, query = ?3, alert = ?4 WHERE id = ?1",
                    params![id, name, q, i.alert],
                )?;
                if n == 0 {
                    return Err(invalid("id", "that saved search is gone"));
                }
                id
            }
            None => {
                let count: i64 = c.query_row("SELECT COUNT(*) FROM log_searches", [], |r| r.get(0))?;
                if count as usize >= MAX_SAVED {
                    return Err(invalid("name", &format!("at most {MAX_SAVED} saved searches")));
                }
                c.execute(
                    "INSERT INTO log_searches (name, query, alert) VALUES (?1, ?2, ?3)",
                    params![name, q, i.alert],
                )?;
                c.last_insert_rowid() as LogSearchId
            }
        };
        c.query_row("SELECT id, name, query, alert FROM log_searches WHERE id = ?1", [id], saved_row)
            .optional()?
            .ok_or_else(|| invalid("id", "that saved search is gone"))
    }

    pub fn log_search_delete(&self, id: LogSearchId) -> Result<()> {
        self.conn().execute("DELETE FROM log_searches WHERE id = ?1", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(text: &str) -> LogSearchQuery {
        LogSearchQuery { text: text.into(), ..Default::default() }
    }

    #[test]
    fn matches_plain_and_regex() {
        let m = Matcher::new(&q("Timeout")).unwrap();
        assert!(m.is_match("upstream timeout after 30s"));
        assert_eq!(m.ranges("a TIMEOUT b timeout"), vec![(2, 9), (12, 19)]);
        let cs = Matcher::new(&LogSearchQuery { case_sensitive: true, ..q("Timeout") }).unwrap();
        assert!(!cs.is_match("timeout"));
        let re = Matcher::new(&LogSearchQuery { regex: true, ..q(r"status=(5\d\d)") }).unwrap();
        assert!(re.is_match("GET / status=502"));
        assert!(!re.is_match("GET / status=200"));
        assert_eq!(re.ranges("x status=503"), vec![(2, 12)]);
        assert!(Matcher::new(&LogSearchQuery { regex: true, ..q("(unclosed") }).is_err());
        assert!(Matcher::new(&q("")).unwrap().is_match("anything"));
    }

    #[test]
    fn validates() {
        assert!(validate(&q("x")).is_ok());
        assert!(validate(&LogSearchQuery { journal: false, containers: false, ..q("x") }).is_err());
        assert!(validate(&LogSearchQuery { unit: Some("ssh; rm -rf /".into()), ..q("x") }).is_err());
        assert!(validate(&LogSearchQuery { since_mins: 0, ..q("x") }).is_err());
        assert!(validate(&LogSearchQuery { priority: Some(9), ..q("x") }).is_err());
    }

    #[test]
    fn builds_journal_commands() {
        let c = journal_command(
            &LogSearchQuery { unit: Some("nginx.service".into()), priority: Some(4), ..q("it's down") },
            100,
        );
        assert_eq!(
            c,
            "journalctl --since @100 -o short-unix --no-pager -q -u 'nginx.service' -p 0..4 2>/dev/null | grep -i -F -e 'it'\\''s down' | tail -n 20000"
        );
        // Regex and case-sensitive searches match in the app only.
        assert!(!journal_command(&LogSearchQuery { regex: true, ..q("a.b") }, 0).contains("grep"));
        assert!(!journal_command(&q(""), 0).contains("grep"));
    }

    #[test]
    fn parses_hits_and_hides_secrets() {
        let out = "1731664800.5 host sshd[123]: Failed password for root\n\
                   1731664801.0 host app[9]: password=hunter2 timeout\n\
                   garbage\n\
                   1731664802.0 host kernel: eth0 link timeout\n";
        let m = Matcher::new(&q("timeout")).unwrap();
        let hits = journal_hits(3, out, &m);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].text, HIDDEN);
        assert_eq!((hits[1].source.as_str(), hits[1].ts), ("journal/kernel", 1_731_664_802.0));
        let c = container_hit(3, "web", Some("2026-11-16T05:00:00.123456789Z"), "GET /x timeout", &m).unwrap();
        assert_eq!(c.source, "container/web");
        assert_eq!(c.ts, 1_794_805_200.0);
        assert!(container_hit(3, "web", None, "fine", &m).is_none());
        let (merged, cut) = merge(vec![hits[1].clone(), c.clone()]);
        assert_eq!(merged[0], c);
        assert!(!cut);
        assert!(tidy(&"x".repeat(3000)).ends_with('…'));
    }

    #[test]
    fn saves_searches() {
        let s = Store::open_in_memory().unwrap();
        let mut i = SavedSearchInput { id: None, name: "502s".into(), query: q("status=502"), alert: true };
        let a = s.log_search_save(&i).unwrap();
        assert_eq!(a.query.text, "status=502");
        i.id = Some(a.id);
        i.name = "Bad gateway".into();
        assert_eq!(s.log_search_save(&i).unwrap().name, "Bad gateway");
        assert!(s
            .log_search_save(&SavedSearchInput { id: None, name: "all".into(), query: q(""), alert: true })
            .is_err());
        assert_eq!(s.log_searches().unwrap().len(), 1);
        s.log_search_delete(a.id).unwrap();
        assert!(s.log_searches().unwrap().is_empty());
    }
}
