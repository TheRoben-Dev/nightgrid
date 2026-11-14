//! Folders on a host that hold compose stacks (e.g. `/opt/stacks`), scanned
//! for compose files in addition to the stacks found from container labels.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

pub type StackDirId = i32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StackDir {
    pub id: StackDirId,
    pub host_id: HostId,
    pub path: String,
}

/// Absolute (`/...`) or home-relative (`~` / `~/...`), no control characters.
pub fn valid_stack_dir(path: &str) -> bool {
    (path.starts_with('/') || path == "~" || path.starts_with("~/"))
        && path.len() <= 1024
        && !path.chars().any(char::is_control)
}

impl Store {
    pub fn list_stack_dirs(&self, host_id: HostId) -> Result<Vec<StackDir>> {
        let mut stmt =
            self.conn().prepare("SELECT id, host_id, path FROM stack_dirs WHERE host_id = ?1 ORDER BY path")?;
        let rows =
            stmt.query_map([host_id], |r| Ok(StackDir { id: r.get(0)?, host_id: r.get(1)?, path: r.get(2)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Adds a folder (idempotent for the same path).
    pub fn add_stack_dir(&mut self, host_id: HostId, path: &str) -> Result<StackDir> {
        let path = path.trim();
        let path = if path.len() > 1 { path.trim_end_matches('/') } else { path };
        if !valid_stack_dir(path) {
            return Err(Error::Invalid {
                field: "path",
                reason: "use an absolute path or one starting with ~/".into(),
            });
        }
        self.conn().execute(
            "INSERT INTO stack_dirs (host_id, path) VALUES (?1, ?2) ON CONFLICT(host_id, path) DO NOTHING",
            params![host_id, path],
        )?;
        let id = self.conn().query_row(
            "SELECT id FROM stack_dirs WHERE host_id = ?1 AND path = ?2",
            params![host_id, path],
            |r| r.get(0),
        )?;
        Ok(StackDir { id, host_id, path: path.to_owned() })
    }

    pub fn delete_stack_dir(&mut self, id: StackDirId) -> Result<()> {
        if self.conn().execute("DELETE FROM stack_dirs WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("stack folder"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::HostInput;

    #[test]
    fn add_list_delete_and_cascade() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&HostInput { name: "lumen".into(), hostname: "lumen.lan".into(), ..Default::default() })
            .unwrap();
        let a = s.add_stack_dir(h.id, "/opt/stacks/").unwrap();
        assert_eq!(a.path, "/opt/stacks");
        assert_eq!(s.add_stack_dir(h.id, "/opt/stacks").unwrap().id, a.id);
        s.add_stack_dir(h.id, "~/compose").unwrap();
        assert!(s.add_stack_dir(h.id, "relative/dir").is_err());
        assert!(s.add_stack_dir(h.id, "/bad\npath").is_err());
        assert_eq!(s.list_stack_dirs(h.id).unwrap().len(), 2);
        s.delete_stack_dir(a.id).unwrap();
        assert!(matches!(s.delete_stack_dir(a.id), Err(Error::NotFound(_))));
        s.delete_host(h.id).unwrap();
        assert!(s.list_stack_dirs(h.id).unwrap().is_empty());
    }
}
