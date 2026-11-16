//! A small HTTP/1.1 client for webhooks and integrations: one request per
//! connection, no redirects, bounded response size. Shares the TLS setup with
//! the service checker so certificates are verified against the OS store; an
//! untrusted certificate is an error unless the caller allows it (internal
//! CAs, self-signed panels).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Bytes;
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::services::check::{connect_error, parse_target, tls_connector, Captured, Target};

/// Responses larger than this are refused.
pub const MAX_BODY: usize = 4 * 1024 * 1024;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
}

#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub allow_untrusted: bool,
    pub timeout: Duration,
}

impl Request {
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            method: Method::Get,
            url: url.into(),
            headers: Vec::new(),
            body: Vec::new(),
            allow_untrusted: false,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn post(url: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        Self { method: Method::Post, body: body.into(), ..Self::get(url) }
    }

    pub fn put(url: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        Self { method: Method::Put, body: body.into(), ..Self::get(url) }
    }

    pub fn delete(url: impl Into<String>) -> Self {
        Self { method: Method::Delete, ..Self::get(url) }
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn json(self, value: &serde_json::Value) -> Self {
        let mut r = self.header("content-type", "application/json");
        r.body = serde_json::to_vec(value).unwrap_or_default();
        r
    }

    pub fn allow_untrusted(mut self, allow: bool) -> Self {
        self.allow_untrusted = allow;
        self
    }

    pub fn timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    /// Response headers, names lower-cased.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    /// First header with this (lower-case) name.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, String> {
        serde_json::from_slice(&self.body).map_err(|e| format!("unexpected response: {e}"))
    }
}

/// Sends one request. Errors are short, user-facing strings.
pub async fn send(req: Request) -> Result<Response, String> {
    let target = parse_target(&req.url)?;
    let started = Instant::now();
    let left = |t: Duration| t.saturating_sub(started.elapsed());
    let tcp = match tokio::time::timeout(
        req.timeout,
        crate::tailnet::dial(&target.host, target.port, req.timeout + Duration::from_millis(50)),
    )
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return Err(connect_error(&e)),
        Err(_) => return Err("timed out connecting".into()),
    };
    let _ = tcp.set_nodelay(true);
    if !target.https {
        return exchange(tcp, &target, &req, left(req.timeout)).await;
    }
    let captured = Arc::new(Mutex::new(Captured::default()));
    let connector = tls_connector(captured.clone()).map_err(|e| format!("TLS setup failed: {e}"))?;
    let name = ServerName::try_from(target.host.clone()).map_err(|_| "host name isn't valid for TLS".to_owned())?;
    let stream = match tokio::time::timeout(left(req.timeout), connector.connect(name, tcp)).await {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return Err(format!("TLS handshake failed: {e}")),
        Err(_) => return Err("timed out during TLS handshake".into()),
    };
    let (verified, problem) = captured.lock().map(|c| (c.verified, c.problem.clone())).unwrap_or((false, None));
    if !verified && !req.allow_untrusted {
        // Nothing has been sent yet: the request (and any token in it) never
        // reaches a server we can't verify.
        return Err(format!("certificate not trusted ({})", problem.as_deref().unwrap_or("unknown reason")));
    }
    exchange(stream, &target, &req, left(req.timeout)).await
}

/// Like [`send`], following up to `max` redirects to HTTPS URLs only (used
/// for registry blobs, which redirect to a CDN). Headers are dropped when a
/// redirect leaves the original host, so a token never follows it.
pub async fn send_following(mut req: Request, max: usize) -> Result<Response, String> {
    let origin = parse_target(&req.url)?.host;
    for _ in 0..=max {
        let res = send(req.clone()).await?;
        if !matches!(res.status, 301 | 302 | 303 | 307 | 308) {
            return Ok(res);
        }
        let to = res.header("location").ok_or("redirect without a location")?;
        let next = if to.starts_with("https://") {
            to.to_owned()
        } else if to.starts_with('/') {
            let t = parse_target(&req.url)?;
            format!("https://{}{to}", t.authority)
        } else {
            return Err("refused a redirect to a non-HTTPS URL".into());
        };
        if parse_target(&next)?.host != origin {
            req.headers.clear();
        }
        req.url = next;
        req.method = Method::Get;
        req.body.clear();
    }
    Err("too many redirects".into())
}

async fn exchange<S>(io: S, target: &Target, req: &Request, timeout: Duration) -> Result<Response, String>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let work = async {
        let (mut sender, conn) = hyper::client::conn::http1::handshake::<_, Full<Bytes>>(TokioIo::new(io))
            .await
            .map_err(|e| format!("HTTP error: {e}"))?;
        let driver = tokio::spawn(async move {
            let _ = conn.await;
        });
        let mut b = hyper::Request::builder()
            .method(match req.method {
                Method::Get => hyper::Method::GET,
                Method::Post => hyper::Method::POST,
                Method::Put => hyper::Method::PUT,
                Method::Delete => hyper::Method::DELETE,
            })
            .uri(&target.path)
            .header(hyper::header::HOST, &target.authority)
            .header(hyper::header::USER_AGENT, concat!("Nightgrid/", env!("CARGO_PKG_VERSION")))
            .header(hyper::header::CONNECTION, "close");
        let has = |n: &str| req.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(n));
        if !has("accept") {
            b = b.header(hyper::header::ACCEPT, "*/*");
        }
        for (k, v) in &req.headers {
            b = b.header(k.as_str(), v.as_str());
        }
        let r = b.body(Full::new(Bytes::from(req.body.clone()))).map_err(|e| format!("bad request: {e}"))?;
        let res = sender.send_request(r).await.map_err(|e| format!("HTTP error: {e}"));
        let res = match res {
            Ok(r) => r,
            Err(e) => {
                driver.abort();
                return Err(e);
            }
        };
        let status = res.status().as_u16();
        let headers = res
            .headers()
            .iter()
            .filter_map(|(k, v)| Some((k.as_str().to_ascii_lowercase(), v.to_str().ok()?.to_owned())))
            .collect();
        let body = Limited::new(res.into_body(), MAX_BODY).collect().await;
        driver.abort();
        let body = body.map_err(|e| format!("reading the response failed: {e}"))?.to_bytes().to_vec();
        Ok::<_, String>(Response { status, headers, body })
    };
    tokio::time::timeout(timeout, work).await.map_err(|_| "timed out waiting for a response".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Serves one canned response and hands back the raw request it got.
    async fn serve_once(response: &'static str) -> (u16, tokio::task::JoinHandle<String>) {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let h = tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            let mut buf = vec![0u8; 8192];
            let mut got = Vec::new();
            // Read until the headers and the announced body have arrived.
            loop {
                let n = s.read(&mut buf).await.unwrap();
                got.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&got).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let len = text
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase().strip_prefix("content-length: ").map(|v| v.parse().unwrap())
                        })
                        .unwrap_or(0usize);
                    if got.len() >= end + 4 + len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let _ = s.write_all(response.as_bytes()).await;
            let _ = s.shutdown().await;
            String::from_utf8_lossy(&got).to_string()
        });
        (port, h)
    }

    #[tokio::test]
    async fn post_json_and_read_body() {
        let (port, req) =
            serve_once("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 11\r\n\r\n{\"ok\":true}")
                .await;
        let r = send(
            Request::post(format!("http://127.0.0.1:{port}/hook?x=1"), Vec::new())
                .header("X-Token", "abc")
                .json(&serde_json::json!({"msg": "hi"})),
        )
        .await
        .unwrap();
        assert!(r.ok());
        let v: serde_json::Value = r.json().unwrap();
        assert_eq!(v["ok"], true);
        let raw = req.await.unwrap();
        assert!(raw.starts_with("POST /hook?x=1 HTTP/1.1\r\n"), "{raw}");
        assert!(raw.to_ascii_lowercase().contains("x-token: abc"));
        assert!(raw.ends_with("{\"msg\":\"hi\"}"));
    }

    #[tokio::test]
    async fn chunked_and_errors() {
        let (port, _) =
            serve_once("HTTP/1.1 404 Not Found\r\ntransfer-encoding: chunked\r\n\r\n5\r\nnope!\r\n0\r\n\r\n").await;
        let r = send(Request::get(format!("http://127.0.0.1:{port}/"))).await.unwrap();
        assert_eq!((r.status, r.text().as_str()), (404, "nope!"));
        assert!(!r.ok());

        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        // A parallel test may grab the freed port first, so only an error is certain.
        assert!(send(Request::get(format!("http://127.0.0.1:{port}/"))).await.is_err());
        assert!(send(Request::get("ftp://x/")).await.is_err());
    }

    #[tokio::test]
    async fn timeout() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let hold = tokio::spawn(async move {
            let (_s, _) = l.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(5)).await;
        });
        let e = send(Request::get(format!("http://127.0.0.1:{port}/")).timeout(Duration::from_millis(300)))
            .await
            .unwrap_err();
        assert!(e.contains("timed out"), "{e}");
        hold.abort();
    }

    #[tokio::test]
    async fn not_tls_fails_handshake() {
        let (port, _) = serve_once("HTTP/1.1 200 OK\r\n\r\n").await;
        let e = send(Request::get(format!("https://127.0.0.1:{port}/")).allow_untrusted(true)).await.unwrap_err();
        assert!(e.contains("TLS"), "{e}");
    }
}
