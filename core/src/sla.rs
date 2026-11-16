//! Uptime reports (M10t): availability, downtime and outages per host and
//! service over a period (usually a calendar month), checked against a
//! target such as 99.9%, and rendered as one self-contained HTML page that
//! prints well. Built from the uptime history (`uptime.rs`), so the same
//! rule applies: only time Nightgrid was watching counts, and sleep isn't
//! downtime.

use std::collections::HashMap;

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::depgraph::Node;
use crate::status_page::{esc, stamp};
use crate::store::Store;
use crate::uptime::availability;
use crate::Result;

/// At most this many outages are listed in a report.
pub const INCIDENTS_MAX: usize = 500;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SlaRow {
    pub node: Node,
    pub name: String,
    pub up: u32,
    pub down: u32,
    pub asleep: u32,
    /// Percent up out of up + down; `None` when nothing was seen.
    pub availability: Option<f64>,
    pub outages: u32,
    /// Seconds, inside the period.
    pub longest: u32,
    /// Whether `availability` meets the target; `None` without data.
    pub met: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SlaIncident {
    pub name: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub start: i64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub end: i64,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SlaReport {
    /// "November 2026".
    pub label: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub from: i64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub to: i64,
    /// Percent, e.g. 99.9.
    pub target: f64,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub generated_at: i64,
    /// Hosts first, then services, each by name.
    pub rows: Vec<SlaRow>,
    /// Outages inside the period, oldest first (clipped to it).
    pub incidents: Vec<SlaIncident>,
    /// True when more outages happened than are listed.
    pub incidents_cut: bool,
}

impl SlaReport {
    /// Availability over every host and service together.
    pub fn overall(&self) -> Option<f64> {
        let (up, down) = self.rows.iter().fold((0u64, 0u64), |(u, d), r| (u + u64::from(r.up), d + u64::from(r.down)));
        availability(up, down)
    }
}

pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// "45 s", "12 min", "3 h 5 min", "2 d 4 h".
pub fn dur(secs: i64) -> String {
    let s = secs.max(0);
    match s {
        s if s < 60 => format!("{s} s"),
        s if s < 3600 => format!("{} min", (s + 30) / 60),
        s if s < 86_400 => {
            let m = (s % 3600 + 30) / 60;
            if m == 0 || m == 60 {
                format!("{} h", (s + 1800) / 3600)
            } else {
                format!("{} h {m} min", s / 3600)
            }
        }
        s => {
            let h = (s % 86_400 + 1800) / 3600;
            if h == 0 || h == 24 {
                format!("{} d", (s + 43_200) / 86_400)
            } else {
                format!("{} d {h} h", s / 86_400)
            }
        }
    }
}

/// "99.95%", "100%", "87.3%", "—".
pub fn pct(p: Option<f64>) -> String {
    match p {
        None => "—".into(),
        Some(p) if p >= 100.0 => "100%".into(),
        Some(p) if p >= 99.0 => format!("{:.2}%", (p * 100.0).floor() / 100.0),
        Some(p) => format!("{p:.1}%"),
    }
}

fn kind_node(kind: &str, id: i64) -> Option<Node> {
    let id = i32::try_from(id).ok()?;
    match kind {
        "host" => Some(Node::Host(id)),
        "service" => Some(Node::Service(id)),
        _ => None,
    }
}

impl Store {
    /// Seconds up, down and asleep per host and service in `[from, to)`
    /// (whole hours: a period should start and end on an hour).
    pub fn uptime_totals(&self, from: i64, to: i64) -> Result<HashMap<Node, (u32, u32, u32)>> {
        let mut st = self.conn().prepare(
            "SELECT kind, subject, SUM(up), SUM(down), SUM(asleep) FROM uptime_hours
             WHERE hour >= ?1 AND hour < ?2 GROUP BY kind, subject",
        )?;
        let rows = st.query_map(params![from, to], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, u32>(2)?,
                r.get::<_, u32>(3)?,
                r.get::<_, u32>(4)?,
            ))
        })?;
        let mut out = HashMap::new();
        for row in rows {
            let (kind, id, up, down, asleep) = row?;
            if let Some(n) = kind_node(&kind, id) {
                out.insert(n, (up, down, asleep));
            }
        }
        Ok(out)
    }

    /// Builds a report for `[from, to)`. `subjects` are the hosts and
    /// services to include, with their names, in display order.
    pub fn sla_report(
        &self,
        subjects: &[(Node, String)],
        from: i64,
        to: i64,
        label: &str,
        target: f64,
        now: i64,
    ) -> Result<SlaReport> {
        let totals = self.uptime_totals(from, to)?;
        let mut outages = self.outages(None, from, to)?;
        outages.reverse(); // oldest first
        let names: HashMap<Node, &str> = subjects.iter().map(|(n, s)| (*n, s.as_str())).collect();
        let mut per: HashMap<Node, (u32, i64)> = HashMap::new();
        let mut incidents = Vec::new();
        for o in &outages {
            let Some(name) = names.get(&o.node) else { continue };
            let (start, end) = (o.start.max(from), o.end.min(to));
            let e = per.entry(o.node).or_default();
            e.0 += 1;
            e.1 = e.1.max(end - start);
            incidents.push(SlaIncident { name: (*name).to_owned(), start, end, detail: o.detail.clone() });
        }
        let incidents_cut = incidents.len() > INCIDENTS_MAX;
        incidents.truncate(INCIDENTS_MAX);
        let rows = subjects
            .iter()
            .map(|(n, name)| {
                let (up, down, asleep) = totals.get(n).copied().unwrap_or_default();
                let a = availability(up.into(), down.into());
                let (count, longest) = per.get(n).copied().unwrap_or_default();
                SlaRow {
                    node: *n,
                    name: name.clone(),
                    up,
                    down,
                    asleep,
                    availability: a,
                    outages: count,
                    longest: u32::try_from(longest.max(0)).unwrap_or(u32::MAX),
                    met: a.map(|a| a + 1e-9 >= target),
                }
            })
            .collect();
        Ok(SlaReport { label: label.to_owned(), from, to, target, generated_at: now, rows, incidents, incidents_cut })
    }
}

const STYLE: &str = r#"
:root{--bg:#07080c;--card:#0e1118;--line:#1f2633;--t0:#e6edf3;--t1:#a9b4c2;--t2:#6b7686;--ok:#5ef2c2;--cyan:#38d6ff;--mag:#ff2e88;--warn:#ffd23f;--bad:#ff3b5c}
@media (prefers-color-scheme:light){:root{--bg:#f4f6f8;--card:#fff;--line:#d5dbe3;--t0:#0f1720;--t1:#3c4756;--t2:#6b7686;--ok:#0a8f6a;--cyan:#0077a8;--mag:#c4105f;--warn:#9a6b00;--bad:#c81e3a}}
@media print{:root{--bg:#fff;--card:#fff;--line:#ccc;--t0:#000;--t1:#333;--t2:#666;--ok:#0a7a5a;--cyan:#005f86;--mag:#a00d4e;--warn:#8a5f00;--bad:#b0102c}}
*{box-sizing:border-box}html,body{margin:0;background:var(--bg);color:var(--t0);font:13px/1.45 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}
main{max-width:1000px;margin:0 auto;padding:20px 16px}
header{border-bottom:1px solid var(--line);padding-bottom:10px;margin-bottom:14px}
h1{font-size:18px;letter-spacing:.18em;margin:0}h1 b{color:var(--mag)}
h2{font-size:12px;letter-spacing:.18em;color:var(--cyan);margin:22px 0 8px;font-weight:600}
.muted{color:var(--t2)}
.sum{display:grid;grid-template-columns:repeat(auto-fit,minmax(160px,1fr));gap:10px;margin-top:12px}
.sum div{background:var(--card);border:1px solid var(--line);padding:10px 12px}.sum b{display:block;font-size:20px;margin-top:2px}
table{width:100%;border-collapse:collapse;background:var(--card);border:1px solid var(--line)}
th,td{text-align:left;padding:6px 8px;border-bottom:1px solid var(--line);vertical-align:top}
th{font-size:11px;letter-spacing:.1em;color:var(--t1);font-weight:600}td.n{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}
.ok{color:var(--ok)}.warn{color:var(--warn)}.bad{color:var(--bad)}
.k{font-size:10px;letter-spacing:.1em;color:var(--cyan)}
footer{margin-top:22px;font-size:11px;color:var(--t2)}
"#;

fn tone(p: Option<f64>, target: f64) -> &'static str {
    match p {
        None => "muted",
        Some(p) if p + 1e-9 >= target => "ok",
        Some(p) if p >= 99.0 => "warn",
        Some(_) => "bad",
    }
}

/// The report as one self-contained HTML page (no scripts, no external loads).
pub fn render_html(r: &SlaReport, utc_offset_min: i32) -> String {
    let mut h = String::with_capacity(16_384);
    let period = (r.to - r.from).max(1);
    let overall = r.overall();
    let with_data: Vec<&SlaRow> = r.rows.iter().filter(|x| x.availability.is_some()).collect();
    let met = with_data.iter().filter(|x| x.met == Some(true)).count();
    let total_down: i64 = r.rows.iter().map(|x| i64::from(x.down)).sum();
    h.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">");
    h.push_str("<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">");
    h.push_str("<meta name=\"robots\" content=\"noindex,nofollow\"><meta name=\"referrer\" content=\"no-referrer\">");
    h.push_str(&format!("<title>Uptime report · {}</title><style>{STYLE}</style></head><body><main>", esc(&r.label)));
    h.push_str(&format!(
        "<header><h1><b>//</b> UPTIME REPORT · {}</h1><div class=\"muted\">{} to {} · target {}</div>",
        esc(&r.label.to_uppercase()),
        esc(&stamp(r.from as f64, utc_offset_min)),
        esc(&stamp(r.to as f64, utc_offset_min)),
        esc(&pct(Some(r.target)))
    ));
    h.push_str("<div class=\"sum\">");
    h.push_str(&format!(
        "<div><span class=\"k\">AVAILABILITY</span><b class=\"{}\">{}</b></div>",
        tone(overall, r.target),
        esc(&pct(overall))
    ));
    h.push_str(&format!(
        "<div><span class=\"k\">MET TARGET</span><b class=\"{}\">{met} of {}</b></div>",
        if met == with_data.len() { "ok" } else { "bad" },
        with_data.len()
    ));
    h.push_str(&format!("<div><span class=\"k\">OUTAGES</span><b>{}</b></div>", r.incidents.len()));
    h.push_str(&format!("<div><span class=\"k\">DOWNTIME</span><b>{}</b></div>", esc(&dur(total_down))));
    h.push_str("</div></header>");

    h.push_str("<h2>// BY HOST AND SERVICE</h2><table><thead><tr><th>NAME</th><th>KIND</th><th class=\"n\">AVAILABILITY</th><th class=\"n\">DOWNTIME</th><th class=\"n\">OUTAGES</th><th class=\"n\">LONGEST</th><th class=\"n\">WATCHED</th><th>TARGET</th></tr></thead><tbody>");
    for x in &r.rows {
        let watched = f64::from(x.up + x.down + x.asleep) * 100.0 / period as f64;
        h.push_str(&format!(
            "<tr><td>{}</td><td class=\"muted\">{}</td><td class=\"n {}\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n muted\">{:.0}%</td><td class=\"{}\">{}</td></tr>",
            esc(&x.name),
            match x.node {
                Node::Host(_) => "host",
                Node::Service(_) => "service",
            },
            tone(x.availability, r.target),
            esc(&pct(x.availability)),
            if x.down > 0 { esc(&dur(x.down.into())) } else { "—".into() },
            x.outages,
            if x.longest > 0 { esc(&dur(x.longest.into())) } else { "—".into() },
            watched.min(100.0),
            match x.met {
                Some(true) => "ok",
                Some(false) => "bad",
                None => "muted",
            },
            match x.met {
                Some(true) => "met",
                Some(false) => "missed",
                None => "no data",
            }
        ));
    }
    if r.rows.is_empty() {
        h.push_str("<tr><td colspan=\"8\" class=\"muted\">No hosts or services.</td></tr>");
    }
    h.push_str("</tbody></table>");

    h.push_str("<h2>// OUTAGES</h2>");
    if r.incidents.is_empty() {
        h.push_str("<p class=\"muted\">None.</p>");
    } else {
        h.push_str("<table><thead><tr><th>STARTED</th><th>WHAT</th><th class=\"n\">LASTED</th><th>FIRST ERROR</th></tr></thead><tbody>");
        for i in &r.incidents {
            h.push_str(&format!(
                "<tr><td class=\"n\">{}</td><td>{}</td><td class=\"n bad\">{}</td><td class=\"muted\">{}</td></tr>",
                esc(&stamp(i.start as f64, utc_offset_min)),
                esc(&i.name),
                esc(&dur(i.end - i.start)),
                esc(&i.detail)
            ));
        }
        h.push_str("</tbody></table>");
        if r.incidents_cut {
            h.push_str(&format!("<p class=\"muted\">Only the first {INCIDENTS_MAX} outages are listed.</p>"));
        }
    }
    h.push_str(&format!(
        "<footer>Availability is time up out of time up or down while Nightgrid was watching; time it wasn't running is left out (WATCHED), and planned sleep isn't downtime. Generated {} by Nightgrid.</footer></main></body></html>",
        esc(&stamp(r.generated_at as f64, utc_offset_min))
    ));
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uptime::{Observation, Recorder, Seen};

    fn obs(n: Node, seen: Seen) -> Observation {
        Observation { node: n, seen, detail: "HTTP 502 <bad>".into() }
    }

    #[test]
    fn builds_and_renders_a_report() {
        let mut s = Store::open_in_memory().unwrap();
        let (h, v) = (Node::Host(1), Node::Service(2));
        let mut r = Recorder::default();
        let t0 = 10 * 86_400;
        let mut t = t0;
        r.tick(&mut s, t, &[obs(h, Seen::Up), obs(v, Seen::Up)]).unwrap();
        for i in 1..=100 {
            t += 30;
            let sv = if (40..44).contains(&i) { Seen::Down } else { Seen::Up };
            r.tick(&mut s, t, &[obs(h, Seen::Up), obs(v, sv)]).unwrap();
        }
        let subjects = vec![(h, "server01".to_owned()), (v, "Photos".to_owned()), (Node::Host(9), "new".to_owned())];
        let rep = s.sla_report(&subjects, t0, t0 + 86_400, "January 1970", 99.9, t).unwrap();
        assert_eq!(rep.rows[0].availability, Some(100.0));
        assert_eq!(rep.rows[0].met, Some(true));
        assert_eq!((rep.rows[1].down, rep.rows[1].outages, rep.rows[1].longest), (120, 1, 120));
        assert_eq!(rep.rows[1].availability, Some(96.0));
        assert_eq!(rep.rows[1].met, Some(false));
        assert_eq!(rep.rows[2].availability, None);
        assert_eq!(rep.incidents.len(), 1);
        assert_eq!(rep.incidents[0].name, "Photos");
        assert!((rep.overall().unwrap() - 98.0).abs() < 1e-9);
        // A period that starts mid-outage clips it.
        let late = s.sla_report(&subjects, t0 + 41 * 30, t0 + 86_400, "x", 99.0, t).unwrap();
        assert_eq!(late.incidents[0].start, t0 + 41 * 30);

        let html = render_html(&rep, 60);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("UPTIME REPORT · JANUARY 1970"));
        assert!(html.contains("96.0%") && html.contains("100%") && html.contains("missed") && html.contains("no data"));
        assert!(html.contains("1 of 2"));
        assert!(html.contains("HTTP 502 &lt;bad&gt;") && !html.contains("<bad>"));
        assert!(!html.contains("<script") && !html.contains("http://") && !html.contains("https://"));
    }

    #[test]
    fn words() {
        assert_eq!(dur(42), "42 s");
        assert_eq!(dur(720), "12 min");
        assert_eq!(dur(3 * 3600 + 300), "3 h 5 min");
        assert_eq!(dur(7200), "2 h");
        assert_eq!(dur(2 * 86_400 + 4 * 3600), "2 d 4 h");
        assert_eq!(pct(Some(99.999)), "99.99%");
        assert_eq!(pct(Some(100.0)), "100%");
        assert_eq!(pct(Some(87.34)), "87.3%");
        assert_eq!(pct(None), "—");
    }
}
