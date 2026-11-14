//! Read-only `~/.ssh/config` parser for importing hosts.
//!
//! Supports `Host` blocks with patterns and negation, `Include` (with `*`/`?`
//! globs, relative to `~/.ssh`), `Key value` and `Key=value` syntax, quoted
//! values and comments. `Match` blocks are skipped (their conditions depend on
//! runtime state). Resolution is first-value-wins across all matching blocks,
//! as in OpenSSH. Nothing here ever writes to the user's config.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// One `Host` alias resolved to the options NIGHTGRID imports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SshConfigHost {
    pub alias: String,
    pub hostname: String,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub proxy_jump: Option<String>,
}

#[derive(Debug, Default)]
struct Block {
    patterns: Vec<String>,
    /// (lowercase key, value) in file order.
    options: Vec<(String, String)>,
}

const MAX_INCLUDE_DEPTH: usize = 8;

/// Splits a config line into (key, args). Handles `Key=value`, quotes and comments.
fn split_line(line: &str) -> Option<(String, Vec<String>)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (key, rest) = match line.find(|c: char| c.is_whitespace() || c == '=') {
        Some(i) => (&line[..i], &line[i..]),
        None => (line, ""),
    };
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest).trim_start();
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut has = false;
    for ch in rest.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                has = true;
            }
            '#' if !in_quotes => break,
            c if c.is_whitespace() && !in_quotes => {
                if has {
                    args.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            c => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has {
        args.push(cur);
    }
    Some((key.to_ascii_lowercase(), args))
}

/// OpenSSH-style wildcard match (`*`, `?`), case-insensitive.
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    fn rec(p: &[u8], t: &[u8]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some(b'*'), _) => rec(&p[1..], t) || (!t.is_empty() && rec(p, &t[1..])),
            (Some(b'?'), Some(_)) => rec(&p[1..], &t[1..]),
            (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => rec(&p[1..], &t[1..]),
            _ => false,
        }
    }
    rec(pattern.as_bytes(), text.as_bytes())
}

fn host_matches(patterns: &[String], alias: &str) -> bool {
    let mut matched = false;
    for p in patterns {
        if let Some(neg) = p.strip_prefix('!') {
            if wildcard_match(neg, alias) {
                return false;
            }
        } else if wildcard_match(p, alias) {
            matched = true;
        }
    }
    matched
}

fn is_concrete(pattern: &str) -> bool {
    !pattern.contains(['*', '?', '!'])
}

pub struct SshConfigParser {
    home: PathBuf,
    blocks: Vec<Block>,
    seen: HashSet<PathBuf>,
}

impl SshConfigParser {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self {
            home: home.into(),
            blocks: vec![Block { patterns: vec!["*".into()], ..Default::default() }],
            seen: HashSet::new(),
        }
    }

    fn ssh_dir(&self) -> PathBuf {
        self.home.join(".ssh")
    }

    fn expand_tilde(&self, p: &str) -> PathBuf {
        if p == "~" {
            self.home.clone()
        } else if let Some(rest) = p.strip_prefix("~/") {
            self.home.join(rest)
        } else {
            PathBuf::from(p)
        }
    }

    fn include_paths(&self, pattern: &str) -> Vec<PathBuf> {
        let p = self.expand_tilde(pattern);
        let p = if p.is_absolute() { p } else { self.ssh_dir().join(p) };
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !name.contains(['*', '?']) {
            return vec![p];
        }
        let Some(dir) = p.parent() else { return vec![] };
        let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| wildcard_match(&name, &e.file_name().to_string_lossy()))
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        out.sort();
        out
    }

    /// Parses a file (and its includes). Missing files are ignored.
    pub fn parse_file(&mut self, path: &Path) -> &mut Self {
        self.parse_file_depth(path, 0);
        self
    }

    fn parse_file_depth(&mut self, path: &Path, depth: usize) {
        if depth > MAX_INCLUDE_DEPTH || !self.seen.insert(path.to_owned()) {
            return;
        }
        if let Ok(text) = std::fs::read_to_string(path) {
            self.parse_str_depth(&text, depth);
        }
    }

    pub fn parse_str(&mut self, text: &str) -> &mut Self {
        self.parse_str_depth(text, 0);
        self
    }

    fn parse_str_depth(&mut self, text: &str, depth: usize) {
        // `skipping` is true inside a Match block.
        let mut skipping = false;
        for line in text.lines() {
            let Some((key, args)) = split_line(line) else { continue };
            match key.as_str() {
                "host" => {
                    skipping = false;
                    self.blocks.push(Block { patterns: args, options: vec![] });
                }
                "match" => skipping = true,
                "include" => {
                    for a in &args {
                        for p in self.include_paths(a) {
                            self.parse_file_depth(&p, depth + 1);
                        }
                    }
                }
                _ if skipping => {}
                _ => {
                    let value = args.join(" ");
                    if let Some(b) = self.blocks.last_mut() {
                        b.options.push((key, value));
                    }
                }
            }
        }
    }

    fn lookup(&self, alias: &str, key: &str) -> Option<String> {
        self.blocks
            .iter()
            .filter(|b| host_matches(&b.patterns, alias))
            .flat_map(|b| b.options.iter())
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }

    /// Concrete aliases (no wildcards), in file order, deduplicated.
    pub fn aliases(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        self.blocks
            .iter()
            .flat_map(|b| b.patterns.iter())
            .filter(|p| is_concrete(p))
            .filter(|p| seen.insert(p.to_lowercase()))
            .cloned()
            .collect()
    }

    fn expand_tokens(&self, s: &str, alias: &str, hostname: &str, user: Option<&str>, port: Option<u16>) -> String {
        let mut out = String::new();
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c != '%' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('%') => out.push('%'),
                Some('h') => out.push_str(hostname),
                Some('n') => out.push_str(alias),
                Some('r') => out.push_str(user.unwrap_or("")),
                Some('p') => out.push_str(&port.unwrap_or(22).to_string()),
                Some('d') => out.push_str(&self.home.to_string_lossy()),
                Some(other) => {
                    out.push('%');
                    out.push(other);
                }
                None => out.push('%'),
            }
        }
        out
    }

    pub fn resolve(&self, alias: &str) -> SshConfigHost {
        let hostname = self.lookup(alias, "hostname").map(|h| h.replace("%h", alias)).unwrap_or_else(|| alias.into());
        let user = self.lookup(alias, "user");
        let port = self.lookup(alias, "port").and_then(|p| p.parse().ok());
        let identity_file = self
            .lookup(alias, "identityfile")
            .filter(|f| !f.eq_ignore_ascii_case("none"))
            .map(|f| self.expand_tokens(&f, alias, &hostname, user.as_deref(), port))
            .map(|f| self.expand_tilde(&f).to_string_lossy().into_owned());
        let proxy_jump = self.lookup(alias, "proxyjump").filter(|j| !j.eq_ignore_ascii_case("none"));
        SshConfigHost { alias: alias.into(), hostname, user, port, identity_file, proxy_jump }
    }

    pub fn hosts(&self) -> Vec<SshConfigHost> {
        self.aliases().iter().map(|a| self.resolve(a)).collect()
    }
}

/// Reads `<home>/.ssh/config` and returns importable hosts.
pub fn read_user_config(home: &Path) -> Vec<SshConfigHost> {
    let mut p = SshConfigParser::new(home);
    p.parse_file(&home.join(".ssh").join("config"));
    p.hosts()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_lines() {
        assert_eq!(split_line("  HostName  lumen.lan # c"), Some(("hostname".into(), vec!["lumen.lan".into()])));
        assert_eq!(split_line("Port=2222"), Some(("port".into(), vec!["2222".into()])));
        assert_eq!(split_line("Port = 2222"), Some(("port".into(), vec!["2222".into()])));
        assert_eq!(
            split_line(r#"IdentityFile "~/My Keys/id_ed25519""#),
            Some(("identityfile".into(), vec!["~/My Keys/id_ed25519".into()]))
        );
        assert_eq!(split_line("# comment"), None);
        assert_eq!(split_line("   "), None);
    }

    #[test]
    fn wildcards() {
        assert!(wildcard_match("*.lan", "lumen.lan"));
        assert!(wildcard_match("srv-??", "srv-01"));
        assert!(!wildcard_match("srv-??", "srv-1"));
        assert!(wildcard_match("LUMEN", "lumen"));
        assert!(host_matches(&["*".into(), "!bastion".into()], "lumen"));
        assert!(!host_matches(&["*".into(), "!bastion".into()], "bastion"));
    }

    #[test]
    fn resolves_fixture() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let ssh = home.join(".ssh");
        std::fs::create_dir_all(ssh.join("config.d")).unwrap();
        std::fs::write(ssh.join("config"), include_str!("../../tests/fixtures/ssh/config")).unwrap();
        std::fs::write(ssh.join("config.d/work.conf"), include_str!("../../tests/fixtures/ssh/work.conf")).unwrap();

        let hosts = read_user_config(home);
        let aliases: Vec<_> = hosts.iter().map(|h| h.alias.as_str()).collect();
        assert_eq!(aliases, vec!["work-box", "lumen", "nas", "bastion", "inner", "lumen-ts"]);

        let lumen = &hosts[1];
        assert_eq!(lumen.hostname, "192.168.1.10");
        assert_eq!(lumen.user.as_deref(), Some("roben"));
        assert_eq!(lumen.port, None);
        let id = home.join(".ssh/id_ed25519").to_string_lossy().into_owned();
        assert_eq!(lumen.identity_file.as_deref(), Some(id.as_str()), "falls through to Host * and expands ~");

        let nas = &hosts[2];
        assert_eq!(nas.port, Some(2222));
        assert_eq!(nas.user.as_deref(), Some("admin"), "first value wins over Host *");
        assert_eq!(nas.identity_file, None, "IdentityFile none");

        let inner = &hosts[4];
        assert_eq!(inner.proxy_jump.as_deref(), Some("bastion"));
        assert_eq!(inner.hostname, "10.0.0.5");

        let bastion = &hosts[3];
        assert_eq!(bastion.user.as_deref(), Some("jump"), "Match block ignored");
        assert_eq!(bastion.proxy_jump, None);

        let work = &hosts[0];
        assert_eq!(work.hostname, "work-box.corp.example");
        let wid = home.join(".ssh/keys/work-box_roben").to_string_lossy().into_owned();
        assert_eq!(work.identity_file.as_deref(), Some(wid.as_str()), "tokens %n and %r expanded");

        let ts = &hosts[5];
        assert_eq!(ts.hostname, "lumen.tail1234.ts.net");
    }

    #[test]
    fn include_cycles_terminate() {
        let dir = tempfile::tempdir().unwrap();
        let ssh = dir.path().join(".ssh");
        std::fs::create_dir_all(&ssh).unwrap();
        std::fs::write(ssh.join("config"), "Include config\nHost a\n  HostName a.lan\n").unwrap();
        let hosts = read_user_config(dir.path());
        assert_eq!(hosts.len(), 1);
    }

    #[test]
    fn missing_config_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_user_config(dir.path()).is_empty());
    }
}
