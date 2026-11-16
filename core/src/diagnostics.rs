//! "Export diagnostics": one plain-text report to attach to a bug report or
//! hand to someone helping. Secrets never go in: settings hold none (tokens
//! and passwords live in the keychain), and any log line that looks like it
//! carries one is replaced, as are URL passwords and bearer tokens.

use std::fmt::Write as _;

use crate::redact::line_has_secret;

/// Log lines kept per file.
pub const LOG_TAIL: usize = 1500;

/// What goes into the report. Each part is already gathered by the app.
#[derive(Debug, Default)]
pub struct Input {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub schema: i64,
    pub keychain: String,
    /// When the report was made (local time, readable).
    pub generated: String,
    pub counts: Vec<(String, usize)>,
    /// The settings as JSON.
    pub settings: String,
    /// Active and recent alerts, one per line.
    pub alerts: Vec<String>,
    /// (file name, contents) of the newest app log files.
    pub logs: Vec<(String, String)>,
}

/// One line with anything secret-looking taken out.
pub fn clean_line(line: &str) -> String {
    if line_has_secret(line) {
        return "[line removed: it looked like it held a password or token]".into();
    }
    line.to_owned()
}

pub fn report(i: &Input) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "NIGHTGRID DIAGNOSTICS");
    let _ = writeln!(out, "Generated: {}", i.generated);
    let _ = writeln!(out, "Version: {} ({}/{})", i.version, i.os, i.arch);
    let _ = writeln!(out, "Database schema: v{}", i.schema);
    let _ = writeln!(out, "Keychain: {}", i.keychain);
    let _ = writeln!(out, "Secrets: none included. Lines that looked like they held one were removed.");
    if !i.counts.is_empty() {
        let c: Vec<String> = i.counts.iter().map(|(k, n)| format!("{k} {n}")).collect();
        let _ = writeln!(out, "Counts: {}", c.join(", "));
    }
    let _ = writeln!(out, "\n== SETTINGS ==");
    for l in i.settings.lines() {
        let _ = writeln!(out, "{}", clean_line(l));
    }
    let _ = writeln!(out, "\n== ALERTS ==");
    if i.alerts.is_empty() {
        let _ = writeln!(out, "(none)");
    }
    for a in &i.alerts {
        let _ = writeln!(out, "{}", clean_line(a));
    }
    for (name, text) in &i.logs {
        let lines: Vec<&str> = text.lines().collect();
        let skip = lines.len().saturating_sub(LOG_TAIL);
        let _ = writeln!(out, "\n== LOG {name} (last {} of {} lines) ==", lines.len() - skip, lines.len());
        for l in &lines[skip..] {
            let _ = writeln!(out, "{}", clean_line(l));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_never_reach_the_report() {
        let r = report(&Input {
            version: "0.10.53".into(),
            os: "macos".into(),
            arch: "aarch64".into(),
            schema: 34,
            keychain: "available".into(),
            generated: "2026-11-16 19:00".into(),
            counts: vec![("hosts".into(), 4)],
            settings: "{\n  \"theme\": \"nightgrid\"\n}".into(),
            alerts: vec!["NEW ERROR · convertx on Lumen · error: NOT_FOUND".into()],
            logs: vec![(
                "nightgrid.2026-11-16.log".into(),
                "INFO started\nDEBUG connecting with password=hunter22\nGET https://admin:Tr0ub4dor99@example.com/x\nAuthorization: Bearer abcdef0123456789abcdef\nINFO done".into(),
            )],
        });
        assert!(r.contains("Version: 0.10.53 (macos/aarch64)"));
        assert!(r.contains("\"theme\": \"nightgrid\""));
        assert!(r.contains("convertx on Lumen"));
        assert!(r.contains("INFO done"));
        for s in ["hunter22", "Tr0ub4dor99", "abcdef0123456789abcdef"] {
            assert!(!r.contains(s), "{s} leaked");
        }
        assert_eq!(r.matches("[line removed").count(), 3);
    }

    #[test]
    fn long_logs_keep_the_tail() {
        let text: String = (0..LOG_TAIL + 10).map(|i| format!("line {i}\n")).collect();
        let r = report(&Input { logs: vec![("a.log".into(), text)], ..Default::default() });
        assert!(r.contains(&format!("(last {LOG_TAIL} of {} lines)", LOG_TAIL + 10)));
        assert!(!r.contains("line 9\n"));
        assert!(r.contains(&format!("line {}\n", LOG_TAIL + 9)));
    }
}
