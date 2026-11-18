//! The quest log (1.3): Library guides the user started as quests. A quest
//! is one guide on one host (or on no host, for a guide with nothing to
//! check) and remembers which of its steps are done, so the top bar can show
//! every active quest with its current step. The guide text stays in the app
//! bundle; only the article id, its title and the done step keys are kept.
//! Quests are config: they travel in the profile export.

use std::collections::BTreeSet;

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

pub type QuestId = i32;

/// At most this many quests (started and finished).
pub const MAX_QUESTS: i64 = 500;
/// At most this many done steps per quest.
pub const MAX_TICKS: usize = 1_000;
pub const MAX_TITLE: usize = 200;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Quest {
    pub id: QuestId,
    /// Library article id (`guides/proxmox-media-server`).
    pub article_id: String,
    pub host_id: Option<HostId>,
    /// The guide's title when the quest was started.
    pub title: String,
    /// Done steps, as the Library's step keys (`"12-0"`).
    pub ticks: Vec<String>,
    /// Unix seconds.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub started_at: i64,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub updated_at: i64,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub completed_at: Option<i64>,
}

fn inv(field: &'static str, reason: impl Into<String>) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

/// `shelf/slug`, as the Library names articles.
pub fn check_article(id: &str) -> Result<()> {
    let ok = id.len() <= 120
        && id.split_once('/').is_some_and(|(shelf, slug)| {
            !shelf.is_empty()
                && !slug.is_empty()
                && shelf.chars().all(|c| c.is_ascii_lowercase())
                && slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        });
    if ok {
        Ok(())
    } else {
        Err(inv("articleId", "not a Library article id"))
    }
}

/// A step key is `block-item`, two small numbers.
fn check_key(k: &str) -> bool {
    k.len() <= 11
        && k.split_once('-').is_some_and(|(a, b)| {
            !a.is_empty()
                && !b.is_empty()
                && a.chars().all(|c| c.is_ascii_digit())
                && b.chars().all(|c| c.is_ascii_digit())
        })
}

/// Valid keys, deduplicated and sorted, or an error naming the first bad one.
pub fn clean_ticks(ticks: &[String]) -> Result<Vec<String>> {
    if ticks.len() > MAX_TICKS {
        return Err(inv("ticks", format!("at most {MAX_TICKS} steps")));
    }
    if let Some(bad) = ticks.iter().find(|k| !check_key(k)) {
        return Err(inv("ticks", format!("{bad:?} isn't a step")));
    }
    Ok(ticks.iter().cloned().collect::<BTreeSet<_>>().into_iter().collect())
}

const COLS: &str = "id, article_id, host_id, title, ticks, started_at, updated_at, completed_at";

fn row(r: &Row<'_>) -> rusqlite::Result<Quest> {
    let ticks: String = r.get(4)?;
    Ok(Quest {
        id: r.get(0)?,
        article_id: r.get(1)?,
        host_id: r.get(2)?,
        title: r.get(3)?,
        // A hand-edited or damaged row loses its ticks, never the quest.
        ticks: serde_json::from_str::<Vec<String>>(&ticks)
            .map(|v| v.into_iter().filter(|k| check_key(k)).collect())
            .unwrap_or_default(),
        started_at: r.get(5)?,
        updated_at: r.get(6)?,
        completed_at: r.get(7)?,
    })
}

impl Store {
    /// Every quest, active ones first, most recently touched first.
    pub fn quests(&self) -> Result<Vec<Quest>> {
        let mut st = self.conn().prepare(&format!(
            "SELECT {COLS} FROM quests ORDER BY completed_at IS NOT NULL, updated_at DESC, id DESC"
        ))?;
        let rows = st.query_map([], row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn quest(&self, id: QuestId) -> Result<Option<Quest>> {
        Ok(self.conn().query_row(&format!("SELECT {COLS} FROM quests WHERE id = ?1"), [id], row).optional()?)
    }

    /// Starts a guide as a quest on a host. Starting one that already exists
    /// returns it (a finished one is reopened with its ticks kept).
    pub fn start_quest(
        &mut self,
        article_id: &str,
        host_id: Option<HostId>,
        title: &str,
        ticks: &[String],
        now: i64,
    ) -> Result<Quest> {
        check_article(article_id)?;
        let title = title.trim();
        if title.is_empty() || title.chars().count() > MAX_TITLE || title.chars().any(char::is_control) {
            return Err(inv("title", format!("1 to {MAX_TITLE} characters")));
        }
        let ticks = clean_ticks(ticks)?;
        let existing: Option<QuestId> = self
            .conn()
            .query_row(
                "SELECT id FROM quests WHERE article_id = ?1 AND host_id IS ?2",
                params![article_id, host_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            self.conn().execute(
                "UPDATE quests SET completed_at = NULL, updated_at = ?2, title = ?3 WHERE id = ?1",
                params![id, now, title],
            )?;
            return self.quest(id)?.ok_or_else(|| inv("id", "no such quest"));
        }
        let n: i64 = self.conn().query_row("SELECT COUNT(*) FROM quests", [], |r| r.get(0))?;
        if n >= MAX_QUESTS {
            return Err(inv("quests", format!("at most {MAX_QUESTS} quests; abandon some finished ones first")));
        }
        if let Some(h) = host_id {
            let known: bool =
                self.conn().query_row("SELECT EXISTS(SELECT 1 FROM hosts WHERE id = ?1)", [h], |r| r.get(0))?;
            if !known {
                return Err(inv("hostId", "no such host"));
            }
        }
        self.conn().execute(
            "INSERT INTO quests (article_id, host_id, title, ticks, started_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![article_id, host_id, title, serde_json::to_string(&ticks)?, now],
        )?;
        let id = self.conn().last_insert_rowid() as QuestId;
        self.quest(id)?.ok_or_else(|| inv("id", "no such quest"))
    }

    /// Replaces a quest's done steps; `complete` marks it finished (or
    /// reopens it when false).
    pub fn set_quest_ticks(&mut self, id: QuestId, ticks: &[String], complete: bool, now: i64) -> Result<Quest> {
        let ticks = clean_ticks(ticks)?;
        let n = self.conn().execute(
            "UPDATE quests SET ticks = ?2, updated_at = ?3,
               completed_at = CASE WHEN ?4 THEN COALESCE(completed_at, ?3) ELSE NULL END
             WHERE id = ?1",
            params![id, serde_json::to_string(&ticks)?, now, complete],
        )?;
        if n == 0 {
            return Err(inv("id", "no such quest"));
        }
        self.quest(id)?.ok_or_else(|| inv("id", "no such quest"))
    }

    /// Abandons (deletes) a quest.
    pub fn delete_quest(&mut self, id: QuestId) -> Result<bool> {
        Ok(self.conn().execute("DELETE FROM quests WHERE id = ?1", [id])? > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::HostInput;

    fn store() -> Store {
        Store::open_in_memory().unwrap()
    }

    fn host(s: &mut Store) -> HostId {
        s.create_host(&HostInput { name: "box".into(), hostname: "10.0.0.2".into(), ..Default::default() }).unwrap().id
    }

    #[test]
    fn validates_ids_and_keys() {
        assert!(check_article("guides/proxmox-media-server").is_ok());
        assert!(check_article("apps/k3s").is_ok());
        assert!(check_article("guides").is_err());
        assert!(check_article("guides/../x").is_err());
        assert!(check_article("Guides/x").is_err());
        assert_eq!(clean_ticks(&["3-1".into(), "1-0".into(), "3-1".into()]).unwrap(), vec!["1-0", "3-1"]);
        assert!(clean_ticks(&["1".into()]).is_err());
        assert!(clean_ticks(&["a-b".into()]).is_err());
    }

    #[test]
    fn start_tick_complete_and_reopen() {
        let mut s = store();
        let h = host(&mut s);
        let q = s.start_quest("guides/game-server-host", Some(h), "Game server", &["0-1".into()], 100).unwrap();
        assert_eq!(q.ticks, vec!["0-1"]);
        assert_eq!(q.host_id, Some(h));
        // The same guide on the same host is the same quest.
        let again = s.start_quest("guides/game-server-host", Some(h), "Game server", &[], 110).unwrap();
        assert_eq!(again.id, q.id);
        assert_eq!(again.ticks, vec!["0-1"]);
        // On no host it's another one.
        let other = s.start_quest("guides/game-server-host", None, "Game server", &[], 120).unwrap();
        assert_ne!(other.id, q.id);
        assert_eq!(s.start_quest("guides/game-server-host", None, "Game server", &[], 121).unwrap().id, other.id);

        let done = s.set_quest_ticks(q.id, &["0-1".into(), "0-2".into()], true, 200).unwrap();
        assert_eq!(done.completed_at, Some(200));
        // Completing twice keeps the first time.
        assert_eq!(s.set_quest_ticks(q.id, &["0-1".into(), "0-2".into()], true, 300).unwrap().completed_at, Some(200));
        // Active quests list first.
        assert_eq!(s.quests().unwrap()[0].id, other.id);
        // Starting a finished quest reopens it.
        let re = s.start_quest("guides/game-server-host", Some(h), "Game server", &[], 400).unwrap();
        assert_eq!(re.completed_at, None);
        assert_eq!(re.ticks.len(), 2);

        assert!(s.delete_quest(q.id).unwrap());
        assert!(s.quest(q.id).unwrap().is_none());
        assert!(s.set_quest_ticks(q.id, &[], false, 500).is_err());
    }

    #[test]
    fn removing_a_host_drops_its_quests() {
        let mut s = store();
        let h = host(&mut s);
        s.start_quest("apps/caddy", Some(h), "Caddy", &[], 1).unwrap();
        assert!(s.start_quest("apps/caddy", Some(h + 99), "Caddy", &[], 1).is_err());
        s.delete_host(h).unwrap();
        assert!(s.quests().unwrap().is_empty());
    }
}
