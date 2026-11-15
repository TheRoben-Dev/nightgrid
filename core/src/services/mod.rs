//! Services board: HTTP(S) endpoints with health checks (status, latency,
//! TLS certificate expiry), optionally tied to a host, importable from a
//! Homepage `services.yaml`.

pub mod check;
pub mod homepage;

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

pub type ServiceId = i32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub id: ServiceId,
    pub name: String,
    pub url: String,
    pub host_id: Option<HostId>,
    pub group: String,
    pub description: String,
    pub icon: Option<String>,
    pub check_enabled: bool,
    /// Status codes that count as up: ranges and single codes, e.g. `200-399` or `200,401`.
    pub expect_status: String,
    /// Self-signed / internal CA: shown, but doesn't make the service "down".
    pub allow_untrusted: bool,
    pub sort_order: i32,
    /// `manual` or `homepage`.
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct ServiceInput {
    pub name: String,
    pub url: String,
    pub host_id: Option<HostId>,
    pub group: String,
    pub description: String,
    pub icon: Option<String>,
    pub check_enabled: bool,
    pub expect_status: String,
    pub allow_untrusted: bool,
}

/// One stored check result (for uptime and the latency trend).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CheckPoint {
    /// Unix seconds.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub ts: i64,
    pub up: bool,
    pub status: Option<u16>,
    pub latency_ms: Option<u32>,
}

/// Parses and validates a service URL (http/https with a host, no credentials).
pub fn validate_url(url: &str) -> Result<String> {
    let url = url.trim();
    let invalid = |reason: &str| Error::Invalid { field: "url", reason: reason.into() };
    let (scheme, rest) = url.split_once("://").ok_or_else(|| invalid("use http:// or https://"))?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return Err(invalid("use http:// or https://"));
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return Err(invalid("missing host"));
    }
    if authority.contains('@') {
        return Err(invalid("don't put credentials in the URL"));
    }
    if url.len() > 2048 || url.chars().any(|c| c.is_control() || c == ' ') {
        return Err(invalid("contains spaces or control characters"));
    }
    Ok(url.to_owned())
}

/// `200-399`, `200,204,401`, `2xx`… → does `code` match? Empty means 200–399.
pub fn status_matches(spec: &str, code: u16) -> bool {
    let spec = spec.trim();
    if spec.is_empty() {
        return (200..400).contains(&code);
    }
    spec.split(',').map(str::trim).filter(|p| !p.is_empty()).any(|part| {
        if let Some(d) = part.strip_suffix("xx").and_then(|d| d.parse::<u16>().ok()) {
            return code / 100 == d;
        }
        match part.split_once('-') {
            Some((a, b)) => match (a.trim().parse::<u16>(), b.trim().parse::<u16>()) {
                (Ok(a), Ok(b)) => (a..=b).contains(&code),
                _ => false,
            },
            None => part.parse::<u16>().is_ok_and(|c| c == code),
        }
    })
}

fn valid_status_spec(spec: &str) -> bool {
    spec.trim().is_empty()
        || spec.split(',').map(str::trim).all(|p| {
            p.strip_suffix("xx").is_some_and(|d| matches!(d, "1" | "2" | "3" | "4" | "5"))
                || p.parse::<u16>().is_ok_and(|c| (100..600).contains(&c))
                || p.split_once('-').is_some_and(|(a, b)| {
                    matches!((a.trim().parse::<u16>(), b.trim().parse::<u16>()), (Ok(a), Ok(b)) if a <= b && (100..600).contains(&a) && b < 600)
                })
        })
}

fn normalize(input: &ServiceInput) -> Result<ServiceInput> {
    let name = input.name.trim();
    if name.is_empty() || name.len() > 120 {
        return Err(Error::Invalid { field: "name", reason: "1–120 characters".into() });
    }
    let url = validate_url(&input.url)?;
    if !valid_status_spec(&input.expect_status) {
        return Err(Error::Invalid {
            field: "expectStatus",
            reason: "use codes or ranges like 200-399, 401 or 2xx".into(),
        });
    }
    Ok(ServiceInput {
        name: name.to_owned(),
        url,
        host_id: input.host_id,
        group: input.group.trim().chars().take(80).collect(),
        description: input.description.trim().chars().take(300).collect(),
        icon: input.icon.as_ref().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty() && s.len() <= 200),
        check_enabled: input.check_enabled,
        expect_status: if input.expect_status.trim().is_empty() {
            "200-399".into()
        } else {
            input.expect_status.trim().into()
        },
        allow_untrusted: input.allow_untrusted,
    })
}

const COLS: &str = "id, name, url, host_id, group_name, description, icon, check_enabled, expect_status, allow_untrusted, sort_order, source";

fn from_row(r: &Row<'_>) -> rusqlite::Result<Service> {
    Ok(Service {
        id: r.get(0)?,
        name: r.get(1)?,
        url: r.get(2)?,
        host_id: r.get(3)?,
        group: r.get(4)?,
        description: r.get(5)?,
        icon: r.get(6)?,
        check_enabled: r.get(7)?,
        expect_status: r.get(8)?,
        allow_untrusted: r.get(9)?,
        sort_order: r.get(10)?,
        source: r.get(11)?,
    })
}

impl Store {
    pub fn list_services(&self) -> Result<Vec<Service>> {
        let mut stmt = self.conn().prepare(&format!("SELECT {COLS} FROM services ORDER BY sort_order, id"))?;
        let rows = stmt.query_map([], from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn get_service(&self, id: ServiceId) -> Result<Service> {
        self.conn()
            .query_row(&format!("SELECT {COLS} FROM services WHERE id = ?1"), [id], from_row)
            .optional()?
            .ok_or(Error::NotFound("service"))
    }

    pub fn create_service(&mut self, input: &ServiceInput) -> Result<Service> {
        let s = normalize(input)?;
        self.conn().execute(
            "INSERT INTO services (name, url, host_id, group_name, description, icon, check_enabled, expect_status, allow_untrusted, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM services))",
            params![s.name, s.url, s.host_id, s.group, s.description, s.icon, s.check_enabled, s.expect_status, s.allow_untrusted],
        )?;
        let id = self.conn().last_insert_rowid() as ServiceId;
        self.get_service(id)
    }

    pub fn update_service(&mut self, id: ServiceId, input: &ServiceInput) -> Result<Service> {
        let s = normalize(input)?;
        let n = self.conn().execute(
            "UPDATE services SET name = ?2, url = ?3, host_id = ?4, group_name = ?5, description = ?6, icon = ?7,
                    check_enabled = ?8, expect_status = ?9, allow_untrusted = ?10, updated_at = unixepoch()
             WHERE id = ?1",
            params![
                id,
                s.name,
                s.url,
                s.host_id,
                s.group,
                s.description,
                s.icon,
                s.check_enabled,
                s.expect_status,
                s.allow_untrusted
            ],
        )?;
        if n == 0 {
            return Err(Error::NotFound("service"));
        }
        self.get_service(id)
    }

    pub fn delete_service(&mut self, id: ServiceId) -> Result<()> {
        if self.conn().execute("DELETE FROM services WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("service"));
        }
        Ok(())
    }

    pub fn record_check(
        &self,
        id: ServiceId,
        ts: i64,
        up: bool,
        status: Option<u16>,
        latency_ms: Option<u32>,
    ) -> Result<()> {
        self.conn().execute(
            "INSERT INTO service_checks (service_id, ts, up, status, latency_ms) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(service_id, ts) DO UPDATE SET up = excluded.up, status = excluded.status, latency_ms = excluded.latency_ms",
            params![id, ts, up, status, latency_ms],
        )?;
        Ok(())
    }

    /// Checks since `from` (unix seconds), oldest first.
    pub fn service_checks(&self, id: ServiceId, from: i64) -> Result<Vec<CheckPoint>> {
        let mut stmt = self.conn().prepare(
            "SELECT ts, up, status, latency_ms FROM service_checks WHERE service_id = ?1 AND ts >= ?2 ORDER BY ts",
        )?;
        let rows = stmt.query_map(params![id, from], |r| {
            Ok(CheckPoint { ts: r.get(0)?, up: r.get(1)?, status: r.get(2)?, latency_ms: r.get(3)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Drops check results older than `days`.
    pub fn prune_service_checks(&self, now: i64, days: i64) -> Result<usize> {
        Ok(self.conn().execute("DELETE FROM service_checks WHERE ts < ?1", [now - days * 86_400])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::HostInput;

    fn input(name: &str, url: &str) -> ServiceInput {
        ServiceInput { name: name.into(), url: url.into(), check_enabled: true, ..Default::default() }
    }

    #[test]
    fn url_validation() {
        assert!(validate_url("https://jellyfin.lan:8096/web").is_ok());
        assert!(validate_url(" http://10.0.0.5 ").is_ok());
        assert!(validate_url("ftp://x").is_err());
        assert!(validate_url("https://").is_err());
        assert!(validate_url("https://user:pw@host").is_err());
        assert!(validate_url("https://a b").is_err());
        assert!(validate_url("host:80").is_err());
    }

    #[test]
    fn status_specs() {
        assert!(status_matches("", 200));
        assert!(status_matches("200-399", 302));
        assert!(!status_matches("200-399", 401));
        assert!(status_matches("200, 401", 401));
        assert!(status_matches("2xx,3xx", 204));
        assert!(!status_matches("2xx", 500));
        assert!(valid_status_spec("200-399,401,5xx"));
        assert!(!valid_status_spec("abc"));
        assert!(!valid_status_spec("399-200"));
        assert!(!valid_status_spec("9xx"));
    }

    #[test]
    fn crud_and_checks() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&HostInput { name: "server01".into(), hostname: "server01.lan".into(), ..Default::default() })
            .unwrap();
        let mut i = input("  Media ", "https://media.lan/");
        i.host_id = Some(h.id);
        i.group = "Apps".into();
        let a = s.create_service(&i).unwrap();
        assert_eq!(a.name, "Media");
        assert_eq!(a.expect_status, "200-399");
        assert_eq!(a.source, "manual");
        assert!(s.create_service(&input("bad", "nope")).is_err());
        let mut j = i.clone();
        j.expect_status = "200,401".into();
        let a2 = s.update_service(a.id, &j).unwrap();
        assert_eq!(a2.expect_status, "200,401");

        s.record_check(a.id, 100, true, Some(200), Some(12)).unwrap();
        s.record_check(a.id, 160, false, None, None).unwrap();
        let pts = s.service_checks(a.id, 0).unwrap();
        assert_eq!(pts.len(), 2);
        assert!(!pts[1].up);
        assert_eq!(s.prune_service_checks(160 + 86_400, 1).unwrap(), 1);

        // Deleting the host keeps the service, unlinked.
        s.delete_host(h.id).unwrap();
        assert_eq!(s.get_service(a.id).unwrap().host_id, None);
        s.delete_service(a.id).unwrap();
        assert!(s.service_checks(a.id, 0).unwrap().is_empty());
        assert!(matches!(s.delete_service(a.id), Err(Error::NotFound(_))));
    }
}
