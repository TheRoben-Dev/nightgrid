//! Notes and runbooks (M10e): markdown text, optionally tied to a host.
//!
//! Notes are config: they go into the encrypted export like snippets do.
//! Text that looks like it holds a password, token or key is refused on save
//! (security rule 1); the UI offers to mask it first.

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::hosts::HostId;
use crate::redact::secret_hint;
use crate::store::Store;
use crate::{Error, Result};

pub type NoteId = i32;

pub const MAX_TITLE: usize = 120;
pub const MAX_BODY: usize = 100_000;
pub const MAX_TAGS: usize = 10;
pub const MAX_TAG: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: NoteId,
    pub title: String,
    /// Markdown.
    pub body: String,
    /// `None` = not tied to a host.
    pub host_id: Option<HostId>,
    pub tags: Vec<String>,
    pub pinned: bool,
    /// Unix seconds.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub updated_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct NoteInput {
    pub title: String,
    pub body: String,
    pub host_id: Option<HostId>,
    pub tags: Vec<String>,
    pub pinned: bool,
}

/// Trims and checks a note before it is stored.
pub fn normalize_note(i: &NoteInput) -> Result<NoteInput> {
    let title = i.title.trim().to_owned();
    if title.is_empty() || title.chars().count() > MAX_TITLE {
        return Err(Error::Invalid { field: "title", reason: format!("must be 1-{MAX_TITLE} characters") });
    }
    let body = i.body.trim_end().to_owned();
    if body.len() > MAX_BODY {
        return Err(Error::Invalid { field: "body", reason: format!("at most {} KB", MAX_BODY / 1000) });
    }
    let mut tags: Vec<String> = Vec::new();
    for t in &i.tags {
        let t = t.trim().trim_start_matches('#').to_lowercase();
        if t.is_empty() {
            continue;
        }
        if t.chars().count() > MAX_TAG || !t.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(Error::Invalid {
                field: "tags",
                reason: format!("tags are up to {MAX_TAG} letters, digits, - or _"),
            });
        }
        if !tags.contains(&t) {
            tags.push(t);
        }
    }
    if tags.len() > MAX_TAGS {
        return Err(Error::Invalid { field: "tags", reason: format!("at most {MAX_TAGS} tags") });
    }
    for (field, text) in [("title", &title), ("body", &body)] {
        if let Some(hint) = secret_hint(text) {
            return Err(Error::Invalid {
                field,
                reason: format!(
                    "looks like it contains {hint}. Mask it (or keep it in a password manager) before saving"
                ),
            });
        }
    }
    Ok(NoteInput { title, body, host_id: i.host_id, tags, pinned: i.pinned })
}

const COLS: &str = "id, title, body, host_id, tags, pinned, updated_at";

fn note_row(r: &Row<'_>) -> rusqlite::Result<Note> {
    let tags: String = r.get(4)?;
    Ok(Note {
        id: r.get(0)?,
        title: r.get(1)?,
        body: r.get(2)?,
        host_id: r.get(3)?,
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        pinned: r.get(5)?,
        updated_at: r.get(6)?,
    })
}

impl Store {
    /// All notes: pinned first, then most recently changed.
    pub fn list_notes(&self) -> Result<Vec<Note>> {
        let mut stmt =
            self.conn().prepare(&format!("SELECT {COLS} FROM notes ORDER BY pinned DESC, updated_at DESC, id DESC"))?;
        let rows = stmt.query_map([], note_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn get_note(&self, id: NoteId) -> Result<Note> {
        self.conn()
            .query_row(&format!("SELECT {COLS} FROM notes WHERE id = ?1"), [id], note_row)
            .optional()?
            .ok_or(Error::NotFound("note"))
    }

    pub fn create_note(&mut self, input: &NoteInput) -> Result<Note> {
        let n = normalize_note(input)?;
        let tags = serde_json::to_string(&n.tags).unwrap_or_else(|_| "[]".into());
        self.conn().execute(
            "INSERT INTO notes (title, body, host_id, tags, pinned) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![n.title, n.body, n.host_id, tags, n.pinned],
        )?;
        let id = self.conn().last_insert_rowid() as NoteId;
        self.get_note(id)
    }

    pub fn update_note(&mut self, id: NoteId, input: &NoteInput) -> Result<Note> {
        let n = normalize_note(input)?;
        let tags = serde_json::to_string(&n.tags).unwrap_or_else(|_| "[]".into());
        let changed = self.conn().execute(
            "UPDATE notes SET title = ?2, body = ?3, host_id = ?4, tags = ?5, pinned = ?6, updated_at = unixepoch()
             WHERE id = ?1",
            params![id, n.title, n.body, n.host_id, tags, n.pinned],
        )?;
        if changed == 0 {
            return Err(Error::NotFound("note"));
        }
        self.get_note(id)
    }

    pub fn delete_note(&mut self, id: NoteId) -> Result<()> {
        if self.conn().execute("DELETE FROM notes WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("note"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::HostInput;

    #[test]
    fn crud_validation_and_host_unlink() {
        let mut s = Store::open_in_memory().unwrap();
        let h =
            s.create_host(&HostInput { name: "nas".into(), hostname: "nas.lan".into(), ..Default::default() }).unwrap();
        let a = s
            .create_note(&NoteInput {
                title: "  Replace a disk ".into(),
                body: "1. `zpool status`\n".into(),
                host_id: Some(h.id),
                tags: vec!["#ZFS".into(), "zfs".into(), " ".into()],
                pinned: false,
            })
            .unwrap();
        assert_eq!(a.title, "Replace a disk");
        assert_eq!(a.body, "1. `zpool status`");
        assert_eq!(a.tags, vec!["zfs"]);
        let b = s.create_note(&NoteInput { title: "Pinned".into(), pinned: true, ..Default::default() }).unwrap();
        let order: Vec<_> = s.list_notes().unwrap().into_iter().map(|n| n.id).collect();
        assert_eq!(order, vec![b.id, a.id]);

        let up = s
            .update_note(a.id, &NoteInput { title: "Disk swap".into(), host_id: Some(h.id), ..Default::default() })
            .unwrap();
        assert_eq!(up.title, "Disk swap");
        assert!(up.tags.is_empty());

        // A secret is refused, a masked one is fine.
        let bad = NoteInput { title: "db".into(), body: "DB_PASSWORD=hunter22".into(), ..Default::default() };
        assert!(matches!(s.create_note(&bad), Err(Error::Invalid { field: "body", .. })));
        let ok = NoteInput { body: "DB_PASSWORD=••••".into(), ..bad };
        s.create_note(&ok).unwrap();

        assert!(s.create_note(&NoteInput::default()).is_err());
        assert!(s
            .create_note(&NoteInput { title: "t".into(), tags: vec!["a b".into()], ..Default::default() })
            .is_err());

        // Deleting the host keeps the note, unlinked.
        s.delete_host(h.id).unwrap();
        assert_eq!(s.get_note(a.id).unwrap().host_id, None);
        s.delete_note(a.id).unwrap();
        assert!(matches!(s.delete_note(a.id), Err(Error::NotFound(_))));
    }
}
