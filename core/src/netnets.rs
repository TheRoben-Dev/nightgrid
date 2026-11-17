//! Saved networks for the network map (D-154). A scan is matched to a
//! network by its fingerprint: the subnet the scanner sits on and its
//! router's address and MAC. The router's MAC tells two networks that both
//! use `192.168.1.0/24` apart (a home and a friend's place), and the subnet
//! tells a laptop's Wi-Fi from the server LAN it reaches over a tailnet.
//!
//! Devices remember the network they were last seen on, so a scan of one
//! network never calls another network's devices missing; they are known to
//! be part of a different one. The last scan of each network is kept so the
//! map can show it, or overlay several, without scanning again.

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::netmap::{in_v4_subnet, is_container_bridge, AddrKind, Discovery};
use crate::store::Store;
use crate::{Error, Result};

/// Which saved network a scan was on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NetworkTag {
    #[cfg_attr(feature = "specta", specta(type = i32))]
    pub id: i64,
    pub name: String,
    /// First scan of this network.
    pub is_new: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SavedNetwork {
    #[cfg_attr(feature = "specta", specta(type = i32))]
    pub id: i64,
    pub name: String,
    /// `192.168.1.0/24`
    pub subnet: Option<String>,
    /// The router's address.
    pub gateway: Option<String>,
    pub gateway_mac: Option<String>,
    /// Where it was last scanned from: a host id or `local`.
    pub origin: String,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub first_seen: i64,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub last_seen: i64,
    /// When the kept scan ran; none when it was too big to keep.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub snapshot_at: Option<i64>,
    /// Devices in the inventory last seen on it.
    pub devices: u32,
}

/// What identifies a network in a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    pub subnet: Option<String>,
    pub gateway: Option<String>,
    pub gateway_mac: Option<String>,
}

/// The most networks kept; the one seen longest ago goes first.
pub const MAX_NETWORKS: usize = 64;
/// Biggest scan kept per network (JSON bytes).
const MAX_SNAPSHOT: usize = 4 << 20;

fn network_of(addr: &str, prefix: u8) -> Option<String> {
    let a: std::net::Ipv4Addr = addr.parse().ok()?;
    let mask = if prefix == 0 { 0 } else { u32::MAX << (32 - u32::from(prefix)) };
    Some(format!("{}/{prefix}", std::net::Ipv4Addr::from(u32::from(a) & mask)))
}

/// The scan's fingerprint: the private IPv4 subnet holding the router (or
/// the first one, with no router), the router's address and its MAC. None
/// when the scanner has no LAN at all (only a tailnet, say).
pub fn fingerprint(d: &Discovery) -> Option<Fingerprint> {
    let lans: Vec<_> = d
        .ifaces
        .iter()
        .filter(|i| {
            i.kind == AddrKind::Private && !i.addr.contains(':') && !is_container_bridge(&i.name) && i.prefix >= 8
        })
        .collect();
    let gw = d.gateway.clone().filter(|g| g.parse::<std::net::Ipv4Addr>().is_ok());
    let lan = match &gw {
        Some(g) => lans.iter().find(|i| in_v4_subnet(g, &i.addr, i.prefix)).or(lans.first()),
        None => lans.first(),
    };
    let subnet = lan.and_then(|i| network_of(&i.addr, i.prefix));
    if subnet.is_none() && gw.is_none() {
        return None;
    }
    let gateway_mac = gw
        .as_deref()
        .and_then(|g| d.devices.iter().find(|x| x.ip == g))
        .and_then(|x| x.mac.as_deref())
        .map(|m| m.trim().to_ascii_lowercase().replace('-', ":"))
        .filter(|m| !m.is_empty());
    Some(Fingerprint { subnet, gateway: gw, gateway_mac })
}

/// How well a saved network matches a fingerprint: 2 for the same router
/// (MAC), 1 for the same subnet and router address with no MAC to compare,
/// 0 for a different network. Two routers with different MACs are two
/// networks, however alike their addresses.
pub fn match_score(n: &SavedNetwork, fp: &Fingerprint) -> u8 {
    match (&n.gateway_mac, &fp.gateway_mac) {
        (Some(a), Some(b)) => u8::from(a == b) * 2,
        _ if n.subnet.is_some() && n.subnet == fp.subnet && n.gateway == fp.gateway => 1,
        _ => 0,
    }
}

fn valid_name(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 60 || n.chars().any(char::is_control) {
        return Err(Error::Invalid { field: "name", reason: "1–60 printable characters".into() });
    }
    Ok(n.to_owned())
}

const COLS: &str =
    "n.id, n.name, n.subnet, n.gateway, n.gateway_mac, n.origin, n.first_seen, n.last_seen, n.snapshot_at,
    (SELECT count(*) FROM net_devices d WHERE d.network = n.id)";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SavedNetwork> {
    Ok(SavedNetwork {
        id: r.get(0)?,
        name: r.get(1)?,
        subnet: r.get(2)?,
        gateway: r.get(3)?,
        gateway_mac: r.get(4)?,
        origin: r.get(5)?,
        first_seen: r.get(6)?,
        last_seen: r.get(7)?,
        snapshot_at: r.get(8)?,
        devices: r.get::<_, i64>(9)? as u32,
    })
}

impl Store {
    /// Saved networks, most recently seen first.
    pub fn net_networks(&self) -> Result<Vec<SavedNetwork>> {
        let mut st =
            self.conn().prepare(&format!("SELECT {COLS} FROM net_networks n ORDER BY n.last_seen DESC, n.id"))?;
        let rows = st.query_map([], row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn net_network(&self, id: i64) -> Result<SavedNetwork> {
        self.conn()
            .query_row(&format!("SELECT {COLS} FROM net_networks n WHERE n.id = ?1"), [id], row)
            .optional()?
            .ok_or_else(|| Error::Invalid { field: "id", reason: "no such saved network".into() })
    }

    /// Finds (or saves) the network a scan was on and tags the scan with
    /// it. A scan with no LAN (tailnet only) gets no network.
    pub fn net_network_identify(&mut self, origin: &str, d: &mut Discovery, now: i64) -> Result<()> {
        let Some(fp) = fingerprint(d) else {
            d.network = None;
            return Ok(());
        };
        let nets = self.net_networks()?;
        let best = nets
            .iter()
            .map(|n| (match_score(n, &fp), n))
            .filter(|(s, _)| *s > 0)
            .max_by_key(|(s, n)| (*s, n.last_seen));
        let tag = if let Some((_, n)) = best {
            self.conn().execute(
                "UPDATE net_networks SET subnet = coalesce(?2, subnet), gateway = coalesce(?3, gateway),
                   gateway_mac = coalesce(?4, gateway_mac), origin = ?5, last_seen = ?6 WHERE id = ?1",
                params![n.id, fp.subnet, fp.gateway, fp.gateway_mac, origin, now],
            )?;
            NetworkTag { id: n.id, name: n.name.clone(), is_new: false }
        } else {
            let base = format!("Network {}", fp.subnet.as_deref().or(fp.gateway.as_deref()).unwrap_or("?"));
            let mut name = base.clone();
            let mut i = 2;
            while nets.iter().any(|n| n.name == name) {
                name = format!("{base} ({i})");
                i += 1;
            }
            let tx = self.conn_mut().transaction()?;
            tx.execute(
                "INSERT INTO net_networks (name, subnet, gateway, gateway_mac, origin, first_seen, last_seen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![name, fp.subnet, fp.gateway, fp.gateway_mac, origin, now],
            )?;
            let id = tx.last_insert_rowid();
            let old: Vec<i64> = {
                let mut st = tx.prepare(&format!(
                    "SELECT id FROM net_networks ORDER BY last_seen DESC, id DESC LIMIT -1 OFFSET {MAX_NETWORKS}"
                ))?;
                let ids = st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
                ids
            };
            for o in old {
                tx.execute("UPDATE net_devices SET network = NULL WHERE network = ?1", [o])?;
                tx.execute("DELETE FROM net_networks WHERE id = ?1", [o])?;
            }
            tx.commit()?;
            NetworkTag { id, name, is_new: true }
        };
        d.network = Some(tag);
        Ok(())
    }

    /// Keeps the scan as its network's last scan (skipped when huge).
    pub fn net_network_keep(&mut self, d: &Discovery, now: i64) -> Result<()> {
        let Some(tag) = &d.network else { return Ok(()) };
        let json = serde_json::to_string(d).map_err(|e| Error::Invalid { field: "snapshot", reason: e.to_string() })?;
        let (json, at) = if json.len() <= MAX_SNAPSHOT { (Some(json), Some(now)) } else { (None, None) };
        self.conn().execute(
            "UPDATE net_networks SET snapshot = ?2, snapshot_at = ?3 WHERE id = ?1",
            params![tag.id, json, at],
        )?;
        Ok(())
    }

    /// The last scan of a saved network, if one was kept.
    pub fn net_network_snapshot(&self, id: i64) -> Result<Option<Discovery>> {
        let json: Option<String> = self
            .conn()
            .query_row("SELECT snapshot FROM net_networks WHERE id = ?1", [id], |r| r.get(0))
            .optional()?
            .flatten();
        // A scan kept by an older version that no longer reads is just gone.
        Ok(json.and_then(|j| serde_json::from_str::<Discovery>(&j).ok()).map(|mut d| {
            if let Ok(n) = self.net_network(id) {
                d.network = Some(NetworkTag { id, name: n.name, is_new: false });
            }
            d
        }))
    }

    pub fn net_network_rename(&mut self, id: i64, name: &str) -> Result<SavedNetwork> {
        let name = valid_name(name)?;
        if self.conn().execute("UPDATE net_networks SET name = ?2 WHERE id = ?1", params![id, name])? == 0 {
            return Err(Error::Invalid { field: "id", reason: "no such saved network".into() });
        }
        self.net_network(id)
    }

    /// Forgets a saved network. Its devices stay in the inventory, on no
    /// network, until a scan sees them again.
    pub fn net_network_forget(&mut self, id: i64) -> Result<()> {
        let tx = self.conn_mut().transaction()?;
        tx.execute("UPDATE net_devices SET network = NULL WHERE network = ?1", [id])?;
        tx.execute("DELETE FROM net_networks WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    /// Folds `from` into `into` (a router was replaced, so the same network
    /// came back as a new one): devices move over, the newer fingerprint and
    /// last scan win, `into` keeps its name.
    pub fn net_network_merge(&mut self, from: i64, into: i64) -> Result<SavedNetwork> {
        if from == into {
            return Err(Error::Invalid { field: "into", reason: "a different network".into() });
        }
        let a = self.net_network(from)?;
        let b = self.net_network(into)?;
        let tx = self.conn_mut().transaction()?;
        tx.execute("UPDATE net_devices SET network = ?2 WHERE network = ?1", params![from, into])?;
        if a.last_seen > b.last_seen {
            tx.execute(
                "UPDATE net_networks SET (subnet, gateway, gateway_mac, origin, last_seen, snapshot, snapshot_at) =
                   (SELECT subnet, gateway, gateway_mac, origin, last_seen, snapshot, snapshot_at FROM net_networks WHERE id = ?1)
                 WHERE id = ?2",
                params![from, into],
            )?;
        }
        tx.execute(
            "UPDATE net_networks SET first_seen = min(first_seen, ?2) WHERE id = ?1",
            params![into, a.first_seen],
        )?;
        tx.execute("DELETE FROM net_networks WHERE id = ?1", [from])?;
        tx.commit()?;
        self.net_network(into)
    }
}

#[cfg(test)]
mod tests {
    use crate::netmap::parse_discovery;
    use crate::store::Store;

    const M: &str = "@@NGNET@@";

    /// A scan of `net` (`192.168.N`) whose router has MAC `gw_mac`.
    fn scan(net: &str, gw_mac: &str, neigh: &str) -> crate::netmap::Discovery {
        parse_discovery(&format!(
            "{M} addr\n2: eth0    inet {net}.10/24 brd {net}.255 scope global eth0\n\
             {M} route\ndefault via {net}.1 dev eth0\n{M} sweep {net}.0/24\n\
             {M} neigh\n{net}.1 dev eth0 lladdr {gw_mac} REACHABLE\n{neigh}{M} end\n"
        ))
    }

    fn record(s: &mut Store, origin: &str, d: &mut crate::netmap::Discovery, now: i64) {
        s.net_network_identify(origin, d, now).unwrap();
        s.net_inventory_merge(origin, d, now).unwrap();
        s.net_network_keep(d, now).unwrap();
    }

    #[test]
    fn fingerprint_parts() {
        let d = scan("192.168.1", "b8:27:eb:00:00:01", "");
        let fp = super::fingerprint(&d).unwrap();
        assert_eq!(fp.subnet.as_deref(), Some("192.168.1.0/24"));
        assert_eq!(fp.gateway.as_deref(), Some("192.168.1.1"));
        assert_eq!(fp.gateway_mac.as_deref(), Some("b8:27:eb:00:00:01"));
        assert!(super::fingerprint(&parse_discovery(&format!("{M} addr\n{M} end\n"))).is_none());
    }

    #[test]
    fn other_networks_are_never_missing() {
        let mut s = Store::open_in_memory().unwrap();
        // The server LAN, scanned from a server.
        let mut a =
            scan("192.168.0", "aa:00:00:00:00:01", "192.168.0.20 dev eth0 lladdr aa:bb:cc:00:00:20 REACHABLE\n");
        record(&mut s, "3", &mut a, 1000);
        let la = a.network.clone().unwrap();
        assert!(la.is_new);
        assert_eq!(la.name, "Network 192.168.0.0/24");

        // The laptop's Wi-Fi: another subnet, another router.
        let mut b =
            scan("192.168.1", "bb:00:00:00:00:01", "192.168.1.30 dev eth0 lladdr aa:bb:cc:00:00:30 REACHABLE\n");
        record(&mut s, "local", &mut b, 2000);
        assert_ne!(b.network.as_ref().unwrap().id, la.id);
        assert!(b.missing.is_empty(), "the server LAN's devices aren't missing: {:?}", b.missing);

        // A friend's network on the same subnet as the server LAN, other router: a third network.
        let mut c = scan("192.168.0", "cc:00:00:00:00:01", "");
        record(&mut s, "local", &mut c, 3000);
        let lc = c.network.clone().unwrap();
        assert!(lc.is_new && lc.id != la.id);
        assert_eq!(lc.name, "Network 192.168.0.0/24 (2)");
        assert!(c.missing.is_empty(), "same subnet, other router: {:?}", c.missing);

        // Back on the server LAN: the same network, and a device that's gone is missing.
        let mut a2 = scan("192.168.0", "aa:00:00:00:00:01", "");
        record(&mut s, "3", &mut a2, 4000);
        assert_eq!(a2.network.as_ref().unwrap().id, la.id);
        assert_eq!(a2.missing.iter().map(|m| m.ip.as_str()).collect::<Vec<_>>(), ["192.168.0.20"]);

        let nets = s.net_networks().unwrap();
        assert_eq!(nets.len(), 3);
        let snap = s.net_network_snapshot(la.id).unwrap().unwrap();
        assert_eq!(snap.gateway.as_deref(), Some("192.168.0.1"));
        assert_eq!(snap.network.as_ref().unwrap().id, la.id);
    }

    #[test]
    fn rename_merge_forget() {
        let mut s = Store::open_in_memory().unwrap();
        let mut a = scan("10.0.0", "aa:00:00:00:00:01", "10.0.0.5 dev eth0 lladdr aa:bb:cc:00:00:05 REACHABLE\n");
        record(&mut s, "local", &mut a, 1000);
        let ida = a.network.unwrap().id;
        assert_eq!(s.net_network_rename(ida, "  Home ").unwrap().name, "Home");
        assert!(s.net_network_rename(ida, "").is_err());
        // New router on the same network: a new network until merged.
        let mut b = scan("10.0.0", "aa:00:00:00:00:02", "10.0.0.5 dev eth0 lladdr aa:bb:cc:00:00:05 REACHABLE\n");
        record(&mut s, "local", &mut b, 2000);
        let idb = b.network.unwrap().id;
        assert_ne!(ida, idb);
        let m = s.net_network_merge(idb, ida).unwrap();
        assert_eq!(
            (m.name.as_str(), m.gateway_mac.as_deref(), m.first_seen),
            ("Home", Some("aa:00:00:00:00:02"), 1000)
        );
        assert_eq!(s.net_networks().unwrap().len(), 1);
        let mut c = scan("10.0.0", "aa:00:00:00:00:02", "");
        record(&mut s, "local", &mut c, 3000);
        assert_eq!(c.network.unwrap().id, ida);
        s.net_network_forget(ida).unwrap();
        assert!(s.net_networks().unwrap().is_empty());
        assert!(s.net_devices().unwrap().iter().all(|d| d.network.is_none()));
    }
}
