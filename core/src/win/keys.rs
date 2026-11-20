//! `authorized_keys` on Windows hosts (OpenSSH for Windows).
//!
//! sshd reads administrators' keys from
//! `%ProgramData%\ssh\administrators_authorized_keys` when its config has the
//! default `Match Group administrators` block, and that file must be
//! writable only by Administrators and SYSTEM or sshd ignores it. Everyone
//! else uses `%USERPROFILE%\.ssh\authorized_keys`. These scripts pick the
//! file sshd will read for the login user and keep its permissions right
//! (SIDs, so a translated Windows works too).

use crate::ssh::keys::key_line;
use crate::Result;

/// Sets `$f` to the file sshd reads for this user and `$adm` to whether it's
/// the administrators' file.
const PICK: &str = r#"$ErrorActionPreference = 'Stop'
$p = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
$isAdmin = $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
$cfg = Join-Path $env:ProgramData 'ssh\sshd_config'
$m = $false
if (Test-Path -LiteralPath $cfg) { $m = [bool](Select-String -LiteralPath $cfg -Pattern '^\s*Match\s+Group\s+administrators' -Quiet) }
$adm = $isAdmin -and $m
if ($adm) { $f = Join-Path $env:ProgramData 'ssh\administrators_authorized_keys' } else { $f = Join-Path $env:USERPROFILE '.ssh\authorized_keys' }
$enc = New-Object Text.UTF8Encoding $false
"#;

/// Prints which file is used (a line `parse_authorized` skips) and its keys.
pub fn read_script() -> String {
    format!("{PICK}\"# file: $f\"\nif (Test-Path -LiteralPath $f) {{ [IO.File]::ReadAllText($f) }}\n")
}

/// Appends the key unless it's there (any comment). Prints `added` or `present`.
pub fn add_script(public: &str) -> Result<String> {
    let (blob, line) = key_line(public)?;
    Ok(format!(
        r#"{PICK}$blob = {blob}
$line = {line}
$dir = Split-Path -Parent $f
if (-not (Test-Path -LiteralPath $dir)) {{ New-Item -ItemType Directory -Path $dir -Force | Out-Null }}
$cur = ''
if (Test-Path -LiteralPath $f) {{ $cur = [IO.File]::ReadAllText($f) }}
if ($cur.Contains($blob)) {{ 'present'; exit 0 }}
$pre = ''
if ($cur.Length -gt 0 -and -not $cur.EndsWith("`n")) {{ $pre = "`n" }}
[IO.File]::AppendAllText($f, $pre + $line + "`n", $enc)
if ($adm) {{ icacls.exe $f /inheritance:r /grant '*S-1-5-32-544:F' /grant '*S-1-5-18:F' | Out-Null }}
'added'
"#,
        blob = super::quote(&blob),
        line = super::quote(&line),
    ))
}

/// Removes every line holding the key, in place (permissions kept). Prints
/// how many lines went.
pub fn remove_script(public: &str) -> Result<String> {
    let (blob, _) = key_line(public)?;
    Ok(format!(
        r#"{PICK}$blob = {blob}
if (-not (Test-Path -LiteralPath $f)) {{ '0'; exit 0 }}
$lines = [IO.File]::ReadAllLines($f)
$keep = @($lines | Where-Object {{ -not $_.Contains($blob) }})
$n = $lines.Count - $keep.Count
if ($n -gt 0) {{
  $text = ''
  if ($keep.Count -gt 0) {{ $text = ($keep -join "`n") + "`n" }}
  [IO.File]::WriteAllText($f, $text, $enc)
}}
"$n"
"#,
        blob = super::quote(&blob),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ED: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl laptop@example";

    #[test]
    fn scripts_quote_the_key() {
        let a = add_script(ED).unwrap();
        assert!(
            a.contains("$blob = 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl'")
        );
        assert!(a.contains("$line = 'ssh-ed25519 AAAA") && a.contains(" laptop@example'"));
        assert!(remove_script(ED).unwrap().contains("$blob = 'ssh-ed25519 "));
        assert!(add_script("not a key").is_err());
        // The marker line is skipped as a comment.
        assert!(crate::ssh::keys::parse_authorized("# file: C:\\x\r\n").is_empty());
    }

    /// Add, add again, list, remove against this machine's profile (Windows CI).
    #[cfg(windows)]
    #[test]
    fn round_trip_locally() {
        let tmp = tempfile::tempdir().unwrap();
        // A temp profile and no sshd_config: the user's own file is used.
        let dir = super::super::quote(&tmp.path().display().to_string());
        let pre = format!("$env:USERPROFILE = {dir}\n$env:ProgramData = {dir}\n");
        let run = |s: String| super::super::run_local(&format!("{pre}{s}"), b"").unwrap();
        let o = run(add_script(ED).unwrap());
        assert_eq!(o.stdout.trim(), "added", "{o:?}");
        assert_eq!(run(add_script(ED).unwrap()).stdout.trim(), "present");
        let listed = run(read_script()).stdout;
        let keys = crate::ssh::keys::parse_authorized(&listed);
        assert_eq!(keys.len(), 1, "{listed}");
        assert_eq!(keys[0].comment, "laptop@example");
        assert_eq!(run(remove_script(ED).unwrap()).stdout.trim(), "1");
        assert!(crate::ssh::keys::parse_authorized(&run(read_script()).stdout).is_empty());
    }
}
