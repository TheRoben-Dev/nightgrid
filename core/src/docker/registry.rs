//! Image update check with changelog hints (M10f). For each image a stack
//! uses, asks its registry (anonymously) for the tag's current digest and
//! compares it with the digest the host pulled. When they differ it reads the
//! new image's labels (OCI version, source, revision) so the UI can say
//! "1.4.2 → 1.5.0" and link the release notes before anything is pulled.
//!
//! Network: only the image's registry, only when the user asks (rule 7).
//! Private registries that need a login are reported, never prompted for.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::http::{self, Request};

const ACCEPT: &str = "application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json";

/// A parsed image reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRef {
    /// Registry host for the API (`registry-1.docker.io` for Docker Hub).
    pub registry: String,
    /// Repository path (`library/nginx`).
    pub repo: String,
    pub tag: String,
    /// Pinned by digest (`@sha256:…`): nothing to check.
    pub digest: Option<String>,
}

impl ImageRef {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() || s.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return None;
        }
        let (name, digest) = match s.split_once('@') {
            Some((n, d)) => (n, Some(d.to_owned())),
            None => (s, None),
        };
        let last_slash = name.rfind('/').map_or(0, |i| i + 1);
        let (name, tag) = match name[last_slash..].rfind(':') {
            Some(i) => (&name[..last_slash + i], name[last_slash + i + 1..].to_owned()),
            None => (name, "latest".to_owned()),
        };
        let (registry, repo) = match name.split_once('/') {
            Some((first, rest)) if first.contains('.') || first.contains(':') || first == "localhost" => {
                (first.to_owned(), rest.to_owned())
            }
            _ => ("docker.io".to_owned(), name.to_owned()),
        };
        let (registry, repo) = if registry == "docker.io" || registry == "index.docker.io" {
            let repo = if repo.contains('/') { repo } else { format!("library/{repo}") };
            ("registry-1.docker.io".to_owned(), repo)
        } else {
            (registry, repo)
        };
        if repo.is_empty() || tag.is_empty() {
            return None;
        }
        Some(Self { registry, repo: repo.to_lowercase(), tag, digest })
    }
}

/// What is known about one image version.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub version: Option<String>,
    pub revision: Option<String>,
    /// RFC 3339 build time.
    pub created: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ImageStatus {
    /// The registry's tag points at the image the host has.
    Current,
    /// The tag moved: pulling would change the image.
    Available,
    /// Pinned by digest.
    Pinned,
    /// Built locally or never pulled from a registry.
    Local,
    /// The registry wants a login (not attempted).
    NeedsLogin,
    /// The check failed (see `error`).
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ImageUpdate {
    pub image: String,
    /// Services of the stack that use it.
    pub services: Vec<String>,
    pub status: ImageStatus,
    pub error: Option<String>,
    pub current: VersionInfo,
    pub latest: Option<VersionInfo>,
    /// Project page (https), from the image's labels.
    pub source: Option<String>,
    /// Release notes or a commit comparison (https), when it can be worked out.
    pub notes: Option<String>,
    /// Held in Nightgrid (D-195): still checked, but not counted and not
    /// pulled by Pull / Update.
    pub held: bool,
}

/// The local side of one image, read from `docker image inspect`.
#[derive(Debug, Clone, Default)]
pub struct LocalImage {
    pub image: String,
    pub services: Vec<String>,
    pub repo_digests: Vec<String>,
    pub labels: HashMap<String, String>,
    pub os: String,
    pub arch: String,
    pub variant: Option<String>,
    pub created: Option<String>,
}

fn label<'a>(labels: &'a HashMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| labels.get(*k)).map(|v| v.trim()).filter(|v| !v.is_empty())
}

/// Version facts from image labels (OCI first, then label-schema and
/// linuxserver's `build_version`).
pub fn version_info(labels: &HashMap<String, String>, created: Option<&str>) -> VersionInfo {
    let version = label(labels, &["org.opencontainers.image.version", "org.label-schema.version"])
        .map(str::to_owned)
        .or_else(|| {
            // linuxserver.io: "Linuxserver.io version:- 1.2.3-ls45 Build-date:- …"
            let b = label(labels, &["build_version"])?;
            let v = b.split("version:-").nth(1)?.split_whitespace().next()?;
            Some(v.to_owned())
        });
    VersionInfo {
        version,
        revision: label(labels, &["org.opencontainers.image.revision", "org.label-schema.vcs-ref"]).map(str::to_owned),
        created: label(labels, &["org.opencontainers.image.created", "org.label-schema.build-date"])
            .or(created)
            .map(str::to_owned),
    }
}

/// An https project URL from the labels, normalised (no `.git`, no trailing `/`).
pub fn source_url(labels: &HashMap<String, String>) -> Option<String> {
    let raw = label(
        labels,
        &[
            "org.opencontainers.image.source",
            "org.label-schema.vcs-url",
            "org.opencontainers.image.url",
            "org.label-schema.url",
        ],
    )?;
    let raw = raw.strip_prefix("git+").unwrap_or(raw);
    let s = raw.trim_end_matches('/').trim_end_matches(".git");
    if !s.starts_with("https://") || s.chars().any(|c| c.is_whitespace() || c == '"' || c == '<' || c == '>') {
        return None;
    }
    Some(s.to_owned())
}

/// Release notes link: a GitHub compare when both revisions are known,
/// GitHub releases otherwise, else the project page.
pub fn notes_url(source: Option<&str>, from: Option<&str>, to: Option<&str>) -> Option<String> {
    let src = source?;
    let gh = src.strip_prefix("https://github.com/").map(|rest| rest.split('/').take(2).collect::<Vec<_>>().join("/"));
    match gh {
        Some(repo) if repo.contains('/') => {
            let hex = |r: &str| r.len() >= 7 && r.chars().all(|c| c.is_ascii_hexdigit());
            match (from, to) {
                (Some(a), Some(b)) if a != b && hex(a) && hex(b) => {
                    Some(format!("https://github.com/{repo}/compare/{a}...{b}"))
                }
                _ => Some(format!("https://github.com/{repo}/releases")),
            }
        }
        _ => Some(src.to_owned()),
    }
}

fn digest_of(res: &http::Response) -> String {
    res.header("docker-content-digest").map(str::to_owned).unwrap_or_else(|| {
        let hex: String = Sha256::digest(&res.body).iter().map(|b| format!("{b:02x}")).collect();
        format!("sha256:{hex}")
    })
}

/// Parses `Bearer realm="…",service="…",scope="…"`.
pub fn parse_challenge(h: &str) -> Option<HashMap<String, String>> {
    let rest = h.trim().strip_prefix("Bearer ").or_else(|| h.trim().strip_prefix("bearer "))?;
    let mut out = HashMap::new();
    let mut s = rest;
    while !s.is_empty() {
        let (k, after) = s.split_once('=')?;
        let after = after.trim_start();
        let (v, next) = if let Some(q) = after.strip_prefix('"') {
            let end = q.find('"')?;
            (&q[..end], &q[end + 1..])
        } else {
            let end = after.find(',').unwrap_or(after.len());
            (&after[..end], &after[end..])
        };
        out.insert(k.trim().to_ascii_lowercase(), v.to_owned());
        s = next.trim_start_matches([',', ' ']);
    }
    Some(out)
}

#[derive(Deserialize)]
struct TokenRes {
    token: Option<String>,
    access_token: Option<String>,
}

enum Fetch {
    Ok(http::Response),
    NeedsLogin,
}

/// One registry, with the anonymous pull token once obtained.
struct Client<'a> {
    r: &'a ImageRef,
    token: Option<String>,
}

impl Client<'_> {
    fn req(&self, path: &str, accept: &str) -> Request {
        let mut q =
            Request::get(format!("https://{}/v2/{}/{path}", self.r.registry, self.r.repo)).header("accept", accept);
        if let Some(t) = &self.token {
            q = q.header("authorization", format!("Bearer {t}"));
        }
        q
    }

    async fn get(&mut self, path: &str, accept: &str, follow: bool) -> Result<Fetch, String> {
        for attempt in 0..2 {
            let req = self.req(path, accept);
            let res = if follow { http::send_following(req, 3).await? } else { http::send(req).await? };
            if res.status == 401 && attempt == 0 && self.token.is_none() {
                let Some(ch) = res.header("www-authenticate").and_then(parse_challenge) else {
                    return Ok(Fetch::NeedsLogin);
                };
                let Some(realm) = ch.get("realm").filter(|r| r.starts_with("https://")) else {
                    return Ok(Fetch::NeedsLogin);
                };
                let scope = format!("repository:{}:pull", self.r.repo);
                let mut url = format!("{realm}?scope={}", enc(&scope));
                if let Some(svc) = ch.get("service") {
                    url.push_str(&format!("&service={}", enc(svc)));
                }
                let t = http::send(Request::get(url)).await?;
                if !t.ok() {
                    return Ok(Fetch::NeedsLogin);
                }
                let tr: TokenRes = t.json()?;
                match tr.token.or(tr.access_token) {
                    Some(tok) if !tok.is_empty() => self.token = Some(tok),
                    _ => return Ok(Fetch::NeedsLogin),
                }
                continue;
            }
            if res.status == 401 || res.status == 403 {
                return Ok(Fetch::NeedsLogin);
            }
            if !res.ok() {
                return Err(format!("{} answered {}", self.r.registry, res.status));
            }
            return Ok(Fetch::Ok(res));
        }
        Ok(Fetch::NeedsLogin)
    }
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[derive(Deserialize)]
struct Platform {
    os: Option<String>,
    architecture: Option<String>,
    variant: Option<String>,
}

#[derive(Deserialize)]
struct Descriptor {
    digest: String,
    platform: Option<Platform>,
}

#[derive(Deserialize)]
struct Manifest {
    #[serde(default)]
    manifests: Vec<Descriptor>,
    config: Option<Descriptor>,
}

#[derive(Deserialize)]
struct ConfigBlob {
    created: Option<String>,
    config: Option<ConfigInner>,
}

#[derive(Deserialize)]
struct ConfigInner {
    #[serde(rename = "Labels", default)]
    labels: Option<HashMap<String, String>>,
    #[serde(rename = "ExposedPorts", default)]
    exposed_ports: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "Volumes", default)]
    volumes: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "Env", default)]
    env: Option<Vec<String>>,
}

/// What an image declares, for the deploy wizard's suggestions.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RemoteImage {
    pub image: String,
    pub version: VersionInfo,
    /// `8080/tcp`, …
    pub ports: Vec<String>,
    /// Container paths declared as volumes.
    pub volumes: Vec<String>,
    /// Env names the image sets (values left out: defaults like PATH are noise,
    /// and a value could be sensitive).
    pub env: Vec<String>,
    pub source: Option<String>,
    pub description: Option<String>,
}

/// Reads an image's config from its registry (no pull).
pub async fn inspect(image: &str, os: &str, arch: &str) -> Result<RemoteImage, String> {
    let r = ImageRef::parse(image).ok_or("not a valid image reference")?;
    let mut c = Client { r: &r, token: None };
    let reference = r.digest.clone().unwrap_or_else(|| r.tag.clone());
    let Fetch::Ok(top) = c.get(&format!("manifests/{reference}"), ACCEPT, false).await? else {
        return Err(format!("{} wants a login for {}", r.registry, r.repo));
    };
    let m: Manifest = top.json()?;
    let manifest = if m.manifests.is_empty() {
        m
    } else {
        let d = pick(&m, os, arch, None).ok_or(format!("no {os}/{arch} image for this tag"))?;
        match c.get(&format!("manifests/{}", d.digest), ACCEPT, false).await? {
            Fetch::Ok(res) => res.json::<Manifest>()?,
            Fetch::NeedsLogin => return Err("login needed".into()),
        }
    };
    let cfg = manifest.config.ok_or("manifest has no config")?;
    let Fetch::Ok(res) = c.get(&format!("blobs/{}", cfg.digest), "*/*", true).await? else {
        return Err("login needed".into());
    };
    let b: ConfigBlob = res.json()?;
    let inner = b.config;
    let labels = inner.as_ref().and_then(|c| c.labels.clone()).unwrap_or_default();
    let mut ports: Vec<String> =
        inner.as_ref().and_then(|c| c.exposed_ports.as_ref()).map(|p| p.keys().cloned().collect()).unwrap_or_default();
    ports.sort();
    let mut volumes: Vec<String> =
        inner.as_ref().and_then(|c| c.volumes.as_ref()).map(|v| v.keys().cloned().collect()).unwrap_or_default();
    volumes.sort();
    const NOISE: &[&str] = &["PATH", "HOME", "LANG", "LC_ALL", "TERM", "HOSTNAME", "LANGUAGE", "SHLVL"];
    let env = inner
        .as_ref()
        .and_then(|c| c.env.as_ref())
        .map(|e| {
            e.iter()
                .filter_map(|kv| kv.split('=').next())
                .filter(|k| !k.is_empty() && !NOISE.contains(k))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Ok(RemoteImage {
        image: image.to_owned(),
        version: version_info(&labels, b.created.as_deref()),
        ports,
        volumes,
        env,
        source: source_url(&labels),
        description: label(&labels, &["org.opencontainers.image.description", "org.label-schema.description"])
            .map(|d| d.chars().take(300).collect()),
    })
}

/// Picks the manifest for the local platform from an index.
fn pick<'a>(m: &'a Manifest, os: &str, arch: &str, variant: Option<&str>) -> Option<&'a Descriptor> {
    let fits = |d: &&Descriptor| {
        d.platform.as_ref().is_some_and(|p| p.os.as_deref() == Some(os) && p.architecture.as_deref() == Some(arch))
    };
    m.manifests
        .iter()
        .filter(fits)
        .find(|d| variant.is_none() || d.platform.as_ref().and_then(|p| p.variant.as_deref()) == variant)
        .or_else(|| m.manifests.iter().find(fits))
}

/// Checks one image against its registry.
pub async fn check(local: &LocalImage) -> ImageUpdate {
    let current = version_info(&local.labels, local.created.as_deref());
    let source = source_url(&local.labels);
    let mut out = ImageUpdate {
        image: local.image.clone(),
        services: local.services.clone(),
        status: ImageStatus::Unknown,
        error: None,
        current: current.clone(),
        latest: None,
        source: source.clone(),
        notes: notes_url(source.as_deref(), None, None),
        held: false,
    };
    let Some(r) = ImageRef::parse(&local.image) else {
        out.error = Some("not a registry image reference".into());
        return out;
    };
    if r.digest.is_some() {
        out.status = ImageStatus::Pinned;
        return out;
    }
    let local_digests: Vec<&str> =
        local.repo_digests.iter().filter_map(|d| d.split_once('@').map(|(_, x)| x)).collect();
    if local_digests.is_empty() {
        out.status = ImageStatus::Local;
        return out;
    }
    match remote(&r, local).await {
        Ok(None) => out.status = ImageStatus::NeedsLogin,
        Ok(Some((digest, latest))) => {
            if local_digests.contains(&digest.as_str()) {
                out.status = ImageStatus::Current;
            } else {
                out.status = ImageStatus::Available;
                let latest_src = latest.as_ref().and_then(|(_, labels)| source_url(labels)).or(source.clone());
                let to_rev = latest.as_ref().and_then(|(v, _)| v.revision.clone());
                out.notes = notes_url(latest_src.as_deref(), current.revision.as_deref(), to_rev.as_deref());
                out.source = latest_src;
                out.latest = latest.map(|(v, _)| v);
            }
        }
        Err(e) => out.error = Some(e),
    }
    out
}

/// (index digest, latest version facts and labels), or `None` when a login is needed.
async fn remote(
    r: &ImageRef,
    local: &LocalImage,
) -> Result<Option<(String, Option<(VersionInfo, HashMap<String, String>)>)>, String> {
    let mut c = Client { r, token: None };
    let Fetch::Ok(top) = c.get(&format!("manifests/{}", r.tag), ACCEPT, false).await? else {
        return Ok(None);
    };
    let digest = digest_of(&top);
    let m: Manifest = top.json()?;
    // Labels are best-effort: a failure here still reports the digest.
    let labels = async {
        let manifest = if m.manifests.is_empty() {
            m
        } else {
            let d = pick(&m, &local.os, &local.arch, local.variant.as_deref()).ok_or("no image for this platform")?;
            match c.get(&format!("manifests/{}", d.digest), ACCEPT, false).await? {
                Fetch::Ok(res) => res.json::<Manifest>()?,
                Fetch::NeedsLogin => return Err("login needed".to_owned()),
            }
        };
        let cfg = manifest.config.ok_or("manifest has no config")?;
        match c.get(&format!("blobs/{}", cfg.digest), "*/*", true).await? {
            Fetch::Ok(res) => {
                let b: ConfigBlob = res.json()?;
                let labels = b.config.and_then(|c| c.labels).unwrap_or_default();
                Ok((version_info(&labels, b.created.as_deref()), labels))
            }
            Fetch::NeedsLogin => Err("login needed".to_owned()),
        }
    }
    .await
    .ok();
    Ok(Some((digest, labels)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(s: &str) -> (String, String, String, bool) {
        let x = ImageRef::parse(s).unwrap();
        (x.registry, x.repo, x.tag, x.digest.is_some())
    }

    #[test]
    fn parses_references() {
        assert_eq!(r("nginx"), ("registry-1.docker.io".into(), "library/nginx".into(), "latest".into(), false));
        assert_eq!(
            r("linuxserver/plex:1.40"),
            ("registry-1.docker.io".into(), "linuxserver/plex".into(), "1.40".into(), false)
        );
        assert_eq!(r("ghcr.io/home-assistant/home-assistant:stable").1, "home-assistant/home-assistant");
        assert_eq!(r("lscr.io/linuxserver/sonarr").0, "lscr.io");
        assert_eq!(r("localhost:5000/app:dev"), ("localhost:5000".into(), "app".into(), "dev".into(), false));
        assert!(r("redis@sha256:abc").3);
        assert!(ImageRef::parse("").is_none());
        assert!(ImageRef::parse("a b").is_none());
    }

    #[test]
    fn versions_and_links() {
        let mut l = HashMap::new();
        l.insert("org.opencontainers.image.version".into(), "1.4.2".into());
        l.insert("org.opencontainers.image.source".into(), "https://github.com/acme/app.git".into());
        l.insert("org.opencontainers.image.revision".into(), "abc1234def".into());
        let v = version_info(&l, Some("2026-01-01T00:00:00Z"));
        assert_eq!(v.version.as_deref(), Some("1.4.2"));
        assert_eq!(v.created.as_deref(), Some("2026-01-01T00:00:00Z"));
        let src = source_url(&l);
        assert_eq!(src.as_deref(), Some("https://github.com/acme/app"));
        assert_eq!(
            notes_url(src.as_deref(), Some("abc1234def"), Some("0123456789")).unwrap(),
            "https://github.com/acme/app/compare/abc1234def...0123456789"
        );
        assert_eq!(notes_url(src.as_deref(), None, None).unwrap(), "https://github.com/acme/app/releases");
        assert_eq!(notes_url(Some("https://gitlab.com/x/y"), None, None).unwrap(), "https://gitlab.com/x/y");

        let mut ls = HashMap::new();
        ls.insert("build_version".into(), "Linuxserver.io version:- 1.40.0.7998-ls230 Build-date:- 2026-02-01".into());
        assert_eq!(version_info(&ls, None).version.as_deref(), Some("1.40.0.7998-ls230"));
        let mut bad = HashMap::new();
        bad.insert("org.opencontainers.image.source".into(), "http://insecure".into());
        assert_eq!(source_url(&bad), None);
    }

    #[test]
    fn config_fields() {
        let b: ConfigBlob = serde_json::from_str(
            r#"{"created":"2026-01-01T00:00:00Z","config":{"ExposedPorts":{"80/tcp":{}},"Volumes":{"/data":{}},"Env":["PATH=/usr/bin","TZ=UTC"],"Labels":null}}"#,
        )
        .unwrap();
        let c = b.config.unwrap();
        assert!(c.exposed_ports.unwrap().contains_key("80/tcp"));
        assert_eq!(c.env.unwrap().len(), 2);
        assert!(c.labels.is_none());
    }

    #[test]
    fn challenge_and_platform() {
        let c = parse_challenge(r#"Bearer realm="https://auth.docker.io/token",service="registry.docker.io",scope="repository:library/nginx:pull""#).unwrap();
        assert_eq!(c["realm"], "https://auth.docker.io/token");
        assert_eq!(c["service"], "registry.docker.io");
        let m: Manifest = serde_json::from_str(
            r#"{"manifests":[
              {"digest":"sha256:a","platform":{"os":"linux","architecture":"amd64"}},
              {"digest":"sha256:b","platform":{"os":"linux","architecture":"arm","variant":"v7"}},
              {"digest":"sha256:c","platform":{"os":"linux","architecture":"arm64","variant":"v8"}}]}"#,
        )
        .unwrap();
        assert_eq!(pick(&m, "linux", "arm64", None).unwrap().digest, "sha256:c");
        assert_eq!(pick(&m, "linux", "amd64", None).unwrap().digest, "sha256:a");
        assert!(pick(&m, "windows", "amd64", None).is_none());
        assert_eq!(enc("repository:a/b:pull"), "repository%3Aa%2Fb%3Apull");
    }
}
