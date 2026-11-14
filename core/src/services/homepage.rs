//! Import from a [Homepage](https://gethomepage.dev) `services.yaml`.
//!
//! The file is a list of groups; each group is a list of single-key maps
//! (`- Name: {href, description, icon, siteMonitor, ...}`). A group entry whose
//! value is itself a list is a nested group, flattened to `Parent / Child`.
//! Group order can come from `settings.yaml`'s `layout:` map.
//!
//! Re-syncing matches entries on `group/name`, updates what the file owns
//! (name, URL, group, description, icon) and keeps what the user set in
//! Nightgrid (checks, expected status, host link).

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use yaml_rust2::{Yaml, YamlLoader};

use super::{normalize, ServiceInput};
use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

pub const SOURCE: &str = "homepage";
/// Refuse absurd files (Homepage configs are a few KB).
const MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HomepageEntry {
    pub group: String,
    pub name: String,
    pub url: String,
    pub description: String,
    pub icon: Option<String>,
}

impl HomepageEntry {
    pub fn key(&self) -> String {
        format!("{}/{}", self.group, self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HomepageParse {
    pub entries: Vec<HomepageEntry>,
    /// Entries that were skipped and why (no URL, template variables…).
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub added: u32,
    pub updated: u32,
    pub unchanged: u32,
    /// Previously imported entries no longer in the file.
    pub removed: u32,
    pub stale: Vec<String>,
}

fn load(text: &str) -> Result<Vec<Yaml>> {
    if text.len() > MAX_BYTES {
        return Err(Error::Invalid { field: "services.yaml", reason: "file is too large".into() });
    }
    YamlLoader::load_from_str(text).map_err(|e| Error::Invalid { field: "services.yaml", reason: e.to_string() })
}

fn as_str(y: &Yaml) -> Option<String> {
    match y {
        Yaml::String(s) => Some(s.trim().to_owned()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Real(r) => Some(r.clone()),
        _ => None,
    }
    .filter(|s| !s.is_empty())
}

/// The single `key: value` of a list item like `- Name: {...}`.
fn single_pair(y: &Yaml) -> Option<(String, &Yaml)> {
    let h = y.as_hash()?;
    if h.len() != 1 {
        return None;
    }
    let (k, v) = h.iter().next()?;
    Some((as_str(k)?, v))
}

/// Parses `services.yaml`. Groups appear in `order` first (from settings.yaml),
/// then in file order.
pub fn parse_services(text: &str, order: &[String]) -> Result<HomepageParse> {
    let docs = load(text)?;
    let mut out = HomepageParse::default();
    let Some(root) = docs.first() else { return Ok(out) };
    let groups = match root {
        Yaml::Array(a) => a,
        Yaml::Null => return Ok(out),
        _ => return Err(Error::Invalid { field: "services.yaml", reason: "expected a list of groups".into() }),
    };
    for g in groups {
        let Some((name, items)) = single_pair(g) else {
            out.skipped.push("a top-level entry that isn't `- Group: [...]`".into());
            continue;
        };
        walk_group(&name, items, &mut out, 0);
    }
    if !order.is_empty() {
        let rank = |g: &str| {
            let top = g.split(" / ").next().unwrap_or(g);
            order.iter().position(|o| o == top).unwrap_or(usize::MAX)
        };
        // Stable: keeps file order within a group and among unlisted groups.
        out.entries.sort_by_key(|e| rank(&e.group));
    }
    Ok(out)
}

fn walk_group(group: &str, items: &Yaml, out: &mut HomepageParse, depth: u8) {
    let Some(items) = items.as_vec() else {
        out.skipped.push(format!("{group}: not a list of services"));
        return;
    };
    for item in items {
        let Some((name, body)) = single_pair(item) else {
            out.skipped.push(format!("{group}: an entry that isn't `- Name: {{...}}`"));
            continue;
        };
        match body {
            Yaml::Array(_) if depth < 4 => walk_group(&format!("{group} / {name}"), body, out, depth + 1),
            Yaml::Hash(_) => {
                let field = |k: &str| as_str(&body[k]);
                let Some(url) = field("href").or_else(|| field("siteMonitor")) else {
                    out.skipped.push(format!("{group}/{name}: no href or siteMonitor"));
                    continue;
                };
                if url.contains("{{") {
                    out.skipped.push(format!("{group}/{name}: URL uses a Homepage variable ({url})"));
                    continue;
                }
                if super::validate_url(&url).is_err() {
                    out.skipped.push(format!("{group}/{name}: not an http(s) URL ({url})"));
                    continue;
                }
                out.entries.push(HomepageEntry {
                    group: group.to_owned(),
                    name,
                    url,
                    description: field("description").unwrap_or_default(),
                    icon: field("icon"),
                });
            }
            _ => out.skipped.push(format!("{group}/{name}: unsupported entry")),
        }
    }
}

/// Group names from `settings.yaml`'s `layout:` map (or list), in order.
pub fn parse_layout_order(text: &str) -> Result<Vec<String>> {
    let docs = load(text)?;
    let Some(root) = docs.first() else { return Ok(Vec::new()) };
    Ok(match &root["layout"] {
        Yaml::Hash(h) => h.keys().filter_map(as_str).collect(),
        Yaml::Array(a) => a.iter().filter_map(|i| single_pair(i).map(|(k, _)| k).or_else(|| as_str(i))).collect(),
        _ => Vec::new(),
    })
}

impl Store {
    /// Creates or updates services from parsed entries. With `prune`, entries
    /// imported earlier but gone from the file are deleted; otherwise they're
    /// listed in `stale`.
    pub fn import_homepage(
        &mut self,
        entries: &[HomepageEntry],
        host_id: Option<HostId>,
        prune: bool,
    ) -> Result<ImportSummary> {
        let tx = self.conn_mut().transaction()?;
        let mut sum = ImportSummary::default();
        let mut next_order: i32 =
            tx.query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM services", [], |r| r.get(0))?;
        let mut seen = std::collections::HashSet::new();
        for e in entries {
            let key = e.key();
            if !seen.insert(key.clone()) {
                continue;
            }
            let existing: Option<(i32, String, String, String, String, Option<String>)> = tx
                .query_row(
                    "SELECT id, name, url, group_name, description, icon FROM services WHERE source = ?1 AND source_key = ?2",
                    params![SOURCE, key],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
                )
                .optional()?;
            let s = normalize(&ServiceInput {
                name: e.name.clone(),
                url: e.url.clone(),
                group: e.group.clone(),
                description: e.description.clone(),
                icon: e.icon.clone(),
                check_enabled: true,
                ..Default::default()
            })?;
            match existing {
                Some((id, name, url, group, desc, icon)) => {
                    if (name, url, group, desc, icon)
                        == (s.name.clone(), s.url.clone(), s.group.clone(), s.description.clone(), s.icon.clone())
                    {
                        sum.unchanged += 1;
                    } else {
                        tx.execute(
                            "UPDATE services SET name = ?2, url = ?3, group_name = ?4, description = ?5, icon = ?6, updated_at = unixepoch()
                             WHERE id = ?1",
                            params![id, s.name, s.url, s.group, s.description, s.icon],
                        )?;
                        sum.updated += 1;
                    }
                }
                None => {
                    tx.execute(
                        "INSERT INTO services (name, url, host_id, group_name, description, icon, check_enabled, expect_status,
                                               allow_untrusted, sort_order, source, source_key)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, '200-399', 0, ?7, ?8, ?9)",
                        params![s.name, s.url, host_id, s.group, s.description, s.icon, next_order, SOURCE, key],
                    )?;
                    next_order += 1;
                    sum.added += 1;
                }
            }
        }
        let mut stmt = tx.prepare("SELECT id, source_key FROM services WHERE source = ?1")?;
        let old: Vec<(i32, String)> =
            stmt.query_map([SOURCE], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        for (id, key) in old.into_iter().filter(|(_, k)| !seen.contains(k)) {
            if prune {
                tx.execute("DELETE FROM services WHERE id = ?1", [id])?;
                sum.removed += 1;
            } else {
                sum.stale.push(key);
            }
        }
        tx.commit()?;
        Ok(sum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SERVICES: &str = r#"
- Media:
    - Jellyfin:
        icon: jellyfin.png
        href: https://media.example.lan/
        description: Movies & shows
        server: my-docker
        container: jellyfin
    - Requests:
        siteMonitor: http://10.0.0.5:5055
    - Broken:
        description: no link
- Infra:
    - Router:
        href: "{{HOMEPAGE_VAR_ROUTER}}"
    - Storage:
        - NAS:
            href: http://nas.example.lan:5000
            description: 1234
- Empty:
"#;

    #[test]
    fn parses_groups_nested_and_skips() {
        let p = parse_services(SERVICES, &[]).unwrap();
        let names: Vec<_> = p.entries.iter().map(|e| e.key()).collect();
        assert_eq!(names, ["Media/Jellyfin", "Media/Requests", "Infra / Storage/NAS"]);
        assert_eq!(p.entries[0].icon.as_deref(), Some("jellyfin.png"));
        assert_eq!(p.entries[0].description, "Movies & shows");
        assert_eq!(p.entries[1].url, "http://10.0.0.5:5055");
        assert_eq!(p.entries[2].description, "1234");
        assert_eq!(p.skipped.len(), 3, "{:?}", p.skipped); // Broken, Router, Empty
    }

    #[test]
    fn layout_orders_groups() {
        let order = parse_layout_order("layout:\n  Infra:\n    style: row\n  Media:\n").unwrap();
        assert_eq!(order, ["Infra", "Media"]);
        let p = parse_services(SERVICES, &order).unwrap();
        assert_eq!(p.entries[0].group, "Infra / Storage");
        assert!(parse_layout_order("title: x").unwrap().is_empty());
    }

    #[test]
    fn rejects_non_lists() {
        assert!(parse_services("foo: bar", &[]).is_err());
        assert!(parse_services(": : [", &[]).is_err());
        assert!(parse_services("", &[]).unwrap().entries.is_empty());
    }

    #[test]
    fn import_and_resync() {
        let mut s = Store::open_in_memory().unwrap();
        let p = parse_services(SERVICES, &[]).unwrap();
        let sum = s.import_homepage(&p.entries, None, false).unwrap();
        assert_eq!((sum.added, sum.updated, sum.unchanged), (3, 0, 0));

        // User tweaks a check setting; re-sync must keep it.
        let jf = s.list_services().unwrap().into_iter().find(|x| x.name == "Jellyfin").unwrap();
        let mut input = ServiceInput {
            name: jf.name.clone(),
            url: jf.url.clone(),
            group: jf.group.clone(),
            check_enabled: true,
            allow_untrusted: true,
            expect_status: "200,401".into(),
            ..Default::default()
        };
        input.description = jf.description.clone();
        input.icon = jf.icon.clone();
        s.update_service(jf.id, &input).unwrap();

        let mut changed = p.entries.clone();
        changed[0].url = "https://jf.example.lan/".into();
        changed.remove(1);
        let sum = s.import_homepage(&changed, None, false).unwrap();
        assert_eq!((sum.added, sum.updated, sum.unchanged, sum.removed), (0, 1, 1, 0));
        assert_eq!(sum.stale, ["Media/Requests"]);
        let jf2 = s.get_service(jf.id).unwrap();
        assert_eq!(jf2.url, "https://jf.example.lan/");
        assert!(jf2.allow_untrusted);
        assert_eq!(jf2.expect_status, "200,401");
        assert_eq!(jf2.source, SOURCE);

        let sum = s.import_homepage(&changed, None, true).unwrap();
        assert_eq!(sum.removed, 1);
        assert_eq!(s.list_services().unwrap().len(), 2);
    }
}
