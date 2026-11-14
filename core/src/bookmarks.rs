//! Path bookmarks for the file browser, per host (or local).

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

pub type BookmarkId = i32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub id: BookmarkId,
    /// `None` = this machine.
    pub host_id: Option<HostId>,
    pub path: String,
    pub label: String,
}

fn default_label(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    match trimmed.rsplit(['/', '\\']).next() {
        Some(last) if !last.is_empty() => last.to_owned(),
        _ => path.to_owned(),
    }
}

impl Store {
    /// Bookmarks for one host (`None` = local), in display order.
    pub fn list_bookmarks(&self, host_id: Option<HostId>) -> Result<Vec<Bookmark>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, host_id, path, label FROM bookmarks WHERE host_id IS ?1 ORDER BY sort_order, label COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([host_id], |r| {
            Ok(Bookmark { id: r.get(0)?, host_id: r.get(1)?, path: r.get(2)?, label: r.get(3)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Adds a bookmark, or relabels the existing one for the same path.
    pub fn add_bookmark(&mut self, host_id: Option<HostId>, path: &str, label: Option<&str>) -> Result<Bookmark> {
        let path = path.trim();
        if path.is_empty() || path.len() > 4096 {
            return Err(Error::Invalid { field: "path", reason: "must be 1-4096 characters".into() });
        }
        let label =
            label.map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).unwrap_or_else(|| default_label(path));
        if label.chars().count() > 80 {
            return Err(Error::Invalid { field: "label", reason: "at most 80 characters".into() });
        }
        self.tx(|tx| {
            let existing: Option<BookmarkId> = tx
                .query_row("SELECT id FROM bookmarks WHERE host_id IS ?1 AND path = ?2", params![host_id, path], |r| {
                    r.get(0)
                })
                .optional()?;
            let id = match existing {
                Some(id) => {
                    tx.execute("UPDATE bookmarks SET label = ?1 WHERE id = ?2", params![label, id])?;
                    id
                }
                None => {
                    let order: i64 = tx.query_row(
                        "SELECT ifnull(max(sort_order), -1) + 1 FROM bookmarks WHERE host_id IS ?1",
                        [host_id],
                        |r| r.get(0),
                    )?;
                    tx.execute(
                        "INSERT INTO bookmarks (host_id, path, label, sort_order) VALUES (?1, ?2, ?3, ?4)",
                        params![host_id, path, label, order],
                    )?;
                    tx.last_insert_rowid() as BookmarkId
                }
            };
            Ok(Bookmark { id, host_id, path: path.to_owned(), label })
        })
    }

    pub fn delete_bookmark(&mut self, id: BookmarkId) -> Result<()> {
        let n = self.conn().execute("DELETE FROM bookmarks WHERE id = ?1", [id])?;
        if n == 0 {
            return Err(Error::NotFound("bookmark"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::HostInput;

    #[test]
    fn add_list_relabel_delete_and_cascade() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&HostInput { name: "lumen".into(), hostname: "lumen.lan".into(), ..Default::default() })
            .unwrap();
        let a = s.add_bookmark(Some(h.id), "/opt/stacks/", None).unwrap();
        assert_eq!(a.label, "stacks");
        s.add_bookmark(Some(h.id), "/srv/media", Some("Media")).unwrap();
        s.add_bookmark(None, "/tmp", None).unwrap();
        assert_eq!(s.list_bookmarks(Some(h.id)).unwrap().len(), 2);
        assert_eq!(s.list_bookmarks(None).unwrap().len(), 1);

        // Same path again relabels instead of duplicating.
        let again = s.add_bookmark(Some(h.id), "/opt/stacks/", Some("Stacks")).unwrap();
        assert_eq!(again.id, a.id);
        let labels: Vec<_> = s.list_bookmarks(Some(h.id)).unwrap().into_iter().map(|b| b.label).collect();
        assert_eq!(labels, vec!["Stacks", "Media"]);

        s.delete_bookmark(a.id).unwrap();
        assert!(matches!(s.delete_bookmark(a.id), Err(Error::NotFound(_))));
        s.delete_host(h.id).unwrap();
        assert!(s.list_bookmarks(Some(h.id)).unwrap().is_empty());
        assert_eq!(s.list_bookmarks(None).unwrap().len(), 1);
        assert!(s.add_bookmark(None, "  ", None).is_err());
    }
}
