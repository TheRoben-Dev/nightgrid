//! Config drift (M10l): what changed in `/etc`, compose files and installed
//! packages, against a baseline saved on the host or against another host.
//!
//! A snapshot is a manifest: the SHA-256 of every readable file under `/etc`
//! (and compose files in the host's stack folders) plus the package list.
//! A baseline is that manifest and a `tar.gz` of the files, kept **on the
//! host** in `~/.nightgrid/baselines/default` (mode 700): file contents can
//! hold secrets and never come to the local database (rule 1). Contents are
//! only fetched to show one file's diff, and `.env` files are compared by
//! hash only.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::files::path::shell_quote;

/// Files bigger than this aren't hashed (logs and binaries don't belong in /etc).
pub const MAX_FILE_KB: u32 = 1024;
/// At most this many files per snapshot.
pub const MAX_FILES: usize = 20_000;
/// At most this much of a file is fetched for a diff.
pub const MAX_DIFF_BYTES: usize = 512 * 1024;
/// Where baselines live on the host, relative to the login's home.
pub const BASELINE_DIR: &str = ".nightgrid/baselines/default";

/// Paths that are meant to differ between two hosts.
const HOST_SPECIFIC: &[&str] = &[
    "/etc/hostname",
    "/etc/machine-id",
    "/etc/hosts",
    "/etc/adjtime",
    "/etc/mailname",
    "/etc/ssh/ssh_host_",
    "/etc/netplan/",
    "/etc/network/interfaces",
    "/etc/NetworkManager/system-connections/",
    "/etc/tailscale/",
    "/etc/wireguard/",
];

/// True for paths expected to differ between hosts.
pub fn host_specific(path: &str) -> bool {
    HOST_SPECIFIC.iter().any(|p| if p.ends_with('/') || p.ends_with('_') { path.starts_with(p) } else { path == *p })
}

/// `.env` files: compared by hash only, never fetched or archived.
pub fn is_secret_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name == ".env" || name.starts_with(".env.")
}

fn snapshot_body(extra_dirs: &[String]) -> String {
    let dirs: Vec<String> = extra_dirs
        .iter()
        .filter(|d| !d.chars().any(char::is_control))
        .filter_map(|d| match d.as_str() {
            "~" => Some("\"$HOME\"".to_owned()),
            d if d.starts_with("~/") => Some(format!("\"$HOME\"/{}", shell_quote(&d[2..]))),
            d if d.starts_with('/') => Some(shell_quote(d)),
            _ => None,
        })
        .take(50)
        .collect();
    let extra = if dirs.is_empty() {
        String::new()
    } else {
        format!(
            "for d in {}; do find \"$d\" -maxdepth 2 -type f \\( -name 'compose*.y*ml' -o -name 'docker-compose*.y*ml' -o -name '.env' \\) -readable 2>/dev/null; done; ",
            dirs.join(" ")
        )
    };
    format!(
        r##"echo "@@now"; date +%s
echo "@@pkg"
if command -v dpkg-query >/dev/null 2>&1; then echo "#apt"; dpkg-query -W -f '${{Package}}\t${{Version}}\n' 2>/dev/null
elif command -v rpm >/dev/null 2>&1; then echo "#rpm"; rpm -qa --qf '%{{NAME}}\t%{{VERSION}}-%{{RELEASE}}\n' 2>/dev/null
elif command -v pacman >/dev/null 2>&1; then echo "#pacman"; pacman -Q 2>/dev/null | tr ' ' '\t'
elif command -v apk >/dev/null 2>&1; then echo "#apk"; apk info -v 2>/dev/null
fi
echo "@@files"
{{ find /etc -xdev -type f -size -{MAX_FILE_KB}k -readable 2>/dev/null; {extra}}} | grep -v -E '^/etc/(ld\.so\.cache|\.pwd\.lock|mtab|\.updated)$' | grep -v -E '\.(dpkg-old|dpkg-dist|rpmnew|rpmsave|swp|bak)$' | head -n {MAX_FILES} | tr '\n' '\0' | xargs -0 -r sha256sum 2>/dev/null
echo "@@end"
"##
    )
}

/// Reads the live manifest (no sudo: files the login can't read are skipped).
pub fn snapshot_command(extra_dirs: &[String]) -> String {
    snapshot_body(extra_dirs)
}

/// Saves a baseline on the host: manifest plus an archive of the files
/// (`.env` excluded), replacing the previous one. Prints `saved`.
pub fn save_baseline_command(extra_dirs: &[String]) -> String {
    format!(
        r#"set -e
umask 077
B="$HOME/{BASELINE_DIR}"
mkdir -p "$(dirname "$B")"
rm -rf "$B.new"; mkdir "$B.new"
snap() {{
{body}}}
snap > "$B.new/manifest.txt"
awk '/^@@files/{{f=1;next}} /^@@/{{f=0}} f{{sub(/^[0-9a-f]+  /,""); print}}' "$B.new/manifest.txt" | grep -v -E '(^|/)\.env(\..*)?$' > "$B.new/list.txt" || true
tar -czf "$B.new/files.tar.gz" -T "$B.new/list.txt" 2>/dev/null || true
rm -rf "$B.old"; if [ -d "$B" ]; then mv "$B" "$B.old"; fi
mv "$B.new" "$B"; rm -rf "$B.old"
echo saved
"#,
        body = snapshot_body(extra_dirs)
    )
}

/// Prints the saved manifest, or `@@none`.
pub fn read_baseline_command() -> String {
    format!("B=\"$HOME/{BASELINE_DIR}\"; if [ -f \"$B/manifest.txt\" ]; then cat \"$B/manifest.txt\"; else echo '@@none'; fi\n")
}

/// Whether `path` may be fetched for a diff.
pub fn fetchable(path: &str) -> bool {
    path.starts_with('/') && path.len() <= 1024 && !path.chars().any(char::is_control) && !is_secret_file(path)
}

/// Reads one live file (capped).
pub fn read_file_command(path: &str) -> Option<String> {
    fetchable(path).then(|| format!("head -c {MAX_DIFF_BYTES} -- {}\n", shell_quote(path)))
}

/// Reads one file from the saved baseline archive (capped).
pub fn read_baseline_file_command(path: &str) -> Option<String> {
    if !fetchable(path) {
        return None;
    }
    let member = path.trim_start_matches('/');
    Some(format!(
        "tar -xzOf \"$HOME/{BASELINE_DIR}/files.tar.gz\" -- {} 2>/dev/null | head -c {MAX_DIFF_BYTES}\n",
        shell_quote(member)
    ))
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// The host's clock when it was taken.
    pub taken: i64,
    /// `apt`, `rpm`, `pacman`, `apk` or empty.
    pub manager: String,
    pub packages: BTreeMap<String, String>,
    /// Path → SHA-256.
    pub files: BTreeMap<String, String>,
}

/// Parses [`snapshot_command`] output (or a saved manifest). `None` for `@@none`.
pub fn parse_snapshot(out: &str) -> Option<Snapshot> {
    let mut s = Snapshot::default();
    let mut section = "";
    for line in out.lines() {
        if let Some(name) = line.strip_prefix("@@") {
            section = match name.trim() {
                "none" => return None,
                "now" => "now",
                "pkg" => "pkg",
                "files" => "files",
                _ => "",
            };
            continue;
        }
        match section {
            "now" => {
                if let Ok(t) = line.trim().parse() {
                    s.taken = t;
                }
            }
            "pkg" => {
                if let Some(m) = line.strip_prefix('#') {
                    s.manager = m.trim().to_owned();
                } else if s.manager == "apk" {
                    // `name-1.2.3-r0`: the version starts at the first `-<digit>`.
                    let b = line.trim().as_bytes();
                    if let Some(i) = (1..b.len()).find(|&i| b[i - 1] == b'-' && b[i].is_ascii_digit()) {
                        s.packages.insert(line[..i - 1].to_owned(), line[i..].trim().to_owned());
                    }
                } else if let Some((n, v)) = line.split_once('\t') {
                    if !n.is_empty() {
                        s.packages.insert(n.trim().to_owned(), v.trim().to_owned());
                    }
                }
            }
            "files" => {
                if let Some((hash, path)) = line.split_once("  ") {
                    if hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit()) && path.starts_with('/') {
                        s.files.insert(path.to_owned(), hash.to_owned());
                    }
                }
            }
            _ => {}
        }
    }
    Some(s)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Change {
    Changed,
    /// Only on the right (the live host, or the second host).
    Added,
    /// Only on the left (the baseline, or the first host).
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub change: Change,
    /// A `.env` file: compared by hash only, contents never fetched.
    pub secret: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PackageChange {
    pub name: String,
    pub left: Option<String>,
    pub right: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SideInfo {
    pub label: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub taken: i64,
    pub manager: String,
    pub files: u32,
    pub packages: u32,
}

impl SideInfo {
    pub fn of(label: impl Into<String>, s: &Snapshot) -> Self {
        Self {
            label: label.into(),
            taken: s.taken,
            manager: s.manager.clone(),
            files: s.files.len() as u32,
            packages: s.packages.len() as u32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DriftReport {
    pub left: SideInfo,
    pub right: SideInfo,
    pub files: Vec<FileChange>,
    pub packages: Vec<PackageChange>,
    /// Host-specific files left out (host-to-host only).
    pub hidden: u32,
    /// Package lists come from different package managers.
    pub mixed_managers: bool,
}

/// What differs from `left` to `right`.
pub fn compare(
    left: &Snapshot,
    right: &Snapshot,
    hide_host_specific: bool,
) -> (Vec<FileChange>, Vec<PackageChange>, u32) {
    let mut files = Vec::new();
    let mut hidden = 0;
    let mut push = |path: &str, change| {
        if hide_host_specific && host_specific(path) {
            hidden += 1;
        } else {
            files.push(FileChange { path: path.to_owned(), change, secret: is_secret_file(path) });
        }
    };
    for (p, h) in &left.files {
        match right.files.get(p) {
            Some(h2) if h2 == h => {}
            Some(_) => push(p, Change::Changed),
            None => push(p, Change::Removed),
        }
    }
    for p in right.files.keys().filter(|p| !left.files.contains_key(*p)) {
        push(p, Change::Added);
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let mut packages = Vec::new();
    for (n, v) in &left.packages {
        match right.packages.get(n) {
            Some(v2) if v2 == v => {}
            other => packages.push(PackageChange { name: n.clone(), left: Some(v.clone()), right: other.cloned() }),
        }
    }
    for (n, v) in right.packages.iter().filter(|(n, _)| !left.packages.contains_key(*n)) {
        packages.push(PackageChange { name: n.clone(), left: None, right: Some(v.clone()) });
    }
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    (files, packages, hidden)
}

pub fn report(left_label: &str, left: &Snapshot, right_label: &str, right: &Snapshot, hide: bool) -> DriftReport {
    let (files, packages, hidden) = compare(left, right, hide);
    DriftReport {
        left: SideInfo::of(left_label, left),
        right: SideInfo::of(right_label, right),
        files,
        packages,
        hidden,
        mixed_managers: !left.manager.is_empty() && !right.manager.is_empty() && left.manager != right.manager,
    }
}

// ---- line diff -------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Tag {
    Same,
    Add,
    Del,
    /// Unchanged lines left out; `text` says how many.
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub tag: Tag,
    /// Line number on the left (1-based).
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub path: String,
    pub lines: Vec<DiffLine>,
    pub added: u32,
    pub removed: u32,
    /// Not text: no line diff.
    pub binary: bool,
    /// Too different (or too long) for a line diff; only counts are given.
    pub too_large: bool,
    /// A side was cut at [`MAX_DIFF_BYTES`].
    pub truncated: bool,
}

/// Edit script by Myers' algorithm: `(tag, left index, right index)`.
/// `None` when more than `max_d` edits are needed.
fn myers<'a>(a: &[&'a str], b: &[&'a str], max_d: usize) -> Option<Vec<(Tag, usize, usize)>> {
    let (n, m) = (a.len() as isize, b.len() as isize);
    let max = (n + m) as usize;
    let off = max as isize + 1;
    let mut v = vec![0isize; 2 * max + 3];
    let mut trace: Vec<Vec<isize>> = Vec::new();
    let mut found = false;
    for d in 0..=(max.min(max_d) as isize) {
        trace.push(v.clone());
        let mut k = -d;
        while k <= d {
            let i = (k + off) as usize;
            let mut x = if k == -d || (k != d && v[i - 1] < v[i + 1]) { v[i + 1] } else { v[i - 1] + 1 };
            let mut y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[i] = x;
            if x >= n && y >= m {
                found = true;
                break;
            }
            k += 2;
        }
        if found {
            break;
        }
    }
    if !found {
        return None;
    }
    // Walk back through the saved frontiers.
    let mut out = Vec::new();
    let (mut x, mut y) = (n, m);
    for d in (0..trace.len() as isize).rev() {
        let v = &trace[d as usize];
        let k = x - y;
        let prev_k =
            if k == -d || (k != d && v[(k - 1 + off) as usize] < v[(k + 1 + off) as usize]) { k + 1 } else { k - 1 };
        let prev_x = v[(prev_k + off) as usize];
        let prev_y = prev_x - prev_k;
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            out.push((Tag::Same, x as usize, y as usize));
        }
        if d > 0 {
            if x == prev_x {
                out.push((Tag::Add, x as usize, prev_y as usize));
            } else {
                out.push((Tag::Del, prev_x as usize, y as usize));
            }
        }
        x = prev_x;
        y = prev_y;
    }
    out.reverse();
    Some(out)
}

/// A unified diff of two texts with `context` unchanged lines around changes.
pub fn line_diff(path: &str, left: &[u8], right: &[u8], context: usize) -> FileDiff {
    let mut d = FileDiff {
        path: path.to_owned(),
        lines: Vec::new(),
        added: 0,
        removed: 0,
        binary: false,
        too_large: false,
        truncated: left.len() >= MAX_DIFF_BYTES || right.len() >= MAX_DIFF_BYTES,
    };
    if left.contains(&0) || right.contains(&0) {
        d.binary = true;
        return d;
    }
    let (ls, rs) = (String::from_utf8_lossy(left), String::from_utf8_lossy(right));
    let a: Vec<&str> = ls.lines().collect();
    let b: Vec<&str> = rs.lines().collect();
    if a.len() + b.len() > 40_000 {
        d.too_large = true;
        return d;
    }
    let Some(script) = myers(&a, &b, 2_000) else {
        d.too_large = true;
        return d;
    };
    d.added = script.iter().filter(|s| s.0 == Tag::Add).count() as u32;
    d.removed = script.iter().filter(|s| s.0 == Tag::Del).count() as u32;
    // Keep unchanged lines within `context` of a change.
    let changed: Vec<usize> = script.iter().enumerate().filter(|(_, s)| s.0 != Tag::Same).map(|(i, _)| i).collect();
    let near = |i: usize| changed.iter().any(|&c| c.abs_diff(i) <= context);
    let mut skipped = 0u32;
    for (i, &(tag, x, y)) in script.iter().enumerate() {
        if tag == Tag::Same && !near(i) {
            skipped += 1;
            continue;
        }
        if skipped > 0 {
            d.lines.push(DiffLine { tag: Tag::Skip, old: None, new: None, text: skipped.to_string() });
            skipped = 0;
        }
        let (old, new, text) = match tag {
            Tag::Add => (None, Some(y as u32 + 1), b[y]),
            Tag::Del => (Some(x as u32 + 1), None, a[x]),
            _ => (Some(x as u32 + 1), Some(y as u32 + 1), a[x]),
        };
        d.lines.push(DiffLine { tag, old, new, text: text.to_owned() });
    }
    if skipped > 0 {
        d.lines.push(DiffLine { tag: Tag::Skip, old: None, new: None, text: skipped.to_string() });
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    const H1: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const H2: &str = "2222222222222222222222222222222222222222222222222222222222222222";

    fn snap(files: &[(&str, &str)], pkgs: &[(&str, &str)]) -> Snapshot {
        Snapshot {
            taken: 1,
            manager: "apt".into(),
            files: files.iter().map(|(p, h)| ((*p).into(), (*h).into())).collect(),
            packages: pkgs.iter().map(|(n, v)| ((*n).into(), (*v).into())).collect(),
        }
    }

    #[test]
    fn parses_a_manifest() {
        let out = format!(
            "@@now\n1800000000\n@@pkg\n#apt\nbash\t5.2-1\nopenssl\t3.0.1\n@@files\n{H1}  /etc/fstab\n{H2}  /etc/ssh/sshd_config\nsha256sum: /etc/x: Permission denied\n@@end\n"
        );
        let s = parse_snapshot(&out).unwrap();
        assert_eq!(s.taken, 1_800_000_000);
        assert_eq!(s.manager, "apt");
        assert_eq!(s.packages["openssl"], "3.0.1");
        assert_eq!(s.files.len(), 2);
        assert!(parse_snapshot("@@none\n").is_none());
        let apk = parse_snapshot("@@pkg\n#apk\nmusl-1.2.4-r2\nca-certificates-bundle-20240226-r0\n").unwrap();
        assert_eq!(apk.packages["musl"], "1.2.4-r2");
        assert_eq!(apk.packages["ca-certificates-bundle"], "20240226-r0");
    }

    #[test]
    fn compares_files_and_packages() {
        let a = snap(
            &[("/etc/fstab", H1), ("/etc/old", H1), ("/etc/hostname", H1), ("/srv/app/.env", H1)],
            &[("bash", "5.1"), ("gone", "1")],
        );
        let b = snap(
            &[("/etc/fstab", H2), ("/etc/new", H1), ("/etc/hostname", H2), ("/srv/app/.env", H2)],
            &[("bash", "5.2"), ("curl", "8")],
        );
        let r = report("baseline", &a, "lab", &b, false);
        let f: Vec<(&str, Change)> = r.files.iter().map(|f| (f.path.as_str(), f.change)).collect();
        assert_eq!(
            f,
            vec![
                ("/etc/fstab", Change::Changed),
                ("/etc/hostname", Change::Changed),
                ("/etc/new", Change::Added),
                ("/etc/old", Change::Removed),
                ("/srv/app/.env", Change::Changed),
            ]
        );
        assert!(r.files[4].secret);
        assert_eq!(r.packages.len(), 3);
        assert_eq!(
            r.packages[0],
            PackageChange { name: "bash".into(), left: Some("5.1".into()), right: Some("5.2".into()) }
        );
        assert_eq!(r.packages[1].left, None);
        assert_eq!(r.packages[2].right, None);
        let hidden = report("a", &a, "b", &b, true);
        assert_eq!(hidden.hidden, 1);
        assert!(!hidden.files.iter().any(|f| f.path == "/etc/hostname"));
        assert!(host_specific("/etc/ssh/ssh_host_ed25519_key.pub"));
        assert!(!host_specific("/etc/ssh/sshd_config"));
    }

    #[test]
    fn scripts_quote_and_refuse() {
        let s = snapshot_command(&["/srv/my stacks".into(), "relative".into(), "~/stacks".into()]);
        assert!(s.contains("'/srv/my stacks'") && !s.contains("relative"));
        assert!(s.contains("\"$HOME\"/'stacks'"));
        assert!(s.contains("-size -1024k"));
        assert!(save_baseline_command(&[]).contains(".nightgrid/baselines/default"));
        assert!(read_file_command("/srv/app/.env").is_none());
        assert!(read_file_command("etc/fstab").is_none());
        assert!(read_baseline_file_command("/etc/fstab").unwrap().contains("-- 'etc/fstab'"));
    }

    #[cfg(unix)]
    #[test]
    fn the_scripts_run() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let run = |cmd: &str| {
            let o = std::process::Command::new("sh").arg("-c").arg(cmd).env("HOME", home).output().unwrap();
            String::from_utf8_lossy(&o.stdout).into_owned()
        };
        assert!(run(&read_baseline_command()).contains("@@none"));
        let saved = run(&save_baseline_command(&[]));
        assert!(saved.trim_end().ends_with("saved"), "{saved}");
        let live = parse_snapshot(&run(&snapshot_command(&[]))).unwrap();
        let base = parse_snapshot(&run(&read_baseline_command())).unwrap();
        assert!(base.taken > 0);
        // Nothing changed in between (on a quiet machine), and both read /etc.
        assert_eq!(live.files.len(), base.files.len());
        if let Some(p) = base.files.keys().find(|p| p.as_str() == "/etc/hostname" || p.as_str() == "/etc/passwd") {
            let from_base = run(&read_baseline_file_command(p).unwrap());
            let from_live = run(&read_file_command(p).unwrap());
            assert_eq!(from_base, from_live);
        }
    }

    #[test]
    fn diffs_lines() {
        let a = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n";
        let b = "a\nb\nc\nD\ne\nf\ng\nh\ni\nj\nk\n";
        let d = line_diff("/etc/x", a.as_bytes(), b.as_bytes(), 1);
        assert_eq!((d.added, d.removed), (2, 1));
        let tags: Vec<(Tag, &str)> = d.lines.iter().map(|l| (l.tag, l.text.as_str())).collect();
        assert_eq!(
            tags,
            vec![
                (Tag::Skip, "2"),
                (Tag::Same, "c"),
                (Tag::Del, "d"),
                (Tag::Add, "D"),
                (Tag::Same, "e"),
                (Tag::Skip, "4"),
                (Tag::Same, "j"),
                (Tag::Add, "k"),
            ]
        );
        assert_eq!(d.lines[2].old, Some(4));
        assert_eq!(d.lines[3].new, Some(4));
        assert_eq!(d.lines[7].new, Some(11));
        let same = line_diff("/x", a.as_bytes(), a.as_bytes(), 3);
        assert_eq!((same.added, same.removed), (0, 0));
        assert!(line_diff("/x", b"a\0b", b"a", 3).binary);
        let empty = line_diff("/x", b"", b"one\ntwo\n", 3);
        assert_eq!(empty.added, 2);
    }
}
