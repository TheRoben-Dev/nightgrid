//! Proxmox VE (M10r): the cluster as Proxmox itself sees it, read and driven
//! through `pvesh` on the host over SSH. `pvesh` speaks the same API as the
//! web UI without an API token or an open port: it runs as root through the
//! usual sudo path (rule 4), and nothing is stored here.
//!
//! Part 1: nodes, guests (with power actions), storage and the task log.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::files::path::shell_quote;

pub mod backup;
pub mod create;
pub mod firewall;
pub mod hardware;
pub mod network;
pub mod sandbox;

/// Marks each `pvesh` answer in a batched script's output.
pub(crate) const MARK: &str = "@@pve";

// ---------------------------------------------------------------- types

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveNode {
    pub name: String,
    pub online: bool,
    /// The node this host is.
    pub local: bool,
    pub ip: Option<String>,
    /// 0..1 of all cores.
    pub cpu: f64,
    pub maxcpu: u32,
    /// Bytes.
    pub mem: f64,
    pub maxmem: f64,
    pub disk: f64,
    pub maxdisk: f64,
    /// Seconds.
    pub uptime: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveGuest {
    pub vmid: u32,
    /// `qemu` (VM) or `lxc` (container).
    pub kind: String,
    pub node: String,
    pub name: String,
    /// running, stopped, paused, suspended, unknown.
    pub status: String,
    pub template: bool,
    pub cpu: f64,
    pub maxcpu: u32,
    pub mem: f64,
    pub maxmem: f64,
    pub disk: f64,
    pub maxdisk: f64,
    pub uptime: f64,
    pub tags: Vec<String>,
    /// backup, snapshot, migrate, … while an operation holds the guest.
    pub lock: Option<String>,
    /// HA state when the guest is managed by HA.
    pub hastate: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveStorage {
    pub storage: String,
    pub node: String,
    /// dir, lvmthin, zfspool, nfs, cifs, pbs, …
    pub plugin: String,
    /// images, rootdir, iso, vztmpl, backup, snippets.
    pub content: Vec<String>,
    pub used: f64,
    pub total: f64,
    pub shared: bool,
    /// available, unknown (offline or not mounted).
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveTask {
    pub upid: String,
    pub node: String,
    /// qmstart, vzdump, vncproxy, …
    pub kind: String,
    /// The guest or object it acted on ("" for node-wide tasks).
    pub id: String,
    pub user: String,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub start: i64,
    /// Unset while it runs.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub end: Option<i64>,
    /// "OK", a warning count, or the error; unset while it runs.
    pub status: Option<String>,
}

impl PveTask {
    pub fn running(&self) -> bool {
        self.end.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveNodeInfo {
    pub pve_version: String,
    pub kernel: String,
    pub cpu_model: String,
    pub sockets: u32,
    pub cores: u32,
    pub threads: u32,
    pub loadavg: Vec<f64>,
    pub swap_used: f64,
    pub swap_total: f64,
    pub boot_mode: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveOverview {
    /// `pvesh` was found on the host.
    pub present: bool,
    /// e.g. 8.2.4.
    pub version: String,
    /// Cluster name, unset on a single node.
    pub cluster: Option<String>,
    pub quorate: Option<bool>,
    pub nodes: Vec<PveNode>,
    pub guests: Vec<PveGuest>,
    pub storages: Vec<PveStorage>,
    pub tasks: Vec<PveTask>,
    /// Details of the node Nightgrid is connected to.
    pub local: Option<PveNodeInfo>,
    /// Errors from single reads; the rest still shows.
    pub notes: Vec<String>,
}

// ---------------------------------------------------------- validation

/// A node name as Proxmox allows it (a host name label).
pub fn valid_node(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 63
        && !n.starts_with('-')
        && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

pub fn valid_vmid(id: u32) -> bool {
    (100..=999_999_999).contains(&id)
}

pub fn valid_kind(k: &str) -> bool {
    k == "qemu" || k == "lxc"
}

/// `UPID:node:pid:pstart:starttime:type:id:user@realm[!token]:`
pub fn valid_upid(u: &str) -> bool {
    let parts: Vec<&str> = u.split(':').collect();
    u.len() <= 256
        && parts.len() == 9
        && parts[0] == "UPID"
        && valid_node(parts[1])
        && parts[2..5].iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_hexdigit()))
        && !parts[5].is_empty()
        && parts[5].chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && parts[6].chars().all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))
        && !parts[7].is_empty()
        && parts[7].chars().all(|c| c.is_ascii_alphanumeric() || "@!_-.".contains(c))
        && parts[8].is_empty()
}

// ------------------------------------------------------------- scripts

pub(crate) fn get(path: &str, extra: &str) -> String {
    format!("echo '{MARK} {path}'; pvesh get {} --output-format json{extra} 2>&1; echo", shell_quote(path))
}

/// `pvesh create PATH --k 'v' …` printing JSON (a task's UPID for most calls).
pub(crate) fn create(path: &str, params: &[(&str, String)]) -> String {
    let mut c = format!("pvesh create {}", shell_quote(path));
    for (k, v) in params {
        c.push_str(&format!(" --{k} {}", shell_quote(v)));
    }
    c.push_str(" --output-format json 2>&1");
    c
}

/// Reads everything part 1 shows in one connection round trip.
pub fn overview_script() -> String {
    let mut s = String::from("if ! command -v pvesh >/dev/null 2>&1; then echo '@@nopve'; exit 0; fi\n");
    for p in ["/version", "/cluster/status", "/cluster/resources", "/cluster/tasks"] {
        s.push_str(&get(p, ""));
        s.push('\n');
    }
    // The node this host is: Proxmox names nodes after the short host name.
    s.push_str(&format!(
        "n=$(hostname -s 2>/dev/null || hostname); echo \"{MARK} /nodes/local/status\"; pvesh get \"/nodes/$n/status\" --output-format json 2>&1; echo\n"
    ));
    s
}

/// Power actions on a guest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PveAction {
    Start,
    Shutdown,
    Reboot,
    Stop,
    Suspend,
    Resume,
}

impl PveAction {
    pub fn path(self) -> &'static str {
        match self {
            PveAction::Start => "start",
            PveAction::Shutdown => "shutdown",
            PveAction::Reboot => "reboot",
            PveAction::Stop => "stop",
            PveAction::Suspend => "suspend",
            PveAction::Resume => "resume",
        }
    }

    /// Needs a confirmation naming the guest (rule 5).
    pub fn is_destructive(self) -> bool {
        !matches!(self, PveAction::Start | PveAction::Resume)
    }
}

/// Starts a power action; prints the task's UPID.
pub fn action_script(node: &str, kind: &str, vmid: u32, action: PveAction) -> Option<String> {
    if !valid_node(node) || !valid_kind(kind) || !valid_vmid(vmid) {
        return None;
    }
    // Containers can't be suspended through this path (CRIU is experimental).
    if kind == "lxc" && matches!(action, PveAction::Suspend | PveAction::Resume) {
        return None;
    }
    let path = format!("/nodes/{node}/{kind}/{vmid}/status/{}", action.path());
    Some(format!("pvesh create {} --output-format json 2>&1", shell_quote(&path)))
}

/// A task's state and its log (the last `limit` lines).
pub fn task_script(node: &str, upid: &str, limit: u32) -> Option<String> {
    if !valid_node(node) || !valid_upid(upid) {
        return None;
    }
    let base = format!("/nodes/{node}/tasks/{upid}");
    Some(format!(
        "{}\n{}\n",
        get(&format!("{base}/status"), ""),
        get(&format!("{base}/log"), &format!(" --limit {}", limit.clamp(1, 5000)))
    ))
}

// ------------------------------------------------------------- parsing

/// Splits a batched script's output into (path, body).
pub(crate) fn sections(out: &str) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    for line in out.lines() {
        if let Some(p) = line.strip_prefix(MARK).and_then(|r| r.strip_prefix(' ')) {
            v.push((p.trim().to_owned(), String::new()));
        } else if let Some((_, body)) = v.last_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    v
}

pub(crate) fn f(v: &Value, k: &str) -> f64 {
    match &v[k] {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

pub(crate) fn s(v: &Value, k: &str) -> String {
    match &v[k] {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

pub(crate) fn opt_s(v: &Value, k: &str) -> Option<String> {
    Some(s(v, k)).filter(|x| !x.is_empty())
}

pub(crate) fn b(v: &Value, k: &str) -> bool {
    match &v[k] {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => s == "1",
        _ => false,
    }
}

/// Turns pvesh's error text into something to act on.
pub fn explain(body: &str) -> String {
    let t = body.trim();
    let first = t.lines().find(|l| !l.trim().is_empty()).unwrap_or("no output").trim();
    if t.contains("ipcc_send_rec") || t.contains("Permission denied") || t.contains("permission denied") {
        format!("{first} (pvesh needs root: read as root)")
    } else if t.contains("Connection refused") && t.contains("pveproxy") {
        format!("{first} (is pveproxy running? systemctl status pveproxy)")
    } else if t.contains("no such cluster node") || t.contains("hostname lookup") {
        format!("{first} (the node name doesn't match this host's name)")
    } else {
        first.to_owned()
    }
}

fn json(path: &str, body: &str, notes: &mut Vec<String>) -> Option<Value> {
    match serde_json::from_str::<Value>(body.trim()) {
        Ok(v) => Some(v),
        Err(_) => {
            notes.push(format!("{path}: {}", explain(body)));
            None
        }
    }
}

fn tags(v: &Value) -> Vec<String> {
    s(v, "tags").split([';', ',', ' ']).filter(|t| !t.is_empty()).map(str::to_owned).collect()
}

pub fn parse_resources(v: &Value, o: &mut PveOverview) {
    let Some(items) = v.as_array() else { return };
    for r in items {
        match r["type"].as_str().unwrap_or("") {
            "node" => o.nodes.push(PveNode {
                name: s(r, "node"),
                online: s(r, "status") == "online",
                local: false,
                ip: None,
                cpu: f(r, "cpu"),
                maxcpu: f(r, "maxcpu") as u32,
                mem: f(r, "mem"),
                maxmem: f(r, "maxmem"),
                disk: f(r, "disk"),
                maxdisk: f(r, "maxdisk"),
                uptime: f(r, "uptime"),
            }),
            kind @ ("qemu" | "lxc") => o.guests.push(PveGuest {
                vmid: f(r, "vmid") as u32,
                kind: kind.to_owned(),
                node: s(r, "node"),
                name: s(r, "name"),
                status: opt_s(r, "status").unwrap_or_else(|| "unknown".into()),
                template: b(r, "template"),
                cpu: f(r, "cpu"),
                maxcpu: f(r, "maxcpu") as u32,
                mem: f(r, "mem"),
                maxmem: f(r, "maxmem"),
                disk: f(r, "disk"),
                maxdisk: f(r, "maxdisk"),
                uptime: f(r, "uptime"),
                tags: tags(r),
                lock: opt_s(r, "lock"),
                hastate: opt_s(r, "hastate"),
            }),
            "storage" => o.storages.push(PveStorage {
                storage: s(r, "storage"),
                node: s(r, "node"),
                plugin: s(r, "plugintype"),
                content: s(r, "content").split(',').filter(|c| !c.is_empty()).map(str::to_owned).collect(),
                used: f(r, "disk"),
                total: f(r, "maxdisk"),
                shared: b(r, "shared"),
                status: opt_s(r, "status").unwrap_or_else(|| "unknown".into()),
            }),
            _ => {}
        }
    }
    o.nodes.sort_by(|a, b| a.name.cmp(&b.name));
    o.guests.sort_by_key(|g| g.vmid);
    o.storages.sort_by(|a, b| (&a.storage, &a.node).cmp(&(&b.storage, &b.node)));
}

pub fn parse_tasks(v: &Value) -> Vec<PveTask> {
    let mut out: Vec<PveTask> = v
        .as_array()
        .map(|a| {
            a.iter()
                .map(|t| PveTask {
                    upid: s(t, "upid"),
                    node: s(t, "node"),
                    kind: s(t, "type"),
                    id: s(t, "id"),
                    user: s(t, "user"),
                    start: f(t, "starttime") as i64,
                    end: Some(f(t, "endtime") as i64).filter(|e| *e > 0),
                    status: opt_s(t, "status"),
                })
                .filter(|t| !t.upid.is_empty())
                .collect()
        })
        .unwrap_or_default();
    // Running first, then newest.
    out.sort_by(|a, b| b.running().cmp(&a.running()).then(b.start.cmp(&a.start)));
    out
}

pub fn parse_node_status(v: &Value) -> PveNodeInfo {
    let ci = &v["cpuinfo"];
    let kernel = v["current-kernel"]["release"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| s(v, "kversion").split_whitespace().nth(1).unwrap_or("").to_owned());
    PveNodeInfo {
        pve_version: s(v, "pveversion").split('/').nth(1).unwrap_or("").to_owned(),
        kernel,
        cpu_model: s(ci, "model"),
        sockets: f(ci, "sockets") as u32,
        cores: f(ci, "cores") as u32,
        threads: f(ci, "cpus") as u32,
        loadavg: v["loadavg"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().and_then(|s| s.parse().ok()).or(x.as_f64())).collect())
            .unwrap_or_default(),
        swap_used: f(&v["swap"], "used"),
        swap_total: f(&v["swap"], "total"),
        boot_mode: s(&v["boot-info"], "mode"),
    }
}

/// Parses [`overview_script`]'s output.
pub fn parse_overview(out: &str) -> PveOverview {
    let mut o = PveOverview::default();
    if out.lines().any(|l| l.trim() == "@@nopve") {
        return o;
    }
    o.present = true;
    let mut notes = Vec::new();
    let mut status: Option<Value> = None;
    for (path, body) in sections(out) {
        let Some(v) = json(&path, &body, &mut notes) else { continue };
        match path.as_str() {
            "/version" => o.version = s(&v, "version"),
            "/cluster/status" => status = Some(v),
            "/cluster/resources" => parse_resources(&v, &mut o),
            "/cluster/tasks" => o.tasks = parse_tasks(&v),
            "/nodes/local/status" => o.local = Some(parse_node_status(&v)),
            _ => {}
        }
    }
    if let Some(items) = status.as_ref().and_then(Value::as_array) {
        for it in items {
            match it["type"].as_str() {
                Some("cluster") => {
                    o.cluster = opt_s(it, "name");
                    o.quorate = Some(b(it, "quorate"));
                }
                Some("node") => {
                    let name = s(it, "name");
                    if let Some(n) = o.nodes.iter_mut().find(|n| n.name == name) {
                        n.local = b(it, "local");
                        n.ip = opt_s(it, "ip");
                    }
                }
                _ => {}
            }
        }
    }
    // A single node without a cluster still knows it is local.
    if o.nodes.len() == 1 {
        o.nodes[0].local = true;
    }
    o.notes = notes;
    o
}

/// The UPID a create call printed (a JSON string, or bare).
pub fn parse_upid(out: &str) -> Result<String, String> {
    let t = out.trim();
    let u = serde_json::from_str::<String>(t).unwrap_or_else(|_| t.to_owned());
    if valid_upid(&u) {
        Ok(u)
    } else {
        Err(explain(out))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveTaskDetail {
    pub running: bool,
    /// "OK" or the error once finished.
    pub exitstatus: Option<String>,
    pub log: Vec<String>,
}

/// Parses [`task_script`]'s output.
pub fn parse_task(out: &str) -> Result<PveTaskDetail, String> {
    let mut d = PveTaskDetail { running: false, exitstatus: None, log: Vec::new() };
    let mut seen = false;
    for (path, body) in sections(out) {
        let v: Value = serde_json::from_str(body.trim()).map_err(|_| explain(&body))?;
        if path.ends_with("/status") {
            seen = true;
            d.running = s(&v, "status") == "running";
            d.exitstatus = opt_s(&v, "exitstatus");
        } else if path.ends_with("/log") {
            let mut lines: Vec<(i64, String)> =
                v.as_array().map(|a| a.iter().map(|l| (f(l, "n") as i64, s(l, "t"))).collect()).unwrap_or_default();
            lines.sort_by_key(|l| l.0);
            // pvesh pads short logs with "no content".
            d.log = lines.into_iter().map(|l| l.1).filter(|t| t != "no content").collect();
        }
    }
    if seen {
        Ok(d)
    } else {
        Err(explain(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERSION: &str = include_str!("fixtures/version.json");
    const STATUS: &str = include_str!("fixtures/cluster_status.json");
    const RESOURCES: &str = include_str!("fixtures/cluster_resources.json");
    const TASKS: &str = include_str!("fixtures/cluster_tasks.json");
    const NODE: &str = include_str!("fixtures/node_status.json");
    const TASK_STATUS: &str = include_str!("fixtures/task_status.json");
    const TASK_LOG: &str = include_str!("fixtures/task_log.json");
    const UPID: &str = "UPID:pve2:00001F00:00ABCDEF:6744B5D0:vzdump::root@pam:";

    fn out(parts: &[(&str, &str)]) -> String {
        parts.iter().map(|(p, b)| format!("{MARK} {p}\n{b}\n")).collect()
    }

    #[test]
    fn validates_names_and_upids() {
        assert!(valid_node("pve1") && valid_node("pve-a.lan"));
        assert!(!valid_node("") && !valid_node("-x") && !valid_node("a b") && !valid_node("a;b"));
        assert!(valid_vmid(100) && !valid_vmid(99) && !valid_vmid(1_000_000_000));
        assert!(valid_upid(UPID));
        assert!(valid_upid("UPID:pve1:0000A1B2:01234567:6744C3F6:qmstart:100:root@pam!tok:"));
        assert!(!valid_upid("UPID:pve1:zz:01234567:6744C3F6:qmstart:100:root@pam:"));
        assert!(!valid_upid("UPID:pve1:0000A1B2:01234567:6744C3F6:qmstart:100:root@pam"));
        assert!(!valid_upid("UPID:pve1:0000A1B2:01234567:6744C3F6:qm start:100:root@pam:"));
        assert!(!valid_upid("UPID:pve1:0000A1B2:01234567:6744C3F6:qmstart:1;rm:root@pam:"));
    }

    #[test]
    fn builds_quoted_scripts() {
        assert_eq!(
            action_script("pve1", "qemu", 100, PveAction::Shutdown).unwrap(),
            "pvesh create '/nodes/pve1/qemu/100/status/shutdown' --output-format json 2>&1"
        );
        assert!(action_script("pve1", "lxc", 200, PveAction::Suspend).is_none());
        assert!(action_script("pve1;x", "qemu", 100, PveAction::Start).is_none());
        assert!(action_script("pve1", "vm", 100, PveAction::Start).is_none());
        let t = task_script("pve2", UPID, 99_999).unwrap();
        assert!(t.contains("--limit 5000") && t.contains(&format!("'/nodes/pve2/tasks/{UPID}/log'")));
        assert!(task_script("pve2", "UPID:x", 10).is_none());
        assert!(overview_script().contains("'/cluster/resources'"));
    }

    #[test]
    fn parses_a_cluster() {
        let o = parse_overview(&out(&[
            ("/version", VERSION),
            ("/cluster/status", STATUS),
            ("/cluster/resources", RESOURCES),
            ("/cluster/tasks", TASKS),
            ("/nodes/local/status", NODE),
        ]));
        assert!(o.present);
        assert_eq!((o.version.as_str(), o.cluster.as_deref(), o.quorate), ("8.2.4", Some("lab"), Some(true)));
        assert_eq!(o.nodes.len(), 2);
        let n = &o.nodes[0];
        assert_eq!((n.name.as_str(), n.local, n.ip.as_deref(), n.maxcpu), ("pve1", true, Some("192.168.1.20"), 16));
        assert!(!o.nodes[1].local);
        assert_eq!(o.guests.iter().map(|g| g.vmid).collect::<Vec<_>>(), [100, 101, 200, 201, 9000]);
        let media = &o.guests[0];
        assert_eq!((media.kind.as_str(), media.status.as_str()), ("qemu", "running"));
        assert_eq!(media.tags, ["media", "prod"]);
        assert!(o.guests[4].template);
        assert_eq!(o.guests[3].lock.as_deref(), Some("backup"));
        assert_eq!(o.guests[2].hastate.as_deref(), Some("started"));
        assert_eq!(o.storages.len(), 5);
        let nas = o.storages.iter().find(|s| s.storage == "nas-backup").unwrap();
        assert!(nas.shared && nas.content == ["backup"]);
        assert_eq!(o.tasks.len(), 3);
        assert!(o.tasks[0].running() && o.tasks[0].kind == "vncproxy");
        assert_eq!(o.tasks[1].status.as_deref(), Some("OK"));
        let l = o.local.unwrap();
        assert_eq!((l.pve_version.as_str(), l.kernel.as_str(), l.threads), ("8.2.4", "6.8.12-1-pve", 16));
        assert_eq!(l.loadavg, [0.52, 0.61, 0.58]);
        assert!(o.notes.is_empty());
    }

    #[test]
    fn keeps_going_when_one_read_fails() {
        let o = parse_overview(&out(&[
            ("/version", VERSION),
            ("/cluster/status", "ipcc_send_rec[1] failed: Permission denied\n"),
            ("/cluster/resources", RESOURCES),
        ]));
        assert_eq!(o.notes.len(), 1);
        assert!(o.notes[0].contains("root"), "{:?}", o.notes);
        assert_eq!(o.guests.len(), 5);
        assert!(!parse_overview("@@nopve\n").present);
        // A single node without a cluster.
        let single = r#"[{"type":"node","node":"pve","status":"online","maxcpu":4}]"#;
        let o = parse_overview(&out(&[("/cluster/status", "[]"), ("/cluster/resources", single)]));
        assert!(o.nodes[0].local && o.cluster.is_none());
    }

    #[test]
    fn parses_upids_and_tasks() {
        assert_eq!(parse_upid(&format!("\"{UPID}\"\n")).unwrap(), UPID);
        assert_eq!(parse_upid(UPID).unwrap(), UPID);
        let e = parse_upid("VM 100 already running\n").unwrap_err();
        assert!(e.contains("already running"));
        let d = parse_task(&out(&[("/nodes/pve2/tasks/x/status", TASK_STATUS), ("/nodes/pve2/tasks/x/log", TASK_LOG)]))
            .unwrap();
        assert!(!d.running);
        assert_eq!(d.exitstatus.as_deref(), Some("job errors"));
        assert_eq!(d.log.len(), 5);
        assert!(d.log[2].starts_with("ERROR"));
        assert!(parse_task("no such task\n").is_err());
    }

    /// The overview script against a fake `pvesh` that answers from the fixtures.
    #[cfg(unix)]
    #[test]
    fn overview_script_runs_with_a_fake_pvesh() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let fx = concat!(env!("CARGO_MANIFEST_DIR"), "/src/proxmox/fixtures");
        let fake = format!(
            "#!/bin/sh\n[ \"$1\" = get ] || exit 2\ncase \"$2\" in\n\
             /version) cat {fx}/version.json;;\n\
             /cluster/status) cat {fx}/cluster_status.json;;\n\
             /cluster/resources) cat {fx}/cluster_resources.json;;\n\
             /cluster/tasks) cat {fx}/cluster_tasks.json;;\n\
             /nodes/*/status) cat {fx}/node_status.json;;\n\
             *) echo \"no such path $2\" >&2; exit 1;;\nesac\n"
        );
        let p = dir.path().join("pvesh");
        std::fs::write(&p, fake).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = format!("{}:{}", dir.path().display(), std::env::var("PATH").unwrap_or_default());
        let run = |script: &str| {
            let o = std::process::Command::new("sh").arg("-c").arg(script).env("PATH", &path).output().unwrap();
            String::from_utf8_lossy(&o.stdout).into_owned()
        };
        let o = parse_overview(&run(&overview_script()));
        assert!(o.present && o.notes.is_empty(), "{:?}", o.notes);
        assert_eq!((o.nodes.len(), o.guests.len(), o.tasks.len()), (2, 5, 3));
        assert!(o.local.is_some());
        let none = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(overview_script())
            .env("PATH", "/nonexistent")
            .output()
            .unwrap();
        assert!(!parse_overview(&String::from_utf8_lossy(&none.stdout)).present);
    }
}
