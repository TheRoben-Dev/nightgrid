//! The `.env` vault (M10w): a compose stack's environment kept here,
//! encrypted, and written to the host's `.env` only when deploying.
//!
//! Every value is encrypted with age to a vault key (an X25519 identity)
//! whose secret half lives in the OS keychain (rule 1); SQLite holds only
//! ciphertext and the key's id. Rows remember which key they were written
//! with, so values imported from another computer's profile stay readable
//! once that profile's key is imported too.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::str::FromStr;

use age::secrecy::ExposeSecret;
use base64::Engine as _;
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub use zeroize::Zeroizing;

use crate::hosts::HostId;
use crate::secrets::{Secret, SecretRef, SecretStore};
use crate::store::Store;
use crate::{Error, Result};

pub type EnvVarId = i32;

/// The largest value kept (a certificate or a long JSON blob fits).
pub const MAX_VALUE: usize = 64 * 1024;
/// Variables per stack.
pub const MAX_VARS: usize = 500;

/// A variable as listed: never the value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub id: EnvVarId,
    pub host_id: HostId,
    pub stack: String,
    pub name: String,
    /// Masked in the UI until revealed.
    pub secret: bool,
    /// The key it was written with isn't in this computer's keychain.
    pub locked: bool,
    pub key_id: String,
    pub position: u32,
    /// Unix seconds.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub updated_at: i64,
}

/// A new or changed variable. `value: None` keeps the stored one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct EnvVarInput {
    pub name: String,
    pub value: Option<String>,
    pub secret: bool,
}

fn inv(field: &'static str, reason: impl Into<String>) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

/// `.env` names: a letter or `_`, then letters, digits and `_`.
pub fn valid_name(n: &str) -> bool {
    let mut c = n.chars();
    c.next().is_some_and(|f| f.is_ascii_alphabetic() || f == '_')
        && c.all(|x| x.is_ascii_alphanumeric() || x == '_')
        && n.len() <= 128
}

/// Whether a variable's name says it holds a secret (for imports).
pub fn looks_secret(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    [
        "pass",
        "secret",
        "token",
        "key",
        "auth",
        "credential",
        "private",
        "salt",
        "cookie",
        "session",
        "dsn",
        "database_url",
        "db_url",
        "cert",
        "webhook",
    ]
    .iter()
    .any(|k| n.contains(k))
}

// ---- keys ------------------------------------------------------------------------

/// A key's id: the first 16 hex digits of the SHA-256 of its public recipient.
pub fn key_id(recipient: &age::x25519::Recipient) -> String {
    let d = Sha256::digest(recipient.to_string().as_bytes());
    d.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// A key id as [`key_id`] makes them (16 hex digits).
pub fn valid_key_id(k: &str) -> bool {
    k.len() == 16 && k.chars().all(|c| c.is_ascii_hexdigit())
}

fn parse_identity(s: &str) -> Result<age::x25519::Identity> {
    age::x25519::Identity::from_str(s.trim()).map_err(|e| Error::Secrets(format!("vault key unreadable: {e}")))
}

/// The key new values are written with, made on first use. Stored twice in
/// the keychain: as `current` and under its id (which travels in profiles).
pub fn current_key(keychain: &dyn SecretStore) -> Result<(String, age::x25519::Identity)> {
    if let Some(s) = keychain.get(&SecretRef::vault("current"))? {
        let id = parse_identity(s.expose())?;
        return Ok((key_id(&id.to_public()), id));
    }
    let id = age::x25519::Identity::generate();
    let kid = key_id(&id.to_public());
    let text = Secret::new(id.to_string().expose_secret().to_owned());
    keychain.set(&SecretRef::vault(&kid), &text)?;
    keychain.set(&SecretRef::vault("current"), &text)?;
    Ok((kid, id))
}

fn key_for(keychain: &dyn SecretStore, kid: &str) -> Result<Option<age::x25519::Identity>> {
    Ok(match keychain.get(&SecretRef::vault(kid))? {
        Some(s) => Some(parse_identity(s.expose())?),
        None => None,
    })
}

/// After importing keys from a profile: a computer with no vault key yet
/// adopts the first imported one as its own.
pub fn adopt_key(keychain: &dyn SecretStore, kid: &str) -> Result<()> {
    if keychain.get(&SecretRef::vault("current"))?.is_none() {
        if let Some(s) = keychain.get(&SecretRef::vault(kid))? {
            keychain.set(&SecretRef::vault("current"), &s)?;
        }
    }
    Ok(())
}

pub fn encrypt(recipient: &age::x25519::Recipient, value: &str) -> Result<String> {
    let enc = age::Encryptor::with_recipients(std::iter::once(recipient as &dyn age::Recipient))
        .map_err(|e| Error::Secrets(format!("encrypt: {e}")))?;
    let mut out = Vec::new();
    let mut w = enc.wrap_output(&mut out).map_err(|e| Error::Secrets(format!("encrypt: {e}")))?;
    w.write_all(value.as_bytes()).map_err(|e| Error::Secrets(format!("encrypt: {e}")))?;
    w.finish().map_err(|e| Error::Secrets(format!("encrypt: {e}")))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(out))
}

pub fn decrypt(identity: &age::x25519::Identity, value: &str) -> Result<Zeroizing<String>> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|_| Error::Secrets("vault value isn't readable".into()))?;
    let dec = age::Decryptor::new(&bytes[..]).map_err(|e| Error::Secrets(format!("decrypt: {e}")))?;
    let mut r = dec
        .decrypt(std::iter::once(identity as &dyn age::Identity))
        .map_err(|e| Error::Secrets(format!("decrypt: {e}")))?;
    let mut out = Zeroizing::new(String::new());
    r.read_to_string(&mut out).map_err(|e| Error::Secrets(format!("decrypt: {e}")))?;
    Ok(out)
}

// ---- .env text ---------------------------------------------------------------------------

/// One `.env` value, quoted only when it has to be. Single quotes are taken
/// literally by Compose; values with a `'` or a line break use double
/// quotes with escapes, and `$` doubled so Compose doesn't interpolate it.
pub fn quote(v: &str) -> String {
    if v.chars().all(|c| c.is_ascii_alphanumeric() || "_./:@%+,=-".contains(c)) {
        return v.to_owned();
    }
    if !v.contains('\'') && !v.contains('\n') && !v.contains('\r') {
        return format!("'{v}'");
    }
    let mut s = String::from("\"");
    for c in v.chars() {
        match c {
            '\\' => s.push_str("\\\\"),
            '"' => s.push_str("\\\""),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '$' => s.push_str("$$"),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

pub const HEADER: &str = "# Written by Nightgrid from its .env vault (Docker → Stacks → ENV).\n# Changes made here are replaced the next time it deploys.\n";

/// The `.env` file for these (name, value) pairs, in order.
pub fn render(vars: &[(String, Zeroizing<String>)]) -> Zeroizing<String> {
    let mut out = Zeroizing::new(String::from(HEADER));
    for (n, v) in vars {
        out.push_str(n);
        out.push('=');
        out.push_str(&quote(v));
        out.push('\n');
    }
    out
}

/// Variables in a `.env` file, as Compose reads them: `KEY=value`,
/// `export KEY=value`, single quotes literal, double quotes with `\n`, `\"`
/// and `\\` escapes, `#` comments (inline ones after a space, unquoted).
pub fn parse_env(text: &str) -> Vec<(String, Zeroizing<String>)> {
    let mut out: Vec<(String, Zeroizing<String>)> = Vec::new();
    let mut lines = text.lines();
    while let Some(raw) = lines.next() {
        let line = raw.trim_start();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((k, v)) = line.split_once('=') else { continue };
        let k = k.trim();
        if !valid_name(k) {
            continue;
        }
        let v = v.trim_start();
        let value = if let Some(rest) = v.strip_prefix('\'') {
            match rest.find('\'') {
                Some(e) => rest[..e].to_owned(),
                None => rest.to_owned(),
            }
        } else if let Some(rest) = v.strip_prefix('"') {
            // May run over several lines.
            let mut buf = rest.to_owned();
            while !closes(&buf) {
                match lines.next() {
                    Some(l) => {
                        buf.push('\n');
                        buf.push_str(l);
                    }
                    None => break,
                }
            }
            unescape(&buf)
        } else {
            let v = match v.find(" #") {
                Some(i) => &v[..i],
                None => v,
            };
            v.trim_end().to_owned()
        };
        let value = Zeroizing::new(value);
        match out.iter_mut().find(|(n, _)| n == k) {
            Some(slot) => slot.1 = value,
            None => out.push((k.to_owned(), value)),
        }
    }
    out
}

/// Whether a double-quoted value (without its opening quote) has its closing quote.
fn closes(s: &str) -> bool {
    let mut esc = false;
    for c in s.chars() {
        match (esc, c) {
            (true, _) => esc = false,
            (false, '\\') => esc = true,
            (false, '"') => return true,
            _ => {}
        }
    }
    false
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        match c {
            '\\' => match it.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some(o) => out.push(o),
                None => {}
            },
            '"' => break,
            '$' if it.clone().next() == Some('$') => {
                it.next();
                out.push('$');
            }
            c => out.push(c),
        }
    }
    out
}

/// What deploying would change in the host's `.env`, by name only.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct EnvDiff {
    /// The host has a `.env` now.
    pub exists: bool,
    pub added: Vec<String>,
    pub changed: Vec<String>,
    /// On the host but not in the vault: deploying drops them.
    pub removed: Vec<String>,
    pub unchanged: u32,
}

pub fn diff(current: &[(String, Zeroizing<String>)], new: &[(String, Zeroizing<String>)], exists: bool) -> EnvDiff {
    let cur: BTreeMap<&str, &str> = current.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let mut d = EnvDiff { exists, ..Default::default() };
    for (k, v) in new {
        match cur.get(k.as_str()) {
            None => d.added.push(k.clone()),
            Some(c) if *c != v.as_str() => d.changed.push(k.clone()),
            Some(_) => d.unchanged += 1,
        }
    }
    for (k, _) in current {
        if !new.iter().any(|(n, _)| n == k) {
            d.removed.push(k.clone());
        }
    }
    d
}

// ---- store ------------------------------------------------------------------------------

const COLS: &str = "id, host_id, stack, name, masked, key_id, position, updated_at";

fn from_row(r: &Row<'_>) -> rusqlite::Result<EnvVar> {
    Ok(EnvVar {
        id: r.get(0)?,
        host_id: r.get(1)?,
        stack: r.get(2)?,
        name: r.get(3)?,
        secret: r.get(4)?,
        key_id: r.get(5)?,
        position: r.get(6)?,
        updated_at: r.get(7)?,
        locked: false,
    })
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl Store {
    /// A stack's variables in order; `locked` set for keys not in `keychain`.
    pub fn env_vars(&self, keychain: &dyn SecretStore, host_id: HostId, stack: &str) -> Result<Vec<EnvVar>> {
        let mut st = self
            .conn()
            .prepare(&format!("SELECT {COLS} FROM env_vars WHERE host_id = ?1 AND stack = ?2 ORDER BY position, id"))?;
        let mut v = st.query_map(params![host_id, stack], from_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut known: BTreeMap<String, bool> = BTreeMap::new();
        for e in &mut v {
            let has = match known.get(&e.key_id) {
                Some(h) => *h,
                None => {
                    let h = keychain.get(&SecretRef::vault(&e.key_id))?.is_some();
                    known.insert(e.key_id.clone(), h);
                    h
                }
            };
            e.locked = !has;
        }
        Ok(v)
    }

    /// Stacks with variables, per host: (host id, stack, count).
    pub fn env_stacks(&self) -> Result<Vec<(HostId, String, u32)>> {
        let mut st = self
            .conn()
            .prepare("SELECT host_id, stack, COUNT(*) FROM env_vars GROUP BY host_id, stack ORDER BY host_id, stack")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(v)
    }

    /// Adds or changes a variable (by name). Values are encrypted with the
    /// current vault key. Renaming is `old_name` → `input.name`.
    pub fn set_env_var(
        &mut self,
        keychain: &dyn SecretStore,
        host_id: HostId,
        stack: &str,
        old_name: Option<&str>,
        input: &EnvVarInput,
    ) -> Result<EnvVar> {
        if !valid_name(&input.name) {
            return Err(inv("name", "letters, digits and _, not starting with a digit"));
        }
        if !crate::docker::compose::valid_project(stack) {
            return Err(inv("stack", "not a compose project name"));
        }
        if input.value.as_ref().is_some_and(|v| v.len() > MAX_VALUE || v.contains('\0')) {
            return Err(inv("value", "too long or not text"));
        }
        let lookup = old_name.unwrap_or(&input.name);
        let existing: Option<(EnvVarId, String, String)> = self
            .conn()
            .query_row(
                "SELECT id, value, key_id FROM env_vars WHERE host_id = ?1 AND stack = ?2 AND name = ?3",
                params![host_id, stack, lookup],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if old_name.is_some_and(|o| o != input.name) {
            let clash: Option<i64> = self
                .conn()
                .query_row(
                    "SELECT id FROM env_vars WHERE host_id = ?1 AND stack = ?2 AND name = ?3",
                    params![host_id, stack, input.name],
                    |r| r.get(0),
                )
                .optional()?;
            if clash.is_some() {
                return Err(inv("name", format!("{} is already set", input.name)));
            }
        }
        let (value, kid) = match (&input.value, &existing) {
            (Some(v), _) => {
                let (kid, id) = current_key(keychain)?;
                (encrypt(&id.to_public(), v)?, kid)
            }
            (None, Some((_, value, kid))) => (value.clone(), kid.clone()),
            (None, None) => return Err(inv("value", "give a value")),
        };
        let id = match existing {
            Some((id, _, _)) => {
                self.conn().execute(
                    "UPDATE env_vars SET name = ?2, value = ?3, key_id = ?4, masked = ?5, updated_at = ?6 WHERE id = ?1",
                    params![id, input.name, value, kid, input.secret, now()],
                )?;
                id
            }
            None => {
                let count: i64 = self.conn().query_row(
                    "SELECT COUNT(*) FROM env_vars WHERE host_id = ?1 AND stack = ?2",
                    params![host_id, stack],
                    |r| r.get(0),
                )?;
                if count as usize >= MAX_VARS {
                    return Err(inv("name", "too many variables in this stack"));
                }
                self.conn().execute(
                    "INSERT INTO env_vars (host_id, stack, name, value, key_id, masked, position, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![host_id, stack, input.name, value, kid, input.secret, count, now()],
                )?;
                self.conn().last_insert_rowid() as EnvVarId
            }
        };
        self.conn().query_row(&format!("SELECT {COLS} FROM env_vars WHERE id = ?1"), [id], from_row).map_err(Into::into)
    }

    pub fn delete_env_var(&mut self, id: EnvVarId) -> Result<()> {
        if self.conn().execute("DELETE FROM env_vars WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("variable"));
        }
        Ok(())
    }

    /// One value, decrypted (for reveal and deploy).
    pub fn env_value(&self, keychain: &dyn SecretStore, id: EnvVarId) -> Result<Zeroizing<String>> {
        let (value, kid): (String, String) = self
            .conn()
            .query_row("SELECT value, key_id FROM env_vars WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?
            .ok_or(Error::NotFound("variable"))?;
        let key = key_for(keychain, &kid)?.ok_or_else(|| {
            Error::Secrets(
                "this value was saved on another computer: import a profile with its secrets to read it".into(),
            )
        })?;
        decrypt(&key, &value)
    }

    /// Every (name, value) of a stack, in order, for the `.env` file.
    pub fn env_values(
        &self,
        keychain: &dyn SecretStore,
        host_id: HostId,
        stack: &str,
    ) -> Result<Vec<(String, Zeroizing<String>)>> {
        let vars = self.env_vars(keychain, host_id, stack)?;
        if let Some(v) = vars.iter().find(|v| v.locked) {
            return Err(Error::Secrets(format!(
                "{} was saved on another computer: import a profile with its secrets first",
                v.name
            )));
        }
        vars.iter().map(|v| Ok((v.name.clone(), self.env_value(keychain, v.id)?))).collect()
    }

    /// Distinct vault key ids in use (for profile exports).
    pub fn env_key_ids(&self) -> Result<Vec<String>> {
        let mut st = self.conn().prepare("SELECT DISTINCT key_id FROM env_vars ORDER BY key_id")?;
        let v = st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(v)
    }

    /// Imports (name, value) pairs into a stack: new names are added, existing
    /// ones get the new value. Returns how many were written.
    pub fn import_env(
        &mut self,
        keychain: &dyn SecretStore,
        host_id: HostId,
        stack: &str,
        vars: &[(String, Zeroizing<String>)],
    ) -> Result<u32> {
        let mut n = 0;
        for (name, value) in vars {
            let secret = self
                .conn()
                .query_row(
                    "SELECT masked FROM env_vars WHERE host_id = ?1 AND stack = ?2 AND name = ?3",
                    params![host_id, stack, name],
                    |r| r.get::<_, bool>(0),
                )
                .optional()?
                .unwrap_or_else(|| looks_secret(name));
            let input = EnvVarInput { name: name.clone(), value: Some(value.to_string()), secret };
            self.set_env_var(keychain, host_id, stack, None, &input)?;
            n += 1;
        }
        Ok(n)
    }

    /// Changes the order of a stack's variables to `ids`.
    pub fn order_env_vars(&mut self, host_id: HostId, stack: &str, ids: &[EnvVarId]) -> Result<()> {
        for (i, id) in ids.iter().enumerate() {
            self.conn().execute(
                "UPDATE env_vars SET position = ?1 WHERE id = ?2 AND host_id = ?3 AND stack = ?4",
                params![i as i64, id, host_id, stack],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::HostInput;
    use crate::secrets::MemoryStore;

    fn z(s: &str) -> Zeroizing<String> {
        Zeroizing::new(s.to_owned())
    }

    #[test]
    fn quotes_and_parses() {
        let vars = vec![
            ("PLAIN".to_owned(), z("abc-123_x.y:z@h/p")),
            ("SPACE".to_owned(), z("two words # not a comment")),
            ("DOLLAR".to_owned(), z("pa$$word")),
            ("QUOTE".to_owned(), z("it's $HOME \"x\" \\ end")),
            ("MULTI".to_owned(), z("line1\nline2")),
            ("EMPTY".to_owned(), z("")),
        ];
        let text = render(&vars);
        assert!(text.starts_with(HEADER));
        assert!(text.contains("PLAIN=abc-123_x.y:z@h/p\n"));
        assert!(text.contains("SPACE='two words # not a comment'\n"));
        assert!(text.contains("DOLLAR='pa$$word'\n"));
        assert!(text.contains("QUOTE=\"it's $$HOME \\\"x\\\" \\\\ end\"\n"));
        assert!(text.contains("EMPTY=\n"));
        let back = parse_env(&text);
        assert_eq!(back.len(), vars.len());
        for ((a, av), (b, bv)) in vars.iter().zip(back.iter()) {
            assert_eq!((a, av.as_str()), (b, bv.as_str()));
        }
    }

    #[test]
    fn parses_real_env_files() {
        let t = "# comment\nexport TZ=Europe/Stockholm\nPUID = 1000\nURL=http://x:80 # inline\nHASH=abc#def\nKEY=\"multi\nline\"\n bad line\n1BAD=x\nTZ=UTC\n";
        let v = parse_env(t);
        let get = |k: &str| v.iter().find(|(n, _)| n == k).map(|(_, x)| x.as_str().to_owned());
        assert_eq!(get("TZ").as_deref(), Some("UTC"), "later value wins");
        assert_eq!(get("PUID").as_deref(), Some("1000"));
        assert_eq!(get("URL").as_deref(), Some("http://x:80"));
        assert_eq!(get("HASH").as_deref(), Some("abc#def"));
        assert_eq!(get("KEY").as_deref(), Some("multi\nline"));
        assert!(get("1BAD").is_none());
        assert_eq!(v.len(), 5);
    }

    #[test]
    fn diffs_by_name() {
        let cur = vec![("A".into(), z("1")), ("B".into(), z("2")), ("OLD".into(), z("x"))];
        let new = vec![("A".into(), z("1")), ("B".into(), z("3")), ("C".into(), z("4"))];
        let d = diff(&cur, &new, true);
        assert_eq!(
            (d.added, d.changed, d.removed, d.unchanged),
            (vec!["C".to_owned()], vec!["B".to_owned()], vec!["OLD".to_owned()], 1)
        );
    }

    #[test]
    fn names() {
        assert!(
            valid_name("DB_PASSWORD") && valid_name("_x") && !valid_name("1A") && !valid_name("A-B") && !valid_name("")
        );
        assert!(
            looks_secret("POSTGRES_PASSWORD")
                && looks_secret("api_key")
                && !looks_secret("TZ")
                && !looks_secret("PUID")
        );
    }

    #[test]
    fn vault_round_trip() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&HostInput { name: "a".into(), hostname: "192.168.1.10".into(), ..Default::default() })
            .unwrap();
        let kc = MemoryStore::new();
        let v = s
            .set_env_var(
                &kc,
                h.id,
                "app",
                None,
                &EnvVarInput { name: "DB_PASSWORD".into(), value: Some("s3cr3t".into()), secret: true },
            )
            .unwrap();
        // The database never holds the value.
        let raw: String = s.conn().query_row("SELECT value FROM env_vars WHERE id = ?1", [v.id], |r| r.get(0)).unwrap();
        assert!(!raw.contains("s3cr3t"));
        assert_eq!(s.env_value(&kc, v.id).unwrap().as_str(), "s3cr3t");
        // Keep the value, change the flag; rename.
        let v2 = s
            .set_env_var(
                &kc,
                h.id,
                "app",
                Some("DB_PASSWORD"),
                &EnvVarInput { name: "DB_PASS".into(), value: None, secret: false },
            )
            .unwrap();
        assert_eq!((v2.id, v2.name.as_str(), v2.secret), (v.id, "DB_PASS", false));
        assert_eq!(s.env_value(&kc, v.id).unwrap().as_str(), "s3cr3t");
        assert!(s
            .set_env_var(&kc, h.id, "app", None, &EnvVarInput { name: "NEW".into(), value: None, secret: true })
            .is_err());
        assert!(s
            .set_env_var(
                &kc,
                h.id,
                "app",
                None,
                &EnvVarInput { name: "bad-name".into(), value: Some("x".into()), secret: true }
            )
            .is_err());

        let n = s.import_env(&kc, h.id, "app", &parse_env("TZ=UTC\nAPI_TOKEN=abc\nDB_PASS=new\n")).unwrap();
        assert_eq!(n, 3);
        let list = s.env_vars(&kc, h.id, "app").unwrap();
        assert_eq!(
            list.iter().map(|v| (v.name.as_str(), v.secret)).collect::<Vec<_>>(),
            [("DB_PASS", false), ("TZ", false), ("API_TOKEN", true)]
        );
        let vals = s.env_values(&kc, h.id, "app").unwrap();
        assert_eq!(vals[0].1.as_str(), "new");
        assert_eq!(s.env_key_ids().unwrap().len(), 1);

        // Another computer: same rows, no key → locked until the key is imported.
        let kc2 = MemoryStore::new();
        assert!(s.env_vars(&kc2, h.id, "app").unwrap().iter().all(|v| v.locked));
        assert!(s.env_values(&kc2, h.id, "app").is_err());
        let kid = &list[0].key_id;
        kc2.set(&SecretRef::vault(kid), &kc.get(&SecretRef::vault(kid)).unwrap().unwrap()).unwrap();
        adopt_key(&kc2, kid).unwrap();
        assert_eq!(s.env_values(&kc2, h.id, "app").unwrap()[1].1.as_str(), "UTC");
        assert_eq!(current_key(&kc2).unwrap().0, *kid, "adopted the imported key");

        s.order_env_vars(h.id, "app", &[list[2].id, list[0].id, list[1].id]).unwrap();
        assert_eq!(s.env_vars(&kc, h.id, "app").unwrap()[0].name, "API_TOKEN");
        s.delete_env_var(list[1].id).unwrap();
        assert_eq!(s.env_stacks().unwrap(), vec![(h.id, "app".to_owned(), 2)]);
    }
}
