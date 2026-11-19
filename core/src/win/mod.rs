//! Windows hosts (OpenSSH for Windows: Windows Server, Windows 10/11).
//!
//! Their login shell is cmd or PowerShell, so Nightgrid's POSIX scripts
//! can't run there. Windows features send PowerShell instead, the same
//! agentless way: nothing is installed on the host.
//!
//! Every script goes through one tiny bootstrap command line that works
//! from cmd, Windows PowerShell 5.1 and PowerShell 7 alike: it reads one
//! base64 line (the UTF-8 script) from stdin, sets UTF-8 output and runs
//! it. The script itself never touches the command line, so cmd's 8 KB
//! limit and its quoting rules don't apply, and after that first line
//! stdin belongs to the script (the metrics sampler reads its requests
//! there).

pub mod disks;
pub mod events;
pub mod keys;
pub mod metrics;
pub mod security;
pub mod services;
pub mod support;
pub mod tasks;

use base64::Engine as _;
use russh::ChannelMsg;

use crate::ssh::client::Connection;

/// Reads the script line, switches output to UTF-8, quiets progress bars
/// (they turn into CLIXML on stderr over SSH) and runs the script.
const BOOTSTRAP: &str = "$ProgressPreference='SilentlyContinue';\
$e=New-Object Text.UTF8Encoding $false;\
[Console]::OutputEncoding=$e;\
$OutputEncoding=$e;\
$s=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String([Console]::In.ReadLine()));\
& ([ScriptBlock]::Create($s))";

/// Prints `True` when the login user has administrator rights (OpenSSH
/// for Windows logs administrators in with their full token).
pub const IS_ADMIN: &str = "(New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)";

/// The arguments after `powershell`.
fn bootstrap_args() -> Vec<String> {
    let utf16: Vec<u8> = BOOTSTRAP.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);
    ["-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand"]
        .iter()
        .map(|s| s.to_string())
        .chain(std::iter::once(encoded))
        .collect()
}

/// The command line that starts the bootstrap (no quoting needed in cmd).
pub fn command_line() -> String {
    format!("powershell {}", bootstrap_args().join(" "))
}

/// What to send on stdin first: the script as one base64 line.
pub fn script_line(script: &str) -> Vec<u8> {
    let mut line = base64::engine::general_purpose::STANDARD.encode(script.as_bytes()).into_bytes();
    line.push(b'\n');
    line
}

/// A PowerShell single-quoted string literal (`'it''s'`).
pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Result of one script run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    /// stderr, or a short reason when the script failed without saying why.
    pub fn error_text(&self) -> String {
        let e = clean_stderr(&self.stderr);
        if e.is_empty() {
            format!("PowerShell exited with {}", self.code)
        } else {
            e
        }
    }
}

/// stderr without PowerShell's CLIXML wrapping (progress and error records
/// some versions write when their output is redirected).
pub fn clean_stderr(s: &str) -> String {
    if !s.contains("#< CLIXML") {
        return s.trim().to_owned();
    }
    // Keep only the text of <S S="Error"> records.
    let mut out = Vec::new();
    for part in s.split("<S S=\"Error\">").skip(1) {
        if let Some(end) = part.find("</S>") {
            let t = part[..end]
                .replace("_x000D__x000A_", "")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&");
            if !t.trim().is_empty() {
                out.push(t.trim().to_owned());
            }
        }
    }
    out.join("\n")
}

/// Starts `script` on the host and returns its channel with stdin still open
/// (for scripts that read requests, like the metrics sampler).
pub async fn open(conn: &Connection, script: &str) -> Result<russh::Channel<russh::client::Msg>, String> {
    let ch = conn.open_exec(&command_line()).await.map_err(|e| e.to_string())?;
    ch.data(&script_line(script)[..]).await.map_err(|e| e.to_string())?;
    Ok(ch)
}

/// Runs `script` on the host and collects its output.
pub async fn run(conn: &Connection, script: &str) -> Result<Output, String> {
    let mut ch = open(conn, script).await?;
    ch.eof().await.map_err(|e| e.to_string())?;
    let (mut out, mut err, mut code) = (Vec::new(), Vec::new(), -1);
    while let Some(msg) = ch.wait().await {
        match msg {
            ChannelMsg::Data { data } => out.extend_from_slice(&data),
            ChannelMsg::ExtendedData { data, .. } => err.extend_from_slice(&data),
            ChannelMsg::ExitStatus { exit_status } => code = exit_status as i32,
            _ => {}
        }
    }
    Ok(Output { code, stdout: text(&out), stderr: text(&err) })
}

/// Runs `script`, calling `on_line` for each output line until it returns
/// `false`, the script ends or `cancel` flips. Returns the exit code
/// (`None` when stopped early: the channel is closed, which ends the
/// script on the host). Error lines have CLIXML removed.
pub async fn stream(
    conn: &Connection,
    script: &str,
    mut cancel: Option<tokio::sync::watch::Receiver<bool>>,
    on_line: &mut (dyn FnMut(crate::system::sudo::Stream, String) -> bool + Send),
) -> Result<Option<i32>, String> {
    use crate::system::sudo::Stream;
    let mut ch = open(conn, script).await?;
    ch.eof().await.map_err(|e| e.to_string())?;
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut code = None;
    let feed = |stream: Stream,
                buf: &mut Vec<u8>,
                flush: bool,
                on_line: &mut (dyn FnMut(Stream, String) -> bool + Send)|
     -> bool {
        let mut lines = crate::system::take_lines(buf);
        if flush && !buf.is_empty() {
            lines.push(crate::system::clean_line(buf));
            buf.clear();
        }
        for line in lines {
            let line = line.strip_prefix('\u{feff}').unwrap_or(&line).to_owned();
            let line = if stream == Stream::Err { clean_stderr(&line) } else { line };
            if stream == Stream::Err && line.trim().is_empty() {
                continue;
            }
            if !on_line(stream, line) {
                return false;
            }
        }
        true
    };
    loop {
        let msg = match cancel.as_mut() {
            Some(rx) => tokio::select! {
                m = ch.wait() => m,
                _ = rx.changed() => { let _ = ch.close().await; return Ok(None); }
            },
            None => ch.wait().await,
        };
        let go = match msg {
            Some(ChannelMsg::Data { data }) => {
                out.extend_from_slice(&data);
                feed(Stream::Out, &mut out, false, on_line)
            }
            Some(ChannelMsg::ExtendedData { data, .. }) => {
                err.extend_from_slice(&data);
                feed(Stream::Err, &mut err, false, on_line)
            }
            Some(ChannelMsg::ExitStatus { exit_status }) => {
                code = Some(exit_status as i32);
                true
            }
            Some(_) => true,
            None => break,
        };
        if !go {
            let _ = ch.close().await;
            return Ok(None);
        }
    }
    feed(Stream::Out, &mut out, true, on_line);
    feed(Stream::Err, &mut err, true, on_line);
    Ok(Some(code.unwrap_or(0)))
}

/// Output bytes as text, without a UTF-8 byte order mark.
pub fn text(b: &[u8]) -> String {
    let s = String::from_utf8_lossy(b);
    s.strip_prefix('\u{feff}').unwrap_or(&s).to_owned()
}

/// Runs `script` with this computer's PowerShell, through the same
/// bootstrap (tests on Windows CI use it to check the real scripts).
#[cfg(windows)]
pub fn run_local(script: &str, extra_stdin: &[u8]) -> std::io::Result<Output> {
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    let mut child = Command::new("powershell")
        .args(bootstrap_args())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    {
        let mut stdin = child.stdin.take().expect("piped");
        stdin.write_all(&script_line(script))?;
        stdin.write_all(extra_stdin)?;
    }
    let o = child.wait_with_output()?;
    Ok(Output { code: o.status.code().unwrap_or(-1), stdout: text(&o.stdout), stderr: text(&o.stderr) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_is_plain_for_cmd() {
        let c = command_line();
        assert!(c.starts_with("powershell -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand "));
        // Nothing cmd would treat specially, and well under its 8 KB limit.
        let enc = c.rsplit(' ').next().unwrap();
        assert!(enc.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b)));
        assert!(c.len() < 2000);
        let raw = base64::engine::general_purpose::STANDARD.decode(enc).unwrap();
        let utf16: Vec<u16> = raw.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        assert_eq!(String::from_utf16(&utf16).unwrap(), BOOTSTRAP);
    }

    #[test]
    fn script_goes_as_one_line() {
        let l = script_line("Write-Output 'hé'\n'two'");
        assert_eq!(l.iter().filter(|b| **b == b'\n').count(), 1);
        assert_eq!(*l.last().unwrap(), b'\n');
    }

    #[test]
    fn quotes() {
        assert_eq!(quote("it's"), "'it''s'");
    }

    #[test]
    fn clixml_errors_are_unwrapped() {
        let s = "#< CLIXML\r\n<Objs Version=\"1.1.0.1\"><S S=\"Error\">boom &lt;x&gt;_x000D__x000A_</S><S S=\"progress\">x</S></Objs>";
        assert_eq!(clean_stderr(s), "boom <x>");
        assert_eq!(clean_stderr(" plain \n"), "plain");
        assert_eq!(
            Output { code: 3, stdout: String::new(), stderr: String::new() }.error_text(),
            "PowerShell exited with 3"
        );
    }

    #[cfg(windows)]
    #[test]
    fn runs_through_local_powershell() {
        let o = run_local("Write-Output ('a' + 'é'); [Console]::Error.WriteLine('err'); exit 3", b"").unwrap();
        assert_eq!(o.stdout.trim(), "aé", "{o:?}");
        assert!(o.stderr.contains("err"), "{o:?}");
        assert_eq!(o.code, 3);
    }
}
