//! Expiry watch (M10l): things that run out on a date. Certificate files on a
//! host (read with `openssl` over SSH), domains (date typed in, or looked up
//! with `whois` on a chosen host when asked), and tokens, licences or
//! anything else with a date typed in. Service and proxy certificates are
//! already checked elsewhere; the UI shows them on the same board.
//!
//! Only names, dates and paths are stored: never the token itself (rule 1).

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::alerts::{AlertKind, Condition, Severity};
use crate::files::path::shell_quote;
use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

pub type WatchId = i32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum WatchKind {
    /// A PEM certificate file on a host; the date is read from it.
    CertFile,
    /// A domain registration.
    Domain,
    /// An API token, key or password with an end date (the value is never stored).
    Token,
    /// A licence or subscription.
    License,
    Other,
}

impl WatchKind {
    fn as_str(self) -> &'static str {
        match self {
            WatchKind::CertFile => "certFile",
            WatchKind::Domain => "domain",
            WatchKind::Token => "token",
            WatchKind::License => "license",
            WatchKind::Other => "other",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "certFile" => WatchKind::CertFile,
            "domain" => WatchKind::Domain,
            "token" => WatchKind::Token,
            "license" => WatchKind::License,
            _ => WatchKind::Other,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            WatchKind::CertFile => "certificate",
            WatchKind::Domain => "domain",
            WatchKind::Token => "token",
            WatchKind::License => "licence",
            WatchKind::Other => "item",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct WatchItem {
    pub id: WatchId,
    pub kind: WatchKind,
    pub name: String,
    /// The host a certificate file is on, or a domain is looked up from.
    pub host_id: Option<HostId>,
    /// Path of a certificate file, or the domain name.
    pub target: String,
    /// Unix seconds; `None` until known.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub expires_at: Option<i64>,
    /// Warn this many days before.
    pub warn_days: u16,
    pub note: String,
    /// When the date was last read from the host.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub checked_at: Option<i64>,
    pub check_error: Option<String>,
    /// Shown, but never alerts.
    pub muted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct WatchInput {
    /// `None` adds a new item.
    pub id: Option<WatchId>,
    pub kind: WatchKind,
    pub name: String,
    pub host_id: Option<HostId>,
    pub target: String,
    /// Typed-in date (unix seconds); ignored for certificate files.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub expires_at: Option<i64>,
    pub warn_days: u16,
    pub note: String,
    #[serde(default)]
    pub muted: bool,
}

fn bad(field: &'static str, reason: &str) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

/// A domain name: letters, digits, hyphens and dots, at least one dot.
pub fn valid_domain(d: &str) -> bool {
    let d = d.trim_end_matches('.');
    d.len() <= 253
        && d.contains('.')
        && d.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

/// An absolute path without control characters.
fn valid_path(p: &str) -> bool {
    p.starts_with('/') && p.len() <= 1024 && !p.chars().any(char::is_control)
}

fn clean(i: &WatchInput) -> Result<WatchInput> {
    let name = i.name.trim().to_owned();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(bad("name", "1 to 120 characters"));
    }
    if crate::redact::line_has_secret(&name) || crate::redact::line_has_secret(&i.note) {
        return Err(bad("name", "that looks like a secret; only the name and date are kept"));
    }
    let mut target = i.target.trim().to_owned();
    match i.kind {
        WatchKind::CertFile => {
            if i.host_id.is_none() {
                return Err(bad("hostId", "pick the host the certificate file is on"));
            }
            if !valid_path(&target) {
                return Err(bad("target", "an absolute path, like /etc/ssl/certs/site.pem"));
            }
        }
        WatchKind::Domain => {
            target = target.to_ascii_lowercase();
            if !valid_domain(&target) {
                return Err(bad("target", "a domain name, like example.com"));
            }
        }
        _ => {
            if target.chars().count() > 200 {
                return Err(bad("target", "at most 200 characters"));
            }
        }
    }
    if !(1..=365).contains(&i.warn_days) {
        return Err(bad("warnDays", "1 to 365 days"));
    }
    let note = i.note.trim().to_owned();
    if note.chars().count() > 1000 {
        return Err(bad("note", "at most 1000 characters"));
    }
    let expires_at = if i.kind == WatchKind::CertFile { None } else { i.expires_at };
    Ok(WatchInput {
        id: i.id,
        kind: i.kind,
        name,
        host_id: i.host_id,
        target,
        expires_at,
        warn_days: i.warn_days,
        note,
        muted: i.muted,
    })
}

const COLS: &str = "id, kind, name, host_id, target, expires_at, warn_days, note, checked_at, check_error, muted";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<WatchItem> {
    Ok(WatchItem {
        id: r.get(0)?,
        kind: WatchKind::parse(&r.get::<_, String>(1)?),
        name: r.get(2)?,
        host_id: r.get(3)?,
        target: r.get(4)?,
        expires_at: r.get(5)?,
        warn_days: r.get::<_, i64>(6)?.clamp(1, 365) as u16,
        note: r.get(7)?,
        checked_at: r.get(8)?,
        check_error: r.get(9)?,
        muted: r.get(10)?,
    })
}

impl Store {
    pub fn watch_list(&self) -> Result<Vec<WatchItem>> {
        let mut st = self
            .conn()
            .prepare(&format!("SELECT {COLS} FROM watch_items ORDER BY expires_at IS NULL, expires_at, name"))?;
        let rows = st.query_map([], row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn watch_get(&self, id: WatchId) -> Result<Option<WatchItem>> {
        Ok(self.conn().query_row(&format!("SELECT {COLS} FROM watch_items WHERE id = ?1"), [id], row).optional()?)
    }

    /// Adds or updates an item. Changing what a certificate item points at
    /// clears its date until it is read again.
    pub fn watch_save(&self, input: &WatchInput) -> Result<WatchItem> {
        let i = clean(input)?;
        let c = self.conn();
        let id = match i.id {
            None => {
                c.execute(
                    "INSERT INTO watch_items (kind, name, host_id, target, expires_at, warn_days, note, muted)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![i.kind.as_str(), i.name, i.host_id, i.target, i.expires_at, i.warn_days, i.note, i.muted],
                )?;
                c.last_insert_rowid() as WatchId
            }
            Some(id) => {
                let old = self.watch_get(id)?.ok_or(Error::NotFound("watch item"))?;
                let keep_date = i.kind == WatchKind::CertFile
                    && old.kind == WatchKind::CertFile
                    && old.host_id == i.host_id
                    && old.target == i.target;
                let expires = if keep_date { old.expires_at } else { i.expires_at };
                let (checked, err) = if keep_date { (old.checked_at, old.check_error) } else { (None, None) };
                c.execute(
                    "UPDATE watch_items SET kind = ?2, name = ?3, host_id = ?4, target = ?5, expires_at = ?6,
                       warn_days = ?7, note = ?8, checked_at = ?9, check_error = ?10, muted = ?11 WHERE id = ?1",
                    params![
                        id,
                        i.kind.as_str(),
                        i.name,
                        i.host_id,
                        i.target,
                        expires,
                        i.warn_days,
                        i.note,
                        checked,
                        err,
                        i.muted
                    ],
                )?;
                id
            }
        };
        self.watch_get(id)?.ok_or(Error::NotFound("watch item"))
    }

    pub fn watch_delete(&self, id: WatchId) -> Result<()> {
        self.conn().execute("DELETE FROM watch_items WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Stores the result of reading an item's date from a host. An error
    /// keeps the last known date.
    pub fn watch_checked(&self, id: WatchId, result: std::result::Result<i64, String>, now: i64) -> Result<()> {
        match result {
            Ok(at) => self.conn().execute(
                "UPDATE watch_items SET expires_at = ?2, checked_at = ?3, check_error = NULL WHERE id = ?1",
                params![id, at, now],
            )?,
            Err(e) => self.conn().execute(
                "UPDATE watch_items SET checked_at = ?2, check_error = ?3 WHERE id = ?1",
                params![id, now, e.chars().take(300).collect::<String>()],
            )?,
        };
        Ok(())
    }
}

// ---- edits for rows found automatically -------------------------------------

/// What an Expiry Watch row Nightgrid found by itself is about: a service's
/// HTTPS certificate or a reverse-proxy route's certificate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PrefTarget {
    Service { service_id: crate::services::ServiceId },
    Proxy { domain: String },
}

/// The user's edits for one automatic row. Saved with every field at its
/// default, the row is removed (the row goes back to how it was found).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ExpiryPref {
    pub target: PrefTarget,
    /// SHA-256 of the one certificate accepted although the OS doesn't trust
    /// it (self-signed, private CA). Another certificate warns again.
    pub trusted_fingerprint: Option<String>,
    /// Shown, but never alerts.
    pub muted: bool,
    /// A date to use instead of the certificate's (unix seconds).
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub expires_at: Option<i64>,
    /// Instead of Settings → Alerts → certificate days.
    pub warn_days: Option<u16>,
    pub note: String,
}

impl ExpiryPref {
    pub fn empty(target: PrefTarget) -> Self {
        Self { target, trusted_fingerprint: None, muted: false, expires_at: None, warn_days: None, note: String::new() }
    }

    fn is_empty(&self) -> bool {
        self.trusted_fingerprint.is_none()
            && !self.muted
            && self.expires_at.is_none()
            && self.warn_days.is_none()
            && self.note.is_empty()
    }
}

/// Is `fp` a SHA-256 fingerprint? Accepts `AB:CD:…` too; returns it as
/// lowercase hex.
pub fn clean_fingerprint(fp: &str) -> Option<String> {
    let hex: String = fp.chars().filter(|c| *c != ':' && !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
    (hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit())).then_some(hex)
}

fn clean_pref(p: &ExpiryPref) -> Result<ExpiryPref> {
    let target = match &p.target {
        PrefTarget::Proxy { domain } => {
            let d = domain.trim().trim_end_matches('.').to_ascii_lowercase();
            if !valid_domain(&d) {
                return Err(bad("target", "a domain name"));
            }
            if p.trusted_fingerprint.is_some() {
                return Err(bad("trustedFingerprint", "only service certificates can be trusted here"));
            }
            PrefTarget::Proxy { domain: d }
        }
        t => t.clone(),
    };
    let trusted_fingerprint = match &p.trusted_fingerprint {
        None => None,
        Some(f) => Some(clean_fingerprint(f).ok_or_else(|| bad("trustedFingerprint", "a SHA-256 fingerprint"))?),
    };
    if let Some(w) = p.warn_days {
        if !(1..=365).contains(&w) {
            return Err(bad("warnDays", "1 to 365 days"));
        }
    }
    let note = p.note.trim().to_owned();
    if note.chars().count() > 1000 {
        return Err(bad("note", "at most 1000 characters"));
    }
    if crate::redact::line_has_secret(&note) {
        return Err(bad("note", "that looks like a secret"));
    }
    Ok(ExpiryPref {
        target,
        trusted_fingerprint,
        muted: p.muted,
        expires_at: p.expires_at,
        warn_days: p.warn_days,
        note,
    })
}

const PREF_COLS: &str = "service_id, proxy_domain, trusted_fingerprint, muted, expires_at, warn_days, note";

fn pref_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ExpiryPref> {
    let svc: Option<crate::services::ServiceId> = r.get(0)?;
    let domain: Option<String> = r.get(1)?;
    let target = match svc {
        Some(service_id) => PrefTarget::Service { service_id },
        None => PrefTarget::Proxy { domain: domain.unwrap_or_default() },
    };
    Ok(ExpiryPref {
        target,
        trusted_fingerprint: r.get(2)?,
        muted: r.get(3)?,
        expires_at: r.get(4)?,
        warn_days: r.get::<_, Option<i64>>(5)?.map(|w| w.clamp(1, 365) as u16),
        note: r.get(6)?,
    })
}

impl Store {
    pub fn expiry_prefs(&self) -> Result<Vec<ExpiryPref>> {
        let mut st = self.conn().prepare(&format!("SELECT {PREF_COLS} FROM expiry_prefs ORDER BY id"))?;
        let rows = st.query_map([], pref_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Saves the edits for one automatic row; all-default edits remove it.
    pub fn expiry_pref_save(&self, pref: &ExpiryPref) -> Result<()> {
        let p = clean_pref(pref)?;
        let (svc, domain) = match &p.target {
            PrefTarget::Service { service_id } => (Some(*service_id), None),
            PrefTarget::Proxy { domain } => (None, Some(domain.clone())),
        };
        let c = self.conn();
        c.execute("DELETE FROM expiry_prefs WHERE service_id IS ?1 AND proxy_domain IS ?2", params![svc, domain])?;
        if p.is_empty() {
            return Ok(());
        }
        if let Some(id) = svc {
            let known: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM services WHERE id = ?1)", [id], |r| r.get(0))?;
            if !known {
                return Err(Error::NotFound("service"));
            }
        }
        c.execute(
            "INSERT INTO expiry_prefs (service_id, proxy_domain, trusted_fingerprint, muted, expires_at, warn_days, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![svc, domain, p.trusted_fingerprint, p.muted, p.expires_at, p.warn_days, p.note],
        )?;
        Ok(())
    }
}

/// Whether a service's certificate problem is covered by the user trusting
/// that exact certificate.
pub fn trust_state(pref: Option<&ExpiryPref>, tls: &crate::services::check::TlsInfo) -> TrustState {
    if tls.trusted {
        return TrustState::Trusted;
    }
    match (pref.and_then(|p| p.trusted_fingerprint.as_deref()), tls.fingerprint.as_deref()) {
        (Some(pin), Some(fp)) if pin == fp => TrustState::Pinned,
        (Some(_), _) => TrustState::Changed,
        (None, _) => TrustState::Untrusted,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustState {
    /// The OS trusts it.
    Trusted,
    /// Not trusted by the OS, but the user accepted this exact certificate.
    Pinned,
    /// The user accepted a certificate, and the server now sends another one.
    Changed,
    Untrusted,
}

/// A service check result as the certificate-expiry alert should see it, and
/// the warning days to use: the user's date and days instead of the
/// certificate's, or no certificate at all when muted.
pub fn service_cert_view(
    pref: Option<&ExpiryPref>,
    r: &crate::services::check::CheckResult,
    cert_days: u16,
) -> (crate::services::check::CheckResult, u16) {
    let mut r = r.clone();
    let Some(p) = pref else { return (r, cert_days) };
    if let Some(t) = r.tls.as_mut() {
        if p.muted {
            t.expires = None;
        } else if p.expires_at.is_some() {
            t.expires = p.expires_at;
        }
    }
    (r, p.warn_days.unwrap_or(cert_days))
}

// ---- reading dates from hosts ----------------------------------------------

/// Reads `notAfter` from each certificate file. One line per path:
/// `f<TAB>path<TAB>notAfter=…` or the error.
pub fn cert_files_command(paths: &[String]) -> String {
    let list: Vec<String> = paths.iter().filter(|p| valid_path(p)).take(100).map(|p| shell_quote(p)).collect();
    format!(
        "command -v openssl >/dev/null 2>&1 || {{ echo 'noopenssl'; exit 0; }}\n\
         for f in {}; do e=$(openssl x509 -noout -enddate -in \"$f\" 2>&1 | head -n 1); printf 'f\\t%s\\t%s\\n' \"$f\" \"$e\"; done\n",
        if list.is_empty() { "''".to_owned() } else { list.join(" ") }
    )
}

/// Path → date or error, from [`cert_files_command`] output.
pub fn parse_cert_files(out: &str) -> Vec<(String, std::result::Result<i64, String>)> {
    if out.lines().any(|l| l.trim() == "noopenssl") {
        return vec![("*".into(), Err("openssl isn't installed on the host".into()))];
    }
    out.lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("f\t")?;
            let (path, what) = rest.split_once('\t').unwrap_or((rest, ""));
            let r = match what.trim().strip_prefix("notAfter=") {
                Some(d) => crate::docker::proxy::parse_openssl_date(d)
                    .map(|t| t as i64)
                    .ok_or_else(|| format!("unreadable date: {d}")),
                None if what.contains("No such file") => Err("file not found".into()),
                None if what.contains("Permission denied") => Err("permission denied (the login can't read it)".into()),
                None if what.trim().is_empty() => Err("not a certificate".into()),
                None => Err(what.trim().chars().take(200).collect()),
            };
            Some((path.to_owned(), r))
        })
        .collect()
}

/// Asks `whois` on the host for the domain's expiry lines. Only on a click:
/// the host contacts the registry's whois server.
pub fn whois_command(domain: &str) -> Option<String> {
    if !valid_domain(domain) {
        return None;
    }
    Some(format!(
        "command -v whois >/dev/null 2>&1 || {{ echo 'nowhois'; exit 0; }}\n\
         timeout 20 whois {} 2>&1 | grep -iE 'expir|paid-till|renewal date|valid until' | head -n 20\n",
        shell_quote(domain)
    ))
}

fn month(s: &str) -> Option<u32> {
    let m = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
    m.iter().position(|x| s.to_ascii_lowercase().starts_with(x)).map(|i| i as u32 + 1)
}

fn civil(y: i64, m: u32, d: u32) -> Option<i64> {
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || !(1990..=2200).contains(&y) {
        return None;
    }
    let (y2, m2) = if m <= 2 { (y - 1, m as i64 + 9) } else { (y, m as i64 - 3) };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * m2 + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146_097 + doe - 719_468) * 86_400)
}

/// The first date in a whois value: `2027-03-01T12:00:00Z`, `2027-03-01`,
/// `2027.03.01`, `01-Mar-2027`, `01.03.2027`. Midnight UTC of that day.
pub fn parse_date(s: &str) -> Option<i64> {
    let tokens = s.split(|c: char| c.is_whitespace() || c == 'T' || c == ',');
    for t in tokens {
        let parts: Vec<&str> = t.split(['-', '.', '/']).filter(|p| !p.is_empty()).collect();
        if parts.len() < 3 {
            continue;
        }
        let (a, b, c) = (parts[0], parts[1], parts[2]);
        let num = |x: &str| x.chars().take_while(char::is_ascii_digit).collect::<String>().parse::<i64>().ok();
        let got = if a.len() == 4 {
            civil(num(a)?, num(b)? as u32, num(c)? as u32)
        } else if let Some(m) = month(b) {
            civil(num(c)?, m, num(a)? as u32)
        } else if c.len() >= 4 {
            civil(num(c)?, num(b)? as u32, num(a)? as u32)
        } else {
            None
        };
        if got.is_some() {
            return got;
        }
    }
    None
}

/// The expiry date from `whois` output, or why there isn't one.
pub fn parse_whois(out: &str) -> std::result::Result<i64, String> {
    if out.lines().any(|l| l.trim() == "nowhois") {
        return Err("whois isn't installed on that host".into());
    }
    for l in out.lines() {
        let Some((_, v)) = l.split_once(':') else { continue };
        if let Some(t) = parse_date(v) {
            return Ok(t);
        }
    }
    Err("whois didn't show an expiry date for this domain".into())
}

// ---- alerts ----------------------------------------------------------------------

/// Whole days left (negative once expired).
pub fn days_left(expires_at: i64, now: i64) -> i64 {
    (expires_at - now).div_euclid(86_400)
}

/// Items inside their warning window, plus certificate files that couldn't be read.
pub fn conditions(items: &[WatchItem], now: i64) -> Vec<Condition> {
    let mut out = Vec::new();
    for i in items.iter().filter(|i| !i.muted) {
        let Some(at) = i.expires_at else { continue };
        let left = days_left(at, now);
        if left >= i64::from(i.warn_days) {
            continue;
        }
        let w = i.kind.word();
        out.push(Condition {
            key: format!("expiry:{}", i.id),
            kind: AlertKind::ExpirySoon,
            severity: if left <= 3 { Severity::Critical } else { Severity::Warn },
            title: i.name.clone(),
            detail: match left {
                l if l < 0 => format!("{w} expired {} days ago", -l),
                0 => format!("{w} expires today"),
                1 => format!("{w} expires tomorrow"),
                l => format!("{w} expires in {l} days"),
            },
            host_id: i.host_id,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(kind: WatchKind, name: &str, target: &str) -> WatchInput {
        WatchInput {
            id: None,
            kind,
            name: name.into(),
            host_id: None,
            target: target.into(),
            expires_at: Some(1_900_000_000),
            warn_days: 30,
            note: String::new(),
            muted: false,
        }
    }

    #[test]
    fn validates_and_saves() {
        let s = Store::open_in_memory().unwrap();
        s.conn().execute("INSERT INTO hosts (name, hostname) VALUES ('a', 'a.example.com')", []).unwrap();
        let h = s.conn().last_insert_rowid() as HostId;
        assert!(s.watch_save(&input(WatchKind::Domain, "Site", "not a domain")).is_err());
        assert!(s.watch_save(&input(WatchKind::CertFile, "Cert", "/etc/ssl/a.pem")).is_err(), "needs a host");
        assert!(s.watch_save(&input(WatchKind::Token, "token=abcdef123456789", "")).is_err());
        let d = s.watch_save(&input(WatchKind::Domain, "Site", "Example.COM")).unwrap();
        assert_eq!(d.target, "example.com");
        let mut c = input(WatchKind::CertFile, "Cert", "/etc/ssl/a.pem");
        c.host_id = Some(h);
        let c = s.watch_save(&c).unwrap();
        assert_eq!(c.expires_at, None, "read from the file, not typed");
        s.watch_checked(c.id, Ok(1_800_000_000), 5).unwrap();
        // Renaming keeps the read date; pointing elsewhere drops it.
        let mut edit =
            WatchInput { id: Some(c.id), name: "Cert 2".into(), ..input(WatchKind::CertFile, "", "/etc/ssl/a.pem") };
        edit.host_id = Some(h);
        assert_eq!(s.watch_save(&edit).unwrap().expires_at, Some(1_800_000_000));
        edit.target = "/etc/ssl/b.pem".into();
        assert_eq!(s.watch_save(&edit).unwrap().expires_at, None);
        s.watch_checked(c.id, Err("file not found".into()), 6).unwrap();
        let got = s.watch_get(c.id).unwrap().unwrap();
        assert_eq!(got.check_error.as_deref(), Some("file not found"));
        assert_eq!(s.watch_list().unwrap().len(), 2);
        s.conn().execute("DELETE FROM hosts WHERE id = ?1", [h]).unwrap();
        assert_eq!(s.watch_list().unwrap().len(), 1, "a host's certificate items go with it");
        s.watch_delete(d.id).unwrap();
        assert!(s.watch_list().unwrap().is_empty());
    }

    #[test]
    fn reads_certificate_files() {
        let cmd = cert_files_command(&["/etc/a b.pem".into(), "relative".into()]);
        assert!(cmd.contains("'/etc/a b.pem'") && !cmd.contains("relative"));
        let out = "f\t/a.pem\tnotAfter=Feb  1 12:00:00 2027 GMT\nf\t/b.pem\tCould not open file or uri for loading certificate from /b.pem: No such file or directory\nf\t/c.pem\t\n";
        let r = parse_cert_files(out);
        assert_eq!(r[0].1, Ok(1_801_483_200));
        assert_eq!(r[1].1, Err("file not found".into()));
        assert_eq!(r[2].1, Err("not a certificate".into()));
        assert!(parse_cert_files("noopenssl\n")[0].1.is_err());
    }

    #[test]
    fn reads_whois_dates() {
        assert!(whois_command("example.com; rm -rf /").is_none());
        assert!(whois_command("example.com").unwrap().contains("whois 'example.com'"));
        let day = civil(2027, 3, 1).unwrap();
        for v in [
            "Registry Expiry Date: 2027-03-01T12:00:00Z",
            "Expiration Date: 01-Mar-2027",
            "expires:             2027-03-01",
            "paid-till: 2027.03.01",
            "Expiry date: 01.03.2027",
            "Registrar Registration Expiration Date: 2027-03-01T00:00:00+0000",
        ] {
            assert_eq!(parse_whois(v), Ok(day), "{v}");
        }
        assert!(parse_whois("No match for domain").is_err());
        assert!(parse_whois("nowhois").unwrap_err().contains("installed"));
    }

    #[test]
    fn alerts_inside_the_window() {
        let now = 1_800_000_000;
        let item = |id, days: i64, warn| WatchItem {
            id,
            kind: WatchKind::Token,
            name: format!("t{id}"),
            host_id: None,
            target: String::new(),
            expires_at: Some(now + days * 86_400 + 60),
            warn_days: warn,
            note: String::new(),
            checked_at: None,
            check_error: None,
            muted: id == 5,
        };
        let c = conditions(&[item(1, 40, 30), item(2, 20, 30), item(3, 2, 30), item(4, -5, 30), item(5, 1, 30)], now);
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].severity, Severity::Warn);
        assert_eq!(c[0].detail, "token expires in 20 days");
        assert_eq!(c[1].severity, Severity::Critical);
        assert_eq!(c[2].detail, "token expired 5 days ago");
        assert_eq!(c[0].key, "expiry:2");
    }

    #[test]
    fn prefs_for_automatic_rows() {
        use crate::services::check::{CheckResult, TlsInfo};
        let s = Store::open_in_memory().unwrap();
        s.conn().execute("INSERT INTO services (name, url) VALUES ('web', 'https://web.lan/')", []).unwrap();
        let svc = s.conn().last_insert_rowid() as crate::services::ServiceId;
        let fp = "AB:".repeat(31) + "AB";
        let mut p = ExpiryPref::empty(PrefTarget::Service { service_id: svc });
        p.trusted_fingerprint = Some("nope".into());
        assert!(s.expiry_pref_save(&p).is_err());
        p.trusted_fingerprint = Some(fp);
        p.warn_days = Some(400);
        assert!(s.expiry_pref_save(&p).is_err());
        p.warn_days = Some(7);
        s.expiry_pref_save(&p).unwrap();
        s.expiry_pref_save(&p).unwrap();
        let got = s.expiry_prefs().unwrap();
        assert_eq!(got.len(), 1, "one row per service");
        assert_eq!(got[0].trusted_fingerprint.as_deref(), Some("ab".repeat(32).as_str()));
        assert!(
            s.expiry_pref_save(&ExpiryPref::empty(PrefTarget::Service { service_id: 999 })).is_ok(),
            "empty: nothing to keep"
        );
        let mut ghost = ExpiryPref::empty(PrefTarget::Service { service_id: 999 });
        ghost.muted = true;
        assert!(s.expiry_pref_save(&ghost).is_err());
        let mut px = ExpiryPref::empty(PrefTarget::Proxy { domain: "Site.Example.com.".into() });
        px.muted = true;
        s.expiry_pref_save(&px).unwrap();
        px.trusted_fingerprint = Some("ab".repeat(32));
        assert!(s.expiry_pref_save(&px).is_err(), "proxy certs have no fingerprint here");
        assert!(s
            .expiry_prefs()
            .unwrap()
            .iter()
            .any(|p| p.target == PrefTarget::Proxy { domain: "site.example.com".into() }));
        // Back to defaults removes the edits; deleting the service removes them too.
        s.expiry_pref_save(&ExpiryPref::empty(PrefTarget::Proxy { domain: "site.example.com".into() })).unwrap();
        assert_eq!(s.expiry_prefs().unwrap().len(), 1);
        s.conn().execute("DELETE FROM services WHERE id = ?1", [svc]).unwrap();
        assert!(s.expiry_prefs().unwrap().is_empty());

        // Trust follows the exact certificate.
        let tls = |fp: &str| TlsInfo {
            trusted: false,
            problem: Some("unknown issuer or self-signed".into()),
            expires: Some(2_000_000_000),
            subject: None,
            issuer: None,
            fingerprint: Some(fp.into()),
        };
        let mut pin = ExpiryPref::empty(PrefTarget::Service { service_id: 1 });
        assert_eq!(trust_state(Some(&pin), &tls("aa")), TrustState::Untrusted);
        pin.trusted_fingerprint = Some("aa".into());
        assert_eq!(trust_state(Some(&pin), &tls("aa")), TrustState::Pinned);
        assert_eq!(trust_state(Some(&pin), &tls("bb")), TrustState::Changed);
        assert_eq!(trust_state(None, &TlsInfo { trusted: true, ..tls("bb") }), TrustState::Trusted);

        // Alerts see the user's date and days, or nothing when muted.
        let r = CheckResult { up: true, status: Some(200), latency_ms: None, error: None, tls: Some(tls("aa")) };
        pin.expires_at = Some(1_900_000_000);
        pin.warn_days = Some(60);
        let (v, d) = service_cert_view(Some(&pin), &r, 14);
        assert_eq!((v.tls.unwrap().expires, d), (Some(1_900_000_000), 60));
        pin.muted = true;
        assert_eq!(service_cert_view(Some(&pin), &r, 14).0.tls.unwrap().expires, None);
        assert_eq!(service_cert_view(None, &r, 14), (r.clone(), 14));
    }
}
