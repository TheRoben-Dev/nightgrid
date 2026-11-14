//! "Edit in Nightgrid": a session-only shell hook that makes `nano FILE` and
//! `$EDITOR` / `$VISUAL` (git commit, crontab -e, sudoedit, ...) open the file
//! in the app's editor instead of a terminal editor.
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

/// The bootstrap script (POSIX sh). It is read from a pipe, so every command
/// that could read stdin is pointed at the terminal instead.
fn script(token: &str) -> String {
    SCRIPT.replace("@HELPER@", &helper(token))
}

const SCRIPT: &str = r#"umask 077
for o in /tmp/nightgrid-edit.*; do
  [ -d "$o" ] && [ -O "$o" ] && find "$o" -maxdepth 0 -mmin +1440 -exec rm -rf {} + 2>/dev/null
done </dev/null
sh_=${SHELL:-/bin/sh}
d=$(mktemp -d /tmp/nightgrid-edit.XXXXXX 2>/dev/null </dev/null)
if [ -z "$d" ]; then
  echo "nightgrid: could not create a temp folder; editing in the app is off for this session." >/dev/tty
  exec "$sh_" -l </dev/tty
fi
cat >"$d/edit" <<'NIGHTGRID_EOF'
@HELPER@NIGHTGRID_EOF
chmod 700 "$d/edit"
NIGHTGRID_EDIT=$d/edit
EDITOR=$d/edit
VISUAL=$d/edit
export NIGHTGRID_EDIT EDITOR VISUAL
case ${sh_##*/} in
bash)
  cat >"$d/rc" <<'NIGHTGRID_EOF'
[ -f /etc/profile ] && . /etc/profile
for f in "$HOME/.bash_profile" "$HOME/.bash_login" "$HOME/.profile"; do
  [ -f "$f" ] && { . "$f"; break; }
done
nano() { if [ $# -eq 0 ]; then command nano; else "$NIGHTGRID_EDIT" "$@"; fi; }
export EDITOR="$NIGHTGRID_EDIT" VISUAL="$NIGHTGRID_EDIT"
NIGHTGRID_EOF
  "$sh_" --rcfile "$d/rc" -i </dev/tty
  ;;
zsh)
  for f in .zshenv .zprofile .zshrc .zlogin; do
    printf 'ZDOTDIR=$HOME; [ -f "$HOME/%s" ] && . "$HOME/%s"; ZDOTDIR=%s\n' "$f" "$f" "$d" >"$d/$f"
  done
  cat >>"$d/.zshrc" <<'NIGHTGRID_EOF'
nano() { if [ $# -eq 0 ]; then command nano; else "$NIGHTGRID_EDIT" "$@"; fi; }
export EDITOR="$NIGHTGRID_EDIT" VISUAL="$NIGHTGRID_EDIT"
NIGHTGRID_EOF
  echo 'ZDOTDIR=$HOME' >>"$d/.zlogin"
  ZDOTDIR=$d "$sh_" -l </dev/tty
  ;;
fish)
  "$sh_" -l -C 'function nano; if test (count $argv) -eq 0; command nano; else; $NIGHTGRID_EDIT $argv; end; end; set -gx EDITOR $NIGHTGRID_EDIT; set -gx VISUAL $NIGHTGRID_EDIT' </dev/tty
  ;;
*)
  "$sh_" -l </dev/tty
  ;;
esac
rc=$?
rm -rf "$d"
exit $rc
"#;

/// Command for the PTY exec channel that starts the user's shell with the
/// hook. Only base64 characters, so every login shell (sh, bash, zsh, fish,
/// csh) runs it the same way.
pub fn bootstrap_command(token: &str) -> String {
    debug_assert!(valid_token(token));
    let b64 = base64::engine::general_purpose::STANDARD.encode(script(token));
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
        let c = bootstrap_command(TOKEN);
        assert!(c.starts_with("echo ") && c.ends_with(" | base64 -d | /bin/sh"));
        let b64 = c.trim_start_matches("echo ").split(' ').next().unwrap();
        assert!(b64.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b)));
    }

    #[cfg(unix)]
    #[test]
    fn scripts_parse_with_sh() {
        use std::io::Write;
        for (name, src) in [("bootstrap", script(TOKEN)), ("helper", helper(TOKEN))] {
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

    #[test]
    fn requests_need_the_token_and_an_absolute_path() {
        let b = base64::engine::general_purpose::STANDARD.encode("/etc/hosts");
        assert_eq!(parse_request(&format!("{TOKEN};{b}"), TOKEN).as_deref(), Some("/etc/hosts"));
        assert_eq!(parse_request(&format!("ffff;{b}"), TOKEN), None);
        let rel = base64::engine::general_purpose::STANDARD.encode("etc/hosts");
        assert_eq!(parse_request(&format!("{TOKEN};{rel}"), TOKEN), None);
    }
}
