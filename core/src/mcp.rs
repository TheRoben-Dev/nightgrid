//! The MCP server (M12g, D-166): lets Claude or any other Model Context
//! Protocol client ask Nightgrid about the homelab. This module is the
//! protocol side, with no app state: JSON-RPC 2.0 messages, the tool list,
//! the Streamable HTTP request parsing and responses. The app answers tool
//! calls (`app/src/mcp_cmd.rs`).
//!
//! Read-only by default. Tools that change something are listed only when
//! the person turned writes on, and the app asks in its own window before
//! each one runs.

use serde::Serialize;
use serde_json::{json, Value};

/// Protocol versions this server speaks, newest first.
pub const PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];
/// The path the HTTP transport answers on.
pub const PATH: &str = "/mcp";
/// Largest request body read.
pub const BODY_MAX: usize = 1 << 20;
/// Largest request head read.
pub const HEAD_MAX: usize = 16 * 1024;
pub const DEFAULT_PORT: u16 = 47_821;

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;

/// One tool as `tools/list` describes it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDef {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    pub annotations: Value,
    /// Changes something (listed only with writes on, confirmed in the app).
    #[serde(skip)]
    pub write: bool,
}

fn host_arg() -> Value {
    json!({ "type": "string", "description": "Host name as Nightgrid shows it (or its numeric id)." })
}

fn read(name: &'static str, title: &'static str, description: &'static str, schema: Value) -> ToolDef {
    ToolDef {
        name,
        title,
        description,
        input_schema: schema,
        annotations: json!({ "title": title, "readOnlyHint": true, "openWorldHint": false }),
        write: false,
    }
}

fn write(
    name: &'static str,
    title: &'static str,
    description: &'static str,
    schema: Value,
    destructive: bool,
) -> ToolDef {
    ToolDef {
        name,
        title,
        description,
        input_schema: schema,
        annotations: json!({ "title": title, "readOnlyHint": false, "destructiveHint": destructive, "openWorldHint": false }),
        write: true,
    }
}

/// Every tool; [`tools`] filters out the write ones unless writes are on.
pub fn all_tools() -> Vec<ToolDef> {
    vec![
        read(
            "list_hosts",
            "List hosts",
            "Every host Nightgrid manages with its state (ok, offline, sleep, auth, off), CPU, memory and fullest disk, uptime, group, tags and role.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        read(
            "host_status",
            "Host status",
            "The latest live sample of one host: CPU and per-core load, memory and swap, every mount's usage, network and disk rates, temperatures, GPUs, kernel and uptime.",
            json!({ "type": "object", "properties": { "host": host_arg() }, "required": ["host"], "additionalProperties": false }),
        ),
        read(
            "metric_history",
            "Metric history",
            "CPU, memory, swap, load, fullest disk, network, disk I/O and temperature of one host over the last N hours (1–2160), as at most 120 averaged points.",
            json!({ "type": "object", "properties": {
                "host": host_arg(),
                "hours": { "type": "integer", "minimum": 1, "maximum": 2160, "default": 24 }
            }, "required": ["host"], "additionalProperties": false }),
        ),
        read(
            "list_alerts",
            "Alerts",
            "Active alerts (worst first) and, optionally, recently resolved ones.",
            json!({ "type": "object", "properties": {
                "history": { "type": "boolean", "default": false, "description": "Also list up to 50 resolved alerts." }
            }, "additionalProperties": false }),
        ),
        read(
            "host_timeline",
            "Host timeline",
            "What happened on one host: alerts firing and clearing, boots, outages, container crashes and restarts, unit failures, OOM kills, package changes, deploys and power actions, newest first.",
            json!({ "type": "object", "properties": {
                "host": host_arg(),
                "hours": { "type": "integer", "minimum": 1, "maximum": 2160, "default": 24 }
            }, "required": ["host"], "additionalProperties": false }),
        ),
        read(
            "service_checks",
            "Service checks",
            "Every HTTP, TCP and ping check with its state (up, down, unknown), latency and daily availability for the last 30 days.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        read(
            "journal",
            "Change journal",
            "Changes made from Nightgrid (deploys, file and crontab edits, firewall, proxy routes, updates, power, Proxmox, keys) and hand-written notes, newest first. Diffs are summarized, not included.",
            json!({ "type": "object", "properties": {
                "text": { "type": "string", "description": "Words that must all appear." },
                "host": host_arg(),
                "days": { "type": "integer", "minimum": 1, "maximum": 730, "default": 14 },
                "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 50 }
            }, "additionalProperties": false }),
        ),
        read(
            "disk_forecast",
            "Disk forecast",
            "Mounts that are filling up, with how full they are and the forecast days until full, from recorded usage.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        read(
            "list_containers",
            "Containers",
            "Docker containers on one host (reads Docker over SSH): name, image, state, status and compose project.",
            json!({ "type": "object", "properties": { "host": host_arg() }, "required": ["host"], "additionalProperties": false }),
        ),
        write(
            "container_action",
            "Start, stop or restart a container",
            "Starts, stops or restarts one Docker container. Needs writes turned on in Nightgrid, and the person approves each call in the app.",
            json!({ "type": "object", "properties": {
                "host": host_arg(),
                "container": { "type": "string", "description": "Container name." },
                "action": { "type": "string", "enum": ["start", "stop", "restart"] }
            }, "required": ["host", "container", "action"], "additionalProperties": false }),
            true,
        ),
        write(
            "add_journal_note",
            "Add a journal note",
            "Writes a hand-written entry into Nightgrid's change journal, optionally for one host. Needs writes turned on, and the person approves it in the app.",
            json!({ "type": "object", "properties": {
                "title": { "type": "string", "maxLength": 200 },
                "detail": { "type": "string", "maxLength": 4000 },
                "host": host_arg()
            }, "required": ["title"], "additionalProperties": false }),
            false,
        ),
    ]
}

/// The tools a client sees.
pub fn tools(writes: bool) -> Vec<ToolDef> {
    all_tools().into_iter().filter(|t| writes || !t.write).collect()
}

pub fn tool(name: &str) -> Option<ToolDef> {
    all_tools().into_iter().find(|t| t.name == name)
}

/// What to do with one JSON-RPC message.
#[derive(Debug, Clone, PartialEq)]
pub enum Dispatch {
    /// Answer with this response.
    Reply(Value),
    /// Run a tool and answer with its result.
    Call { id: Value, name: String, args: Value },
    /// A notification (or a response from the client): no answer.
    Nothing,
}

pub fn ok(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

pub fn error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// A tool result: the data as text (what most clients pass to the model)
/// and as structured content.
pub fn tool_result(data: &Value) -> Value {
    let text = serde_json::to_string_pretty(data).unwrap_or_default();
    let structured = if data.is_object() { data.clone() } else { json!({ "result": data }) };
    json!({ "content": [{ "type": "text", "text": text }], "structuredContent": structured, "isError": false })
}

/// A tool that ran but failed (the model sees why; not a protocol error).
pub fn tool_error(message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}

const INSTRUCTIONS: &str = "Nightgrid manages a homelab over SSH. Start with list_hosts and list_alerts; \
host names from list_hosts are what the other tools take. Data is what Nightgrid has collected; \
list_containers reads the host live. Tools that change something exist only when the person turned \
them on, and each call waits for their approval in Nightgrid.";

/// Handles one message. `writes` decides what `tools/list` shows.
pub fn dispatch(msg: &Value, writes: bool, version: &str) -> Dispatch {
    let Some(obj) = msg.as_object() else {
        return Dispatch::Reply(error(&Value::Null, INVALID_REQUEST, "expected a JSON-RPC object"));
    };
    let id = obj.get("id").cloned();
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        // A response to something we never send, or garbage.
        return match id {
            Some(id) if !obj.contains_key("result") && !obj.contains_key("error") => {
                Dispatch::Reply(error(&id, INVALID_REQUEST, "missing method"))
            }
            _ => Dispatch::Nothing,
        };
    };
    let Some(id) = id else {
        // notifications/initialized, notifications/cancelled, …
        return Dispatch::Nothing;
    };
    let params = obj.get("params").cloned().unwrap_or(Value::Null);
    match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
            let v = PROTOCOL_VERSIONS.iter().find(|v| **v == asked).copied().unwrap_or(PROTOCOL_VERSIONS[0]);
            Dispatch::Reply(ok(
                &id,
                json!({
                    "protocolVersion": v,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "nightgrid", "title": "Nightgrid", "version": version },
                    "instructions": INSTRUCTIONS,
                }),
            ))
        }
        "ping" => Dispatch::Reply(ok(&id, json!({}))),
        "tools/list" => Dispatch::Reply(ok(&id, json!({ "tools": tools(writes) }))),
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return Dispatch::Reply(error(&id, INVALID_PARAMS, "tools/call needs a name"));
            };
            match tool(name) {
                Some(t) if t.write && !writes => Dispatch::Reply(ok(
                    &id,
                    tool_error("This tool changes something, and changes are turned off in Nightgrid (Settings → Connections → MCP server)."),
                )),
                Some(_) => Dispatch::Call {
                    id,
                    name: name.to_owned(),
                    args: params.get("arguments").cloned().unwrap_or_else(|| json!({})),
                },
                None => Dispatch::Reply(error(&id, INVALID_PARAMS, &format!("unknown tool {name}"))),
            }
        }
        "resources/list" => Dispatch::Reply(ok(&id, json!({ "resources": [] }))),
        "prompts/list" => Dispatch::Reply(ok(&id, json!({ "prompts": [] }))),
        _ => Dispatch::Reply(error(&id, METHOD_NOT_FOUND, &format!("{method} isn't supported"))),
    }
}

// ---- arguments ----

/// A string argument, trimmed; `None` when missing or empty.
pub fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty())
}

/// An integer argument clamped to `lo..=hi`, `default` when missing.
pub fn arg_int(args: &Value, key: &str, default: i64, lo: i64, hi: i64) -> i64 {
    args.get(key).and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))).unwrap_or(default).clamp(lo, hi)
}

// ---- HTTP ----

/// The parts of a request head the server looks at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Head {
    pub method: String,
    pub path: String,
    pub content_length: usize,
    pub authorization: Option<String>,
    pub origin: Option<String>,
}

pub fn parse_head(head: &str) -> Head {
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split_whitespace();
    let mut h = Head {
        method: first.next().unwrap_or("").to_owned(),
        path: first.next().unwrap_or("").split('?').next().unwrap_or("").to_owned(),
        ..Default::default()
    };
    for l in lines {
        let Some((k, v)) = l.split_once(':') else { continue };
        let v = v.trim();
        match k.trim().to_ascii_lowercase().as_str() {
            "content-length" => h.content_length = v.parse().unwrap_or(0),
            "authorization" => h.authorization = Some(v.to_owned()),
            "origin" => h.origin = Some(v.to_owned()),
            _ => {}
        }
    }
    h
}

/// The bearer token from an `Authorization` header.
pub fn bearer(auth: Option<&str>) -> Option<&str> {
    let a = auth?;
    let (scheme, token) = a.split_once(' ')?;
    scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
}

/// Browsers send `Origin`; only this computer's own pages may call (DNS
/// rebinding protection). Clients that aren't browsers send none.
pub fn origin_allowed(origin: Option<&str>) -> bool {
    let Some(o) = origin else { return true };
    if o == "null" {
        return false;
    }
    let rest = o.split("://").nth(1).unwrap_or("");
    let host = match rest.strip_prefix('[') {
        Some(r) => r.split(']').next().map(|h| format!("[{h}]")).unwrap_or_default(),
        None => rest.split(['/', ':']).next().unwrap_or("").to_owned(),
    };
    let host = host.as_str();
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

pub fn http_response(status: &str, content_type: &str, body: &[u8]) -> Vec<u8> {
    let mut r = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    r.extend_from_slice(body);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_negotiates_and_lists_tools() {
        let r = dispatch(
            &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}),
            false,
            "1.0.0",
        );
        let Dispatch::Reply(v) = r else { panic!() };
        assert_eq!(v["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(v["result"]["serverInfo"]["version"], "1.0.0");
        let Dispatch::Reply(v) = dispatch(
            &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}),
            false,
            "1",
        ) else {
            panic!()
        };
        assert_eq!(v["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
        let Dispatch::Reply(v) = dispatch(&json!({"jsonrpc":"2.0","id":"a","method":"tools/list"}), false, "1") else {
            panic!()
        };
        let names: Vec<&str> =
            v["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"list_hosts"));
        assert!(!names.contains(&"container_action"));
        assert!(v["result"]["tools"][0]["inputSchema"]["type"] == "object");
        assert_eq!(v["result"]["tools"][0]["annotations"]["readOnlyHint"], true);
        let Dispatch::Reply(v) = dispatch(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}), true, "1") else {
            panic!()
        };
        assert!(v["result"]["tools"].as_array().unwrap().iter().any(|t| t["name"] == "container_action"));
    }

    #[test]
    fn calls_notifications_and_errors() {
        assert_eq!(
            dispatch(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}), false, "1"),
            Dispatch::Nothing
        );
        assert_eq!(
            dispatch(
                &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"host_status","arguments":{"host":"a"}}}),
                false,
                "1"
            ),
            Dispatch::Call { id: json!(3), name: "host_status".into(), args: json!({"host":"a"}) }
        );
        let Dispatch::Reply(v) = dispatch(
            &json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"container_action","arguments":{}}}),
            false,
            "1",
        ) else {
            panic!()
        };
        assert_eq!(v["result"]["isError"], true, "write tools answer with a tool error while writes are off");
        let Dispatch::Reply(v) =
            dispatch(&json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"rm_rf"}}), true, "1")
        else {
            panic!()
        };
        assert_eq!(v["error"]["code"], INVALID_PARAMS);
        let Dispatch::Reply(v) = dispatch(&json!({"jsonrpc":"2.0","id":6,"method":"sampling/x"}), true, "1") else {
            panic!()
        };
        assert_eq!(v["error"]["code"], METHOD_NOT_FOUND);
        let Dispatch::Reply(v) = dispatch(&json!([1, 2]), true, "1") else { panic!() };
        assert_eq!(v["error"]["code"], INVALID_REQUEST);
        assert_eq!(dispatch(&json!({"jsonrpc":"2.0","id":7,"result":{}}), true, "1"), Dispatch::Nothing);
    }

    #[test]
    fn every_tool_has_an_object_schema_and_hints() {
        for t in all_tools() {
            assert_eq!(t.input_schema["type"], "object", "{}", t.name);
            assert_eq!(t.annotations["readOnlyHint"], !t.write, "{}", t.name);
            assert!(t.name.chars().all(|c| c.is_ascii_lowercase() || c == '_'), "{}", t.name);
        }
    }

    #[test]
    fn http_heads_tokens_and_origins() {
        let h = parse_head("POST /mcp?x=1 HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 42\r\nAuthorization: Bearer abc\r\nOrigin: http://localhost:5173\r\n\r\n");
        assert_eq!((h.method.as_str(), h.path.as_str(), h.content_length), ("POST", "/mcp", 42));
        assert_eq!(bearer(h.authorization.as_deref()), Some("abc"));
        assert_eq!(bearer(Some("Basic abc")), None);
        assert!(origin_allowed(h.origin.as_deref()));
        assert!(origin_allowed(None));
        assert!(!origin_allowed(Some("https://evil.example")));
        assert!(!origin_allowed(Some("http://localhost.evil.example")));
        assert!(!origin_allowed(Some("null")));
        assert!(origin_allowed(Some("http://[::1]:8080")));
        assert_eq!(arg_int(&json!({"hours": 9999}), "hours", 24, 1, 2160), 2160);
        assert_eq!(arg_int(&json!({}), "hours", 24, 1, 2160), 24);
        assert_eq!(arg_str(&json!({"host": "  "}), "host"), None);
    }
}
