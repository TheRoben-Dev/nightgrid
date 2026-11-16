//! The helper's model: one pinned file, its download and its on-disk state.
//!
//! The model is too big for the installers, so it is downloaded once, only
//! when the user clicks Download (the spec's single exception to "no network
//! calls"), from a URL pinned to an exact repository commit and checked
//! against a pinned SHA-256. A finished, verified file gets a `.sha256`
//! marker next to it so later starts don't re-hash gigabytes.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use http_body_util::{BodyExt, Empty};
use hyper::body::Bytes;
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

use crate::services::check::{connect_error, parse_target, tls_connector, Captured, Target};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelSpec {
    pub id: &'static str,
    /// Shown in the UI.
    pub name: &'static str,
    /// `light`, `balanced` or `best`.
    pub tier: &'static str,
    /// Parameter count as people say it ("8B parameters", "30B parameters, 3B active").
    pub params: &'static str,
    /// What it's like to use, one line.
    pub blurb: &'static str,
    pub license: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub bytes: u64,
    /// Memory on top of the file while answering (8k context cache and
    /// buffers), from the model's layer and head counts.
    pub overhead: u64,
    /// A hybrid model that thinks unless told not to: DRIFT asks it not to
    /// and drops any `<think>` block.
    pub no_think: bool,
}

/// The models DRIFT offers, small to big. URLs are pinned to a repository
/// commit and every file is checked against its SHA-256 (D-063, D-139).
pub const MODELS: &[ModelSpec] = &[
    // Qwen3-4B-Instruct-2507, Q4_K_M: fits 8 GB Apple Silicon and 6–8 GB GPUs.
    ModelSpec {
        id: "qwen3-4b-instruct-2507-q4km",
        name: "Qwen3 4B Instruct (2507), Q4_K_M",
        tier: "light",
        params: "4B parameters",
        blurb: "Quick and small. Good for questions about the app and short log explanations.",
        license: "Apache-2.0",
        file: "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
        url: "https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF/resolve/a06e946bb6b655725eafa393f4a9745d460374c9/Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
        sha256: "3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597",
        bytes: 2_497_281_120,
        overhead: 1_000_000_000,
        no_think: false,
    },
    // Qwen3-8B (hybrid), Q4_K_M: 16 GB machines.
    ModelSpec {
        id: "qwen3-8b-q4km",
        name: "Qwen3 8B, Q4_K_M",
        tier: "balanced",
        params: "8B parameters",
        blurb: "Twice the size: follows the facts it's given more closely and explains errors better. A little slower.",
        license: "Apache-2.0",
        file: "Qwen3-8B-Q4_K_M.gguf",
        url: "https://huggingface.co/unsloth/Qwen3-8B-GGUF/resolve/a6adef130ffb23ddaf1a62fec9dced968c9bc482/Qwen3-8B-Q4_K_M.gguf",
        sha256: "120307ba529eb2439d6c430d94104dabd578497bc7bfe7e322b5d9933b449bd4",
        bytes: 5_027_784_512,
        overhead: 1_500_000_000,
        no_think: true,
    },
    // Qwen3-30B-A3B-Instruct-2507 (mixture of experts, 3B active per token),
    // Q4_K_M: 32 GB+ machines; about as fast as a 4B model once loaded.
    ModelSpec {
        id: "qwen3-30b-a3b-instruct-2507-q4km",
        name: "Qwen3 30B-A3B Instruct (2507), Q4_K_M",
        tier: "best",
        params: "30B parameters, 3B active",
        blurb: "The most knowledgeable, and quick for its size because only part of it works on each word. Needs a lot of memory.",
        license: "Apache-2.0",
        file: "Qwen3-30B-A3B-Instruct-2507-Q4_K_M.gguf",
        url: "https://huggingface.co/unsloth/Qwen3-30B-A3B-Instruct-2507-GGUF/resolve/eea7b2be5805a5f151f8847ede8e5f9a9284bf77/Qwen3-30B-A3B-Instruct-2507-Q4_K_M.gguf",
        sha256: "6c997b8af17debdfb01d890214400ccbab00db6acc0ba8da5de1cc906c4774d0",
        bytes: 18_556_686_752,
        overhead: 1_300_000_000,
        no_think: false,
    },
];

/// The model used until the user picks another.
pub const DEFAULT_MODEL: ModelSpec = MODELS[0];

pub fn model_by_id(id: &str) -> Option<&'static ModelSpec> {
    MODELS.iter().find(|m| m.id == id)
}

/// Hosts a model download may touch, including redirects (suffix match).
pub const DOWNLOAD_HOSTS: &[&str] = &["huggingface.co", "hf.co"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelState {
    Missing,
    /// A download was interrupted; this many bytes are kept for resuming.
    Partial(u64),
    Ready,
}

pub fn model_path(dir: &Path, spec: &ModelSpec) -> PathBuf {
    dir.join(spec.file)
}

fn part_path(dir: &Path, spec: &ModelSpec) -> PathBuf {
    dir.join(format!("{}.part", spec.file))
}

fn marker_path(dir: &Path, spec: &ModelSpec) -> PathBuf {
    dir.join(format!("{}.sha256", spec.file))
}

pub fn state(dir: &Path, spec: &ModelSpec) -> ModelState {
    let file = model_path(dir, spec);
    let ok = std::fs::metadata(&file).is_ok_and(|m| m.len() == spec.bytes)
        && std::fs::read_to_string(marker_path(dir, spec)).is_ok_and(|s| s.trim() == spec.sha256);
    if ok {
        return ModelState::Ready;
    }
    match std::fs::metadata(part_path(dir, spec)) {
        Ok(m) if m.len() > 0 => ModelState::Partial(m.len()),
        _ => ModelState::Missing,
    }
}

/// Removes the model and any partial download.
pub fn delete(dir: &Path, spec: &ModelSpec) -> std::io::Result<()> {
    for p in [model_path(dir, spec), part_path(dir, spec), marker_path(dir, spec)] {
        match std::fs::remove_file(&p) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

/// Where a download may go: hosts (suffix match) and whether plain http is
/// allowed (tests only).
#[derive(Debug, Clone)]
pub struct Policy {
    pub hosts: Vec<String>,
    pub allow_http: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self { hosts: DOWNLOAD_HOSTS.iter().map(|s| s.to_string()).collect(), allow_http: false }
    }
}

impl Policy {
    fn check(&self, t: &Target) -> Result<(), String> {
        if !t.https && !self.allow_http {
            return Err(format!("refusing a plain-http download from {}", t.host));
        }
        let h = t.host.to_ascii_lowercase();
        if self.hosts.iter().any(|a| h == *a || h.ends_with(&format!(".{a}"))) {
            Ok(())
        } else {
            Err(format!("refusing to download from {h}"))
        }
    }
}

const MAX_REDIRECTS: usize = 6;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// No bytes for this long ends the download (it can be resumed).
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

type Body = hyper::body::Incoming;

async fn open<S>(io: S, t: &Target, range_from: u64) -> Result<hyper::Response<Body>, String>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut sender, conn) = hyper::client::conn::http1::handshake::<_, Empty<Bytes>>(TokioIo::new(io))
        .await
        .map_err(|e| format!("HTTP error: {e}"))?;
    tokio::spawn(async move {
        let _ = conn.await;
    });
    let mut b = hyper::Request::builder()
        .uri(&t.path)
        .header(hyper::header::HOST, &t.authority)
        .header(hyper::header::USER_AGENT, concat!("Nightgrid/", env!("CARGO_PKG_VERSION")))
        .header(hyper::header::ACCEPT, "*/*");
    if range_from > 0 {
        b = b.header(hyper::header::RANGE, format!("bytes={range_from}-"));
    }
    let req = b.body(Empty::new()).map_err(|e| format!("bad request: {e}"))?;
    sender.send_request(req).await.map_err(|e| format!("HTTP error: {e}"))
}

async fn request(url: &str, range_from: u64) -> Result<hyper::Response<Body>, String> {
    let t = parse_target(url)?;
    let tcp = match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect((t.host.as_str(), t.port))).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return Err(connect_error(&e)),
        Err(_) => return Err("timed out connecting".into()),
    };
    if !t.https {
        return open(tcp, &t, range_from).await;
    }
    let captured = Arc::new(Mutex::new(Captured::default()));
    let connector = tls_connector(captured.clone()).map_err(|e| format!("TLS setup failed: {e}"))?;
    let name = ServerName::try_from(t.host.clone()).map_err(|_| "host name isn't valid for TLS".to_owned())?;
    let stream = tokio::time::timeout(CONNECT_TIMEOUT, connector.connect(name, tcp))
        .await
        .map_err(|_| "timed out during TLS handshake".to_owned())?
        .map_err(|e| format!("TLS handshake failed: {e}"))?;
    let verified = captured.lock().map(|c| c.verified).unwrap_or(false);
    if !verified {
        return Err("the download server's certificate isn't trusted".into());
    }
    open(stream, &t, range_from).await
}

fn resolve(base: &str, location: &str) -> String {
    if location.contains("://") {
        return location.to_owned();
    }
    let (scheme, rest) = base.split_once("://").unwrap_or(("https", base));
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if location.starts_with('/') {
        format!("{scheme}://{authority}{location}")
    } else {
        let dir = rest.rsplit_once('/').map_or(rest, |(d, _)| d);
        format!("{scheme}://{dir}/{location}")
    }
}

fn hash_prefix(path: &Path, len: u64) -> std::io::Result<Sha256> {
    let mut h = Sha256::new();
    let mut f = std::fs::File::open(path)?;
    let mut buf = vec![0u8; 1 << 20];
    let mut left = len;
    while left > 0 {
        let want = left.min(buf.len() as u64) as usize;
        let n = f.read(&mut buf[..want])?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        left -= n as u64;
    }
    Ok(h)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadEnd {
    Done,
    Cancelled,
}

/// Downloads `spec` into `dir`, resuming a partial file, then checks the
/// SHA-256 and moves it into place. `progress(done, total)` is called as
/// bytes arrive. A failed checksum deletes the partial file.
pub async fn download(
    dir: &Path,
    spec: &ModelSpec,
    policy: &Policy,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<DownloadEnd, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("couldn't create {}: {e}", dir.display()))?;
    let part = part_path(dir, spec);
    let mut have = std::fs::metadata(&part).map_or(0, |m| m.len());
    if have > spec.bytes {
        have = 0;
    }
    let mut hasher = if have > 0 {
        let p = part.clone();
        tokio::task::spawn_blocking(move || hash_prefix(&p, have))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("couldn't read the partial download: {e}"))?
    } else {
        Sha256::new()
    };

    let mut url = spec.url.to_owned();
    let mut res = None;
    for _ in 0..=MAX_REDIRECTS {
        policy.check(&parse_target(&url)?)?;
        let r = request(&url, if have < spec.bytes { have } else { 0 }).await?;
        if r.status().is_redirection() {
            let loc = r
                .headers()
                .get(hyper::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or("redirect without a location")?;
            url = resolve(&url, loc);
            continue;
        }
        res = Some(r);
        break;
    }
    let res = res.ok_or("too many redirects")?;
    let status = res.status().as_u16();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&part)
        .map_err(|e| format!("couldn't write {}: {e}", part.display()))?;
    match status {
        206 if have > 0 => {
            file.seek(SeekFrom::Start(have)).map_err(|e| e.to_string())?;
        }
        200 | 206 => {
            // Fresh start (or the server ignored the range).
            have = 0;
            hasher = Sha256::new();
            file.set_len(0).map_err(|e| e.to_string())?;
        }
        416 if have == spec.bytes => {}
        s => return Err(format!("the download server answered {s}")),
    }
    progress(have, spec.bytes);

    let mut body = res.into_body();
    let mut ended = DownloadEnd::Done;
    while have < spec.bytes {
        if cancel.load(Ordering::Relaxed) {
            ended = DownloadEnd::Cancelled;
            break;
        }
        let frame = match tokio::time::timeout(IDLE_TIMEOUT, body.frame()).await {
            Err(_) => return Err("the download stalled; try again to resume".into()),
            Ok(None) => break,
            Ok(Some(Err(e))) => return Err(format!("the download was interrupted ({e}); try again to resume")),
            Ok(Some(Ok(f))) => f,
        };
        let Ok(data) = frame.into_data() else { continue };
        if have + data.len() as u64 > spec.bytes {
            drop(file);
            let _ = std::fs::remove_file(&part);
            return Err("the server sent more data than expected".into());
        }
        file.write_all(&data).map_err(|e| format!("couldn't write the model: {e}"))?;
        hasher.update(&data);
        have += data.len() as u64;
        progress(have, spec.bytes);
    }
    file.flush().map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    if ended == DownloadEnd::Cancelled {
        return Ok(ended);
    }
    if have < spec.bytes {
        return Err("the download ended early; try again to resume".into());
    }
    let got = hex(&hasher.finalize());
    if got != spec.sha256 {
        let _ = std::fs::remove_file(&part);
        return Err("the downloaded file doesn't match its checksum and was deleted".into());
    }
    let dest = model_path(dir, spec);
    std::fs::rename(&part, &dest).map_err(|e| format!("couldn't move the model into place: {e}"))?;
    std::fs::write(marker_path(dir, spec), spec.sha256).map_err(|e| e.to_string())?;
    Ok(DownloadEnd::Done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn spec_for(data: &[u8], url: &'static str) -> ModelSpec {
        let sha: &'static str = Box::leak(hex(&Sha256::digest(data)).into_boxed_str());
        ModelSpec { url, sha256: sha, bytes: data.len() as u64, file: "m.gguf", ..DEFAULT_MODEL }
    }

    /// Serves `data` (honouring Range) on /m, redirects /r to /m, and cuts
    /// the first full response off after `cut` bytes.
    async fn server(data: Vec<u8>, cut: Option<usize>) -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            let mut first = true;
            loop {
                let (mut s, _) = l.accept().await.unwrap();
                let mut buf = vec![0u8; 4096];
                let n = s.read(&mut buf).await.unwrap();
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                if req.starts_with("GET /r ") {
                    let _ = s.write_all(b"HTTP/1.1 302 Found\r\nlocation: /m\r\ncontent-length: 0\r\n\r\n").await;
                    continue;
                }
                let from = req
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("range: bytes=")
                            .map(|v| v.trim_end_matches('-').parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                let body = &data[from..];
                let head = if from > 0 {
                    format!("HTTP/1.1 206 Partial Content\r\ncontent-length: {}\r\n\r\n", body.len())
                } else {
                    format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n", body.len())
                };
                let _ = s.write_all(head.as_bytes()).await;
                match cut {
                    Some(c) if first && from == 0 => {
                        let _ = s.write_all(&body[..c]).await;
                        first = false;
                    }
                    _ => {
                        let _ = s.write_all(body).await;
                    }
                }
                let _ = s.shutdown().await;
            }
        });
        port
    }

    fn local() -> Policy {
        Policy { hosts: vec!["127.0.0.1".into()], allow_http: true }
    }

    #[tokio::test]
    async fn downloads_resumes_and_verifies() {
        let data: Vec<u8> = (0..300_000u32).map(|i| (i * 7 % 251) as u8).collect();
        let port = server(data.clone(), Some(100_000)).await;
        let url: &'static str = Box::leak(format!("http://127.0.0.1:{port}/r").into_boxed_str());
        let spec = spec_for(&data, url);
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(state(dir.path(), &spec), ModelState::Missing);

        // First attempt is cut off: the partial file stays for resuming.
        let e = download(dir.path(), &spec, &local(), &AtomicBool::new(false), |_, _| {}).await.unwrap_err();
        assert!(e.contains("resume"), "{e}");
        assert_eq!(state(dir.path(), &spec), ModelState::Partial(100_000));

        // Second attempt resumes with a Range request and finishes.
        let mut last = (0, 0);
        let end = download(dir.path(), &spec, &local(), &AtomicBool::new(false), |d, t| last = (d, t)).await.unwrap();
        assert_eq!(end, DownloadEnd::Done);
        assert_eq!(last, (300_000, 300_000));
        assert_eq!(state(dir.path(), &spec), ModelState::Ready);
        assert_eq!(std::fs::read(model_path(dir.path(), &spec)).unwrap(), data);

        delete(dir.path(), &spec).unwrap();
        assert_eq!(state(dir.path(), &spec), ModelState::Missing);
    }

    #[tokio::test]
    async fn bad_checksum_deletes_and_policy_blocks() {
        let data = vec![1u8; 5000];
        let port = server(data.clone(), None).await;
        let url: &'static str = Box::leak(format!("http://127.0.0.1:{port}/m").into_boxed_str());
        let mut spec = spec_for(&data, url);
        spec.sha256 = "00";
        let dir = tempfile::tempdir().unwrap();
        let e = download(dir.path(), &spec, &local(), &AtomicBool::new(false), |_, _| {}).await.unwrap_err();
        assert!(e.contains("checksum"), "{e}");
        assert_eq!(state(dir.path(), &spec), ModelState::Missing);

        // Default policy: https and Hugging Face hosts only.
        let e = download(dir.path(), &spec, &Policy::default(), &AtomicBool::new(false), |_, _| {}).await.unwrap_err();
        assert!(e.contains("plain-http"), "{e}");
        let p = Policy::default();
        assert!(p.check(&parse_target("https://us.aws.cdn.hf.co/x").unwrap()).is_ok());
        assert!(p.check(&parse_target("https://huggingface.co/x").unwrap()).is_ok());
        assert!(p.check(&parse_target("https://evilhf.co/x").unwrap()).is_err());
        assert!(p.check(&parse_target("https://example.com/x").unwrap()).is_err());
    }

    #[tokio::test]
    async fn cancel_keeps_partial() {
        let data = vec![9u8; 50_000];
        let port = server(data.clone(), None).await;
        let url: &'static str = Box::leak(format!("http://127.0.0.1:{port}/m").into_boxed_str());
        let spec = spec_for(&data, url);
        let dir = tempfile::tempdir().unwrap();
        let end = download(dir.path(), &spec, &local(), &AtomicBool::new(true), |_, _| {}).await.unwrap();
        assert_eq!(end, DownloadEnd::Cancelled);
        assert_ne!(state(dir.path(), &spec), ModelState::Ready);
    }

    #[test]
    fn resolves_redirects() {
        assert_eq!(resolve("https://a.co/x/y?q", "https://b.co/z"), "https://b.co/z");
        assert_eq!(resolve("https://a.co/x/y?q", "/z?s=1"), "https://a.co/z?s=1");
        assert_eq!(resolve("https://a.co/x/y", "z"), "https://a.co/x/z");
    }
}
