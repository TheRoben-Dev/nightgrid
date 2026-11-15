//! Saved SSH port forwards: local (`-L`), remote (`-R`) and dynamic SOCKS5
//! (`-D`), stored per host. The forwarding itself is in [`forward`].

pub mod forward;
pub mod socks;

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

pub type TunnelId = i32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum TunnelKind {
    /// Listen here, connect from the server (`ssh -L`).
    Local,
    /// Listen on the server, connect from here (`ssh -R`).
    Remote,
    /// SOCKS5 proxy here, connections leave from the server (`ssh -D`).
    Dynamic,
}

impl TunnelKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
            Self::Dynamic => "dynamic",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "local" => Some(Self::Local),
            "remote" => Some(Self::Remote),
            "dynamic" => Some(Self::Dynamic),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Tunnel {
    pub id: TunnelId,
    pub host_id: HostId,
    pub name: String,
    pub kind: TunnelKind,
    pub bind_addr: String,
    /// 0 = any free port (local/dynamic), or let the server pick (remote).
    pub bind_port: u16,
    pub dest_host: String,
    pub dest_port: u16,
    pub auto_start: bool,
}

impl Tunnel {
    /// `ssh`-style spec, e.g. `-L 127.0.0.1:8080:localhost:80`.
    pub fn spec(&self) -> String {
        let bind = format!("{}:{}", bracket(&self.bind_addr), self.bind_port);
        match self.kind {
            TunnelKind::Local => format!("-L {bind}:{}:{}", bracket(&self.dest_host), self.dest_port),
            TunnelKind::Remote => format!("-R {bind}:{}:{}", bracket(&self.dest_host), self.dest_port),
            TunnelKind::Dynamic => format!("-D {bind}"),
        }
    }
}

fn bracket(h: &str) -> String {
    if h.contains(':') {
        format!("[{h}]")
    } else {
        h.to_owned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TunnelInput {
    pub host_id: HostId,
    #[serde(default)]
    pub name: String,
    pub kind: TunnelKind,
    #[serde(default)]
    pub bind_addr: String,
    pub bind_port: u16,
    #[serde(default)]
    pub dest_host: String,
    #[serde(default)]
    pub dest_port: u16,
    #[serde(default)]
    pub auto_start: bool,
}

/// Host names and IP literals only: no spaces, slashes or URL bits.
fn valid_host(h: &str) -> bool {
    !h.is_empty()
        && h.len() <= 253
        && h.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':' | '%'))
}

/// Whether listening on `addr` exposes the port beyond this machine/server.
pub fn is_public_bind(addr: &str) -> bool {
    !matches!(addr, "127.0.0.1" | "localhost" | "::1") && !addr.starts_with("127.")
}

/// Validates and fills defaults (bind address per kind, trimmed fields).
pub fn normalize(i: &TunnelInput) -> Result<TunnelInput> {
    let inv = |field: &'static str, reason: &str| Error::Invalid { field, reason: reason.into() };
    let mut t = i.clone();
    t.name = t.name.trim().chars().take(80).collect();
    t.bind_addr = t.bind_addr.trim().trim_start_matches('[').trim_end_matches(']').to_owned();
    if t.bind_addr.is_empty() {
        t.bind_addr = if t.kind == TunnelKind::Remote { "localhost".into() } else { "127.0.0.1".into() };
    }
    if !valid_host(&t.bind_addr) {
        return Err(inv("bindAddr", "use an IP address or host name"));
    }
    t.dest_host = t.dest_host.trim().trim_start_matches('[').trim_end_matches(']').to_owned();
    if t.kind == TunnelKind::Dynamic {
        t.dest_host.clear();
        t.dest_port = 0;
    } else {
        if !valid_host(&t.dest_host) {
            return Err(inv("destHost", "use an IP address or host name"));
        }
        if t.dest_port == 0 {
            return Err(inv("destPort", "1–65535"));
        }
    }
    Ok(t)
}

const COLS: &str = "id, host_id, name, kind, bind_addr, bind_port, dest_host, dest_port, auto_start";

fn from_row(r: &Row<'_>) -> rusqlite::Result<Tunnel> {
    let kind: String = r.get(3)?;
    Ok(Tunnel {
        id: r.get(0)?,
        host_id: r.get(1)?,
        name: r.get(2)?,
        kind: TunnelKind::parse(&kind).unwrap_or(TunnelKind::Local),
        bind_addr: r.get(4)?,
        bind_port: r.get(5)?,
        dest_host: r.get(6)?,
        dest_port: r.get(7)?,
        auto_start: r.get(8)?,
    })
}

impl Store {
    pub fn list_tunnels(&self) -> Result<Vec<Tunnel>> {
        let mut stmt = self.conn().prepare(&format!("SELECT {COLS} FROM tunnels ORDER BY host_id, id"))?;
        let rows = stmt.query_map([], from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn get_tunnel(&self, id: TunnelId) -> Result<Tunnel> {
        self.conn()
            .query_row(&format!("SELECT {COLS} FROM tunnels WHERE id = ?1"), [id], from_row)
            .optional()?
            .ok_or(Error::NotFound("tunnel"))
    }

    pub fn create_tunnel(&mut self, input: &TunnelInput) -> Result<Tunnel> {
        let t = normalize(input)?;
        self.conn().execute(
            "INSERT INTO tunnels (host_id, name, kind, bind_addr, bind_port, dest_host, dest_port, auto_start)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                t.host_id,
                t.name,
                t.kind.as_str(),
                t.bind_addr,
                t.bind_port,
                t.dest_host,
                t.dest_port,
                t.auto_start
            ],
        )?;
        let id = self.conn().last_insert_rowid() as TunnelId;
        self.get_tunnel(id)
    }

    pub fn update_tunnel(&mut self, id: TunnelId, input: &TunnelInput) -> Result<Tunnel> {
        let t = normalize(input)?;
        let n = self.conn().execute(
            "UPDATE tunnels SET host_id = ?2, name = ?3, kind = ?4, bind_addr = ?5, bind_port = ?6, dest_host = ?7,
                    dest_port = ?8, auto_start = ?9
             WHERE id = ?1",
            params![
                id,
                t.host_id,
                t.name,
                t.kind.as_str(),
                t.bind_addr,
                t.bind_port,
                t.dest_host,
                t.dest_port,
                t.auto_start
            ],
        )?;
        if n == 0 {
            return Err(Error::NotFound("tunnel"));
        }
        self.get_tunnel(id)
    }

    pub fn delete_tunnel(&mut self, id: TunnelId) -> Result<()> {
        if self.conn().execute("DELETE FROM tunnels WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("tunnel"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::HostInput;

    fn input(host_id: HostId, kind: TunnelKind) -> TunnelInput {
        TunnelInput {
            host_id,
            name: String::new(),
            kind,
            bind_addr: String::new(),
            bind_port: 8080,
            dest_host: "localhost".into(),
            dest_port: 80,
            auto_start: false,
        }
    }

    #[test]
    fn crud_defaults_and_validation() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&HostInput { name: "server01".into(), hostname: "server01.lan".into(), ..Default::default() })
            .unwrap();
        let l = s.create_tunnel(&input(h.id, TunnelKind::Local)).unwrap();
        assert_eq!(l.bind_addr, "127.0.0.1");
        assert_eq!(l.spec(), "-L 127.0.0.1:8080:localhost:80");
        let r = s.create_tunnel(&input(h.id, TunnelKind::Remote)).unwrap();
        assert_eq!(r.bind_addr, "localhost");
        let mut d = input(h.id, TunnelKind::Dynamic);
        d.bind_addr = "::1".into();
        d.bind_port = 1080;
        let d = s.create_tunnel(&d).unwrap();
        assert_eq!((d.dest_host.as_str(), d.dest_port), ("", 0));
        assert_eq!(d.spec(), "-D [::1]:1080");

        let mut bad = input(h.id, TunnelKind::Local);
        bad.dest_port = 0;
        assert!(s.create_tunnel(&bad).is_err());
        bad.dest_port = 80;
        bad.dest_host = "http://x".into();
        assert!(s.create_tunnel(&bad).is_err());

        let mut up = input(h.id, TunnelKind::Local);
        up.auto_start = true;
        up.name = " web ".into();
        let l2 = s.update_tunnel(l.id, &up).unwrap();
        assert!(l2.auto_start);
        assert_eq!(l2.name, "web");
        assert_eq!(s.list_tunnels().unwrap().len(), 3);

        // Tunnels go with their host.
        s.delete_host(h.id).unwrap();
        assert!(s.list_tunnels().unwrap().is_empty());
        assert!(matches!(s.delete_tunnel(l.id), Err(Error::NotFound(_))));
    }

    #[test]
    fn public_binds() {
        assert!(!is_public_bind("127.0.0.1"));
        assert!(!is_public_bind("localhost"));
        assert!(!is_public_bind("::1"));
        assert!(is_public_bind("0.0.0.0"));
        assert!(is_public_bind("192.168.1.5"));
    }
}
