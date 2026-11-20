//! Storage links (D-200): "this host's disks are stored on that host's
//! drive", for VMs whose disk images live on a physical drive of their
//! hypervisor (or on a Windows PC's H:).
//!
//! This is a different relation from the dependency graph's "depends on"
//! and from a network share: a host that mounts `/mnt/plex` from a NAS
//! still runs when the NAS fills up, but a VM whose disk file is on a full
//! drive stops. So a link makes the VM's alerts fold under that drive's
//! DISK FULL (and under its hypervisor being offline), while shares are
//! only used to count growth once ([`crate::diskforecast::annotate`]).

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

pub type StorageLinkId = i32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StorageLink {
    pub id: StorageLinkId,
    /// The host (usually a VM) whose disks are stored there.
    pub host_id: HostId,
    pub owner_host_id: HostId,
    /// The owner's mount or drive holding them (`/mnt/h`, `H:\`).
    pub owner_mount: String,
}

impl Store {
    pub fn storage_links(&self) -> Result<Vec<StorageLink>> {
        let mut st =
            self.conn().prepare("SELECT id, host_id, owner_host_id, owner_mount FROM storage_links ORDER BY id")?;
        let rows = st.query_map([], |r| {
            Ok(StorageLink { id: r.get(0)?, host_id: r.get(1)?, owner_host_id: r.get(2)?, owner_mount: r.get(3)? })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Adds "`host`'s disks are stored on `owner`'s `mount`"; an existing
    /// link is returned as it is.
    pub fn storage_link_add(&self, host: HostId, owner: HostId, mount: &str) -> Result<StorageLink> {
        let mount = mount.trim();
        if host == owner {
            return Err(Error::Invalid { field: "storage link", reason: "a host can't be stored on itself".into() });
        }
        if mount.is_empty() || mount.len() > 1024 || mount.chars().any(char::is_control) {
            return Err(Error::Invalid {
                field: "storage link",
                reason: "pick the drive or mount that holds the disks".into(),
            });
        }
        if let Some(l) = self
            .storage_links()?
            .into_iter()
            .find(|l| l.host_id == host && l.owner_host_id == owner && l.owner_mount == mount)
        {
            return Ok(l);
        }
        self.conn().execute(
            "INSERT INTO storage_links (host_id, owner_host_id, owner_mount) VALUES (?1, ?2, ?3)",
            params![host, owner, mount],
        )?;
        let id = self.conn().last_insert_rowid() as StorageLinkId;
        Ok(StorageLink { id, host_id: host, owner_host_id: owner, owner_mount: mount.to_owned() })
    }

    pub fn storage_link_delete(&self, id: StorageLinkId) -> Result<()> {
        self.conn().execute("DELETE FROM storage_links WHERE id = ?1", params![id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let s = Store::open_in_memory().unwrap();
        for n in ["vm", "pc"] {
            s.conn().execute("INSERT INTO hosts (name, hostname) VALUES (?1, ?1)", [n]).unwrap();
        }
        let a = s.storage_link_add(1, 2, " H:\\ ").unwrap();
        assert_eq!(a.owner_mount, "H:\\");
        assert_eq!(s.storage_link_add(1, 2, "H:\\").unwrap().id, a.id);
        assert!(s.storage_link_add(1, 1, "/").is_err());
        assert!(s.storage_link_add(1, 2, "").is_err());
        assert_eq!(s.storage_links().unwrap(), vec![a.clone()]);
        s.conn().execute("DELETE FROM hosts WHERE id = 2", []).unwrap();
        assert!(s.storage_links().unwrap().is_empty(), "goes with its owner");
    }
}
