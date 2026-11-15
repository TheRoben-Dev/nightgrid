//! Host system management over SSH: systemd units, the journal, package
//! updates and SMART disk health (M6), and scheduled jobs (M10c).
//!
//! Everything here runs plain commands on the host's pooled SSH connection
//! (nothing is installed there). Commands that need root go through
//! [`sudo`], which feeds the password on stdin and never on a command line.

pub mod history;
pub mod journal;
pub mod packages;
pub mod pools;
pub mod schedule;
pub mod security;
pub mod smart;
pub mod sudo;
pub mod systemd;
pub mod vms;

use serde::Serialize;

/// Marker that separates the sections of a multi-part probe's output.
pub(crate) const SECTION: &str = "@@NIGHTGRID@@";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SystemError {
    /// sudo wants a password and none was given (or it was wrong: `retry`).
    #[error("{}", if *retry { "sudo: wrong password" } else { "sudo needs your password" })]
    SudoRequired { retry: bool },
    /// The user may not use sudo (or not for this command).
    #[error("sudo refused: {message}")]
    SudoDenied { message: String },
    #[error("sudo is not installed on this host")]
    NoSudo,
    /// A tool the feature needs isn't on the host.
    #[error("{tool} was not found on this host. {hint}")]
    Missing { tool: String, hint: String },
    #[error("{message}")]
    Other { message: String },
}

impl SystemError {
    pub fn other(e: impl std::fmt::Display) -> Self {
        Self::Other { message: e.to_string() }
    }
}

pub type SystemResult<T> = Result<T, SystemError>;

/// Splits `text` at [`SECTION`] lines.
pub(crate) fn sections(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(SECTION) {
        out.push(&rest[..i]);
        rest = &rest[i + SECTION.len()..];
        rest = rest.strip_prefix('\n').unwrap_or(rest);
    }
    out.push(rest);
    out
}

/// Splits a byte buffer into complete lines, leaving a partial tail in `buf`.
pub(crate) fn take_lines(buf: &mut Vec<u8>) -> Vec<String> {
    let mut out = Vec::new();
    while let Some(i) = buf.iter().position(|b| *b == b'\n') {
        let line: Vec<u8> = buf.drain(..=i).collect();
        out.push(clean_line(&line[..line.len() - 1]));
    }
    out
}

/// A display line: lossy UTF-8, last `\r` redraw kept, ANSI escapes removed.
pub(crate) fn clean_line(line: &[u8]) -> String {
    let s = String::from_utf8_lossy(line);
    let s = s.rsplit('\r').find(|p| !p.is_empty()).unwrap_or("");
    crate::docker::strip_ansi(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_sections() {
        let t = format!("a\nb\n{SECTION}\nc\n{SECTION}\n");
        assert_eq!(sections(&t), vec!["a\nb\n", "c\n", ""]);
        assert_eq!(sections("x"), vec!["x"]);
    }

    #[test]
    fn lines_keep_partial_tail() {
        let mut b = b"one\r\ntwo\x1b[1m!\x1b[0m\nthr".to_vec();
        assert_eq!(take_lines(&mut b), vec!["one", "two!"]);
        assert_eq!(b, b"thr");
        let mut b = b"50%\r100%\n".to_vec();
        assert_eq!(take_lines(&mut b), vec!["100%"]);
    }
}
