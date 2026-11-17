//! One connection to a node: newline-delimited JSON over any byte stream
//! (an SSH `direct-streamlocal` channel to the socket, or `nightgrid-node
//! bridge` on an exec channel). Requests go one at a time and wait for the
//! response with their `id`; lines without an `id` are stream frames
//! (protocol 2) and go to the receiver handed out by [`NodeClient::connect`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};

use super::{NodeEvent, Welcome, PROTO_MAX, PROTO_MIN};
use crate::metrics::Sample;

/// How long one request may take (container actions on the node take ~10 s).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Longest line read from the node (its own limit is 4 MiB).
const MAX_LINE: usize = 8 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeError {
    /// The node answered with an error object.
    Remote {
        code: String,
        message: String,
    },
    /// The connection ended.
    Closed,
    Timeout,
    /// Something that isn't the protocol (bad JSON, a write failed).
    Bad(String),
}

impl NodeError {
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Remote { code, .. } => Some(code),
            _ => None,
        }
    }
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Remote { code, message } if message.is_empty() => write!(f, "the node said {code}"),
            Self::Remote { code, message } => write!(f, "the node said {code}: {message}"),
            Self::Closed => f.write_str("the connection to the node ended"),
            Self::Timeout => f.write_str("the node didn't answer in time"),
            Self::Bad(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for NodeError {}

/// A pushed line (protocol 2).
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    Event(Box<NodeEvent>),
    Metrics(Box<Sample>),
    /// The node dropped `missed` frames of `stream` because the app read too slowly.
    Lagged {
        stream: String,
        missed: u64,
    },
    /// A stream the app doesn't handle (yet).
    Other(String),
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, NodeError>>>>>;

pub struct NodeClient {
    writer: tokio::sync::Mutex<Box<dyn AsyncWrite + Send + Unpin>>,
    /// Held for a whole request, so requests go one at a time.
    turn: tokio::sync::Mutex<()>,
    pending: Pending,
    next: AtomicU64,
    closed: Arc<AtomicBool>,
    reader: tokio::task::JoinHandle<()>,
    welcome: Welcome,
}

impl Drop for NodeClient {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

fn parse_frame(stream: &str, data: Value) -> Frame {
    match stream {
        "events" => match serde_json::from_value::<NodeEvent>(data) {
            Ok(e) => Frame::Event(Box::new(e)),
            Err(_) => Frame::Other(stream.into()),
        },
        "metrics" => match serde_json::from_value::<Sample>(data) {
            Ok(s) => Frame::Metrics(Box::new(s)),
            Err(_) => Frame::Other(stream.into()),
        },
        "lagged" => Frame::Lagged {
            stream: data.get("stream").and_then(Value::as_str).unwrap_or("").to_owned(),
            missed: data.get("missed").and_then(Value::as_u64).unwrap_or(0),
        },
        other => Frame::Other(other.into()),
    }
}

/// Routes one line from the node: a response to its waiting request, a frame
/// to the receiver. Unparseable lines are dropped.
fn route(line: &str, pending: &Pending, frames: &mpsc::UnboundedSender<Frame>) {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return;
    };
    if let Some(id) = v.get("id").and_then(Value::as_u64) {
        let Some(tx) = pending.lock().expect("poisoned").remove(&id) else {
            return;
        };
        let res = match (v.get("result"), v.get("error")) {
            (_, Some(e)) if !e.is_null() => Err(NodeError::Remote {
                code: e.get("code").and_then(Value::as_str).unwrap_or("internal").to_owned(),
                message: e.get("message").and_then(Value::as_str).unwrap_or("").to_owned(),
            }),
            (Some(r), _) => Ok(r.clone()),
            (None, _) => Ok(Value::Null),
        };
        let _ = tx.send(res);
    } else if let Some(stream) = v.get("stream").and_then(Value::as_str) {
        let data = v.get("data").cloned().unwrap_or(Value::Null);
        let _ = frames.send(parse_frame(stream, data));
    }
}

impl NodeClient {
    /// Starts reading `stream` and says `hello`. `client` is free text for
    /// the node's log (e.g. `nightgrid/0.10.86`).
    pub async fn connect<S>(stream: S, client: &str) -> Result<(Self, mpsc::UnboundedReceiver<Frame>), NodeError>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        let (read, write) = tokio::io::split(stream);
        let pending: Pending = Arc::default();
        let closed = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::unbounded_channel();
        let reader = {
            let pending = pending.clone();
            let closed = closed.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(read);
                let mut buf = Vec::new();
                loop {
                    buf.clear();
                    match (&mut lines).take(MAX_LINE as u64).read_until(b'\n', &mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) if buf.last() != Some(&b'\n') && buf.len() >= MAX_LINE => break,
                        Ok(_) => route(String::from_utf8_lossy(&buf).trim(), &pending, &tx),
                    }
                }
                closed.store(true, Ordering::SeqCst);
                for (_, w) in pending.lock().expect("poisoned").drain() {
                    let _ = w.send(Err(NodeError::Closed));
                }
            })
        };
        let mut c = Self {
            writer: tokio::sync::Mutex::new(Box::new(write)),
            turn: tokio::sync::Mutex::new(()),
            pending,
            next: AtomicU64::new(1),
            closed,
            reader,
            welcome: Welcome { proto: 0, node: Default::default(), caps: Default::default() },
        };
        let w: Welcome =
            c.call("hello", json!({ "protoMin": PROTO_MIN, "protoMax": PROTO_MAX, "client": client })).await?;
        c.welcome = w;
        Ok((c, rx))
    }

    pub fn welcome(&self) -> &Welcome {
        &self.welcome
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Sends a request and waits for its result.
    pub async fn call<T: DeserializeOwned>(&self, method: &str, params: Value) -> Result<T, NodeError> {
        let v = self.call_value(method, params).await?;
        serde_json::from_value(v).map_err(|e| NodeError::Bad(format!("{method}: unexpected answer ({e})")))
    }

    pub async fn call_value(&self, method: &str, params: Value) -> Result<Value, NodeError> {
        let _turn = self.turn.lock().await;
        if self.is_closed() {
            return Err(NodeError::Closed);
        }
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().expect("poisoned").insert(id, tx);
        let mut line = serde_json::to_vec(&json!({ "id": id, "method": method, "params": params }))
            .map_err(|e| NodeError::Bad(e.to_string()))?;
        line.push(b'\n');
        let sent = async {
            let mut w = self.writer.lock().await;
            w.write_all(&line).await?;
            w.flush().await
        }
        .await;
        if let Err(e) = sent {
            self.pending.lock().expect("poisoned").remove(&id);
            return Err(NodeError::Bad(format!("couldn't write to the node: {e}")));
        }
        match tokio::time::timeout(REQUEST_TIMEOUT, rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => Err(NodeError::Closed),
            Err(_) => {
                self.pending.lock().expect("poisoned").remove(&id);
                Err(NodeError::Timeout)
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod fake {
    //! A scripted node over an in-memory pipe, for tests.
    use super::*;
    use tokio::io::DuplexStream;

    /// Answers each request with `answer(method, params)`; `Err((code, msg))`
    /// becomes an error object. Frames in `push` are sent right after hello.
    pub fn serve<F>(answer: F, push: Vec<Value>) -> DuplexStream
    where
        F: Fn(&str, &Value) -> Result<Value, (String, String)> + Send + 'static,
    {
        let (app, node) = tokio::io::duplex(1 << 20);
        tokio::spawn(async move {
            let (r, mut w) = tokio::io::split(node);
            let mut lines = BufReader::new(r).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let v: Value = serde_json::from_str(&line).unwrap();
                let id = v["id"].clone();
                let method = v["method"].as_str().unwrap_or("").to_owned();
                // A frame before the response, as protocol 2 allows.
                if method == "hello" {
                    for f in &push {
                        let mut out = serde_json::to_vec(f).unwrap();
                        out.push(b'\n');
                        let _ = w.write_all(&out).await;
                    }
                }
                let reply = match answer(&method, &v["params"]) {
                    Ok(r) => json!({ "id": id, "result": r }),
                    Err((code, message)) => json!({ "id": id, "error": { "code": code, "message": message } }),
                };
                let mut out = serde_json::to_vec(&reply).unwrap();
                out.push(b'\n');
                if w.write_all(&out).await.is_err() {
                    break;
                }
            }
        });
        app
    }

    pub fn welcome(proto: u32, caps: &[&str]) -> Value {
        let caps: serde_json::Map<String, Value> = caps.iter().map(|c| ((*c).to_owned(), json!(1))).collect();
        json!({ "proto": proto, "node": { "nodeId": "n1", "version": "0.23.0", "hostname": "server01", "arch": "x86_64" }, "caps": caps })
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{serve, welcome};
    use super::*;

    #[tokio::test]
    async fn hello_then_requests_and_interleaved_frames() {
        let io = serve(
            |m, p| match m {
                "hello" => Ok(welcome(2, &["metrics"])),
                "echo" => Ok(p.clone()),
                _ => Err(("unknown_method".into(), m.into())),
            },
            vec![
                json!({"stream":"events","data":{"seq":4,"at":10,"kind":"boot","level":"info","title":"booted","detail":""}}),
                json!({"stream":"lagged","data":{"stream":"metrics","missed":3}}),
            ],
        );
        let (c, mut frames) = NodeClient::connect(io, "test").await.unwrap();
        assert_eq!(c.welcome().proto, 2);
        assert_eq!(c.welcome().cap("metrics"), 1);
        let v: Value = c.call("echo", json!({"a": 1})).await.unwrap();
        assert_eq!(v, json!({"a": 1}));
        let e = c.call_value("nope", json!({})).await.unwrap_err();
        assert_eq!(e.code(), Some("unknown_method"));
        match frames.recv().await.unwrap() {
            Frame::Event(e) => assert_eq!(e.seq, 4),
            f => panic!("{f:?}"),
        }
        assert_eq!(frames.recv().await.unwrap(), Frame::Lagged { stream: "metrics".into(), missed: 3 });
    }

    #[tokio::test]
    async fn unsupported_protocol_fails_hello() {
        let io = serve(|_, _| Err(("unsupported_proto".into(), "node speaks 3".into())), vec![]);
        let e = NodeClient::connect(io, "test").await.err().unwrap();
        assert_eq!(e.code(), Some("unsupported_proto"));
    }

    #[tokio::test]
    async fn a_closed_connection_fails_waiting_requests() {
        let (app, node) = tokio::io::duplex(1024);
        drop(node);
        let e = NodeClient::connect(app, "test").await.err().unwrap();
        assert!(matches!(e, NodeError::Closed | NodeError::Bad(_)), "{e:?}");
    }
}
