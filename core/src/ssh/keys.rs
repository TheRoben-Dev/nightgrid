//! SSH key manager (M10k): the user's own key pairs (`~/.ssh` and the agent),
//! generating new ed25519 keys, and reading or changing a host's
//! `~/.ssh/authorized_keys` over an existing connection.
//!
//! Nothing here writes the user's SSH config. Private keys never leave the
//! machine: only the public line is sent to hosts.

use std::path::{Path, PathBuf};

use russh::keys::ssh_key::{Algorithm, HashAlg, LineEnding, PrivateKey, PublicKey};
use serde::Serialize;

use crate::{Error, Result};

/// One key the user has: a file pair in `~/.ssh`, an agent identity, or both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SshKey {
    /// `SHA256:…`, the same text `ssh-keygen -l` prints.
    pub fingerprint: String,
    /// `ssh-ed25519`, `ssh-rsa`, `ecdsa-sha2-nistp256`, …
    pub algorithm: String,
    pub comment: String,
    /// The public key line (`algorithm base64 comment`).
    pub public: String,
    /// Private key path, when the pair is on disk.
    pub file: Option<String>,
    /// The private key file is passphrase protected.
    pub encrypted: bool,
    /// The SSH agent holds this key.
    pub in_agent: bool,
}

impl SshKey {
    fn from_public(key: &PublicKey) -> Self {
        Self {
            fingerprint: key.fingerprint(HashAlg::Sha256).to_string(),
            algorithm: key.algorithm().as_str().to_owned(),
            comment: key.comment().as_str_lossy().to_owned(),
            public: key.to_openssh().unwrap_or_default(),
            file: None,
            encrypted: false,
            in_agent: false,
        }
    }

    /// A short name for confirmations: the file name, else the comment.
    pub fn label(&self) -> String {
        self.file
            .as_deref()
            .and_then(|f| Path::new(f).file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| if self.comment.is_empty() { self.fingerprint.clone() } else { self.comment.clone() })
    }
}

fn key_err(e: impl std::fmt::Display) -> Error {
    Error::Invalid { field: "key", reason: e.to_string() }
}

/// Fingerprint of an OpenSSH public key line, if it parses.
pub fn fingerprint_of(public: &str) -> Option<String> {
    PublicKey::from_openssh(public.trim()).ok().map(|k| k.fingerprint(HashAlg::Sha256).to_string())
}

/// Whether a private key file is passphrase protected. OpenSSH-format keys
/// are parsed; old PEM keys carry `ENCRYPTED` in their header.
fn private_is_encrypted(text: &str) -> bool {
    match PrivateKey::from_openssh(text) {
        Ok(k) => k.is_encrypted(),
        Err(_) => text.contains("ENCRYPTED"),
    }
}

/// Key pairs in an `.ssh` directory: every `*.pub` that parses, with the
/// private file next to it when there is one. Sorted by file name.
pub fn scan_dir(dir: &Path) -> Vec<SshKey> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("pub") {
            continue;
        }
        // Certificates (`*-cert.pub`) belong to their key; skip them.
        if path.file_stem().and_then(|s| s.to_str()).is_some_and(|s| s.ends_with("-cert")) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Ok(public) = PublicKey::from_openssh(text.trim()) else { continue };
        let mut key = SshKey::from_public(&public);
        let private = path.with_extension("");
        if let Ok(p) = std::fs::read_to_string(&private) {
            key.encrypted = private_is_encrypted(&p);
            key.file = Some(private.display().to_string());
        }
        out.push(key);
    }
    out.sort_by_key(SshKey::label);
    out
}

/// Joins file keys with agent identities (by fingerprint). Agent-only keys
/// come after the files.
pub fn merge_agent(mut keys: Vec<SshKey>, agent: &[PublicKey]) -> Vec<SshKey> {
    for pk in agent {
        let fp = pk.fingerprint(HashAlg::Sha256).to_string();
        match keys.iter_mut().find(|k| k.fingerprint == fp) {
            Some(k) => k.in_agent = true,
            None => {
                let mut k = SshKey::from_public(pk);
                k.in_agent = true;
                keys.push(k);
            }
        }
    }
    keys
}

/// Public keys the SSH agent offers; an error when there is no agent.
pub async fn agent_keys(path: Option<&Path>) -> std::result::Result<Vec<PublicKey>, String> {
    let mut agent = super::client::connect_agent(path).await?;
    let ids = agent.request_identities().await.map_err(|e| format!("SSH agent: {e}"))?;
    Ok(ids.iter().map(|id| id.public_key().into_owned()).collect())
}

/// A valid key file name: letters, digits, `.`, `_`, `-`; no leading dot.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && !name.ends_with(".pub")
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn write_new(path: &Path, data: &[u8], private: bool) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        o.mode(if private { 0o600 } else { 0o644 });
    }
    #[cfg(not(unix))]
    let _ = private;
    let mut f = o.open(path)?;
    f.write_all(data)?;
    f.sync_all()
}

/// Generates an ed25519 key pair as `dir/name` and `dir/name.pub`, encrypted
/// with `passphrase` when one is given. Never overwrites an existing file.
pub fn generate(dir: &Path, name: &str, comment: &str, passphrase: Option<&str>) -> Result<SshKey> {
    if !valid_name(name) {
        return Err(Error::Invalid {
            field: "name",
            reason: "use letters, digits, '.', '_' or '-' (no leading dot)".into(),
        });
    }
    let comment = comment.trim();
    if comment.chars().any(|c| c.is_control()) {
        return Err(Error::Invalid { field: "comment", reason: "must be one line".into() });
    }
    let private_path = dir.join(name);
    let public_path = dir.join(format!("{name}.pub"));
    if private_path.exists() || public_path.exists() {
        return Err(Error::Invalid { field: "name", reason: format!("{} already exists", private_path.display()) });
    }
    let mut rng = rand_core::UnwrapErr(getrandom::SysRng);
    let mut key = PrivateKey::random(&mut rng, Algorithm::Ed25519).map_err(key_err)?;
    key.set_comment(comment);
    let public = key.public_key().clone();
    let to_write = match passphrase.filter(|p| !p.is_empty()) {
        Some(p) => key.encrypt(&mut rng, p).map_err(key_err)?,
        None => key,
    };
    let pem = to_write.to_openssh(LineEnding::LF).map_err(key_err)?;
    let line = public.to_openssh().map_err(key_err)?;
    create_ssh_dir(dir)?;
    write_new(&private_path, pem.as_bytes(), true)?;
    if let Err(e) = write_new(&public_path, format!("{line}\n").as_bytes(), false) {
        let _ = std::fs::remove_file(&private_path);
        return Err(e.into());
    }
    let mut out = SshKey::from_public(&public);
    out.file = Some(private_path.display().to_string());
    out.encrypted = to_write.is_encrypted();
    Ok(out)
}

fn create_ssh_dir(dir: &Path) -> Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Reads a public key file (`path` or `path.pub`).
pub fn read_public(path: &Path) -> Result<PublicKey> {
    let pub_path = if path.extension().and_then(|e| e.to_str()) == Some("pub") {
        path.to_path_buf()
    } else {
        PathBuf::from(format!("{}.pub", path.display()))
    };
    let text = std::fs::read_to_string(&pub_path)?;
    PublicKey::from_openssh(text.trim())
        .map_err(|e| Error::Invalid { field: "key", reason: format!("{}: {e}", pub_path.display()) })
}

// ---- authorized_keys on a host ------------------------------------------

/// One line of a host's `~/.ssh/authorized_keys`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedKey {
    /// 1-based line number in the file.
    pub line: u32,
    pub fingerprint: String,
    pub algorithm: String,
    pub comment: String,
    /// Options before the key (`from="…",no-pty`), when any.
    pub options: Option<String>,
}

/// Prints the file; prints nothing when it doesn't exist.
pub const READ_AUTHORIZED: &str = "cat ~/.ssh/authorized_keys 2>/dev/null; true";

/// Parses `authorized_keys`. Comments, blank lines and lines that don't hold a
/// key are skipped.
pub fn parse_authorized(text: &str) -> Vec<AuthorizedKey> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((options, key)) = split_options(line) {
            out.push(AuthorizedKey {
                line: i as u32 + 1,
                fingerprint: key.fingerprint(HashAlg::Sha256).to_string(),
                algorithm: key.algorithm().as_str().to_owned(),
                comment: key.comment().as_str_lossy().to_owned(),
                options,
            });
        }
    }
    out
}

/// Finds the key in a line that may start with options. Options can hold
/// quoted spaces, so try each unquoted space as the split point.
fn split_options(line: &str) -> Option<(Option<String>, PublicKey)> {
    if let Ok(k) = PublicKey::from_openssh(line) {
        return Some((None, k));
    }
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ' ' | '\t' if !quoted => {
                if let Ok(k) = PublicKey::from_openssh(line[i..].trim_start()) {
                    return Some((Some(line[..i].to_owned()), k));
                }
            }
            _ => {}
        }
    }
    None
}

/// `algorithm base64` of a public key: safe to put in single quotes (only
/// letters, digits, `-`, `@`, `.`, `+`, `/`, `=`, one space).
fn key_blob(key: &PublicKey) -> Result<String> {
    let line = key.to_openssh().map_err(key_err)?;
    let mut parts = line.split_whitespace();
    let blob = format!("{} {}", parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
    if blob.chars().all(|c| c.is_ascii_alphanumeric() || "-@.+/= ".contains(c)) && blob.contains(' ') {
        Ok(blob)
    } else {
        Err(Error::Invalid { field: "key", reason: "unexpected characters in public key".into() })
    }
}

/// A comment that can sit inside single quotes on a shell line.
fn safe_comment(c: &str) -> String {
    c.chars()
        .filter(|c| c.is_ascii_graphic() && *c != '\'' && *c != '\\' || *c == ' ')
        .collect::<String>()
        .trim()
        .to_owned()
}

/// A key's `algorithm base64` blob and the line to add (blob plus a safe
/// comment): both fit inside single quotes in sh and PowerShell.
pub fn key_line(public: &str) -> Result<(String, String)> {
    let key = &PublicKey::from_openssh(public.trim()).map_err(key_err)?;
    let blob = key_blob(key)?;
    let comment = safe_comment(key.comment().as_str_lossy());
    let line = if comment.is_empty() { blob.clone() } else { format!("{blob} {comment}") };
    Ok((blob, line))
}

/// Shell command that appends `key` to `~/.ssh/authorized_keys` unless the
/// same key (any comment) is already there. Creates `~/.ssh` (0700) and the
/// file (0600) when missing. Prints `added` or `present`.
pub fn add_command(public: &str) -> Result<String> {
    let (blob, line) = key_line(public)?;
    Ok(format!(
        "umask 077; mkdir -p ~/.ssh && chmod 700 ~/.ssh && touch ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys && \
if grep -qF '{blob}' ~/.ssh/authorized_keys; then echo present; else \
{{ [ -s ~/.ssh/authorized_keys ] && [ \"$(tail -c1 ~/.ssh/authorized_keys)\" != \"\" ] && echo; printf '%s\\n' '{line}'; }} >> ~/.ssh/authorized_keys && echo added; fi"
    ))
}

/// Shell command that removes every line holding `key` from
/// `~/.ssh/authorized_keys`, keeping the file's owner and mode (it is
/// rewritten in place). Prints how many lines were removed.
pub fn remove_command(public: &str) -> Result<String> {
    let key = &PublicKey::from_openssh(public.trim()).map_err(key_err)?;
    let blob = key_blob(key)?;
    Ok(format!(
        "f=~/.ssh/authorized_keys; [ -f \"$f\" ] || {{ echo 0; exit 0; }}; \
n=$(grep -cF '{blob}' \"$f\"); [ \"$n\" -gt 0 ] || {{ echo 0; exit 0; }}; \
t=$(mktemp ~/.ssh/.nightgrid-ak.XXXXXX) && grep -vF '{blob}' \"$f\" > \"$t\"; cat \"$t\" > \"$f\" && rm -f \"$t\" && echo \"$n\""
    ))
}

/// The public key line (no options) of the entry with this fingerprint in an
/// `authorized_keys` text.
pub fn find_authorized(text: &str, fingerprint: &str) -> Option<String> {
    text.lines()
        .filter_map(|l| split_options(l.trim()).map(|(_, k)| k))
        .find(|k| k.fingerprint(HashAlg::Sha256).to_string() == fingerprint)
        .and_then(|k| k.to_openssh().ok())
}

/// Reads the output of [`remove_command`].
pub fn parse_removed(out: &str) -> Option<u32> {
    out.lines().rev().find_map(|l| l.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ED: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl laptop@example";

    #[test]
    fn generates_a_key_pair_and_refuses_to_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let ssh = dir.path().join(".ssh");
        let k = generate(&ssh, "id_test", "me@example", None).unwrap();
        assert_eq!(k.algorithm, "ssh-ed25519");
        assert_eq!(k.comment, "me@example");
        assert!(!k.encrypted);
        assert!(k.fingerprint.starts_with("SHA256:"));
        let found = scan_dir(&ssh);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].fingerprint, k.fingerprint);
        assert_eq!(found[0].file.as_deref(), Some(ssh.join("id_test").to_str().unwrap()));
        assert!(generate(&ssh, "id_test", "", None).is_err());
        // Loads with russh like a connection would.
        russh::keys::load_secret_key(ssh.join("id_test"), None).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(ssh.join("id_test")).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn encrypts_with_a_passphrase() {
        let dir = tempfile::tempdir().unwrap();
        let k = generate(dir.path(), "locked", "", Some("correct horse")).unwrap();
        assert!(k.encrypted);
        assert!(scan_dir(dir.path())[0].encrypted);
        assert!(russh::keys::load_secret_key(dir.path().join("locked"), None).is_err());
        russh::keys::load_secret_key(dir.path().join("locked"), Some("correct horse")).unwrap();
    }

    #[test]
    fn validates_names() {
        assert!(valid_name("id_ed25519_2026"));
        assert!(valid_name("lab.key-1"));
        for bad in ["", ".hidden", "a/b", "../x", "x.pub", "with space", "ü"] {
            assert!(!valid_name(bad), "{bad}");
        }
        let dir = tempfile::tempdir().unwrap();
        assert!(generate(dir.path(), "x", "two\nlines", None).is_err());
    }

    #[test]
    fn scans_pub_files_and_merges_agent_keys() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("laptop.pub"), format!("{ED}\n")).unwrap();
        std::fs::write(dir.path().join("laptop-cert.pub"), "ssh-ed25519-cert-v01@openssh.com AAAA x").unwrap();
        std::fs::write(dir.path().join("known_hosts"), "x").unwrap();
        std::fs::write(dir.path().join("broken.pub"), "nope").unwrap();
        let keys = scan_dir(dir.path());
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].file, None, "no private file next to it");
        assert_eq!(keys[0].label(), "laptop@example");

        let agent_only = generate(&dir.path().join("other"), "a", "agent", None).unwrap();
        let merged = merge_agent(
            keys,
            &[PublicKey::from_openssh(ED).unwrap(), PublicKey::from_openssh(&agent_only.public).unwrap()],
        );
        assert_eq!(merged.len(), 2);
        assert!(merged[0].in_agent);
        assert!(merged[1].in_agent && merged[1].file.is_none());
    }

    #[test]
    fn parses_authorized_keys_with_options() {
        let other = generate(tempfile::tempdir().unwrap().path(), "k", "ops", None).unwrap();
        let text =
            format!("# managed\n\n{ED}\nfrom=\"10.0.0.0/8,192.168.1.0/24\",no-pty {}\ngarbage line\n", other.public);
        let found = parse_authorized(&text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].line, 3);
        assert_eq!(found[0].comment, "laptop@example");
        assert_eq!(found[0].options, None);
        assert_eq!(found[1].line, 4);
        assert_eq!(found[1].options.as_deref(), Some("from=\"10.0.0.0/8,192.168.1.0/24\",no-pty"));
        assert_eq!(found[1].fingerprint, other.fingerprint);
        assert_eq!(fingerprint_of(ED).as_deref(), Some(found[0].fingerprint.as_str()));
        let line = find_authorized(&text, &other.fingerprint).unwrap();
        assert_eq!(fingerprint_of(&line).as_deref(), Some(other.fingerprint.as_str()));
        assert_eq!(find_authorized(&text, "SHA256:nope"), None);
    }

    #[test]
    fn builds_quoted_shell_commands() {
        let mut k = PublicKey::from_openssh(ED).unwrap();
        k.set_comment("it's me; rm -rf /");
        let k = k.to_openssh().unwrap();
        let add = add_command(&k).unwrap();
        assert!(
            add.contains("grep -qF 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl'")
        );
        assert!(add.contains(
            "'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl its me; rm -rf /'"
        ));
        // The quote is gone, so the comment can't break out of its quotes.
        assert_eq!(add.matches('\'').count() % 2, 0);
        let rm = remove_command(&k).unwrap();
        assert!(rm.contains("grep -vF 'ssh-ed25519 AAAA"));
        assert!(!rm.contains("rm -rf /"));
        assert_eq!(parse_removed("2\n"), Some(2));
        assert_eq!(parse_removed("junk"), None);
    }

    /// Runs the commands in a real shell against a temp HOME.
    #[cfg(unix)]
    #[test]
    fn add_and_remove_round_trip_in_a_shell() {
        let home = tempfile::tempdir().unwrap();
        let run = |cmd: &str| {
            let out = std::process::Command::new("sh").arg("-c").arg(cmd).env("HOME", home.path()).output().unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            String::from_utf8(out.stdout).unwrap()
        };
        let k = ED;
        let ak = home.path().join(".ssh/authorized_keys");
        // An existing file without a trailing newline keeps its last line intact.
        std::fs::create_dir_all(home.path().join(".ssh")).unwrap();
        std::fs::write(&ak, "ssh-rsa AAAAB3Nza other").unwrap();
        assert_eq!(run(&add_command(k).unwrap()).trim(), "added");
        assert_eq!(run(&add_command(k).unwrap()).trim(), "present");
        let text = std::fs::read_to_string(&ak).unwrap();
        assert_eq!(text, format!("ssh-rsa AAAAB3Nza other\n{ED}\n"));
        assert_eq!(parse_removed(&run(&remove_command(k).unwrap())), Some(1));
        assert_eq!(std::fs::read_to_string(&ak).unwrap(), "ssh-rsa AAAAB3Nza other\n");
        assert_eq!(parse_removed(&run(&remove_command(k).unwrap())), Some(0));
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(std::fs::metadata(&ak).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let leftovers: Vec<_> = std::fs::read_dir(home.path().join(".ssh")).unwrap().flatten().collect();
        assert_eq!(leftovers.len(), 1, "temp file cleaned up");
    }
}
