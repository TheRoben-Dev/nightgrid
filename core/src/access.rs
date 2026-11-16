//! Login & access audit (M10u): who logged in to a host and from where,
//! failed attempts by source, fail2ban / CrowdSec bans, users with a login
//! shell, sudo rules, authorized keys and the sshd settings that matter,
//! read with one portable script. Nothing is stored; the report is read
//! again when asked for.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

use crate::hosts::HostId;
use crate::ssh::keys::{parse_authorized, AuthorizedKey};
use crate::system::schedule::civil_from_days;
use crate::timeline::{days_from_civil, iso_offset};

/// Days of login history read.
pub const DAYS: u32 = 7;
/// Lines of sshd log read at most.
const LOG_LINES: usize = 20_000;
const MAX_LOGINS: usize = 200;
const MAX_SOURCES: usize = 100;
const MAX_SESSIONS: usize = 30;
const MAX_DECISIONS: usize = 200;
/// Failed attempts in [`DAYS`] that make a host without a ban tool worth a
/// warning.
const MANY_FAILURES: u32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum FindingLevel {
    Info,
    Warn,
    Bad,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub level: FindingLevel,
    pub text: String,
}

/// Where login events were read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum LogSource {
    Journal,
    AuthLog,
    /// Neither could be read (no root, or no sshd logs).
    None,
}

/// A successful SSH login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LoginEvent {
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub ts: i64,
    pub user: String,
    pub from: String,
    /// `publickey`, `password`, `keyboard-interactive/pam`…
    pub method: String,
    /// The key's SHA256 fingerprint, for public-key logins.
    pub key: Option<String>,
}

/// Failed attempts from one address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FailSource {
    pub from: String,
    pub count: u32,
    /// User names tried (the first few).
    pub users: Vec<String>,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub last: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Jail {
    pub name: String,
    pub currently_failed: u32,
    pub total_failed: u32,
    pub currently_banned: u32,
    pub total_banned: u32,
    pub banned: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    pub value: String,
    pub scenario: String,
    pub duration: String,
    pub kind: String,
}

/// Whether a tool is there and could be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ToolState {
    Missing,
    /// Installed, but reading it needs root.
    Denied,
    Read,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ShellUser {
    pub name: String,
    pub uid: u32,
    pub shell: String,
    pub home: String,
    /// In a sudo group or named in a sudo rule.
    pub sudo: bool,
    /// A service account (uid under 1000, not root).
    pub system: bool,
    /// Keys in the user's authorized_keys (that could be read).
    pub keys: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SudoRule {
    pub file: String,
    pub rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UserKey {
    pub user: String,
    pub key: AuthorizedKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SshdSetting {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AccessReport {
    pub host_id: HostId,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub at: i64,
    /// Read as root (otherwise only what the login user may see).
    pub as_root: bool,
    pub login_user: String,
    pub days: u32,
    pub log_source: LogSource,
    /// Newest first.
    pub logins: Vec<LoginEvent>,
    /// Most attempts first.
    pub failures: Vec<FailSource>,
    pub failed_total: u32,
    /// `last` output, newest first.
    pub sessions: Vec<String>,
    pub fail2ban: ToolState,
    pub jails: Vec<Jail>,
    pub crowdsec: ToolState,
    pub decisions: Vec<Decision>,
    pub users: Vec<ShellUser>,
    pub sudoers_readable: bool,
    pub sudo_rules: Vec<SudoRule>,
    pub keys: Vec<UserKey>,
    /// `sshd -T` when it could run, otherwise the config files.
    pub sshd_effective: bool,
    pub sshd: Vec<SshdSetting>,
    pub findings: Vec<Finding>,
}

const SSHD_KEYS: &str = "permitrootlogin|passwordauthentication|kbdinteractiveauthentication|pubkeyauthentication|permitemptypasswords|maxauthtries|port|allowusers|allowgroups";

/// The audit script: read-only, POSIX sh, sections marked `@@name`.
pub fn script(since: i64) -> String {
    format!(
        r#"echo '@@id'; id -un; id -u; date +%s; date +%z
echo '@@passwd'; getent passwd 2>/dev/null || cat /etc/passwd
echo '@@group'; getent group 2>/dev/null || cat /etc/group
J=""
if command -v journalctl >/dev/null 2>&1; then
  J=$(journalctl --since @{since} -o short-unix --no-pager -q _COMM=sshd _COMM=sshd-session 2>/dev/null | grep -E 'Accepted |Failed |Invalid user ' | tail -n {LOG_LINES})
fi
if [ -n "$J" ]; then echo '@@journal'; printf '%s\n' "$J"
else echo '@@authlog'; for f in /var/log/auth.log /var/log/secure; do [ -r "$f" ] && grep -hE 'sshd.*(Accepted |Failed |Invalid user )' "$f"; done | tail -n {LOG_LINES}; fi
echo '@@last'; (last -w -n {MAX_SESSIONS} 2>/dev/null || last -n {MAX_SESSIONS} 2>/dev/null) | head -n {MAX_SESSIONS}
echo '@@f2b'
if command -v fail2ban-client >/dev/null 2>&1; then
  if S=$(fail2ban-client status 2>/dev/null); then
    for j in $(printf '%s\n' "$S" | sed -n 's/.*Jail list:[[:space:]]*//p' | tr ',' ' '); do echo "@jail $j"; fail2ban-client status "$j" 2>/dev/null; done
  else echo '@denied'; fi
else echo '@missing'; fi
echo '@@cscli'
if command -v cscli >/dev/null 2>&1; then
  if C=$(cscli decisions list -o json 2>/dev/null); then printf '%s\n' "$C"; else echo '@denied'; fi
else echo '@missing'; fi
echo '@@sudoers'
if [ -r /etc/sudoers ]; then
  for f in /etc/sudoers /etc/sudoers.d/*; do [ -f "$f" ] && [ -r "$f" ] && grep -vE '^[[:space:]]*(#|$|Defaults)' "$f" | sed "s|^|$f	|"; done
else echo '@denied'; fi
echo '@@keys'
(getent passwd 2>/dev/null || cat /etc/passwd) | while IFS=: read -r n _ _ _ _ h s; do
  case "$s" in */nologin|*/false|"") continue;; esac
  for f in "$h/.ssh/authorized_keys" "$h/.ssh/authorized_keys2"; do [ -r "$f" ] && {{ echo "@user $n"; cat "$f"; echo; }}; done
done
echo '@@sshd'
if T=$(sshd -T 2>/dev/null || /usr/sbin/sshd -T 2>/dev/null); then echo '@effective'; printf '%s\n' "$T" | grep -iE '^({SSHD_KEYS}) '
else cat /etc/ssh/sshd_config.d/*.conf /etc/ssh/sshd_config 2>/dev/null | grep -iE '^[[:space:]]*({SSHD_KEYS})[[:space:]]'; fi
echo '@@end'
"#
    )
}

/// Splits script output into its sections.
fn sections(out: &str) -> HashMap<&str, Vec<&str>> {
    let mut map: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut cur: Option<&str> = None;
    for l in out.lines() {
        if let Some(name) = l.strip_prefix("@@") {
            cur = Some(name.trim());
            map.entry(name.trim()).or_default();
        } else if let Some(c) = cur {
            map.entry(c).or_default().push(l);
        }
    }
    map
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// `+0100` → seconds east of UTC.
fn parse_offset(z: &str) -> i64 {
    let z = z.trim();
    let sign = if z.starts_with('-') { -1 } else { 1 };
    let d: String = z.chars().filter(char::is_ascii_digit).collect();
    let hh = d.get(0..2).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
    let mm = d.get(2..4).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
    sign * (hh * 3600 + mm * 60)
}

/// A syslog line's timestamp and the rest: `Nov 16 05:32:55 host …` (host
/// local time, year guessed from the host's clock) or RFC 3339.
fn syslog_line(l: &str, now: i64, offset: i64) -> Option<(i64, &str)> {
    let first = l.split_whitespace().next()?;
    if first.len() >= 19 && first.as_bytes().get(4) == Some(&b'-') {
        // Drop fractional seconds so iso_offset sees the zone.
        let iso = match first.find('.') {
            Some(dot) => {
                let tail = &first[dot + 1..];
                let zone = tail.find(|c: char| !c.is_ascii_digit()).map_or("", |i| &tail[i..]);
                format!("{}{zone}", &first[..dot])
            }
            None => first.to_owned(),
        };
        let ts = iso_offset(&iso)?;
        return Some((ts, l[first.len()..].trim_start()));
    }
    let mut it = l.split_whitespace();
    let mon_word = it.next()?;
    let mon = MONTHS.iter().position(|m| *m == mon_word)? as u32 + 1;
    let day = it.next()?.parse::<u32>().ok()?;
    let hms: Vec<i64> = it.next()?.split(':').filter_map(|x| x.parse().ok()).collect();
    if hms.len() != 3 {
        return None;
    }
    let (year, _, _) = civil_from_days((now + offset).div_euclid(86_400));
    let at = |y: i64| days_from_civil(y, mon, day) * 86_400 + hms[0] * 3600 + hms[1] * 60 + hms[2] - offset;
    let mut ts = at(year);
    if ts > now + 86_400 {
        ts = at(year - 1);
    }
    // The rest after "Mon dd hh:mm:ss ".
    let rest = l.splitn(4, char::is_whitespace).filter(|s| !s.is_empty()).nth(3);
    let rest = match rest {
        Some(r) => r,
        None => {
            let idx = l.find(&format!("{:02}:{:02}:{:02}", hms[0], hms[1], hms[2]))? + 8;
            &l[idx..]
        }
    };
    Some((ts, rest.trim_start()))
}

/// `host sshd[123]: message` → message.
fn after_ident(rest: &str) -> &str {
    rest.split_once(": ").map_or(rest, |(_, m)| m)
}

#[derive(Debug, PartialEq, Eq)]
enum SshEvent {
    Accepted { user: String, from: String, method: String, key: Option<String> },
    Failed { user: String, from: String },
}

/// One sshd message. `Failed … for invalid user` is skipped because the
/// matching `Invalid user` line already counts the attempt.
fn ssh_event(msg: &str) -> Option<SshEvent> {
    let word_after = |s: &str, w: &str| -> Option<String> {
        let i = s.find(w)? + w.len();
        s[i..].split_whitespace().next().map(str::to_owned)
    };
    if let Some(rest) = msg.strip_prefix("Accepted ") {
        let method = rest.split_whitespace().next()?.to_owned();
        let user = word_after(rest, " for ")?;
        let from = word_after(rest, " from ")?;
        let key = rest.split_whitespace().find(|w| w.starts_with("SHA256:")).map(str::to_owned);
        return Some(SshEvent::Accepted { user, from, method, key });
    }
    if let Some(rest) = msg.strip_prefix("Failed ") {
        if rest.contains(" for invalid user ") {
            return None;
        }
        return Some(SshEvent::Failed { user: word_after(rest, " for ")?, from: word_after(rest, " from ")? });
    }
    if let Some(rest) = msg.strip_prefix("Invalid user ") {
        // "Invalid user  from 1.2.3.4" (empty name) has no user word.
        let from = word_after(rest, "from ")?;
        let user = rest.split(" from ").next().unwrap_or("").trim();
        return Some(SshEvent::Failed { user: if user.is_empty() { "(empty)".into() } else { user.to_owned() }, from });
    }
    None
}

fn num_after(l: &str) -> u32 {
    l.rsplit_once(':').and_then(|(_, v)| v.trim().parse().ok()).unwrap_or(0)
}

fn parse_jails(lines: &[&str]) -> Vec<Jail> {
    let mut out: Vec<Jail> = Vec::new();
    for l in lines {
        if let Some(name) = l.strip_prefix("@jail ") {
            out.push(Jail { name: name.trim().to_owned(), ..Default::default() });
            continue;
        }
        let Some(j) = out.last_mut() else { continue };
        let t = l.trim_start_matches(|c: char| c == '|' || c == '`' || c == '-' || c.is_whitespace());
        if t.starts_with("Currently failed:") {
            j.currently_failed = num_after(t);
        } else if t.starts_with("Total failed:") {
            j.total_failed = num_after(t);
        } else if t.starts_with("Currently banned:") {
            j.currently_banned = num_after(t);
        } else if t.starts_with("Total banned:") {
            j.total_banned = num_after(t);
        } else if let Some(ips) = t.strip_prefix("Banned IP list:") {
            j.banned = ips.split_whitespace().take(200).map(str::to_owned).collect();
        }
    }
    out
}

fn parse_decisions(lines: &[&str]) -> Vec<Decision> {
    let text = lines.join("\n");
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return vec![] };
    let s = |d: &serde_json::Value, k: &str| d.get(k).and_then(|x| x.as_str()).unwrap_or("").to_owned();
    let mut out = Vec::new();
    for alert in v.as_array().into_iter().flatten() {
        for d in alert.get("decisions").and_then(|x| x.as_array()).into_iter().flatten() {
            if out.len() >= MAX_DECISIONS {
                return out;
            }
            out.push(Decision {
                value: s(d, "value"),
                scenario: s(d, "scenario"),
                duration: s(d, "duration"),
                kind: s(d, "type"),
            });
        }
    }
    out
}

fn tool_state(lines: &[&str]) -> ToolState {
    match lines.first().map(|l| l.trim()) {
        Some("@missing") => ToolState::Missing,
        Some("@denied") => ToolState::Denied,
        _ => ToolState::Read,
    }
}

fn no_login(shell: &str) -> bool {
    shell.is_empty() || ["/nologin", "/false", "/sync", "/shutdown", "/halt"].iter().any(|e| shell.ends_with(e))
}

/// Builds the report from the script's output. `now` is used when the host
/// didn't print its clock.
pub fn parse(host_id: HostId, out: &str, as_root: bool, now: i64) -> AccessReport {
    let sec = sections(out);
    let get = |k: &str| sec.get(k).cloned().unwrap_or_default();
    let id = get("id");
    let login_user = id.first().map(|s| s.trim().to_owned()).unwrap_or_default();
    let host_now = id.get(2).and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(now);
    let offset = id.get(3).map_or(0, |z| parse_offset(z));
    let since = host_now - i64::from(DAYS) * 86_400;

    // Login events.
    let (log_source, raw): (LogSource, Vec<(i64, &str)>) = if let Some(j) = sec.get("journal") {
        let ev = j
            .iter()
            .filter_map(|l| {
                let (ts, rest) = l.split_once(' ')?;
                Some((ts.parse::<f64>().ok()? as i64, after_ident(rest)))
            })
            .collect();
        (LogSource::Journal, ev)
    } else {
        let a = get("authlog");
        let ev: Vec<(i64, &str)> = a
            .iter()
            .filter_map(|l| syslog_line(l, host_now, offset).map(|(ts, rest)| (ts, after_ident(rest))))
            .filter(|(ts, _)| *ts >= since)
            .collect();
        (if ev.is_empty() { LogSource::None } else { LogSource::AuthLog }, ev)
    };
    let mut logins = Vec::new();
    let mut by_src: BTreeMap<String, FailSource> = BTreeMap::new();
    let mut failed_total = 0u32;
    for (ts, msg) in raw {
        match ssh_event(msg) {
            Some(SshEvent::Accepted { user, from, method, key }) => {
                logins.push(LoginEvent { ts, user, from, method, key })
            }
            Some(SshEvent::Failed { user, from }) => {
                failed_total += 1;
                let e = by_src.entry(from.clone()).or_insert(FailSource { from, count: 0, users: vec![], last: ts });
                e.count += 1;
                e.last = e.last.max(ts);
                if e.users.len() < 5 && !e.users.contains(&user) {
                    e.users.push(user);
                }
            }
            None => {}
        }
    }
    logins.sort_by_key(|a| std::cmp::Reverse(a.ts));
    logins.truncate(MAX_LOGINS);
    let mut failures: Vec<FailSource> = by_src.into_values().collect();
    failures.sort_by(|a, b| b.count.cmp(&a.count).then(b.last.cmp(&a.last)));
    failures.truncate(MAX_SOURCES);

    let sessions: Vec<String> = get("last")
        .iter()
        .map(|l| l.trim_end())
        .filter(|l| !l.is_empty() && !l.starts_with("wtmp begins") && !l.starts_with("btmp begins"))
        .take(MAX_SESSIONS)
        .map(str::to_owned)
        .collect();

    let f2b = get("f2b");
    let cs = get("cscli");
    let fail2ban = tool_state(&f2b);
    let crowdsec = tool_state(&cs);
    let jails = if fail2ban == ToolState::Read { parse_jails(&f2b) } else { vec![] };
    let decisions = if crowdsec == ToolState::Read { parse_decisions(&cs) } else { vec![] };

    // Sudo: rules and groups.
    let sud = get("sudoers");
    let sudoers_readable = tool_state(&sud) != ToolState::Denied;
    let sudo_rules: Vec<SudoRule> = sud
        .iter()
        .filter(|l| !l.starts_with('@'))
        .filter_map(|l| l.split_once('\t'))
        .map(|(f, r)| SudoRule { file: f.to_owned(), rule: r.split_whitespace().collect::<Vec<_>>().join(" ") })
        .filter(|r| !r.rule.is_empty() && !r.rule.starts_with("@include") && !r.rule.starts_with("#include"))
        .collect();
    let mut sudo_groups: HashSet<String> = ["sudo", "wheel", "admin"].iter().map(|s| s.to_string()).collect();
    let mut sudo_users: HashSet<String> = HashSet::new();
    for r in &sudo_rules {
        let who = r.rule.split_whitespace().next().unwrap_or("");
        if let Some(g) = who.strip_prefix('%') {
            sudo_groups.insert(g.to_owned());
        } else if !who.ends_with("_Alias") {
            // Not an alias definition such as Cmnd_Alias / User_Alias.
            sudo_users.insert(who.to_owned());
        }
    }
    let mut primary: HashMap<u32, String> = HashMap::new();
    for l in get("group") {
        let f: Vec<&str> = l.split(':').collect();
        if f.len() < 4 {
            continue;
        }
        if let Ok(gid) = f[2].parse::<u32>() {
            primary.insert(gid, f[0].to_owned());
        }
        if sudo_groups.contains(f[0]) {
            sudo_users.extend(f[3].split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned));
        }
    }

    // Keys by user.
    let mut keys: Vec<UserKey> = Vec::new();
    let mut cur_user: Option<String> = None;
    let mut buf = String::new();
    let flush = |user: &Option<String>, buf: &mut String, keys: &mut Vec<UserKey>| {
        if let Some(u) = user {
            keys.extend(parse_authorized(buf).into_iter().map(|key| UserKey { user: u.clone(), key }));
        }
        buf.clear();
    };
    for l in get("keys") {
        if let Some(u) = l.strip_prefix("@user ") {
            flush(&cur_user, &mut buf, &mut keys);
            cur_user = Some(u.trim().to_owned());
        } else {
            buf.push_str(l);
            buf.push('\n');
        }
    }
    flush(&cur_user, &mut buf, &mut keys);

    let users: Vec<ShellUser> = get("passwd")
        .iter()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            if f.len() < 7 || no_login(f[6]) {
                return None;
            }
            let uid = f[2].parse::<u32>().ok()?;
            let gid = f[3].parse::<u32>().ok();
            let name = f[0].to_owned();
            let in_primary = gid.and_then(|g| primary.get(&g)).is_some_and(|g| sudo_groups.contains(g));
            let n_keys = keys.iter().filter(|k| k.user == name).count() as u32;
            Some(ShellUser {
                sudo: uid == 0 || sudo_users.contains(&name) || in_primary,
                system: uid != 0 && uid < 1000 && !(500..1000).contains(&uid),
                keys: n_keys,
                uid,
                shell: f[6].to_owned(),
                home: f[5].to_owned(),
                name,
            })
        })
        .collect();

    // sshd settings: first value of each key wins (as in sshd_config).
    let sshd_lines = get("sshd");
    let sshd_effective = sshd_lines.first().map(|l| l.trim()) == Some("@effective");
    let mut sshd: Vec<SshdSetting> = Vec::new();
    for l in sshd_lines.iter().filter(|l| !l.starts_with('@')) {
        let mut it = l.split_whitespace();
        let (Some(k), Some(v)) = (it.next(), it.next()) else { continue };
        let k = k.to_ascii_lowercase();
        let rest: Vec<&str> = std::iter::once(v).chain(it).collect();
        let multi = matches!(k.as_str(), "port" | "allowusers" | "allowgroups");
        if let Some(e) = sshd.iter_mut().find(|s| s.key == k) {
            if sshd_effective && multi {
                e.value.push(' ');
                e.value.push_str(&rest.join(" "));
            }
            continue;
        }
        sshd.push(SshdSetting { key: k, value: rest.join(" ") });
    }

    let mut r = AccessReport {
        host_id,
        at: host_now,
        as_root,
        login_user,
        days: DAYS,
        log_source,
        logins,
        failures,
        failed_total,
        sessions,
        fail2ban,
        jails,
        crowdsec,
        decisions,
        users,
        sudoers_readable,
        sudo_rules,
        keys,
        sshd_effective,
        sshd,
        findings: vec![],
    };
    r.findings = findings(&r);
    r
}

fn sshd_value<'a>(r: &'a AccessReport, key: &str) -> Option<&'a str> {
    r.sshd.iter().find(|s| s.key == key).map(|s| s.value.as_str())
}

/// What stands out, worst first.
pub fn findings(r: &AccessReport) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut add = |level, text: String| out.push(Finding { level, text });
    for u in r.users.iter().filter(|u| u.uid == 0 && u.name != "root") {
        add(FindingLevel::Bad, format!("{} has uid 0: it is root under another name", u.name));
    }
    let root_login = sshd_value(r, "permitrootlogin").map(str::to_ascii_lowercase);
    let password = sshd_value(r, "passwordauthentication").map(str::to_ascii_lowercase);
    if root_login.as_deref() == Some("yes") {
        add(FindingLevel::Bad, "sshd lets root log in with a password (PermitRootLogin yes)".into());
    }
    if sshd_value(r, "permitemptypasswords").is_some_and(|v| v.eq_ignore_ascii_case("yes")) {
        add(FindingLevel::Bad, "sshd accepts empty passwords (PermitEmptyPasswords yes)".into());
    }
    let banning = r.fail2ban != ToolState::Missing || r.crowdsec != ToolState::Missing;
    // PasswordAuthentication defaults to yes when it isn't set.
    let passwords_on = match password.as_deref() {
        Some(v) => v == "yes",
        None => r.sshd_effective || !r.sshd.is_empty(),
    };
    if passwords_on {
        if r.failed_total > 0 && !banning {
            add(
                FindingLevel::Bad,
                format!(
                    "Password logins are on, {} failed attempts in {} days, and nothing bans them",
                    r.failed_total, r.days
                ),
            );
        } else {
            add(FindingLevel::Warn, "Password logins are on (PasswordAuthentication)".into());
        }
    } else if r.failed_total >= MANY_FAILURES && !banning {
        add(
            FindingLevel::Warn,
            format!("{} failed attempts in {} days and no fail2ban or CrowdSec", r.failed_total, r.days),
        );
    }
    let pw_logins: Vec<&LoginEvent> =
        r.logins.iter().filter(|l| l.method == "password" || l.method.starts_with("keyboard-interactive")).collect();
    if let Some(l) = pw_logins.first() {
        add(
            FindingLevel::Warn,
            format!(
                "{} password login{} (latest: {} from {})",
                pw_logins.len(),
                if pw_logins.len() == 1 { "" } else { "s" },
                l.user,
                l.from
            ),
        );
    }
    let banned: u32 = r.jails.iter().map(|j| j.currently_banned).sum::<u32>() + r.decisions.len() as u32;
    if banned > 0 {
        add(FindingLevel::Info, format!("{banned} address{} banned right now", if banned == 1 { "" } else { "es" }));
    }
    for rule in r.sudo_rules.iter().filter(|s| s.rule.contains("NOPASSWD") && s.rule.trim_end().ends_with("ALL")) {
        let who = rule.rule.split_whitespace().next().unwrap_or("?");
        add(FindingLevel::Info, format!("{who} can run anything with sudo without a password"));
    }
    out.sort_by_key(|f| match f.level {
        FindingLevel::Bad => 0,
        FindingLevel::Warn => 1,
        FindingLevel::Info => 2,
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl laptop";

    fn sample(journal: bool) -> String {
        let log = if journal {
            "@@journal
1794800000.123456 box sshd[10]: Accepted publickey for roben from 100.64.0.2 port 51000 ssh2: ED25519 SHA256:abcDEF
1794800100.000000 box sshd[11]: Invalid user admin from 203.0.113.9 port 4000
1794800101.000000 box sshd[11]: Failed password for invalid user admin from 203.0.113.9 port 4000 ssh2
1794800200.000000 box sshd-session[12]: Failed password for root from 203.0.113.9 port 4001 ssh2
1794800300.000000 box sshd[13]: Failed password for root from 198.51.100.7 port 4001 ssh2
1794800400.000000 box sshd[14]: Accepted password for alice from 192.168.1.20 port 5000 ssh2"
        } else {
            "@@authlog
Nov 16 05:00:00 box sshd[10]: Accepted publickey for roben from 100.64.0.2 port 51000 ssh2: ED25519 SHA256:abcDEF
2026-11-16T05:10:00.123456+01:00 box sshd[11]: Invalid user test from 203.0.113.9 port 4000
Jan  2 05:00:00 box sshd[10]: Accepted publickey for old from 1.1.1.1 port 1 ssh2"
        };
        format!(
            "@@id
roben
1000
1794801000
+0000
@@passwd
root:x:0:0:root:/root:/bin/bash
daemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin
roben:x:1000:1000:,,,:/home/roben:/bin/bash
alice:x:1001:1001::/home/alice:/bin/zsh
toor:x:0:0::/root:/bin/sh
postgres:x:112:120::/var/lib/postgresql:/bin/bash
@@group
sudo:x:27:roben
alice:x:1001:
{log}
@@last
roben    pts/0        100.64.0.2       Mon Nov 16 05:00   still logged in

wtmp begins Sun Nov  1 00:00:01 2026
@@f2b
@jail sshd
Status for the jail: sshd
|- Filter
|  |- Currently failed:	2
|  |- Total failed:	20
|  `- File list:	/var/log/auth.log
`- Actions
   |- Currently banned:	1
   |- Total banned:	3
   `- Banned IP list:	203.0.113.9
@@cscli
[{{\"decisions\":[{{\"value\":\"198.51.100.7\",\"scenario\":\"crowdsecurity/ssh-bf\",\"duration\":\"3h59m\",\"type\":\"ban\"}}]}}]
@@sudoers
/etc/sudoers\troot ALL=(ALL:ALL) ALL
/etc/sudoers\t%sudo ALL=(ALL:ALL) ALL
/etc/sudoers.d/alice\talice ALL=(ALL) NOPASSWD: ALL
@@keys
@user roben
{KEY}
# comment
@user alice
garbage
@@sshd
@effective
port 22
permitrootlogin prohibit-password
passwordauthentication yes
@@end
"
        )
    }

    #[test]
    fn parses_the_journal_report() {
        let r = parse(3, &sample(true), true, 0);
        assert_eq!(r.login_user, "roben");
        assert_eq!(r.at, 1_794_801_000);
        assert_eq!(r.log_source, LogSource::Journal);
        assert_eq!(r.logins.len(), 2);
        assert_eq!(r.logins[0].user, "alice");
        assert_eq!(r.logins[1].key.as_deref(), Some("SHA256:abcDEF"));
        assert_eq!(r.logins[1].method, "publickey");
        // Invalid user + 2 root failures; the "for invalid user" line isn't double counted.
        assert_eq!(r.failed_total, 3);
        assert_eq!(r.failures[0].from, "203.0.113.9");
        assert_eq!(r.failures[0].count, 2);
        assert_eq!(r.failures[0].users, vec!["admin", "root"]);
        assert_eq!(r.sessions.len(), 1);
        assert_eq!(r.jails.len(), 1);
        assert_eq!((r.jails[0].currently_banned, r.jails[0].total_failed), (1, 20));
        assert_eq!(r.jails[0].banned, vec!["203.0.113.9"]);
        assert_eq!(r.decisions[0].value, "198.51.100.7");
        let names: Vec<&str> = r.users.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, vec!["root", "roben", "alice", "toor", "postgres"]);
        let u = |n: &str| r.users.iter().find(|u| u.name == n).unwrap();
        assert!(u("roben").sudo && u("alice").sudo && !u("postgres").sudo);
        assert!(u("postgres").system && !u("roben").system);
        assert_eq!(u("roben").keys, 1);
        assert_eq!(r.keys.len(), 1);
        assert_eq!(r.keys[0].key.comment, "laptop");
        assert_eq!(r.sudo_rules.len(), 3);
        assert!(r.sshd_effective);
        let texts: Vec<&str> = r.findings.iter().map(|f| f.text.as_str()).collect();
        assert!(texts[0].contains("toor has uid 0"), "{texts:?}");
        assert!(texts.iter().any(|t| t.starts_with("Password logins are on")));
        assert!(texts.iter().any(|t| t.contains("1 password login")));
        assert!(texts.iter().any(|t| t.contains("2 addresses banned")));
        assert!(texts.iter().any(|t| t.starts_with("alice can run anything")));
    }

    #[test]
    fn parses_auth_log_dates() {
        let r = parse(1, &sample(false), false, 0);
        assert_eq!(r.log_source, LogSource::AuthLog);
        // 2026-11-16 05:00:00 UTC; the January line is from last year and too old.
        assert_eq!(r.logins.len(), 1);
        assert_eq!(r.logins[0].ts, days_from_civil(2026, 11, 16) * 86_400 + 5 * 3600);
        assert_eq!(r.failures[0].last, days_from_civil(2026, 11, 16) * 86_400 + 4 * 3600 + 600);
    }

    #[test]
    fn missing_and_denied_tools() {
        let out = "@@id\nu\n1000\n100\n+0000\n@@f2b\n@denied\n@@cscli\n@missing\n@@sudoers\n@denied\n@@sshd\nPasswordAuthentication no\n@@end\n";
        let r = parse(1, out, false, 0);
        assert_eq!(r.fail2ban, ToolState::Denied);
        assert_eq!(r.crowdsec, ToolState::Missing);
        assert!(!r.sudoers_readable);
        assert_eq!(r.log_source, LogSource::None);
        assert!(!r.sshd_effective);
        assert!(r.findings.is_empty(), "{:?}", r.findings);
    }

    #[test]
    fn root_password_login_is_bad() {
        let out = "@@id\nu\n0\n100\n+0000\n@@sshd\nPermitRootLogin yes\npermitrootlogin no\n@@end\n";
        let r = parse(1, out, true, 0);
        assert_eq!(r.sshd[0].value, "yes");
        assert_eq!(r.findings[0].level, FindingLevel::Bad);
    }

    #[test]
    fn syslog_offsets_and_year() {
        // Host at UTC+1 on 2026-01-01 00:30 local; a Dec 31 line is last year.
        let now = days_from_civil(2025, 12, 31) * 86_400 + 23 * 3600 + 1800;
        let (ts, rest) = syslog_line("Dec 31 23:00:00 box sshd[1]: hi", now, 3600).unwrap();
        assert_eq!(ts, days_from_civil(2025, 12, 31) * 86_400 + 22 * 3600);
        assert_eq!(after_ident(rest), "hi");
    }

    #[test]
    fn script_is_quoted_and_sectioned() {
        let s = script(123);
        assert!(s.contains("--since @123"));
        for sec in
            ["@@id", "@@passwd", "@@journal", "@@last", "@@f2b", "@@cscli", "@@sudoers", "@@keys", "@@sshd", "@@end"]
        {
            assert!(s.contains(sec), "{sec}");
        }
    }
}
