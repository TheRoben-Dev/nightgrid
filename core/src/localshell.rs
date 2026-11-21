//! Which shells "This computer" can open in the terminal (D-201).
//!
//! macOS and Linux keep their single login shell: [`list`] is empty there and
//! the UI shows the plain "local shell" entry it always had. On Windows it
//! lists PowerShell 7 (when installed), Windows PowerShell, Command Prompt and
//! one entry per WSL distro. A WSL shell runs
//! `%SystemRoot%\System32\wsl.exe -d <distro> --cd <dir>` and starts in the
//! distro's Linux home, or in the WSL spelling of a Windows folder when one is
//! passed (`C:\Users\me` → `/mnt/c/Users/me`).

use std::path::Path;

use serde::Serialize;

/// One shell the terminal's "+" menu can open on this computer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LocalShell {
    /// `pwsh`, `powershell`, `cmd` or `wsl:<distro>`.
    pub id: String,
    /// "PowerShell 7", "Ubuntu-24.04" …
    pub name: String,
    /// Short note shown beside it ("pwsh.exe", "WSL").
    pub detail: String,
    pub wsl: bool,
}

const WSL_PREFIX: &str = "wsl:";

/// Distros that exist only to back other tools and have no usable shell.
const HIDDEN_DISTROS: &[&str] = &["docker-desktop", "docker-desktop-data", "rancher-desktop", "rancher-desktop-data"];

/// The shells to offer (empty outside Windows). Runs `wsl.exe -l -q`, so call
/// it off the UI thread.
pub fn list() -> Vec<LocalShell> {
    #[cfg(windows)]
    {
        let mut out = Vec::new();
        if on_path("pwsh.exe") {
            out.push(LocalShell {
                id: "pwsh".into(),
                name: "PowerShell 7".into(),
                detail: "pwsh.exe".into(),
                wsl: false,
            });
        }
        out.push(LocalShell {
            id: "powershell".into(),
            name: "Windows PowerShell".into(),
            detail: "powershell.exe".into(),
            wsl: false,
        });
        out.push(LocalShell { id: "cmd".into(), name: "Command Prompt".into(), detail: "cmd.exe".into(), wsl: false });
        out.extend(wsl_distros().into_iter().map(|d| LocalShell {
            id: format!("{WSL_PREFIX}{d}"),
            name: d,
            detail: "WSL".into(),
            wsl: true,
        }));
        out
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[cfg(windows)]
fn on_path(exe: &str) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(exe).is_file())).unwrap_or(false)
}

/// `%SystemRoot%\System32\wsl.exe`.
pub fn wsl_exe() -> String {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    format!(r"{}\System32\wsl.exe", root.trim_end_matches('\\'))
}

/// Installed WSL distros, from `wsl.exe -l -q`. Empty when WSL isn't
/// installed (the inbox `wsl.exe` stub exits non-zero), has no distros, or
/// doesn't answer within a few seconds.
#[cfg(windows)]
fn wsl_distros() -> Vec<String> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let exe = wsl_exe();
    if !Path::new(&exe).is_file() {
        return Vec::new();
    }
    let Ok(mut child) = Command::new(&exe)
        .args(["-l", "-q"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .spawn()
    else {
        return Vec::new();
    };
    // The list is a few hundred bytes, so it never fills the pipe while we wait.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Vec::new();
                }
                let mut buf = Vec::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = std::io::Read::read_to_end(&mut out, &mut buf);
                }
                return parse_distros(&buf);
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            _ => {
                let _ = child.kill();
                tracing::warn!("wsl.exe -l -q did not answer; WSL shells left out");
                return Vec::new();
            }
        }
    }
}

/// Decodes `wsl.exe -l -q` output: UTF-16LE by default, UTF-8 when
/// `WSL_UTF8=1` is set. One distro per line; internal ones are left out.
pub fn parse_distros(raw: &[u8]) -> Vec<String> {
    let looks_utf16 =
        raw.len() >= 2 && (raw.starts_with(&[0xFF, 0xFE]) || raw.iter().skip(1).step_by(2).take(8).all(|&b| b == 0));
    let text = if looks_utf16 {
        let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(raw).into_owned()
    };
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let name = line.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}' || c == '\0');
        if name.is_empty()
            || HIDDEN_DISTROS.iter().any(|h| h.eq_ignore_ascii_case(name))
            || out.iter().any(|d| d == name)
        {
            continue;
        }
        out.push(name.to_string());
    }
    out
}

/// The program and arguments for a shell id from [`list`], started in `cwd`
/// when given. `None` for an id this OS can't run (macOS/Linux ignore ids, so
/// a profile carried over from Windows changes nothing there).
pub fn program_for(id: &str, cwd: Option<&Path>) -> Option<(String, Vec<String>)> {
    if !cfg!(windows) {
        return None;
    }
    program_for_windows(id, cwd)
}

fn program_for_windows(id: &str, cwd: Option<&Path>) -> Option<(String, Vec<String>)> {
    let nologo = || vec!["-NoLogo".to_string()];
    match id {
        "pwsh" => Some(("pwsh.exe".into(), nologo())),
        "powershell" => Some(("powershell.exe".into(), nologo())),
        "cmd" => Some(("cmd.exe".into(), Vec::new())),
        _ => {
            let distro = id.strip_prefix(WSL_PREFIX)?.trim();
            if distro.is_empty() || distro.starts_with('-') {
                return None;
            }
            let dir = cwd.and_then(|p| windows_to_wsl(&p.to_string_lossy(), distro)).unwrap_or_else(|| "~".into());
            Some((wsl_exe(), vec!["-d".into(), distro.into(), "--cd".into(), dir]))
        }
    }
}

/// A Windows path as WSL sees it: `C:\Users\me` → `/mnt/c/Users/me`, and
/// `\\wsl.localhost\<distro>\home\me` (or `\\wsl$\…`) → `/home/me` when it is
/// the same distro. `None` for paths WSL can't reach the same way (other
/// network shares, another distro's files).
pub fn windows_to_wsl(path: &str, distro: &str) -> Option<String> {
    let p = path.replace('/', "\\");
    let p = p.strip_prefix(r"\\?\").unwrap_or(&p);
    let tail_of = |rest: &str| -> String {
        let parts: Vec<&str> = rest.split('\\').filter(|s| !s.is_empty()).collect();
        parts.join("/")
    };
    if let Some(unc) = p.strip_prefix(r"\\") {
        let mut it = unc.splitn(3, '\\');
        let server = it.next()?;
        let share = it.next()?;
        if !(server.eq_ignore_ascii_case("wsl.localhost") || server.eq_ignore_ascii_case("wsl$"))
            || !share.eq_ignore_ascii_case(distro)
        {
            return None;
        }
        return Some(format!("/{}", tail_of(it.next().unwrap_or(""))));
    }
    let mut chars = p.chars();
    let (drive, colon) = (chars.next()?, chars.next()?);
    if !drive.is_ascii_alphabetic() || colon != ':' {
        return None;
    }
    let rest = chars.as_str();
    if !(rest.is_empty() || rest.starts_with('\\')) {
        return None; // "C:foo" is relative to that drive's cwd
    }
    let tail = tail_of(rest);
    let base = format!("/mnt/{}", drive.to_ascii_lowercase());
    Some(if tail.is_empty() { base } else { format!("{base}/{tail}") })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
    }

    #[test]
    fn parses_utf16_list() {
        let raw = utf16("Ubuntu-24.04\r\ndocker-desktop\r\nDebian\r\n\r\n");
        assert_eq!(parse_distros(&raw), vec!["Ubuntu-24.04", "Debian"]);
        let mut bom = vec![0xFF, 0xFE];
        bom.extend(utf16("Ubuntu\r\n"));
        assert_eq!(parse_distros(&bom), vec!["Ubuntu"]);
    }

    #[test]
    fn parses_utf8_list() {
        assert_eq!(parse_distros(b"Ubuntu\nkali-linux\n"), vec!["Ubuntu", "kali-linux"]);
        assert!(parse_distros(b"").is_empty());
        assert!(parse_distros(&utf16("\r\n")).is_empty());
    }

    #[test]
    fn maps_windows_paths() {
        assert_eq!(windows_to_wsl(r"C:\Users\Kim Ek\src", "Ubuntu").as_deref(), Some("/mnt/c/Users/Kim Ek/src"));
        assert_eq!(windows_to_wsl(r"D:\", "Ubuntu").as_deref(), Some("/mnt/d"));
        assert_eq!(windows_to_wsl("e:/games/", "Ubuntu").as_deref(), Some("/mnt/e/games"));
        assert_eq!(windows_to_wsl(r"\\?\C:\x", "Ubuntu").as_deref(), Some("/mnt/c/x"));
        assert_eq!(windows_to_wsl(r"\\wsl.localhost\Ubuntu\home\kim", "Ubuntu").as_deref(), Some("/home/kim"));
        assert_eq!(windows_to_wsl(r"\\wsl$\ubuntu\etc", "Ubuntu").as_deref(), Some("/etc"));
        assert_eq!(windows_to_wsl(r"\\wsl.localhost\Debian\home", "Ubuntu"), None);
        assert_eq!(windows_to_wsl(r"\\nas\share\x", "Ubuntu"), None);
        assert_eq!(windows_to_wsl(r"C:relative", "Ubuntu"), None);
        assert_eq!(windows_to_wsl("relative", "Ubuntu"), None);
    }

    #[test]
    fn programs() {
        let (exe, args) = program_for_windows("wsl:Ubuntu", None).unwrap();
        assert!(exe.ends_with(r"\System32\wsl.exe"), "{exe}");
        assert_eq!(args, ["-d", "Ubuntu", "--cd", "~"]);
        let (_, args) = program_for_windows("wsl:Ubuntu", Some(Path::new(r"C:\Users\kim"))).unwrap();
        assert_eq!(args, ["-d", "Ubuntu", "--cd", "/mnt/c/Users/kim"]);
        assert_eq!(program_for_windows("cmd", None).unwrap().0, "cmd.exe");
        assert_eq!(program_for_windows("pwsh", None).unwrap().1, ["-NoLogo"]);
        assert!(program_for_windows("wsl:", None).is_none());
        assert!(program_for_windows("wsl:--exec", None).is_none());
        assert!(program_for_windows("bash", None).is_none());
        if !cfg!(windows) {
            assert!(program_for("cmd", None).is_none());
            assert!(list().is_empty());
        }
    }
}
