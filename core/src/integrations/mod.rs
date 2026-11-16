//! Integrations: small read-mostly clients for apps people run on their
//! hosts. Each one is a base URL plus, where the app needs it, a secret kept
//! in the OS keychain (Plex token, AMP password). Supported:
//!
//! - **Plex**: server name and version, active streams, transcodes.
//! - **AMP** (CubeCoders): instances with state, players, CPU and memory;
//!   start, stop and restart.
//! - **Uptime Kuma**: monitors from a public status page (no API key needed).
//! - **Portainer**: version check plus deep links into its UI.
//! - **Pi-hole** and **AdGuard Home**: DNS stats, blocking on/off, lists and
//!   local records (M10v).
//!
//! Every client goes through [`crate::http`], so certificates are verified
//! unless the integration allows untrusted ones, and nothing is sent anywhere
//! but the configured URL.

pub mod amp;
pub mod dns;
pub mod kuma;
pub mod plex;
pub mod portainer;

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

pub type IntegrationId = i32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum IntegrationKind {
    Plex,
    Amp,
    Kuma,
    Portainer,
    Pihole,
    Adguard,
}

impl IntegrationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plex => "plex",
            Self::Amp => "amp",
            Self::Kuma => "kuma",
            Self::Portainer => "portainer",
            Self::Pihole => "pihole",
            Self::Adguard => "adguard",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "plex" => Self::Plex,
            "amp" => Self::Amp,
            "kuma" => Self::Kuma,
            "portainer" => Self::Portainer,
            "pihole" => Self::Pihole,
            "adguard" => Self::Adguard,
            _ => return None,
        })
    }

    /// Whether this kind needs a secret (token or password) to work.
    pub fn needs_secret(self) -> bool {
        matches!(self, Self::Plex | Self::Amp | Self::Pihole | Self::Adguard)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Integration {
    pub id: IntegrationId,
    pub kind: IntegrationKind,
    pub name: String,
    /// `http(s)://host[:port][/path]`, no trailing slash.
    pub base_url: String,
    /// AMP login name.
    pub username: String,
    /// Uptime Kuma status page slug.
    pub slug: String,
    /// Portainer environment id.
    pub endpoint_id: u32,
    /// Portainer: the host whose Docker it manages (for container links).
    pub host_id: Option<HostId>,
    pub allow_untrusted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct IntegrationInput {
    pub kind: Option<IntegrationKind>,
    pub name: String,
    pub base_url: String,
    pub username: String,
    pub slug: String,
    pub endpoint_id: u32,
    pub host_id: Option<HostId>,
    pub allow_untrusted: bool,
}

/// Validates and normalises a base URL: http(s) only, no credentials,
/// query or fragment, trailing slashes removed.
pub fn normalize_url(url: &str) -> std::result::Result<String, &'static str> {
    let u = url.trim().trim_end_matches('/');
    let lower = u.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .ok_or("must start with http:// or https://")?;
    let authority = rest.split('/').next().unwrap_or("");
    if authority.is_empty() {
        return Err("missing host");
    }
    if authority.contains('@') {
        return Err("don't put credentials in the URL");
    }
    if u.contains(['?', '#']) || u.chars().any(char::is_whitespace) {
        return Err("no query, fragment or spaces");
    }
    Ok(u.to_owned())
}

pub fn normalize(i: &IntegrationInput) -> Result<(IntegrationKind, IntegrationInput)> {
    let inv = |field: &'static str, reason: &str| Error::Invalid { field, reason: reason.into() };
    let kind = i.kind.ok_or_else(|| inv("kind", "pick an integration type"))?;
    let mut n = i.clone();
    n.name = n.name.trim().chars().take(80).collect();
    n.base_url = normalize_url(&n.base_url).map_err(|r| inv("baseUrl", r))?;
    n.username = n.username.trim().to_owned();
    n.slug = n.slug.trim().trim_matches('/').to_owned();
    if n.name.is_empty() {
        n.name = match kind {
            IntegrationKind::Plex => "Plex",
            IntegrationKind::Amp => "AMP",
            IntegrationKind::Kuma => "Uptime Kuma",
            IntegrationKind::Portainer => "Portainer",
            IntegrationKind::Pihole => "Pi-hole",
            IntegrationKind::Adguard => "AdGuard Home",
        }
        .into();
    }
    match kind {
        IntegrationKind::Amp if n.username.is_empty() => return Err(inv("username", "AMP needs a login name")),
        IntegrationKind::Adguard if n.username.is_empty() => {
            return Err(inv("username", "AdGuard Home needs its login name"))
        }
        IntegrationKind::Kuma => {
            if n.slug.is_empty() {
                return Err(inv("slug", "the status page slug, e.g. \"home\" from /status/home"));
            }
            if !n.slug.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')) {
                return Err(inv("slug", "letters, digits, - and _ only"));
            }
        }
        _ => {}
    }
    if !matches!(kind, IntegrationKind::Amp | IntegrationKind::Adguard) {
        n.username.clear();
    }
    if kind != IntegrationKind::Kuma {
        n.slug.clear();
    }
    if kind != IntegrationKind::Portainer {
        n.endpoint_id = 0;
        n.host_id = None;
    } else if n.endpoint_id == 0 {
        n.endpoint_id = 1;
    }
    Ok((kind, n))
}

/// What an integration reports right now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum IntegrationStatus {
    Plex(plex::PlexStatus),
    Amp(amp::AmpStatus),
    Kuma(kuma::KumaStatus),
    Portainer(portainer::PortainerStatus),
    Pihole(dns::DnsStatus),
    Adguard(dns::DnsStatus),
}

/// Reads an integration's status. `secret` is the token/password from the
/// keychain, when the kind uses one.
pub async fn status(i: &Integration, secret: Option<&str>) -> std::result::Result<IntegrationStatus, String> {
    match i.kind {
        IntegrationKind::Plex => {
            plex::status(i, secret.ok_or("no Plex token saved")?).await.map(IntegrationStatus::Plex)
        }
        IntegrationKind::Amp => {
            amp::status(i, secret.ok_or("no AMP password saved")?).await.map(IntegrationStatus::Amp)
        }
        IntegrationKind::Kuma => kuma::status(i).await.map(IntegrationStatus::Kuma),
        IntegrationKind::Portainer => portainer::status(i).await.map(IntegrationStatus::Portainer),
        IntegrationKind::Pihole => {
            dns::status(i, secret.ok_or("no Pi-hole password saved")?).await.map(IntegrationStatus::Pihole)
        }
        IntegrationKind::Adguard => {
            dns::status(i, secret.ok_or("no AdGuard Home password saved")?).await.map(IntegrationStatus::Adguard)
        }
    }
}

/// Short user-facing message for a non-2xx response.
pub(crate) fn http_error(app: &str, status: u16) -> String {
    match status {
        401 | 403 => format!("{app} refused the credentials (HTTP {status})"),
        404 => format!("{app} answered 404: check the URL"),
        s => format!("{app} answered HTTP {s}"),
    }
}

const COLS: &str = "id, kind, name, base_url, username, slug, endpoint_id, host_id, allow_untrusted";

fn from_row(r: &Row<'_>) -> rusqlite::Result<Integration> {
    let kind: String = r.get(1)?;
    Ok(Integration {
        id: r.get(0)?,
        kind: IntegrationKind::parse(&kind).unwrap_or(IntegrationKind::Portainer),
        name: r.get(2)?,
        base_url: r.get(3)?,
        username: r.get(4)?,
        slug: r.get(5)?,
        endpoint_id: r.get(6)?,
        host_id: r.get(7)?,
        allow_untrusted: r.get(8)?,
    })
}

impl Store {
    pub fn list_integrations(&self) -> Result<Vec<Integration>> {
        let mut stmt = self.conn().prepare(&format!("SELECT {COLS} FROM integrations ORDER BY id"))?;
        let rows = stmt.query_map([], from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn get_integration(&self, id: IntegrationId) -> Result<Integration> {
        self.conn()
            .query_row(&format!("SELECT {COLS} FROM integrations WHERE id = ?1"), [id], from_row)
            .optional()?
            .ok_or(Error::NotFound("integration"))
    }

    pub fn create_integration(&mut self, input: &IntegrationInput) -> Result<Integration> {
        let (kind, n) = normalize(input)?;
        self.conn().execute(
            "INSERT INTO integrations (kind, name, base_url, username, slug, endpoint_id, host_id, allow_untrusted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![kind.as_str(), n.name, n.base_url, n.username, n.slug, n.endpoint_id, n.host_id, n.allow_untrusted],
        )?;
        let id = self.conn().last_insert_rowid() as IntegrationId;
        self.get_integration(id)
    }

    /// Updates everything but the kind.
    pub fn update_integration(&mut self, id: IntegrationId, input: &IntegrationInput) -> Result<Integration> {
        let current = self.get_integration(id)?;
        let (kind, n) = normalize(&IntegrationInput { kind: Some(current.kind), ..input.clone() })?;
        self.conn().execute(
            "UPDATE integrations SET name = ?2, base_url = ?3, username = ?4, slug = ?5, endpoint_id = ?6,
                    host_id = ?7, allow_untrusted = ?8
             WHERE id = ?1 AND kind = ?9",
            params![
                id,
                n.name,
                n.base_url,
                n.username,
                n.slug,
                n.endpoint_id,
                n.host_id,
                n.allow_untrusted,
                kind.as_str()
            ],
        )?;
        self.get_integration(id)
    }

    pub fn delete_integration(&mut self, id: IntegrationId) -> Result<()> {
        if self.conn().execute("DELETE FROM integrations WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("integration"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(normalize_url(" https://plex.lan:32400/ ").unwrap(), "https://plex.lan:32400");
        assert_eq!(normalize_url("http://10.0.0.5/amp").unwrap(), "http://10.0.0.5/amp");
        assert!(normalize_url("ftp://x").is_err());
        assert!(normalize_url("https://user:pw@x").is_err());
        assert!(normalize_url("https://x/?token=1").is_err());
        assert!(normalize_url("https://").is_err());
    }

    #[test]
    fn crud_and_validation() {
        let mut s = Store::open_in_memory().unwrap();
        let plex = s
            .create_integration(&IntegrationInput {
                kind: Some(IntegrationKind::Plex),
                base_url: "http://server01:32400/".into(),
                username: "ignored".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            (plex.name.as_str(), plex.base_url.as_str(), plex.username.as_str()),
            ("Plex", "http://server01:32400", "")
        );

        let bad =
            IntegrationInput { kind: Some(IntegrationKind::Amp), base_url: "http://x".into(), ..Default::default() };
        assert!(s.create_integration(&bad).is_err(), "AMP needs a username");
        let bad = IntegrationInput {
            kind: Some(IntegrationKind::Kuma),
            base_url: "http://x".into(),
            slug: "a b".into(),
            ..Default::default()
        };
        assert!(s.create_integration(&bad).is_err());
        assert!(s.create_integration(&IntegrationInput::default()).is_err());

        let p = s
            .create_integration(&IntegrationInput {
                kind: Some(IntegrationKind::Portainer),
                base_url: "https://portainer.lan:9443".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(p.endpoint_id, 1, "defaults to the local environment");

        // The kind can't change on update.
        let u = s
            .update_integration(
                plex.id,
                &IntegrationInput {
                    kind: Some(IntegrationKind::Kuma),
                    name: "Media".into(),
                    base_url: "http://server01:32400".into(),
                    allow_untrusted: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!((u.kind, u.name.as_str(), u.allow_untrusted), (IntegrationKind::Plex, "Media", true));
        assert_eq!(s.list_integrations().unwrap().len(), 2);
        s.delete_integration(plex.id).unwrap();
        assert!(s.get_integration(plex.id).is_err());
        assert!(s.delete_integration(plex.id).is_err());
    }
}
