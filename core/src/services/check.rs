//! One HTTP(S) health check: status code, time to response headers and, for
//! https, the leaf certificate's trust and expiry.
//!
//! Redirects are not followed (a 3xx inside the expected range counts as up).
//! The certificate is always captured, even when it isn't trusted, so the board
//! can show "self-signed, 40 days left" instead of just "down".

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use http_body_util::{BodyExt, Empty};
use hyper::body::Bytes;
use hyper_util::rt::TokioIo;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use super::status_matches;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TlsInfo {
    /// Chains to a root the OS trusts and matches the host name.
    pub trusted: bool,
    /// Why it isn't trusted (expired, unknown issuer, wrong name…).
    pub problem: Option<String>,
    /// Certificate `notAfter`, unix seconds.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub expires: Option<i64>,
    pub subject: Option<String>,
    pub issuer: Option<String>,
    /// SHA-256 of the leaf certificate (lowercase hex), so a certificate the
    /// user chose to trust can be told apart from a replacement.
    #[serde(default)]
    pub fingerprint: Option<String>,
}

impl TlsInfo {
    pub fn days_left(&self, now: i64) -> Option<i64> {
        self.expires.map(|e| (e - now).div_euclid(86_400))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub up: bool,
    pub status: Option<u16>,
    pub latency_ms: Option<u32>,
    /// Connection/TLS/protocol error, or why the status didn't count as up.
    pub error: Option<String>,
    pub tls: Option<TlsInfo>,
}

impl CheckResult {
    fn failed(error: impl Into<String>, tls: Option<TlsInfo>) -> Self {
        Self { up: false, status: None, latency_ms: None, error: Some(error.into()), tls }
    }
}

/// Scheme, host (without IPv6 brackets), port, and request target.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Target {
    pub https: bool,
    pub host: String,
    pub port: u16,
    /// `host[:port]` exactly as written, for the Host header.
    pub authority: String,
    pub path: String,
}

pub(crate) fn parse_target(url: &str) -> Result<Target, String> {
    let (scheme, rest) = url.split_once("://").ok_or("not an http(s) URL")?;
    let https = match scheme.to_ascii_lowercase().as_str() {
        "https" => true,
        "http" => false,
        _ => return Err("not an http(s) URL".into()),
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    let mut path = rest[end..].split('#').next().unwrap_or("").to_owned();
    if path.is_empty() || path.starts_with('?') {
        path.insert(0, '/');
    }
    let default_port = if https { 443 } else { 80 };
    let (host, port) = if let Some(v6) = authority.strip_prefix('[') {
        let (h, after) = v6.split_once(']').ok_or("bad IPv6 address")?;
        let port = match after.strip_prefix(':') {
            Some(p) => p.parse().map_err(|_| "bad port")?,
            None if after.is_empty() => default_port,
            None => return Err("bad IPv6 address".into()),
        };
        (h.to_owned(), port)
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) => (h.to_owned(), p.parse().map_err(|_| "bad port")?),
            None => (authority.to_owned(), default_port),
        }
    };
    if host.is_empty() {
        return Err("missing host".into());
    }
    Ok(Target { https, host, port, authority: authority.to_owned(), path })
}

/// Runs one check. Never returns an error: failures are a down result.
pub async fn check(url: &str, expect_status: &str, allow_untrusted: bool, timeout: Duration) -> CheckResult {
    let target = match parse_target(url) {
        Ok(t) => t,
        Err(e) => return CheckResult::failed(e, None),
    };
    let started = Instant::now();
    let tcp = match tokio::time::timeout(
        timeout,
        crate::tailnet::dial(&target.host, target.port, timeout + Duration::from_millis(50)),
    )
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return CheckResult::failed(format!("{}{}", connect_error(&e), tried(&target).await), None),
        Err(_) => return CheckResult::failed(format!("timed out connecting{}", tried(&target).await), None),
    };
    let _ = tcp.set_nodelay(true);
    let via = dialed(&tcp, &target);
    let remaining = timeout.saturating_sub(started.elapsed());

    if !target.https {
        return finish(request(tcp, &target, remaining).await, started, expect_status, None, &via);
    }

    let captured = Arc::new(Mutex::new(Captured::default()));
    let connector = match tls_connector(captured.clone()) {
        Ok(c) => c,
        Err(e) => return CheckResult::failed(format!("TLS setup failed: {e}"), None),
    };
    let name = match ServerName::try_from(target.host.clone()) {
        Ok(n) => n,
        Err(_) => return CheckResult::failed("host name isn't valid for TLS", None),
    };
    let tls_stream = tokio::time::timeout(remaining, connector.connect(name, tcp)).await;
    let tls = captured.lock().ok().and_then(|c| c.info());
    let stream = match tls_stream {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return CheckResult::failed(format!("TLS handshake failed: {e}"), tls),
        Err(_) => return CheckResult::failed("timed out during TLS handshake", tls),
    };
    let remaining = timeout.saturating_sub(started.elapsed());
    let mut result = finish(request(stream, &target, remaining).await, started, expect_status, tls, &via);
    if let Some(t) = &result.tls {
        if !t.trusted && !allow_untrusted && result.up {
            result.up = false;
            result.error = Some(format!(
                "certificate not trusted ({}); tick \"allow untrusted\" for self-signed or internal CAs",
                t.problem.as_deref().unwrap_or("unknown reason")
            ));
        }
    }
    result
}

pub(crate) fn connect_error(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::ConnectionRefused => "connection refused".into(),
        _ => e.to_string(),
    }
}

/// Where the connection went, for failure messages: the address dialled, or
/// the built-in tailnet (its proxy is on loopback).
fn dialed(tcp: &tokio::net::TcpStream, target: &Target) -> String {
    let local_target =
        target.host == "localhost" || target.host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback());
    match tcp.peer_addr() {
        Ok(a) if a.ip().is_loopback() && !local_target => "through the built-in tailnet".into(),
        Ok(_) if target.host.parse::<std::net::IpAddr>().is_ok() => String::new(),
        Ok(a) => format!("at {}", a.ip()),
        Err(_) => String::new(),
    }
}

/// " (tried 2001:db8::1, 192.0.2.1)": the addresses a name resolves to, for
/// a connection that failed. Empty for an IP or a failed lookup.
async fn tried(target: &Target) -> String {
    if target.host.parse::<std::net::IpAddr>().is_ok() {
        return String::new();
    }
    let route = match crate::tailnet::route(&target.host) {
        crate::tailnet::Route::Direct => "",
        _ => " via the built-in tailnet",
    };
    let lookup =
        tokio::time::timeout(Duration::from_secs(2), tokio::net::lookup_host((target.host.as_str(), target.port)))
            .await;
    let mut ips: Vec<String> = match lookup {
        Ok(Ok(it)) => it.map(|a| a.ip().to_string()).collect(),
        _ => return route.to_owned(),
    };
    ips.dedup();
    if ips.is_empty() {
        return route.to_owned();
    }
    ips.truncate(4);
    format!(" to {}{route}", ips.join(", "))
}

/// A response that matters for the board.
pub(crate) struct Resp {
    pub status: u16,
    /// `Server` header (which proxy answered).
    pub server: Option<String>,
    /// The start of the body, as text, for error statuses.
    pub snippet: Option<String>,
}

/// Visible text of an error page: tags dropped, whitespace collapsed, cut short.
pub(crate) fn page_text(body: &[u8]) -> Option<String> {
    let raw = String::from_utf8_lossy(body);
    let lower = raw.to_ascii_lowercase();
    let mut out = String::new();
    let (mut in_tag, mut skip) = (false, false); // skip: inside <style> or <script>
    for (i, c) in raw.char_indices() {
        if c == '<' {
            let rest = &lower[i..];
            if rest.starts_with("<style") || rest.starts_with("<script") {
                skip = true;
            } else if rest.starts_with("</style") || rest.starts_with("</script") {
                skip = false;
            }
            in_tag = true;
        } else if c == '>' && in_tag {
            in_tag = false;
            out.push(' ');
        } else if !in_tag && !skip {
            out.push(c);
        }
    }
    let text = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return None;
    }
    Some(if text.chars().count() > 140 { format!("{}…", text.chars().take(140).collect::<String>()) } else { text })
}

fn finish(
    res: Result<Resp, String>,
    started: Instant,
    expect_status: &str,
    tls: Option<TlsInfo>,
    via: &str,
) -> CheckResult {
    match res {
        Ok(r) => {
            let status = r.status;
            let up = status_matches(expect_status, status);
            let error = (!up).then(|| {
                let mut m = format!("HTTP {status} isn't in {}", expect_label(expect_status));
                let from = match (&r.server, via.is_empty()) {
                    (Some(srv), false) => format!(" · from {srv} {via}"),
                    (Some(srv), true) => format!(" · from {srv}"),
                    (None, false) => format!(" · {via}"),
                    (None, true) => String::new(),
                };
                m.push_str(&from);
                if let Some(t) = &r.snippet {
                    m.push_str(&format!(" · \"{t}\""));
                }
                if status == 502 || status == 504 {
                    m.push_str(" · the proxy answered but couldn't reach the app behind it");
                }
                m
            });
            CheckResult {
                up,
                status: Some(status),
                latency_ms: Some(u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX)),
                error,
                tls,
            }
        }
        Err(e) => CheckResult::failed(e, tls),
    }
}

fn expect_label(spec: &str) -> &str {
    if spec.trim().is_empty() {
        "200-399"
    } else {
        spec.trim()
    }
}

async fn request<S>(io: S, target: &Target, timeout: Duration) -> Result<Resp, String>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let work = async {
        let (mut sender, conn) = hyper::client::conn::http1::handshake::<_, Empty<Bytes>>(TokioIo::new(io))
            .await
            .map_err(|e| format!("HTTP error: {e}"))?;
        let driver = tokio::spawn(async move {
            let _ = conn.await;
        });
        let req = hyper::Request::get(&target.path)
            .header(hyper::header::HOST, &target.authority)
            .header(hyper::header::USER_AGENT, concat!("Nightgrid/", env!("CARGO_PKG_VERSION"), " (health check)"))
            .header(hyper::header::ACCEPT, "*/*")
            .header(hyper::header::CONNECTION, "close")
            .body(Empty::new())
            .map_err(|e| format!("bad request: {e}"))?;
        let res = sender.send_request(req).await.map_err(|e| format!("HTTP error: {e}"));
        let res = match res {
            Ok(r) => r,
            Err(e) => {
                driver.abort();
                return Err(e);
            }
        };
        let status = res.status().as_u16();
        let server = res
            .headers()
            .get(hyper::header::SERVER)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.chars().take(40).collect::<String>())
            .filter(|v| !v.is_empty());
        // For an error status, read a little of the page: proxies say why.
        let mut snippet = None;
        if status >= 400 {
            let mut body = res.into_body();
            let mut buf = Vec::new();
            let read = async {
                while buf.len() < 4096 {
                    match body.frame().await {
                        Some(Ok(f)) => {
                            if let Some(d) = f.data_ref() {
                                buf.extend_from_slice(d);
                            }
                        }
                        _ => break,
                    }
                }
            };
            let _ = tokio::time::timeout(Duration::from_millis(1500), read).await;
            snippet = page_text(&buf);
        }
        driver.abort();
        Ok::<_, String>(Resp { status, server, snippet })
    };
    tokio::time::timeout(timeout, work).await.map_err(|_| "timed out waiting for a response".to_owned())?
}

// ---- TLS capture ----

#[derive(Default)]
pub(crate) struct Captured {
    leaf: Option<Vec<u8>>,
    pub(crate) problem: Option<String>,
    pub(crate) verified: bool,
}

impl Captured {
    pub(crate) fn info(&self) -> Option<TlsInfo> {
        let leaf = self.leaf.as_ref()?;
        let (expires, subject, issuer) = match x509_parser::parse_x509_certificate(leaf) {
            Ok((_, cert)) => (
                Some(cert.validity().not_after.timestamp()),
                Some(cert.subject().to_string()),
                Some(cert.issuer().to_string()),
            ),
            Err(_) => (None, None, None),
        };
        Some(TlsInfo {
            trusted: self.verified,
            problem: self.problem.clone(),
            expires,
            subject,
            issuer,
            fingerprint: Some(fingerprint(leaf)),
        })
    }
}

/// SHA-256 of a DER certificate, lowercase hex.
pub fn fingerprint(der: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(der).iter().map(|b| format!("{b:02x}")).collect()
}

/// Wraps the OS verifier: records the leaf and the verdict, then lets the
/// handshake continue so the HTTP status can still be read. The caller decides
/// what an untrusted certificate means. Handshake signatures are still checked.
#[derive(Debug)]
struct CapturingVerifier {
    inner: Arc<dyn ServerCertVerifier>,
    captured: Arc<Mutex<Captured>>,
}

impl std::fmt::Debug for Captured {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Captured").field("verified", &self.verified).finish()
    }
}

impl ServerCertVerifier for CapturingVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let verdict = self.inner.verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now);
        if let Ok(mut c) = self.captured.lock() {
            c.leaf = Some(end_entity.to_vec());
            c.verified = verdict.is_ok();
            c.problem = verdict.err().map(|e| describe_tls_error(&e));
        }
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

fn describe_tls_error(e: &rustls::Error) -> String {
    use rustls::CertificateError as C;
    match e {
        rustls::Error::InvalidCertificate(c) => match c {
            C::Expired | C::ExpiredContext { .. } => "expired".into(),
            C::NotValidYet | C::NotValidYetContext { .. } => "not valid yet".into(),
            C::UnknownIssuer => "unknown issuer or self-signed".into(),
            C::NotValidForName | C::NotValidForNameContext { .. } => "doesn't match the host name".into(),
            C::Revoked => "revoked".into(),
            other => format!("{other:?}"),
        },
        other => other.to_string(),
    }
}

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

pub(crate) fn tls_connector(captured: Arc<Mutex<Captured>>) -> Result<tokio_rustls::TlsConnector, rustls::Error> {
    let provider = provider();
    let inner: Arc<dyn ServerCertVerifier> = Arc::new(rustls_platform_verifier::Verifier::new(provider.clone())?);
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(CapturingVerifier { inner, captured }))
        .with_no_client_auth();
    Ok(tokio_rustls::TlsConnector::from(Arc::new(config)))
}

pub fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn targets() {
        let t = parse_target("https://Example.lan:8443/app?x=1#frag").unwrap();
        assert_eq!(
            t,
            Target {
                https: true,
                host: "Example.lan".into(),
                port: 8443,
                authority: "Example.lan:8443".into(),
                path: "/app?x=1".into()
            }
        );
        let t = parse_target("http://server01").unwrap();
        assert_eq!((t.port, t.path.as_str()), (80, "/"));
        let t = parse_target("http://server01?q").unwrap();
        assert_eq!(t.path, "/?q");
        let t = parse_target("https://[fd00::1]:9000/").unwrap();
        assert_eq!((t.host.as_str(), t.port), ("fd00::1", 9000));
        assert_eq!(parse_target("https://[fd00::1]").unwrap().port, 443);
        assert!(parse_target("ftp://x").is_err());
        assert!(parse_target("http://x:notaport/").is_err());
    }

    async fn serve_once(response: &'static str) -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf).await;
            let _ = s.write_all(response.as_bytes()).await;
            let _ = s.shutdown().await;
        });
        port
    }

    #[test]
    fn error_pages_as_text() {
        assert_eq!(page_text(b"<html><script>x()</script><p>Bad   gateway</p></html>").as_deref(), Some("Bad gateway"));
        assert_eq!(page_text(b"  "), None);
        assert!(page_text("é".repeat(300).as_bytes()).unwrap().ends_with('…'));
    }

    #[tokio::test]
    async fn http_up_and_down() {
        let port = serve_once("HTTP/1.1 204 No Content\r\ncontent-length: 0\r\n\r\n").await;
        let r = check(&format!("http://127.0.0.1:{port}/health"), "", false, DEFAULT_TIMEOUT).await;
        assert!(r.up, "{r:?}");
        assert_eq!(r.status, Some(204));
        assert!(r.latency_ms.is_some() && r.tls.is_none());

        let port = serve_once("HTTP/1.1 503 Service Unavailable\r\ncontent-length: 0\r\n\r\n").await;
        let r = check(&format!("http://127.0.0.1:{port}/"), "200-399", false, DEFAULT_TIMEOUT).await;
        assert!(!r.up);
        assert_eq!(r.status, Some(503));
        assert!(r.error.unwrap().contains("503"));

        let page = "HTTP/1.1 502 Bad Gateway\r\nserver: nginx\r\ncontent-type: text/html\r\ncontent-length: 90\r\n\r\n<html><head><style>b{}</style></head><body><h1>502 Bad Gateway</h1><hr>nginx</body></html>";
        let port = serve_once(page).await;
        let r = check(&format!("http://127.0.0.1:{port}/"), "", false, DEFAULT_TIMEOUT).await;
        let e = r.error.unwrap();
        assert!(
            e.contains("from nginx") && e.contains("\"502 Bad Gateway nginx\"") && e.contains("couldn't reach the app"),
            "{e}"
        );

        let port = serve_once("HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\n\r\n").await;
        let r = check(&format!("http://127.0.0.1:{port}/"), "200-399,401", false, DEFAULT_TIMEOUT).await;
        assert!(r.up);
    }

    #[tokio::test]
    async fn refused_and_timeout() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        let r = check(&format!("http://127.0.0.1:{port}/"), "", false, DEFAULT_TIMEOUT).await;
        assert!(!r.up && r.error.is_some());

        // Accepts but never answers.
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let hold = tokio::spawn(async move {
            let (_s, _) = l.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(5)).await;
        });
        let r = check(&format!("http://127.0.0.1:{port}/"), "", false, Duration::from_millis(300)).await;
        assert!(!r.up);
        assert!(r.error.unwrap().contains("timed out"));
        hold.abort();
    }

    #[tokio::test]
    async fn tls_garbage_is_a_failed_handshake() {
        let port = serve_once("HTTP/1.1 200 OK\r\n\r\n").await;
        let r = check(&format!("https://127.0.0.1:{port}/"), "", true, DEFAULT_TIMEOUT).await;
        assert!(!r.up);
        assert!(r.error.unwrap().contains("TLS"));
    }
    /// Self-signed https: the cert is captured and reported as untrusted; it
    /// only counts as up with `allow_untrusted`. Needs the `openssl` CLI.
    #[tokio::test]
    async fn self_signed_tls() {
        use rustls::pki_types::pem::PemObject;
        use rustls::pki_types::PrivateKeyDer;
        let dir = tempfile::tempdir().unwrap();
        let (cert, key) = (dir.path().join("c.pem"), dir.path().join("k.pem"));
        let ok = std::process::Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "ec",
                "-pkeyopt",
                "ec_paramgen_curve:prime256v1",
                "-nodes",
                "-days",
                "30",
            ])
            .args(["-subj", "/CN=localhost", "-addext", "subjectAltName=DNS:localhost"])
            .arg("-keyout")
            .arg(&key)
            .arg("-out")
            .arg(&cert)
            .output()
            .is_ok_and(|o| o.status.success());
        if !ok {
            eprintln!("skipping: openssl not available");
            return;
        }
        let certs = vec![CertificateDer::from_pem_file(&cert).unwrap()];
        let key = PrivateKeyDer::from_pem_file(&key).unwrap();
        let config = rustls::ServerConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .unwrap();
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let (s, _) = l.accept().await.unwrap();
                let acceptor = acceptor.clone();
                tokio::spawn(async move {
                    if let Ok(mut s) = acceptor.accept(s).await {
                        let mut buf = [0u8; 2048];
                        let _ = s.read(&mut buf).await;
                        let _ = s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n").await;
                        let _ = s.shutdown().await;
                    }
                });
            }
        });
        let url = format!("https://localhost:{port}/");
        let strict = check(&url, "", false, DEFAULT_TIMEOUT).await;
        let tls = strict.tls.clone().expect("cert captured");
        assert!(!tls.trusted);
        assert_eq!(strict.status, Some(200));
        assert!(!strict.up && strict.error.unwrap().contains("not trusted"));
        let days = tls.days_left(now_unix()).unwrap();
        assert!((28..=30).contains(&days), "{days}");
        assert!(tls.subject.unwrap().contains("localhost"));
        assert_eq!(tls.fingerprint.as_deref().map(str::len), Some(64));

        let lenient = check(&url, "", true, DEFAULT_TIMEOUT).await;
        assert!(lenient.up && lenient.error.is_none(), "{lenient:?}");
    }
}
