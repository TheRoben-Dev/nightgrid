//! Shell history search (M10e, opt-in). Reads the login user's bash, zsh and
//! fish history files (last few thousand lines each), keeps the commands that
//! match the query and drops anything that looks like it carries a secret.
//! Nothing is stored; results live only in the UI while the search is open.

use serde::Serialize;

use super::{SystemError, SystemResult};
use crate::redact::line_has_secret;
use crate::ssh::client::Connection;

/// Lines read from the end of each history file.
pub const TAIL: usize = 5000;
/// Results returned at most.
pub const MAX_RESULTS: usize = 200;
const MARK: &str = "@@NGHIST@@";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub command: String,
    /// `bash`, `zsh` or `fish`.
    pub shell: String,
    /// Unix seconds, when the history file records it.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub when: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryResult {
    /// Newest first, duplicates folded into their latest use.
    pub entries: Vec<HistoryEntry>,
    /// Shells whose history file was found.
    pub shells: Vec<String>,
    /// Matching lines left out because they looked like they held a secret.
    pub hidden: u32,
    /// More matches than [`MAX_RESULTS`].
    pub truncated: bool,
}

pub fn read_command() -> String {
    super::sudo::sh(&format!(
        r#"for p in "bash:$HOME/.bash_history" "zsh:${{HISTFILE:-$HOME/.zsh_history}}" "zsh:$HOME/.zhistory" "fish:${{XDG_DATA_HOME:-$HOME/.local/share}}/fish/fish_history"; do
  f=${{p#*:}}
  if [ -r "$f" ] && [ -f "$f" ]; then echo "{MARK} ${{p%%:*}}"; tail -n {TAIL} "$f" 2>/dev/null; fi
done
echo "{MARK} end""#
    ))
}

pub async fn search(conn: &Connection, query: &str) -> SystemResult<HistoryResult> {
    let (_, out, _) = conn.exec(&read_command()).await.map_err(SystemError::other)?;
    Ok(filter(&parse(&String::from_utf8_lossy(&out)), query))
}

/// All commands in file order (oldest first within each shell).
pub fn parse(out: &str) -> Vec<HistoryEntry> {
    let mut v = Vec::new();
    let mut shell = "";
    let mut bash_ts: Option<i64> = None;
    let mut fish_cmd: Option<usize> = None;
    for line in out.lines() {
        if let Some(rest) = line.strip_prefix(MARK) {
            shell = match rest.trim() {
                "bash" => "bash",
                "zsh" => "zsh",
                "fish" => "fish",
                _ => "",
            };
            bash_ts = None;
            fish_cmd = None;
            continue;
        }
        match shell {
            "bash" => {
                // HISTTIMEFORMAT writes "#<unix>" before each command.
                if let Some(ts) = line.strip_prefix('#').and_then(|t| t.trim().parse::<i64>().ok()) {
                    bash_ts = Some(ts);
                    continue;
                }
                push(&mut v, line, "bash", bash_ts.take());
            }
            "zsh" => {
                // Extended history: ": 1700000000:0;command".
                if let Some(rest) = line.strip_prefix(": ") {
                    if let Some((meta, cmd)) = rest.split_once(';') {
                        let ts = meta.split(':').next().and_then(|t| t.trim().parse().ok());
                        push(&mut v, cmd, "zsh", ts);
                        continue;
                    }
                }
                push(&mut v, line, "zsh", None);
            }
            "fish" => {
                if let Some(cmd) = line.strip_prefix("- cmd: ") {
                    let cmd = cmd.replace("\\n", "\n").replace("\\\\", "\\");
                    push(&mut v, &cmd, "fish", None);
                    fish_cmd = Some(v.len() - 1);
                } else if let Some(ts) = line.trim_start().strip_prefix("when: ") {
                    if let (Some(i), Ok(ts)) = (fish_cmd.take(), ts.trim().parse()) {
                        if let Some(e) = v.get_mut(i) {
                            e.when = Some(ts);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    v
}

fn push(v: &mut Vec<HistoryEntry>, cmd: &str, shell: &str, when: Option<i64>) {
    let cmd = cmd.trim();
    if cmd.is_empty() || cmd.len() > 2000 {
        return;
    }
    v.push(HistoryEntry { command: cmd.to_owned(), shell: shell.to_owned(), when });
}

/// Matches every word of `query` (case-insensitive), newest first, deduped.
pub fn filter(all: &[HistoryEntry], query: &str) -> HistoryResult {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut shells: Vec<String> = Vec::new();
    for e in all {
        if !shells.contains(&e.shell) {
            shells.push(e.shell.clone());
        }
    }
    let mut entries: Vec<HistoryEntry> = Vec::new();
    let mut hidden = 0u32;
    let mut truncated = false;
    // Later lines are newer; walk backwards and keep the first sighting.
    // (Shells are read in a fixed order and most files carry no timestamps,
    // so results are newest first per shell, the last-read shell first.)
    for e in all.iter().rev() {
        let lower = e.command.to_lowercase();
        if !words.iter().all(|w| lower.contains(w.as_str())) {
            continue;
        }
        if line_has_secret(&e.command) {
            hidden += 1;
            continue;
        }
        if entries.iter().any(|x| x.command == e.command) {
            continue;
        }
        if entries.len() == MAX_RESULTS {
            truncated = true;
            break;
        }
        entries.push(e.clone());
    }
    HistoryResult { entries, shells, hidden, truncated }
}

/// Commands that mention any of `terms` (case-insensitive), most matching
/// terms first and newest first among equals, without secrets or duplicates.
/// DRIFT uses this to look deeper than the recent lines for what a question
/// is about (M10p).
pub fn relevant(all: &[HistoryEntry], terms: &[String], max: usize) -> Vec<HistoryEntry> {
    let terms: Vec<String> = terms.iter().map(|t| t.trim().to_lowercase()).filter(|t| t.len() >= 3).collect();
    if terms.is_empty() || max == 0 {
        return Vec::new();
    }
    let mut hits: Vec<(usize, usize, &HistoryEntry)> = Vec::new();
    for (i, e) in all.iter().enumerate().rev() {
        let lower = e.command.to_lowercase();
        let n = terms.iter().filter(|t| lower.contains(t.as_str())).count();
        if n == 0 || line_has_secret(&e.command) || hits.iter().any(|(_, _, x)| x.command == e.command) {
            continue;
        }
        hits.push((n, i, e));
    }
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    hits.into_iter().take(max).map(|(_, _, e)| e.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUT: &str = "@@NGHIST@@ bash
ls -la
#1700000100
docker compose up -d
export GITHUB_TOKEN=ghp_0123456789abcdef
ls -la
@@NGHIST@@ zsh
: 1700000200:0;docker ps
plain zsh line
@@NGHIST@@ fish
- cmd: docker logs -f web
  when: 1700000300
- cmd: echo a\\nb
@@NGHIST@@ end
";

    #[test]
    fn finds_relevant_lines() {
        let all = parse(OUT);
        let got: Vec<_> = relevant(&all, &["docker".into(), "logs".into()], 5).into_iter().map(|e| e.command).collect();
        assert_eq!(got, vec!["docker logs -f web", "docker ps", "docker compose up -d"]);
        assert!(relevant(&all, &["github".into()], 5).is_empty(), "secret lines stay out");
        assert!(relevant(&all, &["ls".into()], 5).is_empty(), "too-short terms are ignored");
    }

    #[test]
    fn parses_all_shells() {
        let all = parse(OUT);
        let cmds: Vec<_> = all.iter().map(|e| e.command.as_str()).collect();
        assert_eq!(
            cmds,
            vec![
                "ls -la",
                "docker compose up -d",
                "export GITHUB_TOKEN=ghp_0123456789abcdef",
                "ls -la",
                "docker ps",
                "plain zsh line",
                "docker logs -f web",
                "echo a\nb"
            ]
        );
        assert_eq!(all[1].when, Some(1_700_000_100));
        assert_eq!(all[4].when, Some(1_700_000_200));
        assert_eq!(all[6].when, Some(1_700_000_300));
    }

    #[test]
    fn filters_dedupes_and_hides_secrets() {
        let all = parse(OUT);
        let r = filter(&all, "DOCKER");
        let cmds: Vec<_> = r.entries.iter().map(|e| e.command.as_str()).collect();
        assert_eq!(cmds, vec!["docker logs -f web", "docker ps", "docker compose up -d"]);
        assert_eq!(r.shells, vec!["bash", "zsh", "fish"]);
        assert_eq!(r.hidden, 0);

        let r = filter(&all, "export");
        assert!(r.entries.is_empty());
        assert_eq!(r.hidden, 1);

        let r = filter(&all, "ls");
        assert_eq!(r.entries.iter().filter(|e| e.command == "ls -la").count(), 1);
    }
}

#[cfg(test)]
mod script_tests {
    #[test]
    fn script_runs_under_sh() {
        let out = std::process::Command::new("sh").arg("-c").arg(super::read_command()).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert!(String::from_utf8_lossy(&out.stdout).contains("@@NGHIST@@ end"));
    }
}
