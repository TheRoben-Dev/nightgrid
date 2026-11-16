//! Running commands as root (security rule 4).
//!
//! - The password is written to sudo's stdin (`sudo -S`) on the exec channel,
//!   never put on a command line, in the environment or in a log.
//! - `-k` makes sudo ignore a cached timestamp, so the password line is always
//!   consumed by sudo and never leaks into the command's stdin.
//! - A fixed prompt marker (`-p`) is stripped from the output.
//! - stdin is closed right after the password, so a wrong password fails at
//!   once instead of waiting for another try.

use std::sync::Arc;

use russh::ChannelMsg;
use serde::Serialize;
use tokio::sync::watch;
use zeroize::Zeroizing;

use super::{clean_line, SystemError, SystemResult};
use crate::files::path::shell_quote;
use crate::secrets::Secret;
use crate::ssh::client::Connection;

/// sudo's prompt, set with `-p` and removed from what the user sees.
pub const PROMPT: &str = "[nightgrid-sudo]";

/// What it takes to become root on a host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SudoState {
    /// Already logged in as root.
    Root,
    /// sudo works without a password (NOPASSWD).
    NoPassword,
    /// sudo asks for the user's password.
    Password,
    /// sudo isn't installed.
    Missing,
}

/// How a command gets its privileges.
#[derive(Clone)]
pub enum SudoAuth {
    /// No elevation: run as the login user.
    User,
    Root,
    NoPassword,
    Password(Arc<Secret>),
}

impl std::fmt::Debug for SudoAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::User => "User",
            Self::Root => "Root",
            Self::NoPassword => "NoPassword",
            Self::Password(_) => "Password(<redacted>)",
        })
    }
}

/// Runs `inner` with `sh`, whatever the user's login shell is.
pub fn sh(inner: &str) -> String {
    format!("sh -c {}", shell_quote(inner))
}

/// One round trip: the uid and whether sudo needs a password.
pub fn probe_command() -> String {
    sh("id -u; if command -v sudo >/dev/null 2>&1; then if sudo -n true >/dev/null 2>&1; then echo nopass; else echo pass; fi; else echo missing; fi")
}

pub fn parse_probe(out: &str) -> SudoState {
    let mut lines = out.lines().map(str::trim).filter(|l| !l.is_empty());
    if lines.next() == Some("0") {
        return SudoState::Root;
    }
    match lines.next() {
        Some("nopass") => SudoState::NoPassword,
        Some("missing") => SudoState::Missing,
        _ => SudoState::Password,
    }
}

pub async fn probe(conn: &Connection) -> SystemResult<SudoState> {
    let (_, out, _) = conn.exec(&probe_command()).await.map_err(SystemError::other)?;
    Ok(parse_probe(&String::from_utf8_lossy(&out)))
}

/// The command line for `inner` run as root.
pub fn wrap(auth: &SudoAuth, inner: &str) -> String {
    match auth {
        SudoAuth::User | SudoAuth::Root => sh(inner),
        SudoAuth::NoPassword => format!("sudo -n -- {}", sh(inner)),
        SudoAuth::Password(_) => format!("sudo -S -k -p {} -- {}", shell_quote(PROMPT), sh(inner)),
    }
}

/// sudo's own complaint, if a failed run failed because of sudo.
pub fn classify(code: i32, stderr: &str, used_password: bool) -> Option<SystemError> {
    if code == 0 {
        return None;
    }
    let sudo_lines: Vec<String> = stderr
        .lines()
        .map(|l| l.replace(PROMPT, "").trim().to_owned())
        .filter(|l| l.starts_with("sudo:") || l.starts_with("Sorry, try again"))
        .collect();
    let has = |s: &str| sudo_lines.iter().any(|l| l.contains(s));
    if has("a password is required") || has("a terminal is required") {
        return Some(SystemError::SudoRequired { retry: false });
    }
    if used_password && (has("incorrect password") || has("Sorry, try again") || has("no password was provided")) {
        return Some(SystemError::SudoRequired { retry: true });
    }
    if has("not in the sudoers") || has("is not allowed to") || has("may not run sudo") {
        let message = sudo_lines.iter().find(|l| l.starts_with("sudo:")).map(|l| l.trim_start_matches("sudo:").trim());
        return Some(SystemError::SudoDenied { message: message.unwrap_or("not allowed").to_owned() });
    }
    if code == 127 && stderr.contains("sudo") && stderr.contains("not found") {
        return Some(SystemError::NoSudo);
    }
    None
}

/// stderr without the prompt marker and sudo's retry chatter.
pub fn clean_stderr(s: &str) -> String {
    s.lines()
        .map(|l| l.replace(PROMPT, ""))
        .filter(|l| !l.trim().is_empty() && !l.starts_with("Sorry, try again"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Captured result of a privileged run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

async fn open(conn: &Connection, auth: &SudoAuth, inner: &str) -> SystemResult<russh::Channel<russh::client::Msg>> {
    let ch = conn.open_exec(&wrap(auth, inner)).await.map_err(SystemError::other)?;
    if let SudoAuth::Password(pw) = auth {
        let mut line = Zeroizing::new(Vec::with_capacity(pw.expose().len() + 1));
        line.extend_from_slice(pw.expose().as_bytes());
        line.push(b'\n');
        ch.data(&line[..]).await.map_err(SystemError::other)?;
    }
    ch.eof().await.map_err(SystemError::other)?;
    Ok(ch)
}

/// Starts `inner` as root for a two-way stream: sudo's password (if any) is
/// the first stdin line, after which stdin belongs to the command.
pub async fn open_interactive(
    conn: &Connection,
    auth: &SudoAuth,
    inner: &str,
) -> SystemResult<russh::Channel<russh::client::Msg>> {
    let ch = conn.open_exec(&wrap(auth, inner)).await.map_err(SystemError::other)?;
    if let SudoAuth::Password(pw) = auth {
        let mut line = Zeroizing::new(Vec::with_capacity(pw.expose().len() + 1));
        line.extend_from_slice(pw.expose().as_bytes());
        line.push(b'\n');
        ch.data(&line[..]).await.map_err(SystemError::other)?;
    }
    Ok(ch)
}

/// Runs `inner` as root and collects its output. Fails with a sudo error if
/// sudo itself refused.
pub async fn run(conn: &Connection, auth: &SudoAuth, inner: &str) -> SystemResult<Output> {
    let mut ch = open(conn, auth, inner).await?;
    let (mut out, mut err, mut code) = (Vec::new(), Vec::new(), -1);
    while let Some(msg) = ch.wait().await {
        match msg {
            ChannelMsg::Data { data } => out.extend_from_slice(&data),
            ChannelMsg::ExtendedData { data, .. } => err.extend_from_slice(&data),
            ChannelMsg::ExitStatus { exit_status } => code = exit_status as i32,
            _ => {}
        }
    }
    let stderr = String::from_utf8_lossy(&err).into_owned();
    if let Some(e) = classify(code, &stderr, matches!(auth, SudoAuth::Password(_))) {
        return Err(e);
    }
    Ok(Output { code, stdout: String::from_utf8_lossy(&out).into_owned(), stderr: clean_stderr(&stderr) })
}

/// Checks that `auth` gets root (a wrong password fails here, before a long
/// command starts).
pub async fn validate(conn: &Connection, auth: &SudoAuth) -> SystemResult<()> {
    let o = run(conn, auth, "true").await?;
    if o.code == 0 {
        Ok(())
    } else {
        Err(SystemError::other(format!("sudo failed: {}", o.stderr.trim())))
    }
}

/// Which stream a line came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Out,
    Err,
}

/// Runs `inner` as root, calling `on_line` for each output line (prompt
/// removed) until it returns `false`, the command exits or `cancel` flips.
/// Returns the exit code (`None` if stopped early; the command then gets
/// SIGTERM and its channel is closed).
pub async fn stream(
    conn: &Connection,
    auth: &SudoAuth,
    inner: &str,
    mut cancel: Option<watch::Receiver<bool>>,
    on_line: &mut (dyn FnMut(Stream, String) -> bool + Send),
) -> SystemResult<Option<i32>> {
    let mut ch = open(conn, auth, inner).await?;
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut sudo_err = String::new();
    let mut code = None;
    let mut feed = |stream: Stream, buf: &mut Vec<u8>, sudo_err: &mut String, flush: bool| -> bool {
        let mut lines = super::take_lines(buf);
        if flush && !buf.is_empty() {
            lines.push(clean_line(buf));
            buf.clear();
        }
        for line in lines {
            let line = if stream == Stream::Err { line.replace(PROMPT, "") } else { line };
            if stream == Stream::Err && (line.starts_with("sudo:") || line.starts_with("Sorry, try again")) {
                sudo_err.push_str(&line);
                sudo_err.push('\n');
                if line.starts_with("Sorry, try again") {
                    continue;
                }
            }
            if stream == Stream::Err && line.is_empty() {
                continue;
            }
            if !on_line(stream, line) {
                return false;
            }
        }
        true
    };
    let stop = |ch: russh::Channel<russh::client::Msg>| async move {
        let _ = ch.signal(russh::Sig::TERM).await;
        let _ = ch.close().await;
        Ok(None)
    };
    loop {
        let msg = match cancel.as_mut() {
            Some(rx) => tokio::select! {
                m = ch.wait() => m,
                _ = rx.changed() => return stop(ch).await,
            },
            None => ch.wait().await,
        };
        match msg {
            Some(ChannelMsg::Data { data }) => {
                out.extend_from_slice(&data);
                if !feed(Stream::Out, &mut out, &mut sudo_err, false) {
                    return stop(ch).await;
                }
            }
            Some(ChannelMsg::ExtendedData { data, .. }) => {
                err.extend_from_slice(&data);
                if !feed(Stream::Err, &mut err, &mut sudo_err, false) {
                    return stop(ch).await;
                }
            }
            Some(ChannelMsg::ExitStatus { exit_status }) => code = Some(exit_status as i32),
            Some(_) => {}
            None => break,
        }
    }
    feed(Stream::Out, &mut out, &mut sudo_err, true);
    feed(Stream::Err, &mut err, &mut sudo_err, true);
    let c = code.unwrap_or(-1);
    if let Some(e) = classify(c, &sudo_err, matches!(auth, SudoAuth::Password(_))) {
        return Err(e);
    }
    Ok(Some(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_parses() {
        assert_eq!(parse_probe("0\nnopass\n"), SudoState::Root);
        assert_eq!(parse_probe("1000\nnopass\n"), SudoState::NoPassword);
        assert_eq!(parse_probe("1000\npass\n"), SudoState::Password);
        assert_eq!(parse_probe("1000\nmissing\n"), SudoState::Missing);
        assert_eq!(parse_probe(""), SudoState::Password);
    }

    #[test]
    fn wraps_without_secrets_on_the_line() {
        let pw = SudoAuth::Password(Arc::new(Secret::new("hunter2")));
        let cmd = wrap(&pw, "systemctl restart 'x.service'");
        assert!(!cmd.contains("hunter2"));
        assert!(cmd.starts_with("sudo -S -k -p '[nightgrid-sudo]' -- sh -c "));
        assert_eq!(wrap(&SudoAuth::NoPassword, "true"), "sudo -n -- sh -c 'true'");
        assert_eq!(wrap(&SudoAuth::Root, "a 'b'"), r"sh -c 'a '\''b'\'''");
        assert_eq!(format!("{pw:?}"), "Password(<redacted>)");
    }

    #[test]
    fn classifies_sudo_failures() {
        assert_eq!(classify(0, "sudo: anything", true), None);
        assert_eq!(
            classify(1, "sudo: a password is required\n", false),
            Some(SystemError::SudoRequired { retry: false })
        );
        assert_eq!(
            classify(1, "[nightgrid-sudo]Sorry, try again.\nsudo: no password was provided\nsudo: 1 incorrect password attempt\n", true),
            Some(SystemError::SudoRequired { retry: true })
        );
        assert_eq!(
            classify(
                1,
                "[nightgrid-sudo]sudo: alice is not in the sudoers file.  This incident will be reported.\n",
                true
            ),
            Some(SystemError::SudoDenied {
                message: "alice is not in the sudoers file.  This incident will be reported.".into()
            })
        );
        assert_eq!(classify(127, "sh: 1: sudo: not found\n", false), Some(SystemError::NoSudo));
        // The command's own failure is not sudo's.
        assert_eq!(classify(5, "Failed to restart x.service: Unit x.service not found.\n", true), None);
    }

    #[test]
    fn stderr_is_cleaned() {
        assert_eq!(clean_stderr("[nightgrid-sudo]Failed: x\n\n"), "Failed: x");
        assert_eq!(clean_stderr("[nightgrid-sudo]Sorry, try again.\nreal\n"), "real");
    }
}
