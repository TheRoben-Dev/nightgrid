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
//! - Kinds that report the shared [`app::AppStatus`] (summary, tiles, rows,
//!   actions): **Jellyfin**, **Emby**, **Tautulli**, **Sonarr**, **Radarr**,
//!   **Lidarr**, **Prowlarr**, **Bazarr**, **Overseerr**, **Jellyseerr**
//!   (1.8.0); **qBittorrent**, **Transmission**, **SABnzbd**, **NZBGet**,
//!   **Immich**, **Nextcloud**, **Syncthing**, **Gotify**, **Frigate** (1.11.0);
//!   **TrueNAS**, **Synology DSM**, **Unraid**, **Proxmox Backup Server**,
//!   **OPNsense**, **UniFi Network**, **Nginx Proxy Manager**, **Grafana** and
//!   **Prometheus** (1.13).
//!
//! The database stores the kind as text with no CHECK on it (0050): a kind is
//! valid when [`IntegrationKind::parse`] knows it.
//!
//! Every client goes through [`crate::http`], so certificates are verified
//! unless the integration allows untrusted ones, and nothing is sent anywhere
//! but the configured URL.

pub mod amp;
pub mod app;
pub mod arr;
pub mod dns;
pub mod frigate;
pub mod gotify;
pub mod immich;
pub mod jellyfin;
pub mod kuma;
pub mod monitoring;
pub mod netapps;
pub mod nextcloud;
pub mod plex;
pub mod portainer;
pub mod qbittorrent;
pub mod seerr;
pub mod storage;
pub mod syncthing;
pub mod tautulli;
pub mod transmission;
pub mod unraid;
pub mod usenet;

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
    Jellyfin,
    Emby,
    Tautulli,
    Sonarr,
    Radarr,
    Lidarr,
    Prowlarr,
    Bazarr,
    Overseerr,
    Jellyseerr,
    Qbittorrent,
    Transmission,
    Sabnzbd,
    Nzbget,
    Immich,
    Nextcloud,
    Syncthing,
    Gotify,
    Frigate,
    Truenas,
    Synology,
    Unraid,
    Pbs,
    Opnsense,
    Unifi,
    Npm,
    Grafana,
    Prometheus,
}

/// Whether a kind uses a token, password or API key from the keychain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretUse {
    None,
    Optional,
    Required,
}

impl IntegrationKind {
    /// Every kind (tests save each one).
    pub const ALL: [Self; 34] = [
        Self::Plex,
        Self::Amp,
        Self::Kuma,
        Self::Portainer,
        Self::Pihole,
        Self::Adguard,
        Self::Jellyfin,
        Self::Emby,
        Self::Tautulli,
        Self::Sonarr,
        Self::Radarr,
        Self::Lidarr,
        Self::Prowlarr,
        Self::Bazarr,
        Self::Overseerr,
        Self::Jellyseerr,
        Self::Qbittorrent,
        Self::Transmission,
        Self::Sabnzbd,
        Self::Nzbget,
        Self::Immich,
        Self::Nextcloud,
        Self::Syncthing,
        Self::Gotify,
        Self::Frigate,
        Self::Truenas,
        Self::Synology,
        Self::Unraid,
        Self::Pbs,
        Self::Opnsense,
        Self::Unifi,
        Self::Npm,
        Self::Grafana,
        Self::Prometheus,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plex => "plex",
            Self::Amp => "amp",
            Self::Kuma => "kuma",
            Self::Portainer => "portainer",
            Self::Pihole => "pihole",
            Self::Adguard => "adguard",
            Self::Jellyfin => "jellyfin",
            Self::Emby => "emby",
            Self::Tautulli => "tautulli",
            Self::Sonarr => "sonarr",
            Self::Radarr => "radarr",
            Self::Lidarr => "lidarr",
            Self::Prowlarr => "prowlarr",
            Self::Bazarr => "bazarr",
            Self::Overseerr => "overseerr",
            Self::Jellyseerr => "jellyseerr",
            Self::Qbittorrent => "qbittorrent",
            Self::Transmission => "transmission",
            Self::Sabnzbd => "sabnzbd",
            Self::Nzbget => "nzbget",
            Self::Immich => "immich",
            Self::Nextcloud => "nextcloud",
            Self::Syncthing => "syncthing",
            Self::Gotify => "gotify",
            Self::Frigate => "frigate",
            Self::Truenas => "truenas",
            Self::Synology => "synology",
            Self::Unraid => "unraid",
            Self::Pbs => "pbs",
            Self::Opnsense => "opnsense",
            Self::Unifi => "unifi",
            Self::Npm => "npm",
            Self::Grafana => "grafana",
            Self::Prometheus => "prometheus",
        }
    }

    /// The name a new integration of this kind gets.
    pub fn label(self) -> &'static str {
        match self {
            Self::Plex => "Plex",
            Self::Amp => "AMP",
            Self::Kuma => "Uptime Kuma",
            Self::Portainer => "Portainer",
            Self::Pihole => "Pi-hole",
            Self::Adguard => "AdGuard Home",
            Self::Jellyfin => "Jellyfin",
            Self::Emby => "Emby",
            Self::Tautulli => "Tautulli",
            Self::Sonarr => "Sonarr",
            Self::Radarr => "Radarr",
            Self::Lidarr => "Lidarr",
            Self::Prowlarr => "Prowlarr",
            Self::Bazarr => "Bazarr",
            Self::Overseerr => "Overseerr",
            Self::Jellyseerr => "Jellyseerr",
            Self::Qbittorrent => "qBittorrent",
            Self::Transmission => "Transmission",
            Self::Sabnzbd => "SABnzbd",
            Self::Nzbget => "NZBGet",
            Self::Immich => "Immich",
            Self::Nextcloud => "Nextcloud",
            Self::Syncthing => "Syncthing",
            Self::Gotify => "Gotify",
            Self::Frigate => "Frigate",
            Self::Truenas => "TrueNAS",
            Self::Synology => "Synology DSM",
            Self::Unraid => "Unraid",
            Self::Pbs => "Proxmox Backup Server",
            Self::Opnsense => "OPNsense",
            Self::Unifi => "UniFi Network",
            Self::Npm => "Nginx Proxy Manager",
            Self::Grafana => "Grafana",
            Self::Prometheus => "Prometheus",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "plex" => Self::Plex,
            "amp" => Self::Amp,
            "kuma" => Self::Kuma,
            "portainer" => Self::Portainer,
            "pihole" => Self::Pihole,
            "adguard" => Self::Adguard,
            "jellyfin" => Self::Jellyfin,
            "emby" => Self::Emby,
            "tautulli" => Self::Tautulli,
            "sonarr" => Self::Sonarr,
            "radarr" => Self::Radarr,
            "lidarr" => Self::Lidarr,
            "prowlarr" => Self::Prowlarr,
            "bazarr" => Self::Bazarr,
            "overseerr" => Self::Overseerr,
            "jellyseerr" => Self::Jellyseerr,
            "qbittorrent" => Self::Qbittorrent,
            "transmission" => Self::Transmission,
            "sabnzbd" => Self::Sabnzbd,
            "nzbget" => Self::Nzbget,
            "immich" => Self::Immich,
            "nextcloud" => Self::Nextcloud,
            "syncthing" => Self::Syncthing,
            "gotify" => Self::Gotify,
            "frigate" => Self::Frigate,
            "truenas" => Self::Truenas,
            "synology" => Self::Synology,
            "unraid" => Self::Unraid,
            "pbs" => Self::Pbs,
            "opnsense" => Self::Opnsense,
            "unifi" => Self::Unifi,
            "npm" => Self::Npm,
            "grafana" => Self::Grafana,
            "prometheus" => Self::Prometheus,
            _ => return None,
        })
    }

    pub fn secret_use(self) -> SecretUse {
        match self {
            Self::Kuma | Self::Portainer => SecretUse::None,
            Self::Qbittorrent | Self::Transmission | Self::Frigate | Self::Prometheus => SecretUse::Optional,
            _ => SecretUse::Required,
        }
    }

    /// Whether this kind uses a secret (token, password or API key).
    pub fn needs_secret(self) -> bool {
        self.secret_use() != SecretUse::None
    }

    /// The login field's meaning when the kind uses one, and whether it's required.
    pub fn username_use(self) -> Option<(&'static str, bool)> {
        match self {
            Self::Amp => Some(("AMP needs a login name", true)),
            Self::Adguard => Some(("AdGuard Home needs its login name", true)),
            Self::Nzbget => Some(("NZBGet needs its control login name", true)),
            Self::Nextcloud => Some(("Nextcloud needs an admin login name", true)),
            Self::Synology => Some(("DSM needs a login name", true)),
            Self::Pbs => Some(("the API token id, like root@pam!nightgrid", true)),
            Self::Opnsense => Some(("OPNsense needs the API key (the secret goes in the password field)", true)),
            Self::Npm => Some(("Nginx Proxy Manager needs the login email", true)),
            Self::Qbittorrent | Self::Transmission | Self::Frigate | Self::Prometheus => Some(("", false)),
            _ => None,
        }
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
        n.name = kind.label().into();
    }
    match kind.username_use() {
        Some((why, true)) if n.username.is_empty() => return Err(inv("username", why)),
        None => n.username.clear(),
        _ => {}
    }
    if kind == IntegrationKind::Kuma {
        if n.slug.is_empty() {
            return Err(inv("slug", "the status page slug, e.g. \"home\" from /status/home"));
        }
        if !n.slug.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')) {
            return Err(inv("slug", "letters, digits, - and _ only"));
        }
    }
    if !matches!(kind, IntegrationKind::Kuma | IntegrationKind::Unifi) {
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
    /// Every kind that reports the shared shape.
    App(app::AppStatus),
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
        kind => {
            let key = secret.filter(|s| !s.is_empty());
            if kind.secret_use() == SecretUse::Required && key.is_none() {
                return Err(format!("no {} API key saved", kind.label()));
            }
            app_status(i, key.unwrap_or("")).await.map(IntegrationStatus::App)
        }
    }
}

async fn app_status(i: &Integration, key: &str) -> std::result::Result<app::AppStatus, String> {
    use IntegrationKind as K;
    match i.kind {
        K::Jellyfin | K::Emby => jellyfin::status(i, key).await,
        K::Tautulli => tautulli::status(i, key).await,
        K::Sonarr | K::Radarr | K::Lidarr | K::Prowlarr | K::Bazarr => arr::status(i, key).await,
        K::Overseerr | K::Jellyseerr => seerr::status(i, key).await,
        K::Qbittorrent => qbittorrent::status(i, key).await,
        K::Transmission => transmission::status(i, key).await,
        K::Sabnzbd | K::Nzbget => usenet::status(i, key).await,
        K::Immich => immich::status(i, key).await,
        K::Nextcloud => nextcloud::status(i, key).await,
        K::Syncthing => syncthing::status(i, key).await,
        K::Gotify => gotify::status(i, key).await,
        K::Frigate => frigate::status(i, key).await,
        K::Truenas | K::Synology | K::Pbs => storage::status(i, key).await,
        K::Unraid => unraid::status(i, key).await,
        K::Opnsense | K::Unifi | K::Npm => netapps::status(i, key).await,
        K::Grafana | K::Prometheus => monitoring::status(i, key).await,
        K::Plex | K::Amp | K::Kuma | K::Portainer | K::Pihole | K::Adguard => {
            Err("this integration has its own status".into())
        }
    }
}

/// Runs one of the actions an [`app::AppStatus`] offered. `target` is the
/// row's id, empty for card-level actions.
pub async fn act(i: &Integration, secret: Option<&str>, target: &str, action: &str) -> std::result::Result<(), String> {
    use IntegrationKind as K;
    let key = secret.unwrap_or("");
    if i.kind.secret_use() == SecretUse::Required && key.is_empty() {
        return Err(format!("no {} API key saved", i.kind.label()));
    }
    if !target.is_empty() && !app::valid_target(target) {
        return Err("that item id isn't valid".into());
    }
    match i.kind {
        K::Jellyfin | K::Emby => jellyfin::act(i, key, action).await,
        K::Sonarr | K::Radarr | K::Lidarr | K::Prowlarr => arr::act(i, key, target, action).await,
        K::Overseerr | K::Jellyseerr => seerr::act(i, key, target, action).await,
        K::Qbittorrent => qbittorrent::act(i, key, target, action).await,
        K::Transmission => transmission::act(i, key, target, action).await,
        K::Sabnzbd | K::Nzbget => usenet::act(i, key, target, action).await,
        K::Syncthing => syncthing::act(i, key, target, action).await,
        K::Gotify => gotify::act(i, key, target, action).await,
        K::Opnsense | K::Unifi | K::Npm => netapps::act(i, key, target, action).await,
        _ => Err(format!("{} has no action {action}", i.kind.label())),
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

    fn input_for(kind: IntegrationKind) -> IntegrationInput {
        IntegrationInput {
            kind: Some(kind),
            base_url: "http://x.lan".into(),
            username: "admin".into(),
            slug: "home".into(),
            ..Default::default()
        }
    }

    #[test]
    fn every_kind_can_be_saved() {
        // A new variant must be added to ALL (this match won't compile otherwise
        // without touching it), and the table must accept it.
        for k in IntegrationKind::ALL {
            match k {
                IntegrationKind::Plex
                | IntegrationKind::Amp
                | IntegrationKind::Kuma
                | IntegrationKind::Portainer
                | IntegrationKind::Pihole
                | IntegrationKind::Adguard
                | IntegrationKind::Jellyfin
                | IntegrationKind::Emby
                | IntegrationKind::Tautulli
                | IntegrationKind::Sonarr
                | IntegrationKind::Radarr
                | IntegrationKind::Lidarr
                | IntegrationKind::Prowlarr
                | IntegrationKind::Bazarr
                | IntegrationKind::Overseerr
                | IntegrationKind::Jellyseerr
                | IntegrationKind::Qbittorrent
                | IntegrationKind::Transmission
                | IntegrationKind::Sabnzbd
                | IntegrationKind::Nzbget
                | IntegrationKind::Immich
                | IntegrationKind::Nextcloud
                | IntegrationKind::Syncthing
                | IntegrationKind::Gotify
                | IntegrationKind::Frigate
                | IntegrationKind::Truenas
                | IntegrationKind::Synology
                | IntegrationKind::Unraid
                | IntegrationKind::Pbs
                | IntegrationKind::Opnsense
                | IntegrationKind::Unifi
                | IntegrationKind::Npm
                | IntegrationKind::Grafana
                | IntegrationKind::Prometheus => {}
            }
        }
        let mut s = Store::open_in_memory().unwrap();
        for k in IntegrationKind::ALL {
            let i = s.create_integration(&input_for(k)).unwrap_or_else(|e| panic!("{k:?}: {e}"));
            assert_eq!(i.kind, k);
            assert_eq!(IntegrationKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(s.list_integrations().unwrap().len(), IntegrationKind::ALL.len());
    }

    #[test]
    fn migration_keeps_existing_integrations() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        let m = crate::store::migrations();
        m.to_version(&mut conn, 48).unwrap();
        conn.execute_batch(
            "INSERT INTO integrations (id, kind, name, base_url, username, endpoint_id, allow_untrusted)
                 VALUES (7, 'amp', 'Games', 'http://amp.lan', 'admin', 0, 1),
                        (9, 'portainer', 'Docker', 'https://p.lan:9443', '', 2, 0);",
        )
        .unwrap();
        assert!(
            conn.execute("INSERT INTO integrations (kind, base_url) VALUES ('pihole', 'http://pi.lan')", []).is_err(),
            "the old constraint rejected pihole"
        );
        m.to_latest(&mut conn).unwrap();
        let rows: Vec<(i64, String, String, i64, i64)> = conn
            .prepare("SELECT id, kind, username, endpoint_id, allow_untrusted FROM integrations ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(rows, vec![(7, "amp".into(), "admin".into(), 0, 1), (9, "portainer".into(), "".into(), 2, 0)]);
        for k in IntegrationKind::ALL {
            conn.execute("INSERT INTO integrations (kind, base_url) VALUES (?1, 'http://x')", [k.as_str()]).unwrap();
        }
        // 0050 dropped the CHECK on kind: Rust validates it instead.
        conn.execute("INSERT INTO integrations (kind, base_url) VALUES ('newkind', 'http://x')", []).unwrap();
        assert!(conn.execute("INSERT INTO integrations (kind, base_url) VALUES ('', 'http://x')", []).is_err());
        assert_eq!(IntegrationKind::parse("newkind"), None);
    }
}
