//! Hosts, groups and tags.

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::store::Store;
use crate::{Error, Result};

pub type HostId = i32;
pub type GroupId = i32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum AuthMethod {
    /// SSH agent (OpenSSH agent, Pageant, 1Password, Secretive, ...). Preferred.
    #[default]
    Agent,
    /// Private key file, passphrase from the keychain or a prompt.
    Key,
    /// Password, from the keychain (opt-in) or a prompt.
    Password,
}

impl AuthMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Key => "key",
            Self::Password => "password",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "key" => Self::Key,
            "password" => Self::Password,
            _ => Self::Agent,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Host {
    pub id: HostId,
    pub name: String,
    pub hostname: String,
    pub port: u16,
    pub username: Option<String>,
    pub group_id: Option<GroupId>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub favorite: bool,
    pub notes: Option<String>,
    pub auth_method: AuthMethod,
    pub identity_file: Option<String>,
    /// `ProxyJump` value: comma-separated `[user@]host[:port]` or names/aliases of other hosts.
    pub proxy_jump: Option<String>,
    /// Keep the password / key passphrase in the OS keychain.
    pub remember_secret: bool,
    pub ssh_alias: Option<String>,
    pub tags: Vec<String>,
    /// Second address (100.x or MagicDNS name) tried when `hostname` doesn't answer.
    pub tailnet_address: Option<String>,
    /// DRIFT may read this host's recent shell history (opt-in).
    pub drift_history: bool,
    /// DRIFT may run read-only checks on this host (opt-in, D-151).
    pub drift_lookups: bool,
    /// Extra folders DRIFT may read files from (one per line).
    pub drift_paths: Option<String>,
    /// For Wake-on-LAN (`aa:bb:cc:dd:ee:ff`).
    pub mac_address: Option<String>,
    /// Another saved host on this host's LAN that sends the magic packet.
    pub wake_relay_id: Option<HostId>,
    /// Local hours the host is expected to be asleep (`23:00-07:00`).
    pub sleep_window: Option<String>,
    /// `server` or `client` (a laptop, handheld or desktop: never picked by default).
    pub role: String,
}

/// Fields the user edits. Missing optional fields mean "unset".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct HostInput {
    pub name: String,
    pub hostname: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub group_id: Option<GroupId>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub favorite: bool,
    pub notes: Option<String>,
    pub auth_method: AuthMethod,
    pub identity_file: Option<String>,
    pub proxy_jump: Option<String>,
    pub remember_secret: bool,
    pub ssh_alias: Option<String>,
    pub tags: Vec<String>,
    /// Second address (100.x or MagicDNS name) tried when `hostname` doesn't answer.
    pub tailnet_address: Option<String>,
    /// DRIFT may read this host's recent shell history (opt-in).
    pub drift_history: bool,
    /// DRIFT may run read-only checks on this host (opt-in, D-151).
    pub drift_lookups: bool,
    /// Extra folders DRIFT may read files from (one per line).
    pub drift_paths: Option<String>,
    /// For Wake-on-LAN (`aa:bb:cc:dd:ee:ff`).
    pub mac_address: Option<String>,
    /// Another saved host on this host's LAN that sends the magic packet.
    pub wake_relay_id: Option<HostId>,
    /// Local hours the host is expected to be asleep (`23:00-07:00`).
    pub sleep_window: Option<String>,
    /// `server` or `client` (a laptop, handheld or desktop: never picked by default).
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct GroupInput {
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub sort_order: i32,
}

fn blank_to_none(v: &Option<String>) -> Option<String> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned)
}

fn valid_color(c: &Option<String>) -> Result<()> {
    if let Some(c) = c {
        let ok = c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit());
        if !ok {
            return Err(Error::Invalid { field: "color", reason: "use #RRGGBB".into() });
        }
    }
    Ok(())
}

/// Largest ASCII icon: lines × columns.
pub const ICON_MAX_LINES: usize = 8;
pub const ICON_MAX_COLS: usize = 32;

/// Characters an ASCII icon may use: printable ASCII and Latin-1, arrows,
/// box drawing, blocks, geometric shapes and braille (all single-width in
/// monospace fonts).
fn icon_char_ok(c: char) -> bool {
    matches!(c, ' '..='~' | '\u{a0}'..='\u{ff}' | '\u{2190}'..='\u{21ff}' | '\u{2500}'..='\u{25ff}' | '\u{2800}'..='\u{28ff}')
}

/// Normalizes a host's ASCII icon: unix newlines, tabs as spaces, trailing
/// spaces, blank edge lines and the common indent removed. `None` if empty.
pub fn normalize_icon(icon: &Option<String>) -> Result<Option<String>> {
    let Some(raw) = icon else { return Ok(None) };
    let text = raw.replace("\r\n", "\n").replace('\r', "\n").replace('\t', "    ");
    if let Some(c) = text.chars().find(|c| *c != '\n' && !icon_char_ok(*c)) {
        return Err(Error::Invalid {
            field: "icon",
            reason: format!("{c:?} can't be used; stick to ASCII, box drawing and block characters"),
        });
    }
    let mut lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    while lines.first().is_some_and(|l| l.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        return Ok(None);
    }
    let indent =
        lines.iter().filter(|l| !l.is_empty()).map(|l| l.len() - l.trim_start_matches(' ').len()).min().unwrap_or(0);
    let lines: Vec<&str> = lines.iter().map(|l| if l.is_empty() { *l } else { &l[indent..] }).collect();
    if lines.len() > ICON_MAX_LINES {
        return Err(Error::Invalid { field: "icon", reason: format!("at most {ICON_MAX_LINES} lines") });
    }
    if lines.iter().any(|l| l.chars().count() > ICON_MAX_COLS) {
        return Err(Error::Invalid { field: "icon", reason: format!("at most {ICON_MAX_COLS} characters per line") });
    }
    Ok(Some(lines.join("\n")))
}

impl HostInput {
    /// Trims and validates; returns a normalized copy.
    pub fn normalized(&self) -> Result<Self> {
        let mut h = self.clone();
        h.name = h.name.trim().to_owned();
        h.hostname = h.hostname.trim().to_owned();
        if h.name.is_empty() {
            return Err(Error::Invalid { field: "name", reason: "required".into() });
        }
        if h.hostname.is_empty() || h.hostname.chars().any(char::is_whitespace) {
            return Err(Error::Invalid { field: "hostname", reason: "required, no spaces".into() });
        }
        if h.port == Some(0) {
            return Err(Error::Invalid { field: "port", reason: "must be 1–65535".into() });
        }
        h.username = blank_to_none(&h.username);
        h.color = blank_to_none(&h.color);
        h.icon = normalize_icon(&h.icon)?;
        h.notes = blank_to_none(&h.notes);
        h.identity_file = blank_to_none(&h.identity_file);
        h.proxy_jump = blank_to_none(&h.proxy_jump);
        h.ssh_alias = blank_to_none(&h.ssh_alias);
        h.tailnet_address = blank_to_none(&h.tailnet_address);
        h.drift_paths = blank_to_none(&h.drift_paths)
            .map(|p| p.lines().map(str::trim).filter(|l| !l.is_empty()).take(40).collect::<Vec<_>>().join("\n"));
        if let Some(bad) = h.drift_paths.as_deref().and_then(|p| {
            p.lines().find(|l| !(l.starts_with('/') || l.starts_with("~/")) || l.split('/').any(|c| c == ".."))
        }) {
            return Err(Error::Invalid {
                field: "driftPaths",
                reason: format!("\"{bad}\" isn't an absolute folder (start it with / or ~/)"),
            });
        }
        if h.tailnet_address.as_deref().is_some_and(|a| a.chars().any(char::is_whitespace) || a.contains('/')) {
            return Err(Error::Invalid { field: "tailnetAddress", reason: "a host name or address, no spaces".into() });
        }
        h.mac_address = match blank_to_none(&h.mac_address) {
            Some(m) => Some(crate::power::format_mac(&crate::power::parse_mac(&m)?)),
            None => None,
        };
        h.role = match h.role.trim() {
            "" | "server" => "server".into(),
            "client" => "client".into(),
            other => return Err(Error::Invalid { field: "role", reason: format!("{other:?}: use server or client") }),
        };
        h.sleep_window = match blank_to_none(&h.sleep_window) {
            Some(w) => Some(crate::power::SleepWindow::parse(&w)?.to_string()),
            None => None,
        };
        valid_color(&h.color)?;
        if h.auth_method == AuthMethod::Key && h.identity_file.is_none() {
            return Err(Error::Invalid { field: "identityFile", reason: "required for key authentication".into() });
        }
        let mut tags: Vec<String> = h.tags.iter().map(|t| t.trim().to_owned()).filter(|t| !t.is_empty()).collect();
        tags.sort_by_key(|t| t.to_lowercase());
        tags.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        h.tags = tags;
        Ok(h)
    }
}

const HOST_COLS: &str = "id, name, hostname, port, username, group_id, color, icon, favorite, notes, \
     auth_method, identity_file, proxy_jump, remember_secret, ssh_alias, tailnet_address, drift_history, mac_address, wake_relay_id, sleep_window, role, drift_lookups, drift_paths";

fn host_from_row(r: &Row<'_>) -> rusqlite::Result<Host> {
    Ok(Host {
        id: r.get(0)?,
        name: r.get(1)?,
        hostname: r.get(2)?,
        port: r.get(3)?,
        username: r.get(4)?,
        group_id: r.get(5)?,
        color: r.get(6)?,
        icon: r.get(7)?,
        favorite: r.get(8)?,
        notes: r.get(9)?,
        auth_method: AuthMethod::parse(&r.get::<_, String>(10)?),
        identity_file: r.get(11)?,
        proxy_jump: r.get(12)?,
        remember_secret: r.get(13)?,
        ssh_alias: r.get(14)?,
        tags: Vec::new(),
        tailnet_address: r.get(15)?,
        drift_history: r.get(16)?,
        mac_address: r.get(17)?,
        wake_relay_id: r.get(18)?,
        sleep_window: r.get(19)?,
        role: r.get(20)?,
        drift_lookups: r.get(21)?,
        drift_paths: r.get(22)?,
    })
}

fn write_tags(tx: &rusqlite::Transaction<'_>, host: HostId, tags: &[String]) -> Result<()> {
    tx.execute("DELETE FROM host_tags WHERE host_id = ?1", [host])?;
    for t in tags {
        tx.execute("INSERT INTO tags (name) VALUES (?1) ON CONFLICT(name) DO NOTHING", [t])?;
        tx.execute(
            "INSERT INTO host_tags (host_id, tag_id) SELECT ?1, id FROM tags WHERE name = ?2",
            params![host, t],
        )?;
    }
    // Drop tags no host uses any more.
    tx.execute("DELETE FROM tags WHERE id NOT IN (SELECT tag_id FROM host_tags)", [])?;
    Ok(())
}

impl Store {
    pub fn list_hosts(&self) -> Result<Vec<Host>> {
        let mut stmt = self.conn().prepare(&format!(
            "SELECT {HOST_COLS} FROM hosts ORDER BY favorite DESC, sort_order, name COLLATE NOCASE"
        ))?;
        let mut hosts: Vec<Host> = stmt.query_map([], host_from_row)?.collect::<rusqlite::Result<_>>()?;
        let mut tag_stmt = self.conn().prepare(
            "SELECT ht.host_id, t.name FROM host_tags ht JOIN tags t ON t.id = ht.tag_id ORDER BY t.name COLLATE NOCASE",
        )?;
        let pairs = tag_stmt.query_map([], |r| Ok((r.get::<_, HostId>(0)?, r.get::<_, String>(1)?)))?;
        for p in pairs {
            let (id, name) = p?;
            if let Some(h) = hosts.iter_mut().find(|h| h.id == id) {
                h.tags.push(name);
            }
        }
        Ok(hosts)
    }

    pub fn get_host(&self, id: HostId) -> Result<Option<Host>> {
        let host = self
            .conn()
            .query_row(&format!("SELECT {HOST_COLS} FROM hosts WHERE id = ?1"), [id], host_from_row)
            .optional()?;
        let Some(mut host) = host else { return Ok(None) };
        let mut stmt = self.conn().prepare(
            "SELECT t.name FROM host_tags ht JOIN tags t ON t.id = ht.tag_id WHERE ht.host_id = ?1 ORDER BY t.name COLLATE NOCASE",
        )?;
        host.tags = stmt.query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        Ok(Some(host))
    }

    pub fn find_host_by_name_or_alias(&self, name: &str) -> Result<Option<Host>> {
        let id: Option<HostId> = self
            .conn()
            .query_row(
                "SELECT id FROM hosts WHERE ssh_alias = ?1 OR name = ?1 COLLATE NOCASE ORDER BY ssh_alias IS NULL LIMIT 1",
                [name],
                |r| r.get(0),
            )
            .optional()?;
        id.map(|id| self.get_host(id)).transpose().map(Option::flatten)
    }

    pub fn create_host(&mut self, input: &HostInput) -> Result<Host> {
        let h = input.normalized()?;
        let id = self.tx(|tx| {
            tx.execute(
                "INSERT INTO hosts (name, hostname, port, username, group_id, color, icon, favorite, notes,
                    auth_method, identity_file, proxy_jump, remember_secret, ssh_alias, tailnet_address, drift_history,
                    mac_address, wake_relay_id, sleep_window, role, drift_lookups, drift_paths)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
                params![
                    h.name,
                    h.hostname,
                    h.port.unwrap_or(22),
                    h.username,
                    h.group_id,
                    h.color,
                    h.icon,
                    h.favorite,
                    h.notes,
                    h.auth_method.as_str(),
                    h.identity_file,
                    h.proxy_jump,
                    h.remember_secret,
                    h.ssh_alias,
                    h.tailnet_address,
                    h.drift_history,
                    h.mac_address,
                    h.wake_relay_id,
                    h.sleep_window,
                    h.role,
                    h.drift_lookups,
                    h.drift_paths
                ],
            )?;
            let id = tx.last_insert_rowid() as HostId;
            write_tags(tx, id, &h.tags)?;
            Ok(id)
        })?;
        self.get_host(id)?.ok_or_else(|| Error::NotFound("host"))
    }

    pub fn update_host(&mut self, id: HostId, input: &HostInput) -> Result<Host> {
        let h = input.normalized()?;
        let changed = self.tx(|tx| {
            let n = tx.execute(
                "UPDATE hosts SET name = ?2, hostname = ?3, port = ?4, username = ?5, group_id = ?6, color = ?7,
                    icon = ?8, favorite = ?9, notes = ?10, auth_method = ?11, identity_file = ?12,
                    proxy_jump = ?13, remember_secret = ?14, ssh_alias = ?15,
                    tailnet_address = ?16, drift_history = ?17, mac_address = ?18,
                    wake_relay_id = ?19, sleep_window = ?20, role = ?21, drift_lookups = ?22, drift_paths = ?23, updated_at = unixepoch()
                 WHERE id = ?1",
                params![
                    id,
                    h.name,
                    h.hostname,
                    h.port.unwrap_or(22),
                    h.username,
                    h.group_id,
                    h.color,
                    h.icon,
                    h.favorite,
                    h.notes,
                    h.auth_method.as_str(),
                    h.identity_file,
                    h.proxy_jump,
                    h.remember_secret,
                    h.ssh_alias,
                    h.tailnet_address,
                    h.drift_history,
                    h.mac_address,
                    h.wake_relay_id,
                    h.sleep_window,
                    h.role,
                    h.drift_lookups,
                    h.drift_paths
                ],
            )?;
            if n > 0 {
                write_tags(tx, id, &h.tags)?;
            }
            Ok(n)
        })?;
        if changed == 0 {
            return Err(Error::NotFound("host"));
        }
        self.get_host(id)?.ok_or(Error::NotFound("host"))
    }

    /// Sets only the "remember secret in keychain" flag (from an auth prompt).
    pub fn set_remember_secret(&mut self, id: HostId, remember: bool) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE hosts SET remember_secret = ?2, updated_at = unixepoch() WHERE id = ?1",
            rusqlite::params![id, remember],
        )?;
        if n == 0 {
            return Err(Error::NotFound("host"));
        }
        Ok(())
    }

    pub fn delete_host(&mut self, id: HostId) -> Result<()> {
        self.tx(|tx| {
            tx.execute("DELETE FROM hosts WHERE id = ?1", [id])?;
            tx.execute("DELETE FROM tags WHERE id NOT IN (SELECT tag_id FROM host_tags)", [])?;
            Ok(())
        })
    }

    pub fn list_groups(&self) -> Result<Vec<Group>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, name, color, icon, sort_order FROM host_groups ORDER BY sort_order, name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Group { id: r.get(0)?, name: r.get(1)?, color: r.get(2)?, icon: r.get(3)?, sort_order: r.get(4)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn get_group(&self, id: GroupId) -> Result<Group> {
        self.list_groups()?.into_iter().find(|g| g.id == id).ok_or(Error::NotFound("group"))
    }

    pub fn create_group(&mut self, input: &GroupInput) -> Result<Group> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err(Error::Invalid { field: "name", reason: "required".into() });
        }
        valid_color(&input.color)?;
        self.conn().execute(
            "INSERT INTO host_groups (name, color, icon, sort_order) VALUES (?1, ?2, ?3, ?4)",
            params![name, input.color, input.icon, input.sort_order],
        )?;
        let id = self.conn().last_insert_rowid() as GroupId;
        self.get_group(id)
    }

    pub fn update_group(&mut self, id: GroupId, input: &GroupInput) -> Result<Group> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err(Error::Invalid { field: "name", reason: "required".into() });
        }
        valid_color(&input.color)?;
        let n = self.conn().execute(
            "UPDATE host_groups SET name = ?2, color = ?3, icon = ?4, sort_order = ?5 WHERE id = ?1",
            params![id, name, input.color, input.icon, input.sort_order],
        )?;
        if n == 0 {
            return Err(Error::NotFound("group"));
        }
        self.get_group(id)
    }

    /// Deletes a group; its hosts become ungrouped.
    pub fn delete_group(&mut self, id: GroupId) -> Result<()> {
        self.conn().execute("DELETE FROM host_groups WHERE id = ?1", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(name: &str) -> HostInput {
        HostInput { name: name.into(), hostname: format!("{name}.lan"), ..Default::default() }
    }

    #[test]
    fn role_defaults_to_server_and_round_trips() {
        let mut store = Store::open_in_memory().unwrap();
        let h = store.create_host(&input("lumen")).unwrap();
        assert_eq!(h.role, "server");
        let mut i = input("ally");
        i.role = "client".into();
        let c = store.create_host(&i).unwrap();
        assert_eq!(c.role, "client");
        i.role = "server".into();
        assert_eq!(store.update_host(c.id, &i).unwrap().role, "server");
        i.role = "laptop".into();
        assert!(store.create_host(&i).is_err());
    }

    #[test]
    fn ascii_icons_are_normalized_and_limited() {
        let n = |s: &str| normalize_icon(&Some(s.to_owned()));
        assert_eq!(n("\r\n   ┌──┐  \r\n   │▓▓│\r\n   └──┘\n\n").unwrap().as_deref(), Some("┌──┐\n│▓▓│\n└──┘"));
        assert_eq!(n("  \n \n").unwrap(), None);
        assert_eq!(normalize_icon(&None).unwrap(), None);
        assert_eq!(n("\ta\n  b").unwrap().as_deref(), Some("  a\nb"));
        assert!(n("x\u{1b}[31m").is_err(), "control characters");
        assert!(n("🖥").is_err(), "emoji are double width");
        assert!(n(&"x\n".repeat(ICON_MAX_LINES + 1)).is_err());
        assert!(n(&"=".repeat(ICON_MAX_COLS + 1)).is_err());
        assert!(n(&"█".repeat(ICON_MAX_COLS)).is_ok(), "limit counts characters, not bytes");
    }

    #[test]
    fn create_list_update_delete() {
        let mut s = Store::open_in_memory().unwrap();
        let g = s.create_group(&GroupInput { name: "Servers".into(), ..Default::default() }).unwrap();
        let mut i = input("lumen");
        i.group_id = Some(g.id);
        i.tags = vec!["docker".into(), " media ".into(), "Docker".into(), "".into()];
        i.username = Some("  ".into());
        let h = s.create_host(&i).unwrap();
        assert_eq!(h.port, 22);
        assert_eq!(h.username, None);
        assert_eq!(h.tags, vec!["docker", "media"]);
        assert_eq!(h.auth_method, AuthMethod::Agent);

        let mut u = i.clone();
        u.port = Some(2222);
        u.favorite = true;
        u.tags = vec!["media".into()];
        let h2 = s.update_host(h.id, &u).unwrap();
        assert_eq!(h2.port, 2222);
        assert!(h2.favorite);
        assert_eq!(h2.tags, vec!["media"]);

        s.create_host(&input("aux")).unwrap();
        let all = s.list_hosts().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].name, "lumen", "favorites first");
        assert_eq!(all[0].tags, vec!["media"]);

        s.delete_host(h.id).unwrap();
        assert!(s.get_host(h.id).unwrap().is_none());
        let orphan_tags: i64 = s.conn().query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0)).unwrap();
        assert_eq!(orphan_tags, 0);
    }

    #[test]
    fn validation() {
        let mut s = Store::open_in_memory().unwrap();
        assert!(s.create_host(&HostInput { name: "x".into(), ..Default::default() }).is_err());
        assert!(s.create_host(&HostInput { name: "x".into(), hostname: "a b".into(), ..Default::default() }).is_err());
        let mut bad_color = input("c");
        bad_color.color = Some("red".into());
        assert!(s.create_host(&bad_color).is_err());
        let mut key_no_file = input("k");
        key_no_file.auth_method = AuthMethod::Key;
        assert!(s.create_host(&key_no_file).is_err());
        assert!(s.update_host(999, &input("z")).is_err());
    }

    #[test]
    fn deleting_group_ungroups_hosts() {
        let mut s = Store::open_in_memory().unwrap();
        let g = s.create_group(&GroupInput { name: "G".into(), ..Default::default() }).unwrap();
        let mut i = input("a");
        i.group_id = Some(g.id);
        let h = s.create_host(&i).unwrap();
        s.delete_group(g.id).unwrap();
        assert_eq!(s.get_host(h.id).unwrap().unwrap().group_id, None);
    }

    #[test]
    fn find_by_alias_prefers_alias() {
        let mut s = Store::open_in_memory().unwrap();
        let mut a = input("lumen");
        a.ssh_alias = Some("lumen".into());
        let h = s.create_host(&a).unwrap();
        assert_eq!(s.find_host_by_name_or_alias("lumen").unwrap().unwrap().id, h.id);
        assert!(s.find_host_by_name_or_alias("nope").unwrap().is_none());
    }

    #[test]
    fn ssh_alias_is_unique() {
        let mut s = Store::open_in_memory().unwrap();
        let mut a = input("a");
        a.ssh_alias = Some("dup".into());
        s.create_host(&a).unwrap();
        let mut b = input("b");
        b.ssh_alias = Some("dup".into());
        assert!(s.create_host(&b).is_err());
    }
}
