//! Network performance between your own hosts (M10x): `iperf3` throughput in
//! both directions plus `ping` latency, run from one host to another over
//! their SSH connections. Nightgrid starts a short-lived iperf3 server on the
//! target (with a watchdog that stops it), runs the client on the source,
//! then stops the server. Results are kept to spot a link that got slower.

use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

use crate::files::path::shell_quote;
use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

pub type NetTestId = i64;

/// Default iperf3 port.
pub const PORT: u16 = 5201;
/// Seconds the iperf3 server may live, whatever happens to Nightgrid.
pub const SERVER_TTL: u32 = 120;
/// Runs kept per host pair.
pub const KEEP: i64 = 200;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NetTest {
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub id: NetTestId,
    /// Runs the client.
    pub from_host: HostId,
    /// Runs the server.
    pub to_host: HostId,
    /// Address the client connected to.
    pub target: String,
    /// Unix seconds.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub at: i64,
    /// From → to, bits per second (as received by the server).
    pub up_bps: Option<f64>,
    /// To → from, bits per second.
    pub down_bps: Option<f64>,
    /// TCP retransmits, both directions together.
    pub retransmits: Option<u32>,
    /// Average round trip, ms.
    pub rtt_ms: Option<f64>,
    /// Round-trip spread (mdev / stddev), ms.
    pub jitter_ms: Option<f64>,
    /// Packets lost by ping, %.
    pub loss_pct: Option<f64>,
    pub error: Option<String>,
}

fn inv(field: &'static str, reason: impl Into<String>) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

/// A host name or IP address the client can connect to: letters, digits,
/// `.`, `-`, `:` (IPv6), `%` (zone) and `_`; no leading `-`.
pub fn valid_target(t: &str) -> bool {
    !t.is_empty()
        && t.len() <= 253
        && !t.starts_with('-')
        && t.chars().all(|c| c.is_ascii_alphanumeric() || ".-:%_".contains(c))
}

/// The address to use when none is typed: the target host's saved hostname,
/// unless it is an SSH alias-looking value with a user part.
pub fn default_target(hostname: &str) -> String {
    hostname.rsplit('@').next().unwrap_or(hostname).trim().to_owned()
}

/// Starts an iperf3 server in the background with a pid file and a watchdog.
pub fn server_script(port: u16) -> String {
    format!(
        "command -v iperf3 >/dev/null 2>&1 || {{ echo @@noiperf; exit 0; }}\n\
         f=\"${{TMPDIR:-/tmp}}/nightgrid-iperf-$$.pid\"\n\
         if ! out=$(iperf3 -s -p {port} -D -I \"$f\" 2>&1); then printf '%s\\n' \"$out\" | sed 's/^/@@err /'; exit 0; fi\n\
         i=0; while [ ! -s \"$f\" ] && [ $i -lt 20 ]; do sleep 0.1 2>/dev/null || sleep 1; i=$((i+1)); done\n\
         pid=$(cat \"$f\" 2>/dev/null)\n\
         [ -n \"$pid\" ] || {{ echo '@@err the iperf3 server did not start (is port {port} in use?)'; exit 0; }}\n\
         nohup sh -c \"sleep {SERVER_TTL}; kill $pid 2>/dev/null; rm -f '$f'\" >/dev/null 2>&1 &\n\
         echo \"@@server $pid $f\"\n"
    )
}

/// Stops the server started by [`server_script`].
pub fn stop_script(pid: u32, file: &str) -> String {
    format!("kill {pid} 2>/dev/null; rm -f -- {}", shell_quote(file))
}

/// Parses `@@server <pid> <file>`; refuses anything that isn't our pid file.
pub fn parse_server(out: &str) -> std::result::Result<(u32, String), String> {
    let mut errs = Vec::new();
    for line in out.lines() {
        if line.trim() == "@@noiperf" {
            return Err("iperf3 isn't installed on the target host (apt install iperf3, dnf install iperf3 …)".into());
        }
        if let Some(rest) = line.strip_prefix("@@server ") {
            let mut p = rest.splitn(2, ' ');
            let pid = p.next().and_then(|x| x.trim().parse().ok());
            let file = p.next().map(str::trim).unwrap_or("");
            let ok_file = file.contains("/nightgrid-iperf-") && file.ends_with(".pid") && !file.contains('\'');
            if let (Some(pid), true) = (pid, ok_file) {
                return Ok((pid, file.to_owned()));
            }
        }
        if let Some(e) = line.strip_prefix("@@err ") {
            errs.push(e.trim().to_owned());
        }
    }
    Err(if errs.is_empty() { "the iperf3 server didn't start".into() } else { errs.join("\n") })
}

/// Ping, then iperf3 in both directions, from the source host.
pub fn client_script(target: &str, port: u16, secs: u8) -> String {
    let t = shell_quote(target);
    let secs = secs.clamp(1, 30);
    format!(
        "command -v iperf3 >/dev/null 2>&1 || {{ echo @@noiperf; exit 0; }}\n\
         export LC_ALL=C\n\
         echo @@ping; ping -c 5 -q {t} 2>&1 | tail -n 3\n\
         sleep 1\n\
         echo @@up; iperf3 -c {t} -p {port} -t {secs} -J 2>&1\n\
         echo @@down; iperf3 -c {t} -p {port} -t {secs} -R -J 2>&1\n\
         echo @@end\n"
    )
}

/// Throughput and retransmits from `iperf3 -J`, or its error.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub bps: Option<f64>,
    pub retransmits: Option<u32>,
    pub error: Option<String>,
}

pub fn parse_iperf(json: &str) -> Run {
    let start = json.find('{');
    let v: Option<serde_json::Value> = start.and_then(|s| serde_json::from_str(&json[s..]).ok());
    let Some(v) = v else {
        let msg = json.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("no output from iperf3");
        return Run { bps: None, retransmits: None, error: Some(explain_iperf(msg)) };
    };
    if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
        return Run { bps: None, retransmits: None, error: Some(explain_iperf(e)) };
    }
    let end = &v["end"];
    // What arrived on the receiving side is the honest number.
    let bps = end["sum_received"]["bits_per_second"].as_f64().or_else(|| end["sum_sent"]["bits_per_second"].as_f64());
    let retransmits = end["sum_sent"]["retransmits"].as_u64().map(|r| r as u32);
    Run { bps, retransmits, error: bps.is_none().then(|| "iperf3 reported no result".into()) }
}

fn explain_iperf(e: &str) -> String {
    let l = e.to_lowercase();
    let hint = if l.contains("connection refused") {
        " The target's iperf3 server isn't reachable on that port: a firewall on the target may block it."
    } else if l.contains("no route to host") || l.contains("timed out") || l.contains("unable to connect") {
        " The source host can't reach that address; try the target's LAN address."
    } else if l.contains("busy") {
        " Another test is using the server; try again in a minute."
    } else {
        ""
    };
    format!("{}{hint}", e.trim())
}

/// `(avg, spread, loss %)` from ping's summary (Linux, BSD/macOS, busybox).
pub fn parse_ping(text: &str) -> (Option<f64>, Option<f64>, Option<f64>) {
    let mut loss = None;
    let mut avg = None;
    let mut spread = None;
    for line in text.lines() {
        if let Some(i) = line.find("% packet loss") {
            let head = &line[..i];
            loss = head.rsplit([' ', ',']).next().and_then(|x| x.parse().ok());
        }
        if line.contains("min/avg/max") {
            if let Some(nums) = line.split('=').nth(1) {
                let v: Vec<f64> = nums
                    .trim()
                    .trim_end_matches("ms")
                    .trim()
                    .split('/')
                    .filter_map(|x| x.trim().parse().ok())
                    .collect();
                avg = v.get(1).copied();
                spread = v.get(3).copied();
            }
        }
    }
    (avg, spread, loss)
}

/// Builds a result from the client script's output.
pub fn parse_client(from: HostId, to: HostId, target: &str, at: i64, out: &str) -> NetTest {
    let mut t = NetTest {
        id: 0,
        from_host: from,
        to_host: to,
        target: target.to_owned(),
        at,
        up_bps: None,
        down_bps: None,
        retransmits: None,
        rtt_ms: None,
        jitter_ms: None,
        loss_pct: None,
        error: None,
    };
    if out.lines().any(|l| l.trim() == "@@noiperf") {
        t.error = Some("iperf3 isn't installed on the source host (apt install iperf3, dnf install iperf3 …)".into());
        return t;
    }
    let section = |name: &str| -> Option<&str> {
        let tag = format!("@@{name}\n");
        let s = out.find(&tag)? + tag.len();
        let e = out[s..].find("\n@@").map_or(out.len(), |e| s + e);
        Some(&out[s..e])
    };
    if let Some(p) = section("ping") {
        (t.rtt_ms, t.jitter_ms, t.loss_pct) = parse_ping(p);
    }
    let mut errors = Vec::new();
    let mut retrans = None::<u32>;
    for (name, slot) in [("up", &mut t.up_bps), ("down", &mut t.down_bps)] {
        match section(name).map(parse_iperf) {
            Some(r) => {
                *slot = r.bps;
                if let Some(x) = r.retransmits {
                    retrans = Some(retrans.unwrap_or(0) + x);
                }
                if let Some(e) = r.error {
                    errors.push(format!("{}: {e}", if name == "up" { "upload" } else { "download" }));
                }
            }
            None => errors.push(format!("{name}: no output")),
        }
    }
    t.retransmits = retrans;
    if !errors.is_empty() {
        errors.dedup();
        t.error = Some(errors.join("\n"));
    }
    t
}

/// A note about a result worth a look, if any.
pub fn finding(t: &NetTest, history: &[NetTest]) -> Option<String> {
    let best = t.up_bps.into_iter().chain(t.down_bps).fold(None::<f64>, |a, b| Some(a.map_or(b, |a| a.max(b))))?;
    // ~100 Mbit/s on both directions often means a link negotiated Fast Ethernet.
    if (85e6..=100e6).contains(&best) {
        return Some(
            "About 100 Mbit/s: a link on the way may have negotiated Fast Ethernet. Check the cable and the port's speed (ethtool)."
                .into(),
        );
    }
    let mut past: Vec<f64> = history
        .iter()
        .filter(|h| h.id != t.id && h.from_host == t.from_host && h.to_host == t.to_host && h.error.is_none())
        .filter_map(|h| h.up_bps.into_iter().chain(h.down_bps).reduce(f64::max))
        .collect();
    if past.len() >= 3 {
        past.sort_by(|a, b| a.total_cmp(b));
        let median = past[past.len() / 2];
        if best < median * 0.7 {
            return Some(format!(
                "{:.0}% slower than usual for this pair (median {}).",
                (1.0 - best / median) * 100.0,
                format_bps(median)
            ));
        }
    }
    None
}

pub fn format_bps(b: f64) -> String {
    if b >= 1e9 {
        format!("{:.2} Gbit/s", b / 1e9)
    } else {
        format!("{:.0} Mbit/s", b / 1e6)
    }
}

// ---- store --------------------------------------------------------------------

const COLS: &str =
    "id, from_host, to_host, target, at, up_bps, down_bps, retransmits, rtt_ms, jitter_ms, loss_pct, error";

fn row(r: &Row<'_>) -> rusqlite::Result<NetTest> {
    Ok(NetTest {
        id: r.get(0)?,
        from_host: r.get(1)?,
        to_host: r.get(2)?,
        target: r.get(3)?,
        at: r.get(4)?,
        up_bps: r.get(5)?,
        down_bps: r.get(6)?,
        retransmits: r.get(7)?,
        rtt_ms: r.get(8)?,
        jitter_ms: r.get(9)?,
        loss_pct: r.get(10)?,
        error: r.get(11)?,
    })
}

impl Store {
    /// Saves a result and keeps the newest [`KEEP`] for its pair.
    pub fn save_net_test(&mut self, t: &NetTest) -> Result<NetTest> {
        if t.from_host == t.to_host {
            return Err(inv("to_host", "pick two different hosts"));
        }
        self.conn().execute(
            "INSERT INTO net_tests (from_host, to_host, target, at, up_bps, down_bps, retransmits, rtt_ms, jitter_ms, loss_pct, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                t.from_host,
                t.to_host,
                t.target,
                t.at,
                t.up_bps,
                t.down_bps,
                t.retransmits,
                t.rtt_ms,
                t.jitter_ms,
                t.loss_pct,
                t.error
            ],
        )?;
        let id = self.conn().last_insert_rowid();
        self.conn().execute(
            "DELETE FROM net_tests WHERE from_host = ?1 AND to_host = ?2 AND id NOT IN
             (SELECT id FROM net_tests WHERE from_host = ?1 AND to_host = ?2 ORDER BY at DESC, id DESC LIMIT ?3)",
            params![t.from_host, t.to_host, KEEP],
        )?;
        Ok(NetTest { id, ..t.clone() })
    }

    /// The newest results, newest first.
    pub fn net_tests(&self, limit: u32) -> Result<Vec<NetTest>> {
        let mut stmt =
            self.conn().prepare(&format!("SELECT {COLS} FROM net_tests ORDER BY at DESC, id DESC LIMIT ?1"))?;
        let rows = stmt.query_map([limit.min(5000)], row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_net_tests(&mut self, from: HostId, to: HostId) -> Result<usize> {
        Ok(self.conn().execute("DELETE FROM net_tests WHERE from_host = ?1 AND to_host = ?2", params![from, to])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UP: &str = r#"{"start":{},"end":{"sum_sent":{"bits_per_second":941234567.0,"retransmits":12},"sum_received":{"bits_per_second":938000000.0}}}"#;

    #[test]
    fn parses_iperf_json_and_errors() {
        let r = parse_iperf(UP);
        assert_eq!((r.bps, r.retransmits, r.error), (Some(938000000.0), Some(12), None));
        let e = parse_iperf(r#"{"start":{},"end":{},"error":"unable to connect to server: Connection refused"}"#);
        assert!(e.error.unwrap().contains("firewall"));
        let g = parse_iperf("iperf3: error - unable to connect to server: No route to host\n");
        assert!(g.error.unwrap().contains("LAN address"));
    }

    #[test]
    fn parses_ping_summaries() {
        let linux = "--- 10.0.0.2 ping statistics ---\n5 packets transmitted, 5 received, 0% packet loss, time 4005ms\nrtt min/avg/max/mdev = 0.312/0.401/0.522/0.071 ms\n";
        assert_eq!(parse_ping(linux), (Some(0.401), Some(0.071), Some(0.0)));
        let mac = "--- 10.0.0.2 ping statistics ---\n5 packets transmitted, 4 packets received, 20.0% packet loss\nround-trip min/avg/max/stddev = 1.1/2.2/3.3/0.4 ms\n";
        assert_eq!(parse_ping(mac), (Some(2.2), Some(0.4), Some(20.0)));
        let busybox =
            "5 packets transmitted, 5 packets received, 0% packet loss\nround-trip min/avg/max = 0.1/0.2/0.3 ms\n";
        assert_eq!(parse_ping(busybox), (Some(0.2), None, Some(0.0)));
    }

    #[test]
    fn builds_a_result_from_client_output() {
        let down = UP.replace("938000000.0", "512000000.0").replace("\"retransmits\":12", "\"retransmits\":3");
        let out = format!(
            "@@ping\n5 packets transmitted, 5 received, 0% packet loss, time 4005ms\nrtt min/avg/max/mdev = 0.3/0.4/0.5/0.1 ms\n@@up\n{UP}\n@@down\n{down}\n@@end\n"
        );
        let t = parse_client(1, 2, "10.0.0.2", 100, &out);
        assert_eq!((t.up_bps, t.down_bps, t.retransmits, t.rtt_ms), (Some(938e6), Some(512e6), Some(15), Some(0.4)));
        assert!(t.error.is_none());
        let t = parse_client(1, 2, "x", 100, "@@noiperf\n");
        assert!(t.error.unwrap().contains("source host"));
    }

    #[test]
    fn server_output_and_targets() {
        assert_eq!(
            parse_server("@@server 4242 /tmp/nightgrid-iperf-99.pid\n").unwrap(),
            (4242, "/tmp/nightgrid-iperf-99.pid".into())
        );
        assert!(parse_server("@@server 1 /etc/passwd\n").is_err());
        assert!(parse_server("@@noiperf\n").unwrap_err().contains("target host"));
        assert!(parse_server(
            "@@err iperf3: error - unable to start listener for connections: Address already in use\n"
        )
        .unwrap_err()
        .contains("Address already in use"));
        for ok in ["10.0.0.2", "nas.lan", "fe80::1%eth0", "server_01"] {
            assert!(valid_target(ok), "{ok}");
        }
        for bad in ["", "-x", "a b", "a;b", "$(x)"] {
            assert!(!valid_target(bad), "{bad}");
        }
        assert_eq!(default_target("admin@nas.lan"), "nas.lan");
    }

    #[test]
    fn findings_flag_fast_ethernet_and_slowdowns() {
        let t = |id, up: f64| NetTest {
            id,
            from_host: 1,
            to_host: 2,
            target: "x".into(),
            at: id,
            up_bps: Some(up),
            down_bps: Some(up * 0.9),
            retransmits: None,
            rtt_ms: None,
            jitter_ms: None,
            loss_pct: None,
            error: None,
        };
        assert!(finding(&t(1, 94e6), &[]).unwrap().contains("Fast Ethernet"));
        let hist = vec![t(1, 940e6), t(2, 930e6), t(3, 945e6)];
        assert!(finding(&t(4, 400e6), &hist).unwrap().contains("slower than usual"));
        assert!(finding(&t(4, 920e6), &hist).is_none());
    }

    /// The server and stop scripts against a fake iperf3 that daemonizes a sleep.
    #[cfg(unix)]
    #[test]
    fn server_script_starts_and_stops_with_a_fake_iperf3() {
        use std::os::unix::fs::PermissionsExt;
        let t = tempfile::tempdir().unwrap();
        let bin = t.path().join("iperf3");
        std::fs::write(
            &bin,
            "#!/bin/sh\n# -s -p PORT -D -I FILE\nf=\"$6\"\nsleep 30 >/dev/null 2>&1 &\necho $! > \"$f\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = format!("{}:{}", t.path().display(), std::env::var("PATH").unwrap_or_default());
        let run = |script: &str| {
            let o = std::process::Command::new("sh")
                .arg("-c")
                .arg(script)
                .env("PATH", &path)
                .env("TMPDIR", t.path())
                .output()
                .unwrap();
            String::from_utf8_lossy(&o.stdout).into_owned()
        };
        let out = run(&server_script(PORT));
        let (pid, file) = parse_server(&out).unwrap_or_else(|e| panic!("{e}: {out}"));
        assert!(std::path::Path::new(&file).exists());
        // Running and not a zombie (an orphan may wait to be reaped in a container).
        let alive = |pid: u32| {
            let o = std::process::Command::new("ps").args(["-o", "stat=", "-p", &pid.to_string()]).output().unwrap();
            let st = String::from_utf8_lossy(&o.stdout).trim().to_owned();
            !st.is_empty() && !st.starts_with('Z')
        };
        assert!(alive(pid));
        run(&stop_script(pid, &file));
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(!std::path::Path::new(&file).exists());
        assert!(!alive(pid));
    }

    #[test]
    fn store_keeps_results_per_pair() {
        let mut s = Store::open_in_memory().unwrap();
        s.conn().execute_batch("INSERT INTO hosts (id, name, hostname) VALUES (1, 'a', 'a'), (2, 'b', 'b');").unwrap();
        let base = parse_client(1, 2, "b", 10, "@@noiperf\n");
        let saved = s.save_net_test(&base).unwrap();
        assert!(saved.id > 0);
        assert!(s.save_net_test(&NetTest { to_host: 1, ..base.clone() }).is_err());
        assert_eq!(s.net_tests(10).unwrap().len(), 1);
        assert_eq!(s.delete_net_tests(1, 2).unwrap(), 1);
    }
}
