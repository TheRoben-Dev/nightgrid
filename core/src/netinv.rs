//! Network device inventory: every device the map has seen, so a scan can
//! say what's new and what's missing, and the user can mark devices as
//! trusted (known and expected). Keys match device labels: the MAC, or
//! `ip:<address>` when no MAC can be seen.

use rusqlite::params;
use serde::Serialize;

use crate::device_labels::key_for;
use crate::netmap::{in_v4_subnet, Discovery, MissingDevice};
use crate::store::Store;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NetDevice {
    pub key: String,
    pub ip: String,
    pub vendor: Option<String>,
    pub kind: String,
    /// The first name the scan found for it.
    pub name: Option<String>,
    /// A host id or `local`.
    pub origin: String,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub first_seen: i64,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub last_seen: i64,
    pub trusted: bool,
    /// The saved network it was last seen on (D-154); none for tailnet-only
    /// peers and devices from before saved networks.
    #[cfg_attr(feature = "specta", specta(type = Option<i32>))]
    pub network: Option<i64>,
}

/// Random (locally administered) MACs change; such a device is keyed by MAC
/// anyway, since phones keep one per network.
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<NetDevice> {
    Ok(NetDevice {
        key: r.get(0)?,
        ip: r.get(1)?,
        vendor: r.get(2)?,
        kind: r.get(3)?,
        name: r.get(4)?,
        origin: r.get(5)?,
        first_seen: r.get(6)?,
        last_seen: r.get(7)?,
        trusted: r.get::<_, i64>(8)? != 0,
        network: r.get(9)?,
    })
}

const COLS: &str = "key, ip, vendor, kind, name, origin, first_seen, last_seen, trusted, network";

/// The most devices kept; the ones seen longest ago go first.
pub const MAX_DEVICES: usize = 5000;

impl Store {
    pub fn net_devices(&self) -> Result<Vec<NetDevice>> {
        let mut st = self.conn().prepare(&format!("SELECT {COLS} FROM net_devices ORDER BY last_seen DESC"))?;
        let rows = st.query_map([], row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Records what a scan saw (from `origin` at `now`) and fills the
    /// discovery's inventory fields: first seen, new, trusted, and the
    /// known devices in the swept ranges that didn't answer.
    ///
    /// With a saved network on the scan (`net_network_identify` first), a
    /// device is missing only when it was last seen on that same network: a
    /// device from another network (same addresses or not) is just
    /// elsewhere. Devices no scan has placed on a network yet are never
    /// missing from one that has.
    pub fn net_inventory_merge(&mut self, origin: &str, d: &mut Discovery, now: i64) -> Result<()> {
        let known: std::collections::HashMap<String, NetDevice> =
            self.net_devices()?.into_iter().map(|x| (x.key.clone(), x)).collect();
        let tx = self.conn_mut().transaction()?;
        let mut seen_keys = Vec::new();
        let net = d.network.as_ref().map(|n| n.id);
        for dev in &mut d.devices {
            // A tailnet peer can be anywhere: it keeps the network it had.
            let on_net = if dev.seen.iter().all(|s| *s == crate::netmap::Seen::Tailscale) { None } else { net };
            let key = key_for(dev.mac.as_deref(), &dev.ip);
            let prev = known.get(&key);
            dev.first_seen = Some(prev.map_or(now, |p| p.first_seen));
            dev.is_new = prev.is_none();
            dev.trusted = prev.is_some_and(|p| p.trusted);
            tx.execute(
                "INSERT INTO net_devices (key, ip, vendor, kind, name, origin, first_seen, last_seen, network)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8)
                 ON CONFLICT(key) DO UPDATE SET ip = ?2, vendor = coalesce(?3, vendor), kind = ?4,
                   name = coalesce(?5, name), origin = ?6, last_seen = ?7, network = coalesce(?8, network)",
                params![key, dev.ip, dev.vendor, dev.kind, dev.names.first(), origin, now, on_net],
            )?;
            seen_keys.push(key);
        }
        tx.execute(
            &format!(
                "DELETE FROM net_devices WHERE key IN (SELECT key FROM net_devices ORDER BY last_seen DESC LIMIT -1 OFFSET {MAX_DEVICES})"
            ),
            [],
        )?;
        tx.commit()?;
        // Missing: known, in a range this scan swept, not seen now.
        let ranges: Vec<(String, u8)> = d
            .ranges
            .iter()
            .filter_map(|r| {
                let (n, p) = r.split_once('/')?;
                Some((n.to_owned(), p.parse().ok()?))
            })
            .collect();
        let mut missing: Vec<MissingDevice> = known
            .into_values()
            .filter(|x| !seen_keys.contains(&x.key) && ranges.iter().any(|(n, p)| in_v4_subnet(&x.ip, n, *p)))
            .filter(|x| x.network == net)
            .map(|x| MissingDevice {
                key: x.key,
                ip: x.ip,
                name: x.name,
                vendor: x.vendor,
                kind: x.kind,
                last_seen: x.last_seen,
                trusted: x.trusted,
            })
            .collect();
        missing.sort_by(|a, b| b.trusted.cmp(&a.trusted).then(b.last_seen.cmp(&a.last_seen)));
        d.missing = missing;
        Ok(())
    }

    pub fn net_device_trust(&mut self, key: &str, trusted: bool) -> Result<()> {
        let n = self.conn().execute(
            "UPDATE net_devices SET trusted = ?2 WHERE key = ?1",
            params![key.trim().to_ascii_lowercase(), i64::from(trusted)],
        )?;
        if n == 0 {
            return Err(Error::Invalid { field: "key", reason: "no such device in the inventory".into() });
        }
        Ok(())
    }

    /// Forgets a device (it counts as new the next time it's seen).
    pub fn net_device_forget(&mut self, key: &str) -> Result<()> {
        self.conn().execute("DELETE FROM net_devices WHERE key = ?1", [key.trim().to_ascii_lowercase()])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::netmap::parse_discovery;
    use crate::store::Store;

    const M: &str = "@@NGNET@@";

    fn scan(neigh: &str) -> crate::netmap::Discovery {
        parse_discovery(&format!(
            "{M} addr\n2: eth0    inet 192.168.1.10/24 brd 192.168.1.255 scope global eth0\n\
             {M} route\ndefault via 192.168.1.1 dev eth0\n{M} sweep 192.168.1.0/24\n\
             {M} neigh\n{neigh}{M} end\n"
        ))
    }

    #[test]
    fn new_missing_and_trusted() {
        let mut s = Store::open_in_memory().unwrap();
        let mut d = scan(
            "192.168.1.1 dev eth0 lladdr b8:27:eb:00:00:01 REACHABLE\n192.168.1.30 dev eth0 lladdr aa:bb:cc:00:00:30 REACHABLE\n",
        );
        s.net_inventory_merge("3", &mut d, 1000).unwrap();
        assert!(d.devices.iter().all(|x| x.is_new && x.first_seen == Some(1000)));
        s.net_device_trust("aa:bb:cc:00:00:30", true).unwrap();

        let mut d = scan("192.168.1.1 dev eth0 lladdr b8:27:eb:00:00:01 REACHABLE\n192.168.1.31 dev eth0 lladdr aa:bb:cc:00:00:31 REACHABLE\n");
        s.net_inventory_merge("3", &mut d, 2000).unwrap();
        let gw = d.devices.iter().find(|x| x.ip == "192.168.1.1").unwrap();
        assert!(!gw.is_new);
        assert_eq!(gw.first_seen, Some(1000));
        assert!(d.devices.iter().find(|x| x.ip == "192.168.1.31").unwrap().is_new);
        assert_eq!(d.missing.len(), 1);
        assert_eq!(d.missing[0].ip, "192.168.1.30");
        assert!(d.missing[0].trusted);

        s.net_device_forget("aa:bb:cc:00:00:30").unwrap();
        assert_eq!(s.net_devices().unwrap().len(), 2);
        assert!(s.net_device_trust("aa:bb:cc:00:00:30", true).is_err());
    }
}
