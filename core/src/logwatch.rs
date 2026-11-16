//! Log anomaly detection (M10g): error bursts and new error patterns in
//! container logs and the journal.
//!
//! Each round, the app hands the error lines of one source (a container, or a
//! journal identifier like `sshd`) since the last round to [`Detector`]. A
//! line becomes a *pattern* by replacing what changes between occurrences
//! (numbers, ids, addresses, quoted values), so "timeout after 3012 ms" and
//! "timeout after 87 ms" count as the same error. The first look at a source
//! only learns; after that a pattern never seen before is reported once, and a
//! round with several times the usual number of error lines is a burst.
//! Everything here is pure; the app crate fetches the lines and raises alerts.

use std::collections::{HashMap, HashSet, VecDeque};

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::redact::line_has_secret;

/// Fewest error events in one round that can count as a burst.
pub const BURST_MIN: usize = 20;
/// A burst is this many times the usual count per round.
pub const BURST_FACTOR: f64 = 4.0;
/// Fewest not-found (404) events in one round that can count as a spike.
pub const NOT_FOUND_MIN: usize = 40;
/// A 404 spike is this many times the source's usual count.
pub const NOT_FOUND_FACTOR: f64 = 5.0;
/// Rounds remembered for the usual count.
const HISTORY: usize = 12;
/// Patterns remembered per source.
const KNOWN_MAX: usize = 2000;
/// Sample lines kept per source for "what broke".
pub const SAMPLES: usize = 40;
pub const PATTERN_MAX: usize = 160;
/// A pattern first seen this long ago is no longer "new", however often it repeats.
pub const NEW_FOR: i64 = 24 * 3600;
/// Continuation lines kept per event.
const EVENT_LINES: usize = 24;
/// A continuation line must be within this many seconds of its event's first line.
const EVENT_GAP: i64 = 2;

/// A log line's severity, from its level field or, without one, its wording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

fn level_word(w: &str) -> Option<Level> {
    Some(match w.to_ascii_lowercase().as_str() {
        "trace" | "debug" | "dbg" | "verbose" => Level::Debug,
        "info" | "inf" | "notice" | "information" => Level::Info,
        "warn" | "warning" | "wrn" => Level::Warn,
        "error" | "err" | "eror" | "severe" => Level::Error,
        "fatal" | "crit" | "critical" | "panic" | "emerg" | "emergency" | "alert" => Level::Fatal,
        _ => return None,
    })
}

/// The level a line declares, if any: `level=warn`, `"level":"error"`, an
/// upper-case `WARN`/`ERROR` among the first words, `[warn]`, or a leading
/// `error:`. Words inside the message ("GlobalExceptionHandler") don't count.
pub fn declared_level(line: &str) -> Option<Level> {
    let body = strip_timestamp(line);
    let lower = body.to_ascii_lowercase();
    for key in ["level=", "lvl=", "\"level\":\"", "\"level\": \"", "severity=", "\"severity\":\""] {
        if let Some(i) = lower.find(key) {
            let rest = &lower[i + key.len()..];
            let w: String = rest.trim_start_matches('"').chars().take_while(char::is_ascii_alphabetic).collect();
            if let Some(l) = level_word(&w) {
                return Some(l);
            }
        }
    }
    for (n, tok) in body.split_whitespace().take(8).enumerate() {
        let bracketed = tok.starts_with('[') || tok.starts_with('<') || tok.starts_with('(');
        let w = tok.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        if w.is_empty() {
            continue;
        }
        let upper = w.chars().all(|c| c.is_ascii_uppercase());
        // "error:" / "warning:" opening the message (Node, Go, CLI tools).
        let leading = n == 0 && tok.ends_with(':');
        if upper || bracketed || leading {
            if let Some(l) = level_word(w) {
                return Some(l);
            }
        }
    }
    None
}

/// Wording that marks a line as a crash whatever its level says.
fn crash_words(l: &str) -> bool {
    [
        "panic:",
        "panicked at",
        "fatal error",
        "segmentation fault",
        "segfault",
        "out of memory",
        "oom-kill",
        "core dumped",
    ]
    .iter()
    .any(|w| l.contains(w))
}

/// Does this log line report an error? Lines with a level are judged by it
/// (WARN/INFO/DEBUG never are errors); lines without one by their wording.
pub fn is_error_line(line: &str) -> bool {
    line_level(line) >= Level::Error
}

/// The effective level: the declared one, raised to Fatal by crash wording,
/// or guessed from the wording when nothing is declared.
pub fn line_level(line: &str) -> Level {
    let l = line.to_ascii_lowercase();
    let crash = crash_words(&l);
    if let Some(lv) = declared_level(line) {
        return if crash && lv >= Level::Error { Level::Fatal } else { lv };
    }
    if crash {
        return Level::Fatal;
    }
    // Common "all good" lines that still contain the word.
    if l.contains("0 errors") || l.contains("no error") || l.contains("errors=0") || l.contains("error_log") {
        return Level::Info;
    }
    let errorish = [
        "error",
        "exception",
        "traceback",
        "uncaught",
        "unhandled",
        "[err]",
        "[crit]",
        "failed to",
        "permission denied",
        "connection refused",
    ]
    .iter()
    .any(|w| l.contains(w));
    if errorish {
        Level::Error
    } else {
        Level::Info
    }
}

/// One log line with the daemon's timestamp (unix seconds), when known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawLine {
    pub ts: Option<i64>,
    pub text: String,
}

impl RawLine {
    pub fn new(ts: Option<i64>, text: impl Into<String>) -> Self {
        Self { ts, text: text.into() }
    }
}

/// One thing that happened: a line plus its continuation (a stack trace's
/// `at …` lines, indented detail, `status:`/`code:` fields).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub ts: Option<i64>,
    pub level: Level,
    /// The first line.
    pub head: String,
    /// Every line, joined with newlines (capped).
    pub text: String,
    pub lines: usize,
    /// An HTTP 404 / NOT_FOUND: caused by a request, not a fault.
    pub not_found: bool,
}

/// Is this line part of the event above it rather than a new one?
fn is_continuation(line: &str, head_is_trace: bool) -> bool {
    if line.trim().is_empty() {
        return true;
    }
    if line.starts_with([' ', '\t']) {
        return true;
    }
    let t = line.trim_start();
    if ["at ", "... ", "Caused by:", "Suppressed:", "}", "]", ")", "File \"", "During handling", "The above exception"]
        .iter()
        .any(|p| t.starts_with(p))
    {
        return true;
    }
    // `status: 404,` / `code: "NOT_FOUND"`: a short lower-case field that
    // isn't a level word (`error:` starts a new event).
    if let Some((k, _)) = t.split_once(": ") {
        if !k.is_empty()
            && k.len() <= 24
            && k.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            && level_word(k).is_none()
            && t.len() <= 160
        {
            return true;
        }
        // Python's last traceback line: `ValueError: bad thing`.
        if head_is_trace && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_') {
            return true;
        }
    }
    false
}

fn not_found(text: &str) -> bool {
    let l = text.to_ascii_lowercase();
    if l.contains("not_found")
        || l.contains("notfounderror")
        || l.contains("no static resource")
        || l.contains("no resource at")
    {
        return true;
    }
    let has404 = l.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == "404");
    has404 && (l.contains("status") || l.contains("http") || l.contains("not found"))
}

/// Groups lines into events and classifies them. Journal lines are already
/// filtered by priority, so a journal line without a level counts as an error.
pub fn events(lines: &[RawLine], journal: bool) -> Vec<Event> {
    let mut out: Vec<Event> = Vec::new();
    let mut trace = false;
    for l in lines {
        let text = l.text.trim_end();
        let close = |e: &Event| match (e.ts, l.ts) {
            (Some(a), Some(b)) => (b - a).abs() <= EVENT_GAP,
            _ => true,
        };
        if let Some(e) = out.last_mut() {
            if close(e) && is_continuation(text, trace) {
                e.lines += 1;
                if e.lines <= EVENT_LINES && e.text.len() < 2000 {
                    e.text.push('\n');
                    e.text.push_str(text);
                }
                // A continuation can carry the crash ("fatal error: …" in a Go dump).
                if crash_words(&text.to_ascii_lowercase()) && e.level >= Level::Error {
                    e.level = Level::Fatal;
                }
                continue;
            }
        }
        if text.is_empty() {
            continue;
        }
        trace = text.to_ascii_lowercase().starts_with("traceback");
        let mut level = line_level(text);
        if journal && declared_level(text).is_none() && level < Level::Error {
            level = Level::Error;
        }
        out.push(Event { ts: l.ts, level, head: text.to_owned(), text: text.to_owned(), lines: 1, not_found: false });
    }
    for e in &mut out {
        e.not_found = not_found(&e.text);
    }
    out
}

/// A stable id for a pattern (stored instead of the pattern text, which
/// could hold something sensitive).
pub fn fingerprint(pattern: &str) -> String {
    let d = Sha256::digest(pattern.as_bytes());
    d.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// A user's "don't alert on this": a fingerprint ("Mute this pattern") or a
/// regex, for one source (`container/<name>`, `journal/<ident>`) or all ("").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct LogMute {
    pub source: String,
    /// Matched against every line of the event.
    pub regex: String,
    pub fingerprint: String,
    /// What it mutes, for the list (a pattern with values blanked, or the regex).
    pub label: String,
}

impl LogMute {
    /// Why it isn't usable, or None.
    pub fn problem(&self) -> Option<String> {
        if self.source.len() > 200 || self.label.len() > 200 || self.regex.len() > 300 {
            return Some("too long".into());
        }
        if self.regex.is_empty() && self.fingerprint.is_empty() {
            return Some("needs a regex or a fingerprint".into());
        }
        if !self.fingerprint.is_empty()
            && (self.fingerprint.len() != 16 || !self.fingerprint.chars().all(|c| c.is_ascii_hexdigit()))
        {
            return Some("fingerprint must be 16 hex digits".into());
        }
        if !self.regex.is_empty() {
            if let Err(e) = Regex::new(&self.regex) {
                return Some(format!("regex: {e}"));
            }
        }
        None
    }
}

/// How log alerts are decided (from the alert settings).
#[derive(Debug, Clone)]
pub struct Rules {
    /// A new pattern alerts once it has happened this many times within `repeat_secs`.
    pub repeat_count: usize,
    pub repeat_secs: i64,
    /// After the container (or the reverse proxy on its host) starts, log
    /// lines within this many seconds are notes, not alerts.
    pub grace_secs: i64,
    /// 404s / NOT_FOUND never alert on their own, only a spike does.
    pub not_found_noise: bool,
    mutes: Vec<(String, Option<Regex>, String)>,
}

impl Default for Rules {
    fn default() -> Self {
        Self { repeat_count: 3, repeat_secs: 600, grace_secs: 300, not_found_noise: true, mutes: Vec::new() }
    }
}

impl Rules {
    pub fn new(
        repeat_count: usize,
        repeat_secs: i64,
        grace_secs: i64,
        not_found_noise: bool,
        mutes: &[LogMute],
    ) -> Self {
        let mutes = mutes
            .iter()
            .filter(|m| m.problem().is_none())
            .map(|m| {
                let re = (!m.regex.is_empty()).then(|| Regex::new(&m.regex).ok()).flatten();
                (m.source.clone(), re, m.fingerprint.clone())
            })
            .collect();
        Self {
            repeat_count: repeat_count.max(1),
            repeat_secs: repeat_secs.max(60),
            grace_secs: grace_secs.max(0),
            not_found_noise,
            mutes,
        }
    }

    fn muted(&self, source: &str, e: &Event, fp: &str) -> bool {
        self.mutes.iter().any(|(src, re, f)| {
            (src.is_empty() || src == source)
                && ((!f.is_empty() && f == fp) || re.as_ref().is_some_and(|r| e.text.lines().any(|l| r.is_match(l))))
        })
    }
}

/// What the app knows about a source besides its lines.
#[derive(Debug, Clone, Default)]
pub struct SourceCtx {
    /// Journal lines (already filtered to priority err and worse).
    pub journal: bool,
    /// When the container started (unix seconds).
    pub started_at: Option<i64>,
    /// When a reverse proxy on the same host last started.
    pub proxy_started_at: Option<i64>,
    pub unhealthy: bool,
    pub restarting: bool,
}

/// Why a new pattern is a note and not an alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum NoteWhy {
    /// Fewer times than the repeat threshold.
    Once,
    /// Right after the container or the reverse proxy started.
    Grace,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewPattern {
    pub pattern: String,
    pub fingerprint: String,
    /// One event with this pattern (`None` if it looked like it held a secret).
    pub sample: Option<String>,
    /// Events this round.
    pub count: usize,
    /// Events within the repeat window.
    pub recent: usize,
    pub fatal: bool,
    /// Why it alerts even below the threshold, if it does.
    pub loud: Option<&'static str>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Finding {
    /// Error events this round and the usual count, when it's a burst.
    pub burst: Option<(usize, f64)>,
    /// Not-found events this round and the usual count, when it's a spike.
    pub not_found_spike: Option<(usize, f64)>,
    /// New patterns that alert.
    pub new_patterns: Vec<NewPattern>,
    /// New patterns that are only noted.
    pub notes: Vec<(NewPattern, NoteWhy)>,
    /// Events left out by a mute.
    pub muted: usize,
    /// Not-found events left out as noise.
    pub not_found: usize,
}

#[derive(Debug, Default)]
struct SourceState {
    /// fingerprint → first seen (unix seconds).
    known: HashMap<String, i64>,
    /// fingerprint → when it happened, within the repeat window.
    hits: HashMap<String, VecDeque<i64>>,
    /// fingerprint → pattern text, for labels (memory only).
    text: HashMap<String, String>,
    history: VecDeque<usize>,
    nf_history: VecDeque<usize>,
    learned: bool,
    samples: VecDeque<String>,
}

#[derive(Debug, Default)]
pub struct Detector {
    sources: HashMap<String, SourceState>,
    /// (source, fingerprint, first seen) learned since the last `take_learned`.
    learned: Vec<(String, String, i64)>,
}

impl Detector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Patterns known from earlier runs (the database), so "new" survives a restart.
    pub fn seed(&mut self, source: &str, known: impl IntoIterator<Item = (String, i64)>) {
        let st = self.sources.entry(source.to_owned()).or_default();
        st.known.extend(known);
        st.learned = st.learned || !st.known.is_empty();
    }

    /// Patterns first seen since the last call, to be stored.
    pub fn take_learned(&mut self) -> Vec<(String, String, i64)> {
        std::mem::take(&mut self.learned)
    }

    /// The pattern text behind a fingerprint, if seen since the app started.
    pub fn pattern_of(&self, source: &str, fp: &str) -> Option<String> {
        self.sources.get(source)?.text.get(fp).cloned()
    }

    /// Feeds one round of a source's lines (all of them; errors are picked here).
    pub fn observe(&mut self, source: &str, lines: &[RawLine], ctx: &SourceCtx, rules: &Rules, now: i64) -> Finding {
        let st = self.sources.entry(source.to_owned()).or_default();
        let mut finding = Finding::default();
        let mut errors = 0usize;
        let mut nf = 0usize;
        let mut fresh: Vec<(NewPattern, i64)> = Vec::new();
        let mut new_known = Vec::new();
        let in_grace = |ts: i64| {
            [ctx.started_at, ctx.proxy_started_at]
                .into_iter()
                .flatten()
                .any(|start| ts >= start - 5 && ts <= start + rules.grace_secs)
        };
        let mut all_in_grace = true;
        for e in events(lines, ctx.journal) {
            if e.not_found {
                nf += 1;
                if rules.not_found_noise && e.level < Level::Fatal {
                    finding.not_found += 1;
                    continue;
                }
            }
            if e.level < Level::Error {
                continue;
            }
            let p = pattern(&e.head);
            if p.is_empty() {
                continue;
            }
            let fp = fingerprint(&p);
            if rules.muted(source, &e, &fp) {
                finding.muted += 1;
                continue;
            }
            errors += 1;
            let ts = e.ts.unwrap_or(now);
            all_in_grace &= in_grace(ts);
            let safe = (!e.text.lines().any(line_has_secret)).then(|| e.text.chars().take(400).collect::<String>());
            if let Some(s) = &safe {
                st.samples.push_back(s.clone());
                if st.samples.len() > SAMPLES {
                    st.samples.pop_front();
                }
            }
            st.hits.entry(fp.clone()).or_default().push_back(ts);
            if st.text.len() < KNOWN_MAX && !line_has_secret(&p) {
                st.text.entry(fp.clone()).or_insert_with(|| p.clone());
            }
            let first = match st.known.get(&fp) {
                Some(t) => *t,
                None if st.known.len() < KNOWN_MAX => {
                    st.known.insert(fp.clone(), now);
                    new_known.push((source.to_owned(), fp.clone(), now));
                    now
                }
                None => now,
            };
            match fresh.iter_mut().find(|(n, _)| n.fingerprint == fp) {
                Some((n, grace_ts)) => {
                    n.count += 1;
                    n.fatal |= e.level == Level::Fatal;
                    if n.sample.is_none() {
                        n.sample = safe;
                    }
                    if !in_grace(ts) {
                        *grace_ts = ts;
                    }
                }
                None if now - first <= NEW_FOR => fresh.push((
                    NewPattern {
                        pattern: p,
                        fingerprint: fp,
                        sample: safe,
                        count: 1,
                        recent: 0,
                        fatal: e.level == Level::Fatal,
                        loud: None,
                    },
                    ts,
                )),
                None => {}
            }
        }
        for q in st.hits.values_mut() {
            while q.front().is_some_and(|t| *t < now - rules.repeat_secs) {
                q.pop_front();
            }
        }
        st.hits.retain(|_, q| !q.is_empty());
        if st.learned {
            for (mut n, ts) in fresh {
                n.recent = st.hits.get(&n.fingerprint).map_or(n.count, VecDeque::len).max(n.count);
                n.loud = if n.fatal {
                    Some("fatal")
                } else if ctx.restarting {
                    Some("the container is restarting")
                } else if ctx.unhealthy {
                    Some("the container is unhealthy")
                } else {
                    None
                };
                if n.loud.is_some() {
                    finding.new_patterns.push(n);
                } else if in_grace(ts) {
                    finding.notes.push((n, NoteWhy::Grace));
                } else if n.recent >= rules.repeat_count {
                    finding.new_patterns.push(n);
                } else {
                    finding.notes.push((n, NoteWhy::Once));
                }
            }
            if st.history.len() >= 3 && !(all_in_grace && errors > 0 && !ctx.unhealthy && !ctx.restarting) {
                let usual = st.history.iter().sum::<usize>() as f64 / st.history.len() as f64;
                if errors >= BURST_MIN && errors as f64 >= BURST_FACTOR * usual.max(1.0) {
                    finding.burst = Some((errors, usual));
                }
            }
            if st.nf_history.len() >= 3 {
                let usual = st.nf_history.iter().sum::<usize>() as f64 / st.nf_history.len() as f64;
                if nf >= NOT_FOUND_MIN && nf as f64 >= NOT_FOUND_FACTOR * usual.max(1.0) {
                    finding.not_found_spike = Some((nf, usual));
                }
            }
        }
        for (h, n) in [(&mut st.history, errors), (&mut st.nf_history, nf)] {
            h.push_back(n);
            if h.len() > HISTORY {
                h.pop_front();
            }
        }
        st.learned = true;
        self.learned.extend(new_known);
        finding
    }

    /// Recent error events of a source (secret-looking ones left out).
    pub fn samples(&self, source: &str) -> Vec<String> {
        self.sources.get(source).map(|s| s.samples.iter().cloned().collect()).unwrap_or_default()
    }

    /// Sources seen before whose key starts with `prefix`.
    pub fn sources_with_prefix(&self, prefix: &str) -> Vec<String> {
        self.sources.keys().filter(|k| k.starts_with(prefix)).cloned().collect()
    }

    /// Forgets sources whose key starts with `prefix` (a removed host).
    pub fn forget(&mut self, prefix: &str) {
        self.sources.retain(|k, _| !k.starts_with(prefix));
    }
}

/// `2026-11-16T18:07:39.214123Z` (Docker's log timestamps) → unix seconds.
pub fn parse_rfc3339(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (n(0..4)?, n(5..7)?, n(8..10)?, n(11..13)?, n(14..16)?, n(17..19)?);
    // Offset: Z or ±HH:MM after optional fraction.
    let rest = &s[19..];
    let rest = rest.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let off = match rest.as_bytes().first() {
        Some(b'Z') | None => 0,
        Some(b'+' | b'-') if rest.len() >= 6 => {
            let sign = if rest.starts_with('-') { -1 } else { 1 };
            sign * (rest[1..3].parse::<i64>().ok()? * 3600 + rest[4..6].parse::<i64>().ok()? * 60)
        }
        _ => return None,
    };
    // Days from civil (Howard Hinnant).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (mo + if mo > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + se - off)
}

/// Seconds a container has been up, from Docker's status line ("Up 3
/// minutes (healthy)", "Up About an hour", "Up Less than a second").
pub fn uptime_from_status(status: &str) -> Option<i64> {
    let rest = status.strip_prefix("Up ")?;
    let rest = rest.split(" (").next()?.trim();
    let lower = rest.to_ascii_lowercase();
    if lower.starts_with("less than a second") {
        return Some(0);
    }
    let mut words = lower.split_whitespace();
    let first = words.next()?;
    let n: i64 = match first {
        "about" | "an" | "a" => {
            let w = words.next()?;
            if w == "a" || w == "an" {
                1
            } else {
                return unit_secs(w);
            }
        }
        _ => first.parse().ok()?,
    };
    let unit = words.next()?;
    Some(n * unit_secs(unit)?)
}

fn unit_secs(u: &str) -> Option<i64> {
    let u = u.trim_end_matches('s');
    Some(match u {
        "second" => 1,
        "minute" => 60,
        "hour" => 3600,
        "day" => 86_400,
        "week" => 7 * 86_400,
        "month" => 30 * 86_400,
        "year" => 365 * 86_400,
        _ => return None,
    })
}

/// Images that are reverse proxies: a restart of one reloads every site behind it.
pub fn is_reverse_proxy(image: &str) -> bool {
    let i = image.to_ascii_lowercase();
    let name = i.rsplit('/').next().unwrap_or(&i);
    let name = name.split([':', '@']).next().unwrap_or(name);
    ["caddy", "traefik", "nginx", "nginx-proxy-manager", "haproxy", "swag", "envoy", "nginx-proxy", "openresty"]
        .iter()
        .any(|p| name == *p || name.starts_with(&format!("{p}-")))
        || i.contains("nginx-proxy-manager")
}

/// The shape of a line: what stays the same between occurrences.
pub fn pattern(line: &str) -> String {
    let body = strip_timestamp(line);
    let mut out = String::with_capacity(body.len().min(PATTERN_MAX + 8));
    let mut chars = body.chars().peekable();
    let mut last_placeholder = false;
    while let Some(c) = chars.next() {
        if out.len() >= PATTERN_MAX {
            break;
        }
        if c == '"' || c == '\'' {
            // Quoted values vary: keep the quotes, drop the content.
            let q = c;
            let mut closed = false;
            for n in chars.by_ref() {
                if n == q {
                    closed = true;
                    break;
                }
            }
            out.push(q);
            out.push('…');
            if closed {
                out.push(q);
            }
            last_placeholder = false;
            continue;
        }
        if c.is_ascii_alphanumeric() {
            let mut word = String::from(c);
            while let Some(&n) = chars.peek() {
                if n.is_ascii_alphanumeric() || matches!(n, '.' | ':' | '-' | '_' | '/') {
                    word.push(n);
                    chars.next();
                } else {
                    break;
                }
            }
            let word = word.trim_end_matches(['.', ':', '-', '/']);
            if varies(word) {
                if !last_placeholder {
                    out.push('#');
                }
                last_placeholder = true;
            } else {
                out.push_str(word);
                last_placeholder = false;
            }
            continue;
        }
        out.push(c);
        last_placeholder = false;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Numbers, hex ids, UUIDs, addresses and other tokens with digits in them.
fn varies(w: &str) -> bool {
    let digits = w.chars().filter(char::is_ascii_digit).count();
    if digits == 0 {
        // Long hex ids without a digit are rare but exist.
        return w.len() >= 16 && w.chars().all(|c| c.is_ascii_hexdigit());
    }
    // Words like "http2", "ipv4", "utf8", "h264" stay.
    !(digits <= 2
        && w.len() >= 4
        && w.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && !w.contains(['.', ':', '-', '/']))
}

/// Drops a leading ISO or syslog timestamp and log level brackets' times.
fn strip_timestamp(line: &str) -> &str {
    let t = line.trim_start();
    // 2026-11-15T10:00:00.123Z / 2026-11-15 10:00:00,123
    let b = t.as_bytes();
    if b.len() > 19 && b[4] == b'-' && b[7] == b'-' && (b[10] == b'T' || b[10] == b' ') {
        let rest = &t[10..];
        let end = rest.find(|c: char| c.is_whitespace()).map(|i| i + 10);
        if let Some(e) = end {
            // "2026-11-15 10:00:00" has a space at index 10: skip the time part too.
            if b[10] == b' ' {
                let after = &t[11..];
                return after.split_once(' ').map(|(_, r)| r).unwrap_or(after);
            }
            return &t[e..];
        }
    }
    t
}

/// Error lines from the journal since `since` (unix seconds), priority err and worse.
pub fn journal_command(since: i64) -> String {
    format!("journalctl -p 3 --since @{since} -o short-unix --no-pager -q 2>/dev/null | tail -n 1000")
}

/// `1731664800.123456 host sshd[123]: message` → (identifier, message).
pub fn parse_journal(out: &str) -> Vec<(String, String)> {
    out.lines()
        .filter_map(|l| {
            let mut it = l.splitn(3, ' ');
            let ts = it.next()?;
            ts.parse::<f64>().ok()?;
            let _host = it.next()?;
            let rest = it.next()?;
            let (ident, msg) = rest.split_once(": ")?;
            let ident = ident.split('[').next().unwrap_or(ident);
            Some((ident.to_owned(), msg.to_owned()))
        })
        .collect()
}

/// Most patterns stored per host and source.
const STORED_MAX: i64 = 2000;

impl crate::store::Store {
    /// Patterns seen before on a host: (source, fingerprint, first seen).
    pub fn log_patterns(&self, host: crate::hosts::HostId) -> crate::Result<Vec<(String, String, i64)>> {
        let mut st =
            self.conn().prepare("SELECT source, fingerprint, first_seen FROM log_patterns WHERE host_id = ?1")?;
        let rows = st.query_map([host], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Stores newly seen patterns (first sighting wins), keeping the newest per source.
    pub fn log_patterns_add(&self, host: crate::hosts::HostId, rows: &[(String, String, i64)]) -> crate::Result<()> {
        let c = self.conn();
        for (source, fp, at) in rows {
            c.execute(
                "INSERT OR IGNORE INTO log_patterns (host_id, source, fingerprint, first_seen) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![host, source, fp, at],
            )?;
        }
        let sources: HashSet<&str> = rows.iter().map(|r| r.0.as_str()).collect();
        for s in sources {
            c.execute(
                "DELETE FROM log_patterns WHERE host_id = ?1 AND source = ?2 AND fingerprint NOT IN \
                 (SELECT fingerprint FROM log_patterns WHERE host_id = ?1 AND source = ?2 ORDER BY first_seen DESC LIMIT ?3)",
                rusqlite::params![host, s, STORED_MAX],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STIRLING: &str = "18:07:39.214 [jetty-1511] WARN  s.s.S.e.GlobalExceptionHandler - No resource at /api/v1/policies: No static resource api/v1/policies for request '/api/v1/policies'.";
    const CONVERTX: [&str; 4] = [
        "error: NOT_FOUND",
        " status: 404,",
        "   code: \"NOT_FOUND\"",
        "      at NotFoundError (/app/node_modules/elysia/src/error.ts:132:3)",
    ];

    fn raw(lines: &[&str]) -> Vec<RawLine> {
        lines.iter().map(|s| RawLine::new(None, *s)).collect()
    }
    fn at(ts: i64, lines: &[&str]) -> Vec<RawLine> {
        lines.iter().map(|s| RawLine::new(Some(ts), *s)).collect()
    }
    fn ctx() -> SourceCtx {
        SourceCtx::default()
    }
    /// A detector that has had its learning round for `src`.
    fn learned(src: &str) -> Detector {
        let mut d = Detector::new();
        d.observe(src, &raw(&["INFO started"]), &ctx(), &Rules::default(), 0);
        d
    }

    #[test]
    fn levels_decide_before_wording() {
        assert_eq!(declared_level(STIRLING), Some(Level::Warn));
        assert!(!is_error_line(STIRLING), "a WARN line is never an error, whatever its words");
        assert!(!is_error_line("2026-11-16T10:00:00Z INFO Exception handler registered"));
        assert!(!is_error_line("level=debug msg=\"error budget 0\""));
        assert!(is_error_line("level=error msg=\"db down\""));
        assert!(is_error_line("{\"level\":\"error\",\"msg\":\"db down\"}"));
        assert!(is_error_line("error: NOT_FOUND"));
        assert!(is_error_line("[ERROR] could not bind"));
        assert_eq!(line_level("FATAL: out of disk"), Level::Fatal);
        assert_eq!(line_level("panic: runtime error: index out of range"), Level::Fatal);
        // No level: wording decides, as before.
        assert!(is_error_line("Traceback (most recent call last):"));
        assert!(is_error_line("connect: Connection refused"));
        assert!(!is_error_line("compiled with 0 errors"));
        assert!(!is_error_line("GET /health 200"));
    }

    #[test]
    fn a_stack_trace_is_one_event() {
        let ev = events(&raw(&CONVERTX), false);
        assert_eq!(ev.len(), 1, "{ev:?}");
        assert_eq!(ev[0].lines, 4);
        assert_eq!(ev[0].level, Level::Error);
        assert!(ev[0].not_found);
        // Java and Python traces too; a new log line starts a new event.
        let ev = events(
            &raw(&[
                "ERROR boom",
                "java.lang.IllegalStateException: x",
                "\tat a.b.C.d(C.java:10)",
                "Caused by: java.io.IOException",
                "INFO next request",
                "Traceback (most recent call last):",
                "  File \"app.py\", line 3, in <module>",
                "ValueError: bad",
            ]),
            false,
        );
        assert_eq!(ev.len(), 4, "{ev:?}");
        assert_eq!(ev[3].lines, 3);
        // Lines far apart in time don't join.
        let mut lines = at(100, &["error: one"]);
        lines.extend(at(200, &["  detail"]));
        assert_eq!(events(&lines, false).len(), 2);
    }

    #[test]
    fn stirling_warn_never_alerts() {
        let mut d = learned("container/stirling-pdf");
        for round in 1..=5 {
            let f = d.observe(
                "container/stirling-pdf",
                &raw(&[STIRLING, STIRLING, STIRLING, STIRLING]),
                &ctx(),
                &Rules::default(),
                round * 600,
            );
            assert!(f.new_patterns.is_empty() && f.notes.is_empty() && f.burst.is_none(), "{f:?}");
        }
    }

    #[test]
    fn convertx_404_is_one_event_and_no_alert() {
        let mut d = learned("container/convertx");
        let f = d.observe("container/convertx", &raw(&CONVERTX), &ctx(), &Rules::default(), 600);
        assert!(f.new_patterns.is_empty(), "{f:?}");
        assert_eq!(f.not_found, 1, "the four lines are one not-found event");
        // Even with 404s not treated as noise, one occurrence is only a note.
        let rules = Rules::new(3, 600, 300, false, &[]);
        let mut d = learned("container/convertx");
        let f = d.observe("container/convertx", &raw(&CONVERTX), &ctx(), &rules, 600);
        assert!(f.new_patterns.is_empty());
        assert_eq!(f.notes.len(), 1);
        assert_eq!(f.notes[0].1, NoteWhy::Once);
        assert_eq!(f.notes[0].0.count, 1);
    }

    #[test]
    fn a_404_spike_still_alerts() {
        let mut d = learned("container/web");
        for r in 1..=4 {
            d.observe("container/web", &raw(&["WARN GET /x 404 not found"; 3]), &ctx(), &Rules::default(), r * 600);
        }
        let f = d.observe("container/web", &raw(&["WARN GET /x 404 not found"; 60]), &ctx(), &Rules::default(), 3000);
        assert_eq!(f.not_found_spike.map(|s| s.0), Some(60));
        assert!(f.new_patterns.is_empty());
    }

    #[test]
    fn repetition_turns_a_note_into_an_alert() {
        let mut d = learned("container/app");
        let f = d.observe("container/app", &at(600, &["ERROR db timeout after 30s"]), &ctx(), &Rules::default(), 600);
        assert_eq!(f.notes.len(), 1);
        let f = d.observe(
            "container/app",
            &at(1000, &["ERROR db timeout after 31s", "ERROR db timeout after 9s"]),
            &ctx(),
            &Rules::default(),
            1000,
        );
        assert_eq!(f.new_patterns.len(), 1, "3 in 10 minutes: {f:?}");
        assert_eq!(f.new_patterns[0].recent, 3);
        // Seen yesterday: not new any more, however often.
        let later = 1000 + NEW_FOR + 60;
        let f =
            d.observe("container/app", &at(later, &["ERROR db timeout after 1s"; 5]), &ctx(), &Rules::default(), later);
        assert!(f.new_patterns.is_empty() && f.notes.is_empty());
    }

    #[test]
    fn fatal_and_unhealthy_alert_at_once() {
        let mut d = learned("container/app");
        let f = d.observe("container/app", &raw(&["panic: runtime error: nil map"]), &ctx(), &Rules::default(), 600);
        assert_eq!(f.new_patterns.len(), 1);
        assert!(f.new_patterns[0].fatal);
        let sick = SourceCtx { unhealthy: true, ..ctx() };
        let f = d.observe("container/app", &raw(&["ERROR cannot reach db"]), &sick, &Rules::default(), 1200);
        assert_eq!(f.new_patterns[0].loud, Some("the container is unhealthy"));
        let looping = SourceCtx { restarting: true, ..ctx() };
        let f = d.observe("container/app", &raw(&["ERROR config invalid"]), &looping, &Rules::default(), 1800);
        assert_eq!(f.new_patterns.len(), 1);
    }

    #[test]
    fn grace_after_a_restart_downgrades() {
        let mut d = learned("container/app");
        let proxy = SourceCtx { proxy_started_at: Some(1000), ..ctx() };
        let lines = at(1010, &["ERROR upstream reset", "ERROR upstream reset", "ERROR upstream reset"]);
        let f = d.observe("container/app", &lines, &proxy, &Rules::default(), 1200);
        assert!(f.new_patterns.is_empty());
        assert_eq!(f.notes[0].1, NoteWhy::Grace);
        // Fatal ignores the grace period.
        let started = SourceCtx { started_at: Some(2000), ..ctx() };
        let f =
            d.observe("container/app", &at(2005, &["FATAL cannot open data dir"]), &started, &Rules::default(), 2100);
        assert_eq!(f.new_patterns.len(), 1);
    }

    #[test]
    fn mutes_by_fingerprint_or_regex_are_counted() {
        let fp = fingerprint(&pattern("ERROR cache miss storm"));
        let rules = Rules::new(
            1,
            600,
            0,
            true,
            &[
                LogMute { source: "container/app".into(), fingerprint: fp, ..Default::default() },
                LogMute { source: String::new(), regex: "(?i)deprecated".into(), ..Default::default() },
            ],
        );
        let mut d = learned("container/app");
        let f = d.observe(
            "container/app",
            &raw(&["ERROR cache miss storm", "ERROR DEPRECATED api", "ERROR real thing"]),
            &ctx(),
            &rules,
            600,
        );
        assert_eq!(f.muted, 2);
        assert_eq!(f.new_patterns.len(), 1);
        assert!(LogMute { regex: "(".into(), ..Default::default() }.problem().is_some());
        assert!(LogMute::default().problem().is_some());
    }

    #[test]
    fn known_patterns_survive_a_restart() {
        let mut d = learned("container/app");
        d.observe("container/app", &raw(&["ERROR disk slow 120ms"]), &ctx(), &Rules::default(), 600);
        let stored = d.take_learned();
        assert!(stored.iter().any(|(s, _, _)| s == "container/app"));
        let mut d2 = Detector::new();
        d2.seed("container/app", stored.into_iter().map(|(_, fp, t)| (fp, t - NEW_FOR - 1)));
        let f = d2.observe("container/app", &raw(&["ERROR disk slow 9ms"; 5]), &ctx(), &Rules::default(), 700);
        assert!(f.new_patterns.is_empty() && f.notes.is_empty(), "seen before the restart: {f:?}");
    }

    #[test]
    fn patterns_fold_variable_parts() {
        assert_eq!(pattern("timeout after 3012 ms to 10.0.0.5:5432"), pattern("timeout after 87 ms to 10.0.0.9:5432"));
        assert_eq!(
            pattern("2026-11-15T10:00:00.123Z ERROR request 6f1c2a9e-1b2c-4d5e-8f90-123456789abc failed"),
            "ERROR request # failed"
        );
        assert_eq!(pattern("user \"bob\" not found"), pattern("user \"alice\" not found"));
        assert_eq!(pattern("http2 stream error"), "http2 stream error");
        assert_ne!(pattern("disk full on /data"), pattern("certificate expired"));
        // Timestamps, thread ids and request paths with numbers fold away.
        assert_eq!(
            pattern(STIRLING),
            pattern(&STIRLING.replace("18:07:39.214", "09:01:02.003").replace("jetty-1511", "jetty-77"))
        );
    }

    #[test]
    fn bursts_against_the_usual_count() {
        let mut d = Detector::new();
        for r in 0..4 {
            d.observe("j/sshd", &raw(&["error: kex failed", "error: kex failed"]), &ctx(), &Rules::default(), r * 600);
        }
        let many: Vec<String> = (0..30).map(|i| format!("error: kex failed {i}")).collect();
        let many: Vec<&str> = many.iter().map(String::as_str).collect();
        let f = d.observe("j/sshd", &raw(&many), &ctx(), &Rules::default(), 3000);
        assert_eq!(f.burst.map(|b| b.0), Some(30));
        let f = d.observe("j/sshd", &raw(&many[..10]), &ctx(), &Rules::default(), 3600);
        assert!(f.burst.is_none());
    }

    #[test]
    fn secrets_stay_out_of_samples() {
        let mut d = learned("c/x");
        d.observe("c/x", &raw(&["error: login failed"]), &ctx(), &Rules::default(), 600);
        let f = d.observe("c/x", &raw(&["error: bad password=hunter2222"]), &ctx(), &Rules::default(), 700);
        assert_eq!(f.notes.len(), 1);
        assert!(f.notes[0].0.sample.is_none());
        assert_eq!(d.samples("c/x"), vec!["error: login failed"]);
    }

    #[test]
    fn docker_times_and_status() {
        assert_eq!(parse_rfc3339("1970-01-01T00:00:10Z"), Some(10));
        assert_eq!(parse_rfc3339("2026-11-16T18:07:39.214123456Z"), Some(1_794_852_459));
        assert_eq!(parse_rfc3339("2026-11-16T19:07:39+01:00"), Some(1_794_852_459));
        assert_eq!(parse_rfc3339("nope"), None);
        assert_eq!(uptime_from_status("Up 3 minutes (healthy)"), Some(180));
        assert_eq!(uptime_from_status("Up About an hour"), Some(3600));
        assert_eq!(uptime_from_status("Up Less than a second"), Some(0));
        assert_eq!(uptime_from_status("Up 2 days"), Some(2 * 86_400));
        assert_eq!(uptime_from_status("Exited (0) 2 days ago"), None);
        assert!(is_reverse_proxy("caddy:2-alpine"));
        assert!(is_reverse_proxy("jc21/nginx-proxy-manager:latest"));
        assert!(is_reverse_proxy("traefik:v3"));
        assert!(!is_reverse_proxy("ghcr.io/c4illin/convertx"));
    }

    #[test]
    fn stored_patterns_round_trip() {
        let s = crate::store::Store::open_in_memory().unwrap();
        s.conn().execute("INSERT INTO hosts (name, hostname) VALUES ('a', 'a.example.com')", []).unwrap();
        s.log_patterns_add(1, &[("container/x".into(), "00112233aabbccdd".into(), 5)]).unwrap();
        s.log_patterns_add(1, &[("container/x".into(), "00112233aabbccdd".into(), 9)]).unwrap();
        assert_eq!(s.log_patterns(1).unwrap(), vec![("container/x".to_owned(), "00112233aabbccdd".to_owned(), 5)]);
    }

    #[test]
    fn journal_lines() {
        let out = "1731664800.123456 nas sshd[123]: error: kex_exchange_identification: Connection closed\n\
                   1731664801.000000 nas kernel: Out of memory: Killed process 42\n-- No entries --\n";
        let j = parse_journal(out);
        assert_eq!(j.len(), 2);
        assert_eq!(j[0].0, "sshd");
        assert_eq!(j[1], ("kernel".into(), "Out of memory: Killed process 42".into()));
        // Journal lines are errors by priority, unless they say otherwise.
        let ev = events(&raw(&["disk sda: I/O timeout", "WARN only a warning"]), true);
        assert_eq!(ev[0].level, Level::Error);
        assert_eq!(ev[1].level, Level::Warn);
    }
}
