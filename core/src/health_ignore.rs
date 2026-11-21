//! Ignored healthchecks (D-201): containers that are "running but
//! unhealthy" by design.
//!
//! Some images ship a healthcheck that never passes in a given setup (a
//! probe for a port the user turned off, a check that needs internet the
//! box doesn't have). Ignoring it makes the container's health count as "no
//! healthcheck" everywhere Nightgrid judges it: no CONTAINER DOWN alert for
//! "running but unhealthy", no amber/red status, no "unhealthy" hint in the
//! log watch or DRIFT's context. Exits with an error, restart loops and
//! crashes still alert as before.
//!
//! An ignore is keyed by host plus compose project/service (or the
//! container name for a plain `docker run` container), so recreating the
//! container keeps it. Nothing is ignored by default.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::docker::ContainerView;
use crate::holds::valid_service;
use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

pub type HealthIgnoreId = i32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HealthIgnore {
    pub id: HealthIgnoreId,
    pub host_id: HostId,
    /// Compose project; empty for a plain container.
    pub project: String,
    /// Compose service; empty for a plain container.
    pub service: String,
    /// Container name; empty when matched by project/service.
    pub name: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub created_at: i64,
}

impl HealthIgnore {
    /// Whether this ignore covers `c` on `host_id`.
    pub fn matches(&self, host_id: HostId, c: &ContainerView) -> bool {
        if self.host_id != host_id {
            return false;
        }
        if self.project.is_empty() {
            !self.name.is_empty() && self.name == c.name
        } else {
            c.project.as_deref() == Some(self.project.as_str()) && c.service.as_deref() == Some(self.service.as_str())
        }
    }

    /// How the UI names it: `project/service` or the container name.
    pub fn label(&self) -> String {
        if self.project.is_empty() {
            self.name.clone()
        } else {
            format!("{}/{}", self.project, self.service)
        }
    }
}

/// The key a container is ignored by: compose project/service when it has
/// them, else its name. `(project, service, name)`.
pub fn key_of(c: &ContainerView) -> (String, String, String) {
    match (c.project.as_deref(), c.service.as_deref()) {
        (Some(p), Some(s)) if !p.is_empty() && !s.is_empty() => (p.to_owned(), s.to_owned(), String::new()),
        _ => (String::new(), String::new(), c.name.clone()),
    }
}

/// What the UI sends to ignore one container's healthcheck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HealthIgnoreInput {
    pub host_id: HostId,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub service: Option<String>,
    pub name: String,
}

fn invalid(field: &'static str, reason: &str) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

/// Validated `(project, service, name)` to store.
fn clean(i: &HealthIgnoreInput) -> Result<(String, String, String)> {
    let project = i.project.as_deref().map(str::trim).unwrap_or_default();
    let service = i.service.as_deref().map(str::trim).unwrap_or_default();
    let name = i.name.trim().trim_start_matches('/');
    if !project.is_empty() || !service.is_empty() {
        if !valid_service(project) {
            return Err(invalid("project", "a compose project name"));
        }
        if !valid_service(service) {
            return Err(invalid("service", "a compose service name"));
        }
        return Ok((project.to_owned(), service.to_owned(), String::new()));
    }
    if !crate::docker::valid_container_ref(name) {
        return Err(invalid("name", "a container name"));
    }
    Ok((String::new(), String::new(), name.to_owned()))
}

impl Store {
    pub fn health_ignores(&self) -> Result<Vec<HealthIgnore>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, host_id, project, service, name, created_at FROM health_ignores
             ORDER BY host_id, project, service, name",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(HealthIgnore {
                id: r.get(0)?,
                host_id: r.get(1)?,
                project: r.get(2)?,
                service: r.get(3)?,
                name: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Ignores a container's healthcheck (saving the same one twice is fine).
    pub fn add_health_ignore(&mut self, input: &HealthIgnoreInput) -> Result<HealthIgnore> {
        let (project, service, name) = clean(input)?;
        self.conn().execute(
            "INSERT INTO health_ignores (host_id, project, service, name) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(host_id, project, service, name) DO NOTHING",
            params![input.host_id, project, service, name],
        )?;
        self.health_ignores()?
            .into_iter()
            .find(|h| h.host_id == input.host_id && h.project == project && h.service == service && h.name == name)
            .ok_or(Error::NotFound("health ignore"))
    }

    pub fn delete_health_ignore(&mut self, id: HealthIgnoreId) -> Result<()> {
        self.conn().execute("DELETE FROM health_ignores WHERE id = ?1", [id])?;
        Ok(())
    }
}

/// Marks the containers of `host_id` whose healthcheck is ignored
/// (`ContainerView::health_ignored`). Call on every list Nightgrid judges.
pub fn apply(ignores: &[HealthIgnore], host_id: HostId, list: &mut [ContainerView]) {
    for c in list.iter_mut() {
        c.health_ignored = ignores.iter().any(|i| i.matches(host_id, c));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::HostInput;

    fn ctr(name: &str, project: Option<&str>, service: Option<&str>) -> ContainerView {
        ContainerView {
            id: name.into(),
            name: name.into(),
            image: "img".into(),
            state: "running".into(),
            status: "Up 1 hour (unhealthy)".into(),
            health: Some("unhealthy".into()),
            health_ignored: false,
            created: 0,
            ports: vec![],
            project: project.map(Into::into),
            service: service.map(Into::into),
            compose_dir: None,
            compose_files: vec![],
            web: vec![],
        }
    }

    #[test]
    fn keys_survive_recreation_and_stay_on_their_host() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&HostInput { name: "hs02".into(), hostname: "hs02.lan".into(), ..Default::default() })
            .unwrap();
        let other =
            s.create_host(&HostInput { name: "nas".into(), hostname: "nas.lan".into(), ..Default::default() }).unwrap();

        // Bad input is refused.
        let bad = |project: Option<&str>, service: Option<&str>, name: &str| HealthIgnoreInput {
            host_id: h.id,
            project: project.map(Into::into),
            service: service.map(Into::into),
            name: name.into(),
        };
        assert!(s.add_health_ignore(&bad(None, None, "a b")).is_err());
        assert!(s.add_health_ignore(&bad(Some("media"), None, "x")).is_err());
        assert!(s.add_health_ignore(&bad(Some("me dia"), Some("shoko"), "x")).is_err());

        let compose = s.add_health_ignore(&bad(Some("media"), Some("shoko"), "HS02Shoko")).unwrap();
        assert_eq!(compose.label(), "media/shoko");
        assert!(compose.name.is_empty());
        // Twice is the same row.
        assert_eq!(s.add_health_ignore(&bad(Some("media"), Some("shoko"), "HS02Shoko")).unwrap().id, compose.id);
        let plain = s.add_health_ignore(&bad(None, None, "/legacy")).unwrap();
        assert_eq!(plain.label(), "legacy");

        let all = s.health_ignores().unwrap();
        let mut list = vec![
            // Recreated: new name, same compose service.
            ctr("media-shoko-1", Some("media"), Some("shoko")),
            ctr("legacy", None, None),
            ctr("media-plex-1", Some("media"), Some("plex")),
        ];
        apply(&all, h.id, &mut list);
        assert_eq!(list.iter().map(|c| c.health_ignored).collect::<Vec<_>>(), [true, true, false]);
        assert_eq!(list[0].effective_health(), None);
        assert_eq!(list[2].effective_health(), Some("unhealthy"));
        // Another host's container of the same name isn't covered.
        apply(&all, other.id, &mut list);
        assert!(list.iter().all(|c| !c.health_ignored));

        s.delete_health_ignore(plain.id).unwrap();
        assert_eq!(s.health_ignores().unwrap().len(), 1);
        s.delete_host(h.id).unwrap();
        assert!(s.health_ignores().unwrap().is_empty());
    }

    #[test]
    fn key_prefers_compose_service() {
        assert_eq!(key_of(&ctr("a", Some("p"), Some("s"))), ("p".into(), "s".into(), String::new()));
        assert_eq!(key_of(&ctr("a", None, None)), (String::new(), String::new(), "a".into()));
        assert_eq!(key_of(&ctr("a", Some("p"), None)), (String::new(), String::new(), "a".into()));
    }
}
