//! Session-only shell hook for SSH terminals. "Edit in Nightgrid" makes
//! `nano FILE` and `$EDITOR` / `$VISUAL` (git commit, crontab -e, sudoedit,
//! ...) open the file in the app's editor; key hints report the command about
//! to run so the app can show that program's keys (M10j).
//!
//! Nothing is installed on the host. The SSH terminal runs a bootstrap (sent
//! base64-encoded so any login shell can start it) that writes a small helper
//! into a private `mktemp -d` folder, starts the user's normal shell as if it
//! were a login shell, and deletes the folder when that shell exits.
//!
//! The helper prints `ESC ] 7766 ; <token> ; <base64 absolute path> BEL` to
//! the terminal, where the app opens the file over SFTP. It then waits, with
//! echo off, until the app types `<token>` + Enter back (the editor tab was
//! closed), or the user presses Enter to stop waiting.

use base64::Engine;

/// OSC number used for edit requests.
pub const OSC_EDIT: u16 = 7766;

/// Tokens are hex, generated per terminal session by the app.
pub fn valid_token(t: &str) -> bool {
    (16..=64).contains(&t.len()) && t.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The helper used as `$EDITOR` and behind the `nano` function.
fn helper(token: &str) -> String {
    HELPER.replace("@TOKEN@", token).replace("@OSC@", &OSC_EDIT.to_string())
}

const HELPER: &str = r#"#!/bin/sh
# Nightgrid: open files in the app's editor. Session-only; deleted when the shell exits.
T=@TOKEN@
n=0
for a in "$@"; do
  case $a in -*|+*) continue ;; esac
  case $a in /*) p=$a ;; *) p=$(pwd)/$a ;; esac
  n=$((n+1))
  b=$(printf '%s' "$p" | base64 | tr -d '\n')
  printf '\033]@OSC@;%s;%s\007' "$T" "$b" >/dev/tty
  printf 'nightgrid: editing %s in the app. Save and close the tab to continue (Enter stops waiting).\r\n' "$a" >/dev/tty
  s=$(stty -g </dev/tty 2>/dev/null)
  stty -echo </dev/tty 2>/dev/null
  while IFS= read -r l </dev/tty; do
    [ "$l" = "$T" ] && break
    [ -z "$l" ] && break
  done
  [ -n "$s" ] && stty "$s" </dev/tty 2>/dev/null
done
if [ "$n" -eq 0 ]; then
  command -v nano >/dev/null 2>&1 && exec nano "$@"
  echo "usage: $0 FILE..." >&2
  exit 2
fi
"#;

/// What the session-only hook turns on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HookOptions {
    /// `nano FILE` / `$EDITOR` open files in the app's editor.
    pub edit: bool,
    /// Report the command about to run (and the return to the prompt) so the
    /// app can show that program's keys.
    pub hints: bool,
}

impl HookOptions {
    pub fn any(self) -> bool {
        self.edit || self.hints
    }
}

/// OSC number used for "program in front" reports: `OSC 7767 ; <token> ; <command line> BEL`.
/// The command line is reduced to `[A-Za-z0-9 ._/+:=|-]` by the shell before it
/// is printed, so it can't carry escapes; an empty one means "back at the prompt".
pub const OSC_HINT: u16 = 7767;

const BASH_EDIT: &str = r#"nano() { if [ $# -eq 0 ]; then command nano; else "$NIGHTGRID_EDIT" "$@"; fi; }
export EDITOR="$NIGHTGRID_EDIT" VISUAL="$NIGHTGRID_EDIT"
"#;

// bash: a DEBUG trap armed by PROMPT_COMMAND (runs last, so the user's own
// prompt commands aren't reported). Left alone if a DEBUG trap already exists.
const BASH_HINTS: &str = r#"__ng_hint() { local c=${1//[^A-Za-z0-9 ._\/+:=|-]/}; printf '\033]@HOSC@;%s;%s\007' "$NIGHTGRID_TOKEN" "${c:0:160}" >/dev/tty; }
__ng_armed=
__ng_pre() { [ -n "$__ng_armed" ] || return 0; [ -n "$COMP_LINE" ] && return 0; case $BASH_COMMAND in __ng*) return 0 ;; esac; __ng_armed=; local l; l=$(HISTTIMEFORMAT= builtin history 1 2>/dev/null); l=${l#"${l%%[![:space:]0-9]*}"}; __ng_hint "${l:-$BASH_COMMAND}"; }
__ng_post() { __ng_armed=1; __ng_hint ""; }
if [ -z "$(trap -p DEBUG)" ]; then
  trap '__ng_pre' DEBUG
  PROMPT_COMMAND="__ng_armed=;${PROMPT_COMMAND:+$PROMPT_COMMAND;}__ng_post"
fi
"#;

const ZSH_HINTS: &str = r#"__ng_hint() { local c=${1//[^A-Za-z0-9 ._\/+:=|-]/}; printf '\033]@HOSC@;%s;%s\007' "$NIGHTGRID_TOKEN" "${c[1,160]}" >/dev/tty; }
__ng_pre() { __ng_hint "$1"; }
__ng_post() { __ng_hint ""; }
autoload -Uz add-zsh-hook 2>/dev/null && { add-zsh-hook preexec __ng_pre; add-zsh-hook precmd __ng_post; }
"#;

const FISH_EDIT: &str = "function nano; if test (count $argv) -eq 0; command nano; else; $NIGHTGRID_EDIT $argv; end; end; set -gx EDITOR $NIGHTGRID_EDIT; set -gx VISUAL $NIGHTGRID_EDIT; ";
const FISH_HINTS: &str = "function __ng_hint; printf '\\e]@HOSC@;%s;%s\\a' $NIGHTGRID_TOKEN (string sub -l 160 -- (string replace -ra '[^A-Za-z0-9 ._/+:=|-]' '' -- \"$argv\")) >/dev/tty; end; function __ng_pre --on-event fish_preexec; __ng_hint $argv; end; function __ng_post --on-event fish_postexec; __ng_hint ''; end; ";

/// The bootstrap script (POSIX sh). It is read from a pipe, so every command
/// that could read stdin is pointed at the terminal instead.
fn script(token: &str, o: HookOptions) -> String {
    let mut bash = String::new();
    let mut zsh = String::new();
    let mut fish = String::new();
    if o.edit {
        bash.push_str(BASH_EDIT);
        zsh.push_str(BASH_EDIT);
        fish.push_str(FISH_EDIT);
    }
    if o.hints {
        bash.push_str(BASH_HINTS);
        zsh.push_str(ZSH_HINTS);
        fish.push_str(FISH_HINTS);
    }
    let osc = OSC_HINT.to_string();
    let edit_setup = if o.edit { EDIT_SETUP.replace("@HELPER@", &helper(token)) } else { String::new() };
    SCRIPT
        .replace("@EDIT_SETUP@", &edit_setup)
        .replace("@TOKEN@", token)
        .replace("@BASH@", &bash.replace("@HOSC@", &osc))
        .replace("@ZSH@", &zsh.replace("@HOSC@", &osc))
        .replace("@FISH@", &fish.replace("@HOSC@", &osc).replace('\'', "'\\''"))
}

const EDIT_SETUP: &str = r#"cat >"$d/edit" <<'NIGHTGRID_EOF'
@HELPER@NIGHTGRID_EOF
chmod 700 "$d/edit"
NIGHTGRID_EDIT=$d/edit
EDITOR=$d/edit
VISUAL=$d/edit
export NIGHTGRID_EDIT EDITOR VISUAL
"#;

const SCRIPT: &str = r#"umask 077
for o in /tmp/nightgrid-edit.*; do
  [ -d "$o" ] && [ -O "$o" ] && find "$o" -maxdepth 0 -mmin +1440 -exec rm -rf {} + 2>/dev/null
done </dev/null
sh_=${SHELL:-/bin/sh}
# The shell's stdin is the terminal's own device (/dev/pts/N), not /dev/tty:
# sudo (use_pty) doesn't recognise /dev/tty as the user's terminal and stops
# passing keys to the command once it changes terminal modes.
t_=$(tty <&2 2>/dev/null) || t_=
case $t_ in /dev/?*) [ -r "$t_" ] || t_=/dev/tty ;; *) t_=/dev/tty ;; esac
d=$(mktemp -d /tmp/nightgrid-edit.XXXXXX 2>/dev/null </dev/null)
if [ -z "$d" ]; then
  echo "nightgrid: could not create a temp folder; the session hook is off for this session." >/dev/tty
  exec "$sh_" -l <"$t_"
fi
NIGHTGRID_TOKEN=@TOKEN@
export NIGHTGRID_TOKEN
@EDIT_SETUP@case ${sh_##*/} in
bash)
  cat >"$d/rc" <<'NIGHTGRID_EOF'
[ -f /etc/profile ] && . /etc/profile
for f in "$HOME/.bash_profile" "$HOME/.bash_login" "$HOME/.profile"; do
  [ -f "$f" ] && { . "$f"; break; }
done
@BASH@NIGHTGRID_EOF
  "$sh_" --rcfile "$d/rc" -i <"$t_"
  ;;
zsh)
  for f in .zshenv .zprofile .zshrc .zlogin; do
    printf 'ZDOTDIR=$HOME; [ -f "$HOME/%s" ] && . "$HOME/%s"; ZDOTDIR=%s\n' "$f" "$f" "$d" >"$d/$f"
  done
  cat >>"$d/.zshrc" <<'NIGHTGRID_EOF'
@ZSH@NIGHTGRID_EOF
  echo 'ZDOTDIR=$HOME' >>"$d/.zlogin"
  ZDOTDIR=$d "$sh_" -l <"$t_"
  ;;
fish)
  "$sh_" -l -C '@FISH@' <"$t_"
  ;;
*)
  "$sh_" -l <"$t_"
  ;;
esac
rc=$?
rm -rf "$d"
exit $rc
"#;

/// Command for the PTY exec channel that starts the user's shell with the
/// hook. Only base64 characters, so every login shell (sh, bash, zsh, fish,
/// csh) runs it the same way.
pub fn bootstrap_command(token: &str, o: HookOptions) -> String {
    debug_assert!(valid_token(token));
    let b64 = base64::engine::general_purpose::STANDARD.encode(script(token, o));
    format!("echo {b64} | base64 -d | /bin/sh")
}

/// Parses an edit request's OSC payload (`<token>;<base64 path>`).
pub fn parse_request(payload: &str, token: &str) -> Option<String> {
    let (t, b) = payload.split_once(';')?;
    if t != token {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(b.trim()).ok()?;
    let path = String::from_utf8(bytes).ok()?;
    path.starts_with('/').then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn tokens() {
        assert!(valid_token(TOKEN));
        assert!(!valid_token("short"));
        assert!(!valid_token("0123456789abcdef'; rm -rf /"));
    }

    #[test]
    fn bootstrap_is_shell_neutral() {
        let c = bootstrap_command(TOKEN, HookOptions { edit: true, hints: true });
        assert!(c.starts_with("echo ") && c.ends_with(" | base64 -d | /bin/sh"));
        let b64 = c.trim_start_matches("echo ").split(' ').next().unwrap();
        assert!(b64.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b)));
    }

    #[cfg(unix)]
    #[test]
    fn scripts_parse_with_sh() {
        use std::io::Write;
        let all = HookOptions { edit: true, hints: true };
        let hints = HookOptions { edit: false, hints: true };
        let edit = HookOptions { edit: true, hints: false };
        for (name, src) in [
            ("bootstrap", script(TOKEN, all)),
            ("hints only", script(TOKEN, hints)),
            ("edit only", script(TOKEN, edit)),
            ("helper", helper(TOKEN)),
        ] {
            let mut child = std::process::Command::new("sh")
                .arg("-n")
                .stdin(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(src.as_bytes()).unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success(), "{name}: {}", String::from_utf8_lossy(&out.stderr));
        }
    }

    /// The rc snippets parse in the shells they're written for (when installed).
    #[cfg(unix)]
    #[test]
    fn hint_snippets_parse() {
        use std::io::Write;
        for (shell, src) in [("bash", BASH_HINTS), ("zsh", ZSH_HINTS), ("bash", BASH_EDIT)] {
            let Ok(mut child) = std::process::Command::new(shell)
                .arg("-n")
                .stdin(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
            else {
                continue;
            };
            child.stdin.take().unwrap().write_all(src.replace("@HOSC@", "7767").as_bytes()).unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success(), "{shell}: {}", String::from_utf8_lossy(&out.stderr));
        }
    }

    #[test]
    fn hooks_only_include_what_was_asked() {
        let hints = script(TOKEN, HookOptions { edit: false, hints: true });
        assert!(hints.contains("__ng_pre") && !hints.contains("NIGHTGRID_EDIT=$d/edit"));
        let edit = script(TOKEN, HookOptions { edit: true, hints: false });
        assert!(!edit.contains("__ng_pre") && edit.contains("NIGHTGRID_EDIT=$d/edit"));
        assert!(!HookOptions::default().any());
    }

    /// The shell must get the terminal's real device as stdin: with
    /// `</dev/tty`, `sudo` (use_pty) stopped passing keys to programs that
    /// switch terminal modes, like `sudo nightgrid-node fix`.
    #[test]
    fn shell_reads_the_real_terminal_device() {
        let s = script(TOKEN, HookOptions { edit: true, hints: true });
        assert!(s.contains("t_=$(tty <&2"));
        for line in s.lines().filter(|l| l.contains("\"$sh_\"")) {
            assert!(!line.contains("</dev/tty"), "{line}");
            assert!(line.contains("<\"$t_\""), "{line}");
        }
    }

    #[test]
    fn requests_need_the_token_and_an_absolute_path() {
        let b = base64::engine::general_purpose::STANDARD.encode("/etc/hosts");
        assert_eq!(parse_request(&format!("{TOKEN};{b}"), TOKEN).as_deref(), Some("/etc/hosts"));
        assert_eq!(parse_request(&format!("ffff;{b}"), TOKEN), None);
        let rel = base64::engine::general_purpose::STANDARD.encode("etc/hosts");
        assert_eq!(parse_request(&format!("{TOKEN};{rel}"), TOKEN), None);
    }
}
