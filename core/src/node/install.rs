//! Installing, updating and removing the node from the app (app-integration
//! spec 0001 §2–3). The binary comes from nightgrid-node's GitHub releases
//! (the same kind of origin as the app's own updater) and is used only when
//! its SHA-256 matches the release's `SHA256SUMS`; or the person picks a
//! file. It is uploaded to a private temp folder and its own `install`
//! command runs through the app's sudo flow.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{GROUP, SOCKET};
use crate::files::path::shell_quote;

/// The node project's latest release, from GitHub's API.
pub const LATEST_RELEASE: &str = "https://api.github.com/repos/TheRoben-Dev/nightgrid-node/releases/latest";
/// Largest binary the app downloads or uploads (the real one is ~2–6 MB).
pub const MAX_BINARY: usize = 64 << 20;
/// Removes the node without its binary (from the node project).
pub const UNINSTALL_SH: &str = include_str!("uninstall.sh");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Release {
    /// `v0.23.0`.
    pub tag: String,
    pub published: String,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub name: String,
    pub url: String,
}

impl Release {
    pub fn version(&self) -> &str {
        self.tag.trim_start_matches('v')
    }

    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name == name)
    }
}

/// Reads GitHub's release JSON.
pub fn parse_release(json: &str) -> Result<Release, String> {
    #[derive(Deserialize)]
    struct Gh {
        tag_name: String,
        #[serde(default)]
        published_at: Option<String>,
        #[serde(default)]
        assets: Vec<GhAsset>,
    }
    #[derive(Deserialize)]
    struct GhAsset {
        name: String,
        browser_download_url: String,
    }
    let gh: Gh = serde_json::from_str(json).map_err(|e| format!("unexpected release data: {e}"))?;
    Ok(Release {
        tag: gh.tag_name,
        published: gh.published_at.unwrap_or_default(),
        assets: gh.assets.into_iter().map(|a| Asset { name: a.name, url: a.browser_download_url }).collect(),
    })
}

/// The node's name for `uname -m`; `None` when it has no build for it.
pub fn arch(uname_m: &str) -> Option<&'static str> {
    match uname_m.trim() {
        "x86_64" | "amd64" => Some("x86_64"),
        "aarch64" | "arm64" => Some("aarch64"),
        _ => None,
    }
}

/// `nightgrid-node-v0.23.0-x86_64-linux`.
pub fn asset_name(tag: &str, arch: &str) -> String {
    format!("nightgrid-node-{tag}-{arch}-linux")
}

/// The checksum `SHA256SUMS` lists for `name` (lower-case hex).
pub fn listed_sum(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let mut it = l.split_whitespace();
        let sum = it.next()?;
        let file = it.next()?.trim_start_matches('*');
        (file == name && sum.len() == 64 && sum.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| sum.to_ascii_lowercase())
    })
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Checks a downloaded binary against `SHA256SUMS`.
pub fn verify(bytes: &[u8], sums: &str, name: &str) -> Result<(), String> {
    let want = listed_sum(sums, name).ok_or_else(|| format!("SHA256SUMS doesn't list {name}"))?;
    let got = sha256_hex(bytes);
    if got != want {
        return Err(format!("{name} doesn't match its checksum (got {}…, want {}…)", &got[..12], &want[..12]));
    }
    looks_like_binary(bytes)
}

/// A Linux executable at all (ELF), before anything is uploaded.
pub fn looks_like_binary(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_BINARY {
        return Err("the file is too large to be Nightgrid Node".into());
    }
    if !bytes.starts_with(b"\x7fELF") {
        return Err("that isn't a Linux program (no ELF header)".into());
    }
    Ok(())
}

/// What the app finds on a host before talking to the node. One read-only
/// script, no sudo.
pub const PROBE: &str = r#"u=$(id -un 2>/dev/null)
echo "@@user $u"
echo "@@arch $(uname -m 2>/dev/null)"
command -v nightgrid-node >/dev/null 2>&1 && echo "@@bin yes" || echo "@@bin no"
[ -S __SOCK__ ] && echo "@@sock yes" || echo "@@sock no"
[ -w __SOCK__ ] && echo "@@writable yes" || echo "@@writable no"
getent group __GROUP__ 2>/dev/null | cut -d: -f4 | tr ',' '\n' | grep -qx "$u" && echo "@@member yes" || echo "@@member no"
systemctl is-active nightgrid-node.service 2>/dev/null | sed 's/^/@@unit /'
"#;

pub fn probe_script() -> String {
    PROBE.replace("__SOCK__", SOCKET).replace("__GROUP__", GROUP)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Probe {
    pub user: String,
    pub arch: String,
    pub bin: bool,
    pub sock: bool,
    pub writable: bool,
    /// Listed in the node's group (maybe not yet in this login's groups).
    pub member: bool,
    /// `systemctl is-active` for the service.
    pub unit: String,
}

pub fn parse_probe(out: &str) -> Probe {
    let mut p = Probe::default();
    for l in out.lines() {
        let Some((k, v)) = l.strip_prefix("@@").and_then(|l| l.split_once(' ').or(Some((l, "")))) else {
            continue;
        };
        let yes = v.trim() == "yes";
        match k {
            "user" => p.user = v.trim().into(),
            "arch" => p.arch = v.trim().into(),
            "bin" => p.bin = yes,
            "sock" => p.sock = yes,
            "writable" => p.writable = yes,
            "member" => p.member = yes,
            "unit" => p.unit = v.trim().into(),
            _ => {}
        }
    }
    p
}

/// How to reach the node's socket from this login.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// A `direct-streamlocal` channel straight to the socket (falling back
    /// to `nightgrid-node bridge` when sshd doesn't allow it).
    Socket,
    /// The login user is in the group but this login predates it: `sg`.
    Sg,
}

/// What the probe says: `Ok(route)`, or why the node can't be used.
pub fn route(p: &Probe) -> Result<Route, Absence> {
    if !p.sock {
        return Err(if p.bin { Absence::NotRunning } else { Absence::NotInstalled });
    }
    if p.writable {
        Ok(Route::Socket)
    } else if p.member {
        Ok(Route::Sg)
    } else {
        Err(Absence::NoAccess)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Absence {
    NotInstalled,
    /// Installed, but its socket isn't there (stopped or failed).
    NotRunning,
    /// Running, but the login user isn't in its group.
    NoAccess,
}

/// Runs `nightgrid-node bridge` on an exec channel.
pub fn bridge_command(route: Route) -> String {
    match route {
        Route::Socket => "exec nightgrid-node bridge".into(),
        Route::Sg => format!("exec sg {GROUP} -c 'exec nightgrid-node bridge'"),
    }
}

/// The install (or update) run as root: the uploaded binary installs
/// itself and lets `user` use the socket.
pub fn install_command(binary: &str, user: &str) -> String {
    format!("{} install --json --user {}", shell_quote(binary), shell_quote(user))
}

/// Removal as root: the node's own uninstall, or the bundled script when
/// its binary is gone.
pub fn uninstall_command(purge: bool) -> String {
    let flag = if purge { " --purge" } else { "" };
    format!(
        "if command -v nightgrid-node >/dev/null 2>&1; then nightgrid-node uninstall --json{flag}; else sh -s -- {flag} <<'NIGHTGRID_EOF'\n{UNINSTALL_SH}\nNIGHTGRID_EOF\nfi"
    )
}

/// The `--json` report of install and uninstall.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct InstallReport {
    pub action: String,
    pub version: String,
    pub steps: Vec<String>,
    /// Users added to the node's group (they need a new login).
    pub users: Vec<String>,
    pub warnings: Vec<String>,
}

/// Reads the report from the command's output (the last JSON object line);
/// plain text (the fallback script) becomes one step.
pub fn parse_report(out: &str) -> InstallReport {
    out.lines()
        .rev()
        .filter(|l| l.trim_start().starts_with('{'))
        .find_map(|l| serde_json::from_str(l.trim()).ok())
        .unwrap_or_else(|| InstallReport {
            steps: out.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect(),
            ..Default::default()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_assets_and_checksums() {
        let r = parse_release(
            r#"{"tag_name":"v0.23.0","published_at":"2026-11-16T10:00:00Z","assets":[
                {"name":"nightgrid-node-v0.23.0-x86_64-linux","browser_download_url":"https://example.invalid/a"},
                {"name":"SHA256SUMS","browser_download_url":"https://example.invalid/s"}]}"#,
        )
        .unwrap();
        assert_eq!(r.version(), "0.23.0");
        let name = asset_name(&r.tag, arch("x86_64\n").unwrap());
        assert!(r.asset(&name).is_some());
        assert_eq!(arch("armv7l"), None);
        assert_eq!(arch("arm64"), Some("aarch64"));
        let bin = b"\x7fELF fake";
        let sums = format!("{}  {name}\n{}  other\n", sha256_hex(bin), "0".repeat(64));
        assert!(verify(bin, &sums, &name).is_ok());
        assert!(verify(b"\x7fELF other", &sums, &name).unwrap_err().contains("checksum"));
        assert!(verify(bin, &sums, "missing").unwrap_err().contains("doesn't list"));
        let not_elf = b"#!/bin/sh";
        let sums = format!("{}  {name}\n", sha256_hex(not_elf));
        assert!(verify(not_elf, &sums, &name).unwrap_err().contains("ELF"));
    }

    #[test]
    fn probe_picks_a_route() {
        let p = parse_probe(
            "@@user alice\n@@arch x86_64\n@@bin yes\n@@sock yes\n@@writable no\n@@member yes\n@@unit active\n",
        );
        assert_eq!((p.user.as_str(), p.arch.as_str(), p.unit.as_str()), ("alice", "x86_64", "active"));
        assert_eq!(route(&p), Ok(Route::Sg));
        assert_eq!(route(&Probe { writable: true, ..p.clone() }), Ok(Route::Socket));
        assert_eq!(route(&Probe { member: false, ..p.clone() }), Err(Absence::NoAccess));
        assert_eq!(route(&Probe { sock: false, ..p.clone() }), Err(Absence::NotRunning));
        assert_eq!(route(&Probe::default()), Err(Absence::NotInstalled));
    }

    #[test]
    fn scripts_parse_and_quote() {
        for s in [probe_script(), uninstall_command(true), uninstall_command(false)] {
            let out = std::process::Command::new("sh").arg("-n").arg("-c").arg(&s).output().unwrap();
            assert!(out.status.success(), "{s}\n{}", String::from_utf8_lossy(&out.stderr));
        }
        assert_eq!(
            install_command("/tmp/x y/nightgrid-node", "o'brien"),
            "'/tmp/x y/nightgrid-node' install --json --user 'o'\\''brien'"
        );
        assert!(uninstall_command(true).contains("uninstall --json --purge"));
    }

    #[test]
    fn reports_read_json_or_text() {
        let r = parse_report("note\n{\"action\":\"install\",\"version\":\"0.23.0\",\"steps\":[\"a\"],\"users\":[\"alice\"],\"warnings\":[]}\n");
        assert_eq!(r.version, "0.23.0");
        assert_eq!(r.users, vec!["alice"]);
        let r = parse_report("Nightgrid Node removed\n");
        assert_eq!(r.steps, vec!["Nightgrid Node removed"]);
    }
}
