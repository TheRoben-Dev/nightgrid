//! Read-only status page (M10h): one self-contained HTML page with every
//! host's state and a metrics snapshot, active alerts and service checks.
//!
//! The same renderer serves two things:
//! - the **phone status page**, served by the app on the LAN or Tailscale
//!   (off by default, token in the URL, no actions, refreshes itself);
//! - the **static dashboard export**, a file with the same content.
//!
//! The page never contains secrets: no tokens, passwords, env values or
//! commands. Host addresses are left out unless asked for.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HostStatus {
    pub name: String,
    /// Shown only when the export asks for addresses.
    pub address: Option<String>,
    /// "ok", "offline", "auth" (waiting for a login), "sleep" (suspended,
    /// rebooting or in its sleep window), "off" (not monitored).
    pub state: String,
    pub cpu: Option<f32>,
    pub mem: Option<f32>,
    /// Fullest filesystem, percent used, with its mount.
    pub disk: Option<(f32, String)>,
    pub temp: Option<f32>,
    pub uptime_secs: Option<f64>,
    /// Availability per day, oldest first (M10t); `None`: nothing seen that day.
    #[serde(default)]
    pub days: Vec<Option<f32>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AlertLine {
    pub critical: bool,
    pub label: String,
    pub title: String,
    pub detail: String,
    pub since: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ServiceLine {
    pub name: String,
    /// "up", "down", "unknown".
    pub state: String,
    pub latency_ms: Option<u32>,
    /// Availability per day, oldest first (M10t).
    #[serde(default)]
    pub days: Vec<Option<f32>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot {
    /// Unix seconds.
    pub generated_at: f64,
    pub app_version: String,
    pub hosts: Vec<HostStatus>,
    pub alerts: Vec<AlertLine>,
    pub services: Vec<ServiceLine>,
}

pub struct RenderOpts {
    /// `<meta refresh>` interval (the live page); `None` for a static file.
    pub refresh_secs: Option<u32>,
    /// Local time offset in minutes, for the "updated" stamp.
    pub utc_offset_min: i32,
}

pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            c if c.is_control() => {}
            c => o.push(c),
        }
    }
    o
}

fn pct(v: Option<f32>) -> String {
    v.map_or_else(|| "—".into(), |v| format!("{:.0}%", v.clamp(0.0, 100.0)))
}

fn tone(v: Option<f32>, warn: f32, bad: f32) -> &'static str {
    match v {
        Some(v) if v >= bad => "bad",
        Some(v) if v >= warn => "warn",
        Some(_) => "ok",
        None => "",
    }
}

fn bar(v: Option<f32>, warn: f32, bad: f32) -> String {
    let w = v.unwrap_or(0.0).clamp(0.0, 100.0);
    format!("<span class=\"bar\"><span class=\"fill {}\" style=\"width:{w:.0}%\"></span></span>", tone(v, warn, bad))
}

fn uptime(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    let (d, h, m) = (s / 86_400, (s % 86_400) / 3600, (s % 3600) / 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

/// Availability over the days that have data, e.g. "99.95%".
pub fn days_pct(days: &[Option<f32>]) -> Option<f32> {
    let seen: Vec<f32> = days.iter().flatten().copied().collect();
    (!seen.is_empty()).then(|| seen.iter().sum::<f32>() / seen.len() as f32)
}

/// A strip of one bar per day (M10t), with the availability over those days.
fn days_strip(days: &[Option<f32>]) -> String {
    let Some(avg) = days_pct(days) else { return String::new() };
    let mut h = String::from("<div class=\"days\" aria-label=\"daily uptime\">");
    for d in days {
        let cls = match d {
            None => "none",
            Some(p) if *p >= 99.9 => "ok",
            Some(p) if *p >= 99.0 => "warn",
            Some(_) => "bad",
        };
        let title = d.map_or_else(|| "no data".to_owned(), |p| format!("{p:.2}%"));
        h.push_str(&format!("<i class=\"{cls}\" title=\"{title}\"></i>"));
    }
    h.push_str(&format!("<span>{:.2}%</span></div>", avg));
    h
}

/// `2026-11-15 10:42` in local time.
pub fn stamp(unix: f64, offset_min: i32) -> String {
    let t = unix as i64 + i64::from(offset_min) * 60;
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);
    // Civil from days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", secs / 3600, (secs % 3600) / 60)
}

fn ago(now: f64, since: f64) -> String {
    let m = ((now - since).max(0.0) / 60.0) as u64;
    if m < 60 {
        format!("{m}m")
    } else if m < 48 * 60 {
        format!("{}h", m / 60)
    } else {
        format!("{}d", m / 1440)
    }
}

const STYLE: &str = r#"
:root{--bg0:#07080c;--bg1:#0e1118;--line:#1f2633;--t0:#e6edf3;--t1:#a9b4c2;--t2:#6b7686;--mint:#5ef2c2;--cyan:#38d6ff;--mag:#ff2e88;--warn:#ffd23f;--bad:#ff3b5c}
@media (prefers-color-scheme:light){:root{--bg0:#f4f6f8;--bg1:#ffffff;--line:#d5dbe3;--t0:#0f1720;--t1:#3c4756;--t2:#6b7686;--mint:#0a8f6a;--cyan:#0077a8;--mag:#c4105f;--warn:#9a6b00;--bad:#c81e3a}}
*{box-sizing:border-box}html,body{margin:0;background:var(--bg0);color:var(--t0);font:14px/1.45 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}
main{max-width:980px;margin:0 auto;padding:16px}
header{display:flex;align-items:baseline;gap:12px;flex-wrap:wrap;border-bottom:1px solid var(--line);padding-bottom:10px;margin-bottom:14px}
h1{font-size:18px;letter-spacing:.2em;margin:0}h1 b{color:var(--mag)}
h2{font-size:12px;letter-spacing:.18em;color:var(--cyan);margin:18px 0 8px;font-weight:600}
.muted{color:var(--t2)}.sum{margin-left:auto;font-size:12px;letter-spacing:.08em}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:10px}
.card{background:var(--bg1);border:1px solid var(--line);border-left:3px solid var(--mint);padding:10px 12px}
.card.offline,.card.auth{border-left-color:var(--bad)}.card.off{border-left-color:var(--t2)}
.card .name{font-weight:700;letter-spacing:.06em;display:flex;justify-content:space-between;gap:8px}
.state{font-size:11px;letter-spacing:.12em;color:var(--mint)}.offline .state,.auth .state{color:var(--bad)}.off .state{color:var(--t2)}
.row{display:grid;grid-template-columns:42px 44px 1fr;align-items:center;gap:6px;font-size:12px;margin-top:4px;color:var(--t1)}
.bar{height:6px;background:var(--line);display:block}.fill{display:block;height:100%;background:var(--cyan)}
.fill.warn{background:var(--warn)}.fill.bad{background:var(--bad)}
.addr{font-size:11px;color:var(--t2)}
ul{list-style:none;margin:0;padding:0;border:1px solid var(--line);background:var(--bg1)}
li{padding:7px 10px;border-bottom:1px solid var(--line);display:flex;gap:10px;align-items:baseline;flex-wrap:wrap}li:last-child{border-bottom:0}
.k{font-size:11px;letter-spacing:.1em;color:var(--warn)}.k.crit{color:var(--bad)}
.up{color:var(--mint)}.down{color:var(--bad)}
.days{display:flex;gap:1px;align-items:center;margin-top:6px;height:14px}.days i{flex:1;min-width:1px;height:100%;background:var(--mint)}
.days i.warn{background:var(--warn)}.days i.bad{background:var(--bad)}.days i.none{background:var(--line)}.days span{margin-left:6px;font-size:11px;color:var(--t1);white-space:nowrap}
li .days{flex-basis:100%}
footer{margin-top:22px;font-size:11px;color:var(--t2)}
"#;

pub fn render_html(s: &StatusSnapshot, o: &RenderOpts) -> String {
    let mut h = String::with_capacity(8192);
    let down = s.hosts.iter().filter(|x| x.state == "offline" || x.state == "auth").count();
    let crit = s.alerts.iter().filter(|a| a.critical).count();
    h.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">");
    h.push_str("<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">");
    h.push_str("<meta name=\"robots\" content=\"noindex,nofollow\"><meta name=\"referrer\" content=\"no-referrer\">");
    if let Some(r) = o.refresh_secs {
        h.push_str(&format!("<meta http-equiv=\"refresh\" content=\"{r}\">"));
    }
    h.push_str("<title>Nightgrid status</title><style>");
    h.push_str(STYLE);
    h.push_str("</style></head><body><main><header><h1><b>//</b> NIGHTGRID</h1>");
    h.push_str(&format!("<span class=\"muted\">updated {}</span>", esc(&stamp(s.generated_at, o.utc_offset_min))));
    h.push_str(&format!(
        "<span class=\"sum\">{} HOSTS · <span class=\"{}\">{} DOWN</span> · <span class=\"{}\">{} ALERTS</span></span></header>",
        s.hosts.len(),
        if down > 0 { "down" } else { "up" },
        down,
        if crit > 0 { "down" } else if s.alerts.is_empty() { "up" } else { "k" },
        s.alerts.len()
    ));

    h.push_str("<h2>// HOSTS</h2><div class=\"grid\">");
    if s.hosts.is_empty() {
        h.push_str("<p class=\"muted\">No hosts.</p>");
    }
    for x in &s.hosts {
        let word = match x.state.as_str() {
            "ok" => "ONLINE",
            "offline" => "OFFLINE",
            "auth" => "NEEDS LOGIN",
            "sleep" => "ASLEEP",
            _ => "NOT MONITORED",
        };
        h.push_str(&format!(
            "<div class=\"card {}\"><div class=\"name\"><span>{}</span><span class=\"state\">{word}</span></div>",
            esc(&x.state),
            esc(&x.name)
        ));
        if let Some(a) = &x.address {
            h.push_str(&format!("<div class=\"addr\">{}</div>", esc(a)));
        }
        if x.state == "ok" {
            h.push_str(&format!(
                "<div class=\"row\"><span>CPU</span><span>{}</span>{}</div>",
                pct(x.cpu),
                bar(x.cpu, 80.0, 95.0)
            ));
            h.push_str(&format!(
                "<div class=\"row\"><span>MEM</span><span>{}</span>{}</div>",
                pct(x.mem),
                bar(x.mem, 85.0, 95.0)
            ));
            let (dv, dm) = x.disk.as_ref().map_or((None, String::new()), |(v, m)| (Some(*v), m.clone()));
            h.push_str(&format!(
                "<div class=\"row\" title=\"{}\"><span>DSK</span><span>{}</span>{}</div>",
                esc(&dm),
                pct(dv),
                bar(dv, 85.0, 95.0)
            ));
            let mut extra = Vec::new();
            if let Some(t) = x.temp {
                extra.push(format!("{t:.0}°C"));
            }
            if let Some(u) = x.uptime_secs {
                extra.push(format!("up {}", uptime(u)));
            }
            if !extra.is_empty() {
                h.push_str(&format!("<div class=\"addr\">{}</div>", esc(&extra.join(" · "))));
            }
        }
        h.push_str(&days_strip(&x.days));
        h.push_str("</div>");
    }
    h.push_str("</div>");

    h.push_str("<h2>// ALERTS</h2>");
    if s.alerts.is_empty() {
        h.push_str("<p class=\"muted\">All clear.</p>");
    } else {
        h.push_str("<ul>");
        for a in &s.alerts {
            h.push_str(&format!(
                "<li><span class=\"k{}\">{}</span><strong>{}</strong><span class=\"muted\">{}</span><span class=\"muted\" style=\"margin-left:auto\">{}</span></li>",
                if a.critical { " crit" } else { "" },
                esc(&a.label),
                esc(&a.title),
                esc(&a.detail),
                esc(&ago(s.generated_at, a.since))
            ));
        }
        h.push_str("</ul>");
    }

    if !s.services.is_empty() {
        h.push_str("<h2>// SERVICES</h2><ul>");
        for v in &s.services {
            let cls = match v.state.as_str() {
                "up" => "up",
                "down" => "down",
                _ => "muted",
            };
            h.push_str(&format!(
                "<li><span class=\"{cls}\">{}</span><strong>{}</strong>{}{}</li>",
                esc(&v.state.to_uppercase()),
                esc(&v.name),
                v.latency_ms.map(|l| format!("<span class=\"muted\">{l} ms</span>")).unwrap_or_default(),
                days_strip(&v.days)
            ));
        }
        h.push_str("</ul>");
    }

    h.push_str(&format!(
        "<footer>Nightgrid {} · read-only{}</footer></main></body></html>",
        esc(&s.app_version),
        o.refresh_secs.map_or(" · snapshot".to_owned(), |r| format!(" · refreshes every {r} s"))
    ));
    h
}

// ---- the tiny HTTP side ------------------------------------------------------

/// What a request asked for.
#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    /// The page, with the token that came with it.
    Page {
        token: String,
    },
    Favicon,
    NotFound,
    BadMethod,
}

/// Reads the request line: `GET /s/<token> HTTP/1.1` (or `/?t=<token>`).
pub fn route(request_head: &str) -> Route {
    let line = request_head.lines().next().unwrap_or("");
    let mut w = line.split_whitespace();
    let (Some(method), Some(target)) = (w.next(), w.next()) else { return Route::NotFound };
    if method != "GET" && method != "HEAD" {
        return Route::BadMethod;
    }
    if target == "/favicon.ico" {
        return Route::Favicon;
    }
    if let Some(t) = target.strip_prefix("/s/") {
        return Route::Page { token: t.split(['?', '/', '#']).next().unwrap_or("").to_owned() };
    }
    if let Some(q) = target.strip_prefix("/?") {
        if let Some(t) = q.split('&').find_map(|p| p.strip_prefix("t=")) {
            return Route::Page { token: t.to_owned() };
        }
    }
    Route::NotFound
}

/// Constant-time comparison (the token is the only lock on the page).
pub fn token_matches(given: &str, expected: &str) -> bool {
    let (a, b) = (given.as_bytes(), expected.as_bytes());
    if a.len() != b.len() || expected.is_empty() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// A full HTTP/1.1 response with security headers.
pub fn response(status: &str, content_type: &str, body: &str, head_only: bool) -> Vec<u8> {
    let mut r = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nX-Frame-Options: DENY\r\n\
         Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; img-src data:\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    if !head_only {
        r.extend_from_slice(body.as_bytes());
    }
    r
}

/// A fresh random token (32 bytes, hex).
pub fn new_token() -> String {
    use ring::rand::SecureRandom as _;
    let mut b = [0u8; 32];
    ring::rand::SystemRandom::new().fill(&mut b).expect("system random");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap() -> StatusSnapshot {
        StatusSnapshot {
            generated_at: 1_800_000_000.0,
            app_version: "0.10.7".into(),
            hosts: vec![
                HostStatus {
                    name: "nas<script>".into(),
                    address: None,
                    state: "ok".into(),
                    cpu: Some(12.0),
                    mem: Some(91.0),
                    disk: Some((97.0, "/srv".into())),
                    temp: Some(48.0),
                    uptime_secs: Some(90_000.0),
                    days: vec![None, Some(100.0), Some(99.5), Some(80.0)],
                },
                HostStatus {
                    name: "pi".into(),
                    address: Some("pi.lan".into()),
                    state: "offline".into(),
                    cpu: None,
                    mem: None,
                    disk: None,
                    temp: None,
                    uptime_secs: None,
                    days: vec![],
                },
            ],
            alerts: vec![AlertLine {
                critical: true,
                label: "HOST OFFLINE".into(),
                title: "pi".into(),
                detail: "unreachable".into(),
                since: 1_799_999_000.0,
            }],
            services: vec![ServiceLine {
                name: "Jellyfin".into(),
                state: "up".into(),
                latency_ms: Some(42),
                days: vec![Some(100.0)],
            }],
        }
    }

    #[test]
    fn renders_escaped_self_contained_html() {
        let h = render_html(&snap(), &RenderOpts { refresh_secs: Some(30), utc_offset_min: 60 });
        assert!(h.starts_with("<!doctype html>"));
        assert!(h.contains("nas&lt;script&gt;") && !h.contains("nas<script>"));
        assert!(h.contains("http-equiv=\"refresh\" content=\"30\""));
        assert!(h.contains("1 DOWN") && !h.contains("NEEDS LOGIN"));
        assert!(h.contains("fill bad") && h.contains("fill warn"));
        assert!(h.contains("pi.lan"));
        assert!(h.contains("Jellyfin") && h.contains("42 ms"));
        assert!(h.contains("<i class=\"none\"") && h.contains("<i class=\"bad\"") && h.contains("93.17%"));
        assert!(h.contains("100.00%"));
        assert_eq!(days_pct(&[None, None]), None);
        assert!(
            !h.contains("<script") && !h.contains("http://") && !h.contains("https://"),
            "no scripts or external loads"
        );
        let s = render_html(&snap(), &RenderOpts { refresh_secs: None, utc_offset_min: 0 });
        assert!(!s.contains("refresh\" content"));
        assert!(s.contains("snapshot"));
    }

    #[test]
    fn stamps() {
        assert_eq!(stamp(0.0, 0), "1970-01-01 00:00");
        assert_eq!(stamp(1_801_483_200.0, 60), "2027-02-01 13:00");
    }

    #[test]
    fn routes_and_tokens() {
        assert_eq!(route("GET /s/abc123 HTTP/1.1\r\nHost: x\r\n"), Route::Page { token: "abc123".into() });
        assert_eq!(route("GET /?t=abc&x=1 HTTP/1.1"), Route::Page { token: "abc".into() });
        assert_eq!(route("POST /s/abc HTTP/1.1"), Route::BadMethod);
        assert_eq!(route("GET /favicon.ico HTTP/1.1"), Route::Favicon);
        assert_eq!(route("GET / HTTP/1.1"), Route::NotFound);
        assert!(token_matches("abcd", "abcd"));
        assert!(!token_matches("abce", "abcd"));
        assert!(!token_matches("", ""));
        let t = new_token();
        assert_eq!(t.len(), 64);
        assert_ne!(t, new_token());
        let r = String::from_utf8(response("200 OK", "text/html; charset=utf-8", "hi", false)).unwrap();
        assert!(r.contains("Content-Length: 2") && r.contains("no-store") && r.ends_with("\r\n\r\nhi"));
    }
}
