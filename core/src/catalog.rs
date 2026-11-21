//! Service catalog (1.1, D-170): what runs where, built on demand from what
//! the hosts and Nightgrid already know. Per container: image and version,
//! state, published ports, the URLs that reach it (service checks and
//! reverse-proxy routes), its data folders and whether a backup job's
//! command line names them. Nothing is stored: the catalog is read when
//! asked for and exported as Markdown.

use serde::{Deserialize, Serialize};

use crate::docker::proxy::ProxyRoute;
use crate::docker::{ContainerView, MountView};
use crate::hosts::HostId;
use crate::services::Service;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Coverage {
    /// Every data mount is named by a backup job.
    Covered,
    /// Some are.
    Partial,
    /// Data mounts, none named by a backup job.
    None,
    /// No data mounts (stateless) or nothing to judge.
    Stateless,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CatalogMount {
    /// Host path (binds) or the volume's folder on the host.
    pub source: String,
    pub destination: String,
    /// `bind` or `volume`.
    pub kind: String,
    pub name: Option<String>,
    /// The backup job whose command line names this folder or a parent.
    pub backed_up_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub name: String,
    pub image: String,
    /// The image's version label, else its tag (none for `latest`).
    pub version: Option<String>,
    pub state: String,
    pub health: Option<String>,
    pub project: Option<String>,
    /// `8080→80/tcp`.
    pub ports: Vec<String>,
    pub urls: Vec<String>,
    pub mounts: Vec<CatalogMount>,
    pub coverage: Coverage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CatalogHost {
    pub host_id: HostId,
    pub name: String,
    pub entries: Vec<CatalogEntry>,
    /// Docker unreadable, not installed, host unreachable.
    pub error: Option<String>,
    /// Backup jobs on this host whose command line could be read.
    pub jobs_read: u32,
}

/// A backup job's name and the absolute paths in its command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobPaths {
    pub name: String,
    pub paths: Vec<String>,
}

/// Mounts that hold data worth backing up: binds and volumes, not sockets,
/// devices or system files.
pub fn is_data(m: &MountView) -> bool {
    if !matches!(m.kind.as_str(), "bind" | "volume") {
        return false;
    }
    let s = m.source.as_str();
    let system = [
        "/var/run",
        "/run",
        "/dev",
        "/proc",
        "/sys",
        "/etc/localtime",
        "/etc/timezone",
        "/etc/hosts",
        "/etc/resolv.conf",
        "/tmp",
    ];
    !(s.ends_with(".sock") || system.iter().any(|p| s == *p || s.starts_with(&format!("{p}/"))))
}

/// The version an image declares: `org.opencontainers.image.version`, else
/// its tag unless that is `latest` or a digest.
pub fn version_of(image: &str, labels: &[(String, String)]) -> Option<String> {
    if let Some((_, v)) = labels.iter().find(|(k, v)| k == "org.opencontainers.image.version" && !v.trim().is_empty()) {
        return Some(v.trim().to_owned());
    }
    let image = image.split('@').next().unwrap_or(image);
    let last = image.rsplit('/').next().unwrap_or(image);
    let tag = last.split_once(':').map(|(_, t)| t)?;
    (tag != "latest" && !tag.is_empty()).then(|| tag.to_owned())
}

/// Absolute paths in a unit's `ExecStart=` lines and `Environment=` values
/// (restic and borg are usually given folders there). Quotes and `=`
/// prefixes (`--files-from=/x`) are stripped.
pub fn paths_in_unit(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines().map(str::trim) {
        if !(line.starts_with("ExecStart") || line.starts_with("ExecStartPre") || line.starts_with("Environment")) {
            continue;
        }
        let Some((_, rest)) = line.split_once('=') else { continue };
        for word in rest.split(|c: char| c.is_whitespace() || c == ':' || c == ',') {
            let w = word.trim_matches(|c| c == '"' || c == '\'');
            let w = w.rsplit_once('=').map_or(w, |(_, v)| v);
            if w.starts_with('/')
                && w.len() > 1
                && !w.starts_with("/usr/")
                && !w.starts_with("/bin/")
                && !w.starts_with("/sbin/")
            {
                let p = w.trim_end_matches('/').to_owned();
                if !p.is_empty() && !out.contains(&p) {
                    out.push(p);
                }
            }
        }
    }
    out
}

fn under(path: &str, root: &str) -> bool {
    root != "/" && (path == root || path.starts_with(&format!("{root}/")))
}

fn port_words(c: &ContainerView) -> Vec<String> {
    let mut v: Vec<String> =
        c.ports.iter().filter_map(|p| p.public.map(|pub_| format!("{pub_}→{}/{}", p.private, p.proto))).collect();
    v.sort();
    v.dedup();
    v
}

/// The host part of a URL, lowercased, without port or brackets.
pub fn url_host(url: &str) -> Option<(String, Option<u16>)> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let auth = rest.split(['/', '?', '#']).next()?;
    let auth = auth.rsplit_once('@').map_or(auth, |(_, a)| a);
    let (host, port) = if let Some(stripped) = auth.strip_prefix('[') {
        let (h, after) = stripped.split_once(']')?;
        (h, after.strip_prefix(':').and_then(|p| p.parse().ok()))
    } else {
        match auth.rsplit_once(':') {
            Some((h, p)) => (h, p.parse().ok()),
            None => (auth, None),
        }
    };
    (!host.is_empty()).then(|| (host.to_ascii_lowercase(), port))
}

/// A container's id, its mounts and its labels, from inspect.
pub type Inspected = (String, Vec<MountView>, Vec<(String, String)>);

/// Builds one host's entries.
pub fn build(
    containers: &[ContainerView],
    mounts: &[Inspected],
    host_names: &[String],
    services: &[Service],
    routes: &[ProxyRoute],
    jobs: &[JobPaths],
) -> Vec<CatalogEntry> {
    let names: Vec<String> = host_names.iter().map(|h| h.to_ascii_lowercase()).collect();
    let mut out: Vec<CatalogEntry> = containers
        .iter()
        .map(|c| {
            let (ms, labels) = mounts
                .iter()
                .find(|(id, _, _)| *id == c.id)
                .map(|(_, m, l)| (m.as_slice(), l.as_slice()))
                .unwrap_or((&[], &[]));
            let mounts: Vec<CatalogMount> = ms
                .iter()
                .filter(|m| is_data(m))
                .map(|m| CatalogMount {
                    source: m.source.clone(),
                    destination: m.destination.clone(),
                    kind: m.kind.clone(),
                    name: m.name.clone(),
                    backed_up_by: jobs
                        .iter()
                        .find(|j| j.paths.iter().any(|p| under(&m.source, p)))
                        .map(|j| j.name.clone()),
                })
                .collect();
            let covered = mounts.iter().filter(|m| m.backed_up_by.is_some()).count();
            let coverage = match (mounts.len(), covered) {
                (0, _) => Coverage::Stateless,
                (n, k) if k == n => Coverage::Covered,
                (_, 0) => Coverage::None,
                _ => Coverage::Partial,
            };
            let public: Vec<u16> = c.ports.iter().filter_map(|p| p.public).collect();
            let mut urls: Vec<String> = services
                .iter()
                .filter(|s| {
                    url_host(&s.url).is_some_and(|(h, port)| {
                        let port = port.unwrap_or(if s.url.starts_with("https") { 443 } else { 80 });
                        names.contains(&h) && public.contains(&port)
                    })
                })
                .map(|s| s.url.clone())
                .collect();
            for r in routes.iter().filter(|r| r.container.as_deref() == Some(c.name.as_str())) {
                let scheme = if r.tls { "https" } else { "http" };
                for d in &r.domains {
                    urls.push(format!("{scheme}://{d}"));
                }
            }
            urls.sort();
            urls.dedup();
            CatalogEntry {
                name: c.name.clone(),
                image: c.image.clone(),
                version: version_of(&c.image, labels),
                state: c.state.clone(),
                health: c.health.clone(),
                project: c.project.clone(),
                ports: port_words(c),
                urls,
                mounts,
                coverage,
            }
        })
        .collect();
    out.sort_by(|a, b| a.project.cmp(&b.project).then(a.name.cmp(&b.name)));
    out
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

/// The whole catalog as Markdown: one table per host, then data folders
/// without a backup.
pub fn to_markdown(hosts: &[CatalogHost], date: &str) -> String {
    let mut md = format!("# Service catalog\n\nBuilt by Nightgrid on {date}.\n");
    for h in hosts {
        md.push_str(&format!("\n## {}\n\n", h.name));
        if let Some(e) = &h.error {
            md.push_str(&format!("Not read: {}\n", cell(e)));
            continue;
        }
        if h.entries.is_empty() {
            md.push_str("No containers.\n");
            continue;
        }
        md.push_str("| Container | Image | Version | State | Ports | URLs | Backup |\n|---|---|---|---|---|---|---|\n");
        for e in &h.entries {
            let backup = match e.coverage {
                Coverage::Covered => "yes",
                Coverage::Partial => "partly",
                Coverage::None => "**no**",
                Coverage::Stateless => "no data",
            };
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                cell(&e.name),
                cell(&e.image),
                cell(e.version.as_deref().unwrap_or("")),
                cell(&e.state),
                e.ports.join(", "),
                e.urls.join("<br>"),
                backup
            ));
        }
        let loose: Vec<String> = h
            .entries
            .iter()
            .flat_map(|e| {
                e.mounts
                    .iter()
                    .filter(|m| m.backed_up_by.is_none())
                    .map(move |m| format!("- `{}` ({} → `{}`)", m.source, e.name, m.destination))
            })
            .collect();
        if !loose.is_empty() {
            md.push_str("\nData folders no backup job names:\n\n");
            md.push_str(&loose.join("\n"));
            md.push('\n');
        }
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docker::PortView;

    fn ctr(id: &str, name: &str, image: &str, ports: &[(u16, u16)]) -> ContainerView {
        ContainerView {
            id: id.into(),
            name: name.into(),
            image: image.into(),
            state: "running".into(),
            status: "Up".into(),
            health: None,
            health_ignored: false,
            created: 0,
            ports: ports
                .iter()
                .map(|(p, i)| PortView { ip: None, private: *i, public: Some(*p), proto: "tcp".into() })
                .collect(),
            project: Some("media".into()),
            service: None,
            compose_dir: None,
            compose_files: vec![],
            web: vec![],
        }
    }
    fn mount(kind: &str, src: &str, dst: &str) -> MountView {
        MountView { kind: kind.into(), source: src.into(), destination: dst.into(), rw: true, name: None }
    }

    #[test]
    fn versions_and_paths() {
        assert_eq!(version_of("ghcr.io/x/app:1.2.3", &[]), Some("1.2.3".into()));
        assert_eq!(version_of("postgres:latest", &[]), None);
        assert_eq!(version_of("localhost:5000/app", &[]), None);
        let l = vec![("org.opencontainers.image.version".to_owned(), "v2.0".to_owned())];
        assert_eq!(version_of("app:latest", &l), Some("v2.0".into()));
        let unit = "[Service]\nEnvironment=\"RESTIC_REPOSITORY=/mnt/backup/repo\"\nExecStart=/usr/bin/restic backup /srv/data '/opt/stacks/' --exclude-file=/etc/restic/excl\n";
        assert_eq!(paths_in_unit(unit), vec!["/mnt/backup/repo", "/srv/data", "/opt/stacks", "/etc/restic/excl"]);
        assert_eq!(url_host("https://Plex.lan:32400/web"), Some(("plex.lan".into(), Some(32400))));
        assert_eq!(url_host("http://[fd00::1]:8080/"), Some(("fd00::1".into(), Some(8080))));
    }

    #[test]
    fn joins_urls_and_backups() {
        let cs =
            vec![ctr("a", "jellyfin", "jellyfin/jellyfin:10.9", &[(8096, 8096)]), ctr("b", "redis", "redis:7", &[])];
        let ms = vec![
            (
                "a".to_owned(),
                vec![
                    mount("bind", "/srv/media/config", "/config"),
                    mount("bind", "/mnt/cache", "/cache"),
                    mount("bind", "/var/run/docker.sock", "/var/run/docker.sock"),
                ],
                vec![],
            ),
            ("b".to_owned(), vec![], vec![]),
        ];
        let svc = Service {
            id: 1,
            name: "Jellyfin".into(),
            url: "http://server01:8096".into(),
            host_id: None,
            group: String::new(),
            description: String::new(),
            icon: None,
            check_enabled: true,
            expect_status: String::new(),
            allow_untrusted: false,
            sort_order: 0,
            source: "manual".into(),
        };
        let jobs = vec![JobPaths { name: "restic-daily".into(), paths: vec!["/srv".into()] }];
        let e = build(&cs, &ms, &["server01".into()], &[svc], &[], &jobs);
        let j = e.iter().find(|x| x.name == "jellyfin").unwrap();
        assert_eq!(j.urls, vec!["http://server01:8096"]);
        assert_eq!(j.mounts.len(), 2);
        assert_eq!(j.coverage, Coverage::Partial);
        assert_eq!(j.mounts[0].backed_up_by.as_deref(), Some("restic-daily"));
        assert_eq!(e.iter().find(|x| x.name == "redis").unwrap().coverage, Coverage::Stateless);
        let host = CatalogHost { host_id: 1, name: "server01".into(), entries: e, error: None, jobs_read: 1 };
        let md = to_markdown(&[host], "2026-11-18");
        assert!(md.contains("| jellyfin | jellyfin/jellyfin:10.9 | 10.9 | running | 8096→8096/tcp |"));
        assert!(md.contains("`/mnt/cache` (jellyfin → `/cache`)"));
    }
}
