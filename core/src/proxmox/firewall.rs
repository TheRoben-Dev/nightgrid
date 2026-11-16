//! Proxmox VE part 6 (M10r): the Proxmox firewall at datacenter, node and
//! guest level, security groups and aliases, through the same API the web
//! UI uses. Changes that can block the node itself (datacenter, node and
//! group level) run under a rollback armed on the node, like the network:
//! unless kept, the node restores its previous firewall files by itself.

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{explain, get, s, sections, valid_kind, valid_node, valid_vmid};
use crate::files::path::shell_quote;

pub const ROLLBACK_DIR: &str = "/var/lib/nightgrid/pve-fw-rollback";
pub const ROLLBACK_UNIT: &str = "nightgrid-pve-fw-rollback";

// ---------------------------------------------------------------- types

/// Where rules live: the datacenter, a node, a guest, or a security group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveFwScope {
    /// cluster, node, guest or group.
    pub level: String,
    pub node: String,
    pub kind: String,
    pub vmid: u32,
    pub group: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveFwRule {
    pub pos: u32,
    /// in, out or group.
    pub kind: String,
    /// ACCEPT, DROP, REJECT, or the group's name.
    pub action: String,
    pub r#macro: String,
    pub source: String,
    pub dest: String,
    pub proto: String,
    pub sport: String,
    pub dport: String,
    pub iface: String,
    pub log: String,
    pub enable: bool,
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveFwOptions {
    pub enable: bool,
    pub policy_in: String,
    pub policy_out: String,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveFwNamed {
    pub name: String,
    /// Alias address, empty for groups and IP sets.
    pub cidr: String,
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveFirewall {
    pub rules: Vec<PveFwRule>,
    /// None for security groups, which have no options.
    pub options: Option<PveFwOptions>,
    /// Sent back with edits so a list changed elsewhere is refused.
    pub digest: String,
    pub groups: Vec<PveFwNamed>,
    pub aliases: Vec<PveFwNamed>,
    pub ipsets: Vec<PveFwNamed>,
    pub macros: Vec<PveFwNamed>,
    pub rollback_armed: bool,
    pub notes: Vec<String>,
}

/// A rule to add or change. Empty strings mean "not set".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveFwRuleSpec {
    pub kind: String,
    pub action: String,
    pub r#macro: String,
    pub source: String,
    pub dest: String,
    pub proto: String,
    pub sport: String,
    pub dport: String,
    pub iface: String,
    pub log: String,
    pub enable: bool,
    pub comment: String,
}

// ---------------------------------------------------------------- validation

pub fn valid_group(g: &str) -> bool {
    (2..=18).contains(&g.len())
        && g.starts_with(|c: char| c.is_ascii_alphabetic())
        && g.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn valid_alias(a: &str) -> bool {
    valid_group(a) && a.len() <= 64
}

fn valid_addr(a: &str) -> bool {
    a.len() <= 512 && a.chars().all(|c| c.is_ascii_alphanumeric() || ".:/,+-_!".contains(c))
}

fn valid_ports(p: &str) -> bool {
    p.len() <= 128 && p.chars().all(|c| c.is_ascii_alphanumeric() || ":,-".contains(c))
}

const LOGS: &[&str] = &["emerg", "alert", "crit", "err", "warning", "notice", "info", "debug", "nolog"];
const POLICIES: &[&str] = &["ACCEPT", "DROP", "REJECT"];

fn valid_comment(c: &str) -> bool {
    c.len() <= 200 && !c.chars().any(|c| c.is_control())
}

pub fn check_rule(r: &PveFwRuleSpec) -> Result<(), String> {
    match r.kind.as_str() {
        "in" | "out" if POLICIES.contains(&r.action.as_str()) => {}
        "group" if valid_group(&r.action) => {}
        _ => return Err("direction and action: in/out with ACCEPT, DROP or REJECT, or a security group".into()),
    }
    if !r.r#macro.is_empty() && !(r.r#macro.len() <= 32 && r.r#macro.chars().all(|c| c.is_ascii_alphanumeric())) {
        return Err("invalid macro".into());
    }
    if !valid_addr(&r.source) || !valid_addr(&r.dest) {
        return Err("source/destination: addresses, CIDRs, ranges, aliases or +ipsets".into());
    }
    if !r.proto.is_empty() && !(r.proto.len() <= 16 && r.proto.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) {
        return Err("invalid protocol".into());
    }
    if !valid_ports(&r.sport) || !valid_ports(&r.dport) {
        return Err("ports: numbers, ranges (1000:2000), lists or service names".into());
    }
    if (!r.sport.is_empty() || !r.dport.is_empty()) && r.proto.is_empty() && r.r#macro.is_empty() {
        return Err("ports need a protocol (tcp or udp)".into());
    }
    if !r.iface.is_empty()
        && !(r.iface.len() <= 20 && r.iface.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)))
    {
        return Err("invalid interface".into());
    }
    if !r.log.is_empty() && !LOGS.contains(&r.log.as_str()) {
        return Err("invalid log level".into());
    }
    if !valid_comment(&r.comment) {
        return Err("comment: one line, up to 200 characters".into());
    }
    Ok(())
}

fn valid_digest(d: &str) -> bool {
    d.is_empty() || (d.len() <= 64 && d.bytes().all(|b| b.is_ascii_hexdigit()))
}

pub fn valid_token(t: &str) -> bool {
    !t.is_empty() && t.len() <= 12 && t.bytes().all(|b| b.is_ascii_digit())
}

// ---------------------------------------------------------------- paths

/// The API path of a scope's firewall.
pub fn base(s: &PveFwScope) -> Option<String> {
    match s.level.as_str() {
        "cluster" => Some("/cluster/firewall".into()),
        "node" if valid_node(&s.node) => Some(format!("/nodes/{}/firewall", s.node)),
        "guest" if valid_node(&s.node) && valid_kind(&s.kind) && valid_vmid(s.vmid) => {
            Some(format!("/nodes/{}/{}/{}/firewall", s.node, s.kind, s.vmid))
        }
        "group" if valid_group(&s.group) => Some(format!("/cluster/firewall/groups/{}", s.group)),
        _ => None,
    }
}

fn rules_path(s: &PveFwScope) -> Option<String> {
    let b = base(s)?;
    Some(if s.level == "group" { b } else { format!("{b}/rules") })
}

/// Changes here can block the node itself, so they run under a rollback.
pub fn guarded(s: &PveFwScope) -> bool {
    s.level != "guest"
}

// ---------------------------------------------------------------- scripts

/// Rules and options of one scope, plus the datacenter's groups, aliases,
/// IP sets and macros the rule editor offers, and the rollback state.
pub fn firewall_script(s: &PveFwScope) -> Option<String> {
    let mut parts = vec![get(&rules_path(s)?, "")];
    if s.level != "group" {
        parts.push(get(&format!("{}/options", base(s)?), ""));
    }
    for p in ["groups", "aliases", "ipset", "macros"] {
        parts.push(get(&format!("/cluster/firewall/{p}"), ""));
    }
    parts.push(format!(
        "echo '@@armed'; systemctl is-active --quiet {ROLLBACK_UNIT}.timer 2>/dev/null && echo yes; \
for p in {ROLLBACK_DIR}/*/pid; do [ -f \"$p\" ] && kill -0 \"$(cat \"$p\")\" 2>/dev/null && echo yes; done; true"
    ));
    Some(parts.join("\n"))
}

fn rule_args(r: &PveFwRuleSpec, edit: bool) -> String {
    let mut a = format!(" --type {} --action {}", shell_quote(&r.kind), shell_quote(&r.action));
    let mut del: Vec<&str> = Vec::new();
    for (k, v) in [
        ("macro", &r.r#macro),
        ("source", &r.source),
        ("dest", &r.dest),
        ("proto", &r.proto),
        ("sport", &r.sport),
        ("dport", &r.dport),
        ("iface", &r.iface),
        ("log", &r.log),
        ("comment", &r.comment),
    ] {
        if v.is_empty() {
            if edit {
                del.push(k);
            }
        } else {
            a.push_str(&format!(" --{k} {}", shell_quote(v)));
        }
    }
    a.push_str(&format!(" --enable {}", if r.enable { 1 } else { 0 }));
    if !del.is_empty() {
        a.push_str(&format!(" --delete {}", del.join(",")));
    }
    a
}

/// Adds a rule at the top (pos 0), as the web UI does.
pub fn rule_create_cmd(s: &PveFwScope, r: &PveFwRuleSpec) -> Result<String, String> {
    check_rule(r)?;
    let p = rules_path(s).ok_or("invalid scope")?;
    Ok(format!("pvesh create {}{} --pos 0 --output-format json", shell_quote(&p), rule_args(r, false)))
}

pub fn rule_update_cmd(s: &PveFwScope, pos: u32, digest: &str, r: &PveFwRuleSpec) -> Result<String, String> {
    check_rule(r)?;
    if !valid_digest(digest) || pos > 9999 {
        return Err("invalid rule".into());
    }
    let p = format!("{}/{pos}", rules_path(s).ok_or("invalid scope")?);
    let d = if digest.is_empty() { String::new() } else { format!(" --digest {digest}") };
    Ok(format!("pvesh set {}{}{d} --output-format json", shell_quote(&p), rule_args(r, true)))
}

pub fn rule_move_cmd(s: &PveFwScope, pos: u32, to: u32, digest: &str) -> Result<String, String> {
    if !valid_digest(digest) || pos > 9999 || to > 9999 {
        return Err("invalid rule".into());
    }
    let p = format!("{}/{pos}", rules_path(s).ok_or("invalid scope")?);
    let d = if digest.is_empty() { String::new() } else { format!(" --digest {digest}") };
    Ok(format!("pvesh set {} --moveto {to}{d} --output-format json", shell_quote(&p)))
}

pub fn rule_delete_cmd(s: &PveFwScope, pos: u32, digest: &str) -> Result<String, String> {
    if !valid_digest(digest) || pos > 9999 {
        return Err("invalid rule".into());
    }
    let p = format!("{}/{pos}", rules_path(s).ok_or("invalid scope")?);
    let d = if digest.is_empty() { String::new() } else { format!(" --digest {digest}") };
    Ok(format!("pvesh delete {}{d} --output-format json", shell_quote(&p)))
}

pub fn options_cmd(
    s: &PveFwScope,
    enable: bool,
    policy_in: &str,
    policy_out: &str,
    digest: &str,
) -> Result<String, String> {
    if s.level == "group" {
        return Err("security groups have no options".into());
    }
    let ok = |p: &str| p.is_empty() || POLICIES.contains(&p);
    if !ok(policy_in) || !ok(policy_out) || !valid_digest(digest) {
        return Err("policy: ACCEPT, DROP or REJECT".into());
    }
    let mut c = format!(
        "pvesh set {} --enable {}",
        shell_quote(&format!("{}/options", base(s).ok_or("invalid scope")?)),
        u8::from(enable)
    );
    // Guests and the datacenter have both policies; nodes have none.
    if s.level != "node" {
        for (k, v) in [("policy_in", policy_in), ("policy_out", policy_out)] {
            if !v.is_empty() {
                c.push_str(&format!(" --{k} {v}"));
            }
        }
    }
    if !digest.is_empty() {
        c.push_str(&format!(" --digest {digest}"));
    }
    c.push_str(" --output-format json");
    Ok(c)
}

pub fn group_create_cmd(group: &str, comment: &str) -> Result<String, String> {
    if !valid_group(group) || !valid_comment(comment) {
        return Err("group: a letter, then letters, digits, - and _ (2 to 18)".into());
    }
    let mut c = format!("pvesh create /cluster/firewall/groups --group {}", shell_quote(group));
    if !comment.is_empty() {
        c.push_str(&format!(" --comment {}", shell_quote(comment)));
    }
    Ok(c + " --output-format json")
}

pub fn group_delete_cmd(group: &str) -> Result<String, String> {
    valid_group(group)
        .then(|| {
            format!("pvesh delete {} --output-format json", shell_quote(&format!("/cluster/firewall/groups/{group}")))
        })
        .ok_or_else(|| "invalid group".into())
}

pub fn alias_create_cmd(name: &str, cidr: &str, comment: &str) -> Result<String, String> {
    if !valid_alias(name)
        || cidr.is_empty()
        || !valid_addr(cidr)
        || cidr.contains([',', '+', '!'])
        || !valid_comment(comment)
    {
        return Err("alias: a name and one address or CIDR".into());
    }
    let mut c =
        format!("pvesh create /cluster/firewall/aliases --name {} --cidr {}", shell_quote(name), shell_quote(cidr));
    if !comment.is_empty() {
        c.push_str(&format!(" --comment {}", shell_quote(comment)));
    }
    Ok(c + " --output-format json")
}

pub fn alias_delete_cmd(name: &str) -> Result<String, String> {
    valid_alias(name)
        .then(|| {
            format!("pvesh delete {} --output-format json", shell_quote(&format!("/cluster/firewall/aliases/{name}")))
        })
        .ok_or_else(|| "invalid alias".into())
}

const RESTORE: &str =
    "D=$(dirname \"$0\"); for f in cluster host; do t=$(cat \"$D/$f.path\" 2>/dev/null) || continue; \
if [ -f \"$D/$f.fw\" ]; then cp \"$D/$f.fw\" \"$t\"; elif [ -f \"$D/$f.absent\" ]; then rm -f \"$t\"; fi; done; \
pve-firewall restart >/dev/null 2>&1; rm -f \"$D/pid\"; echo restored\n";

/// Runs `cmd` directly (guest level) or under a rollback armed on `local`
/// (the node this host is). The first change of a session saves the firewall
/// files; later changes under the same token keep that first copy and only
/// restart the timer, so Keep or the rollback covers the whole session.
pub fn run_script(scope: &PveFwScope, local: &str, token: &str, secs: u32, cmd: &str) -> Option<String> {
    if !guarded(scope) {
        return Some(format!("{cmd} 2>&1 && echo @@ok"));
    }
    if !valid_node(local) || !valid_token(token) {
        return None;
    }
    let restore = base64::engine::general_purpose::STANDARD.encode(RESTORE);
    let secs = secs.clamp(30, 900);
    Some(format!(
        "umask 077; D={ROLLBACK_DIR}/{token}; fresh=0; if [ ! -d \"$D\" ]; then fresh=1; mkdir -p \"$D\" || exit 2; \
(cd {ROLLBACK_DIR} && ls -1t | tail -n +6 | while IFS= read -r f; do [ \"$f\" = {token} ] || rm -rf -- \"$f\"; done); \
for p in cluster:/etc/pve/firewall/cluster.fw host:/etc/pve/nodes/{local}/host.fw; do f=${{p%%:*}}; t=${{p#*:}}; echo \"$t\" > \"$D/$f.path\"; \
if [ -f \"$t\" ]; then cp \"$t\" \"$D/$f.fw\" || exit 2; else touch \"$D/$f.absent\"; fi; done; \
printf '%s' '{restore}' | base64 -d > \"$D/restore.sh\"; fi; \
if command -v systemd-run >/dev/null 2>&1 && [ -d /run/systemd/system ]; then \
systemctl stop {ROLLBACK_UNIT}.timer >/dev/null 2>&1; systemctl reset-failed {ROLLBACK_UNIT}.timer {ROLLBACK_UNIT}.service >/dev/null 2>&1; \
systemd-run --quiet --unit={ROLLBACK_UNIT} --on-active={secs}s /bin/sh \"$D/restore.sh\" && echo '@@armed systemd' || {{ echo 'could not arm the rollback' >&2; exit 2; }}; \
else [ -f \"$D/pid\" ] && kill \"$(cat \"$D/pid\")\" 2>/dev/null; setsid nohup sh -c \"sleep {secs}; sh '$D/restore.sh'\" >/dev/null 2>&1 < /dev/null & echo $! > \"$D/pid\"; echo '@@armed sleep'; fi; \
if o=$({cmd} 2>&1); then echo \"$o\"; echo @@ok; else echo \"$o\" >&2; \
if [ $fresh = 1 ]; then sh \"$D/restore.sh\" >/dev/null 2>&1; systemctl stop {ROLLBACK_UNIT}.timer >/dev/null 2>&1; rm -rf -- \"$D\"; echo @@disarmed; fi; echo @@failed; fi"
    ))
}

/// Keeps the session's changes: disarms the rollback.
pub fn confirm_script(token: &str) -> Option<String> {
    valid_token(token).then(|| {
        format!(
            "D={ROLLBACK_DIR}/{token}; systemctl stop {ROLLBACK_UNIT}.timer >/dev/null 2>&1; \
if [ -f \"$D/pid\" ]; then kill \"$(cat \"$D/pid\")\" 2>/dev/null; rm -f \"$D/pid\"; fi; rm -rf -- \"$D\"; echo @@ok"
        )
    })
}

/// Restores the files saved at the session's first change, now.
pub fn rollback_script(token: &str) -> Option<String> {
    valid_token(token).then(|| {
        format!(
            "D={ROLLBACK_DIR}/{token}; [ -f \"$D/restore.sh\" ] || {{ echo 'no saved firewall to restore' >&2; exit 2; }}; \
systemctl stop {ROLLBACK_UNIT}.timer >/dev/null 2>&1; if [ -f \"$D/pid\" ]; then kill \"$(cat \"$D/pid\")\" 2>/dev/null; fi; \
sh \"$D/restore.sh\" && rm -rf -- \"$D\" && echo @@ok"
        )
    })
}

/// What a change printed: whether a rollback is armed, or why it failed.
pub fn parse_run(out: &str) -> Result<bool, String> {
    let armed = out.lines().any(|l| l.starts_with("@@armed"));
    if out.lines().any(|l| l.trim() == "@@ok") {
        return Ok(armed);
    }
    let why: String = out.lines().filter(|l| !l.starts_with("@@")).collect::<Vec<_>>().join("\n");
    let disarmed = out.lines().any(|l| l.trim() == "@@disarmed");
    Err(if disarmed { format!("{} (nothing was changed)", explain(&why)) } else { explain(&why) })
}

// ---------------------------------------------------------------- parsing

fn named(v: &Value, key: &str) -> Vec<PveFwNamed> {
    let mut out: Vec<PveFwNamed> = v
        .as_array()
        .map(|a| {
            a.iter()
                .map(|i| PveFwNamed {
                    name: s(i, key),
                    cidr: s(i, "cidr"),
                    comment: s(i, if key == "macro" { "descr" } else { "comment" }),
                })
                .filter(|n| !n.name.is_empty())
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

fn flag(v: &Value, k: &str) -> bool {
    matches!(s(v, k).as_str(), "1" | "true") || v[k].as_bool() == Some(true)
}

pub fn parse_firewall(out: &str, level: &str) -> Result<PveFirewall, String> {
    let mut fw = PveFirewall::default();
    let mut got_rules = false;
    for (path, body) in sections(out) {
        let body = body.split("@@armed").next().unwrap_or(&body).trim().to_owned();
        let Ok(v) = serde_json::from_str::<Value>(&body) else {
            if path.ends_with("/rules") || (level == "group" && path.contains("/groups/")) {
                return Err(explain(&body));
            }
            fw.notes.push(format!("{path}: {}", explain(&body)));
            continue;
        };
        if path.ends_with("/options") {
            fw.options = Some(PveFwOptions {
                enable: flag(&v, "enable"),
                policy_in: s(&v, "policy_in"),
                policy_out: s(&v, "policy_out"),
                digest: s(&v, "digest"),
            });
        } else if path.ends_with("/rules") || (level == "group" && path.contains("/groups/")) {
            got_rules = true;
            let items = v.as_array().cloned().unwrap_or_default();
            fw.digest = items.first().map(|i| s(i, "digest")).unwrap_or_default();
            fw.rules = items
                .iter()
                .map(|i| PveFwRule {
                    pos: i["pos"].as_u64().unwrap_or(0) as u32,
                    kind: s(i, "type"),
                    action: s(i, "action"),
                    r#macro: s(i, "macro"),
                    source: s(i, "source"),
                    dest: s(i, "dest"),
                    proto: s(i, "proto"),
                    sport: s(i, "sport"),
                    dport: s(i, "dport"),
                    iface: s(i, "iface"),
                    log: s(i, "log"),
                    enable: flag(i, "enable"),
                    comment: s(i, "comment"),
                })
                .collect();
            fw.rules.sort_by_key(|r| r.pos);
        } else if path.ends_with("/groups") {
            fw.groups = named(&v, "group");
        } else if path.ends_with("/aliases") {
            fw.aliases = named(&v, "name");
        } else if path.ends_with("/ipset") {
            fw.ipsets = named(&v, "name");
        } else if path.ends_with("/macros") {
            fw.macros = named(&v, "macro");
        }
    }
    if !got_rules {
        return Err(explain(out));
    }
    fw.rollback_armed = out.split_once("@@armed").is_some_and(|(_, r)| r.lines().any(|l| l.trim() == "yes"));
    Ok(fw)
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxmox::MARK;

    fn scope(level: &str) -> PveFwScope {
        PveFwScope {
            level: level.into(),
            node: "pve1".into(),
            kind: "qemu".into(),
            vmid: 100,
            group: "webservers".into(),
        }
    }

    fn rule() -> PveFwRuleSpec {
        PveFwRuleSpec {
            kind: "in".into(),
            action: "ACCEPT".into(),
            r#macro: String::new(),
            source: "192.168.1.0/24".into(),
            dest: String::new(),
            proto: "tcp".into(),
            sport: String::new(),
            dport: "443".into(),
            iface: String::new(),
            log: String::new(),
            enable: true,
            comment: "web".into(),
        }
    }

    #[test]
    fn reads_rules_options_and_lists() {
        let out = [
            format!("{MARK} /cluster/firewall/rules\n{}\n", include_str!("fixtures/fw_rules.json")),
            format!("{MARK} /cluster/firewall/options\n{}\n", include_str!("fixtures/fw_options.json")),
            format!("{MARK} /cluster/firewall/groups\n{}\n", include_str!("fixtures/fw_groups.json")),
            format!("{MARK} /cluster/firewall/aliases\n{}\n", include_str!("fixtures/fw_aliases.json")),
            format!("{MARK} /cluster/firewall/ipset\n{}\n", include_str!("fixtures/fw_ipsets.json")),
            format!("{MARK} /cluster/firewall/macros\n{}\n@@armed\nyes\n", include_str!("fixtures/fw_macros.json")),
        ]
        .join("");
        let fw = parse_firewall(&out, "cluster").unwrap();
        assert_eq!(fw.rules.len(), 4);
        assert_eq!(fw.digest, "5c1e9a");
        let r0 = &fw.rules[0];
        assert_eq!(
            (r0.kind.as_str(), r0.action.as_str(), r0.r#macro.as_str(), r0.source.as_str(), r0.enable),
            ("in", "ACCEPT", "SSH", "+management", true)
        );
        assert_eq!(
            (fw.rules[2].kind.as_str(), fw.rules[2].action.as_str(), fw.rules[2].enable),
            ("group", "webservers", false)
        );
        let o = fw.options.as_ref().unwrap();
        assert_eq!((o.enable, o.policy_in.as_str(), o.policy_out.as_str()), (true, "DROP", "ACCEPT"));
        assert_eq!(fw.groups.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), ["monitoring", "webservers"]);
        assert_eq!(fw.aliases[0].cidr, "192.168.1.0/24");
        assert_eq!(fw.ipsets.len(), 2);
        assert_eq!(fw.macros.iter().find(|m| m.name == "SSH").unwrap().comment, "Secure shell traffic");
        assert!(fw.rollback_armed);
        assert!(
            parse_firewall(&format!("{MARK} /cluster/firewall/rules\nPermission check failed\n"), "cluster").is_err()
        );
    }

    #[test]
    fn scopes_map_to_api_paths() {
        assert_eq!(base(&scope("cluster")).unwrap(), "/cluster/firewall");
        assert_eq!(base(&scope("node")).unwrap(), "/nodes/pve1/firewall");
        assert_eq!(base(&scope("guest")).unwrap(), "/nodes/pve1/qemu/100/firewall");
        assert_eq!(rules_path(&scope("group")).unwrap(), "/cluster/firewall/groups/webservers");
        assert!(base(&PveFwScope { group: "x;y".into(), ..scope("group") }).is_none());
        assert!(base(&scope("bogus")).is_none());
        let s = firewall_script(&scope("group")).unwrap();
        assert!(!s.contains("/options"));
        assert!(firewall_script(&scope("guest")).unwrap().contains("/nodes/pve1/qemu/100/firewall/options"));
        assert!(guarded(&scope("node")) && guarded(&scope("group")) && !guarded(&scope("guest")));
    }

    #[test]
    fn builds_rule_and_option_commands() {
        let c = rule_create_cmd(&scope("guest"), &rule()).unwrap();
        assert_eq!(
            c,
            "pvesh create '/nodes/pve1/qemu/100/firewall/rules' --type 'in' --action 'ACCEPT' --source '192.168.1.0/24' --proto 'tcp' --dport '443' --comment 'web' --enable 1 --pos 0 --output-format json"
        );
        let u = rule_update_cmd(&scope("cluster"), 3, "5c1e9a", &rule()).unwrap();
        assert!(u.starts_with("pvesh set '/cluster/firewall/rules/3' --type 'in'"));
        assert!(u.contains("--delete macro,dest,sport,iface,log --digest 5c1e9a"));
        assert!(rule_move_cmd(&scope("node"), 3, 0, "5c1e9a").unwrap().contains("--moveto 0 --digest 5c1e9a"));
        assert!(rule_delete_cmd(&scope("group"), 1, "")
            .unwrap()
            .starts_with("pvesh delete '/cluster/firewall/groups/webservers/1'"));
        let mut bad = rule();
        bad.action = "ALLOW".into();
        assert!(check_rule(&bad).is_err());
        bad = rule();
        bad.source = "1.2.3.4; reboot".into();
        assert!(check_rule(&bad).is_err());
        bad = rule();
        bad.proto.clear();
        assert!(check_rule(&bad).is_err(), "ports without protocol");
        bad.r#macro = "HTTPS".into();
        bad.dport.clear();
        assert!(check_rule(&bad).is_ok());
        let g = PveFwRuleSpec { kind: "group".into(), action: "webservers".into(), ..rule() };
        assert!(check_rule(&g).is_ok());
        assert_eq!(
            options_cmd(&scope("cluster"), true, "DROP", "ACCEPT", "77aa01").unwrap(),
            "pvesh set '/cluster/firewall/options' --enable 1 --policy_in DROP --policy_out ACCEPT --digest 77aa01 --output-format json"
        );
        assert!(!options_cmd(&scope("node"), true, "DROP", "", "").unwrap().contains("policy"));
        assert!(options_cmd(&scope("group"), true, "", "", "").is_err());
        assert!(options_cmd(&scope("cluster"), true, "ALLOW", "", "").is_err());
        assert!(group_create_cmd("web-servers", "").is_ok() && group_create_cmd("1bad", "").is_err());
        assert!(alias_create_cmd("lan", "192.168.1.0/24", "home")
            .unwrap()
            .contains("--cidr '192.168.1.0/24' --comment 'home'"));
        assert!(alias_create_cmd("lan", "+set", "").is_err());
        assert!(alias_delete_cmd("lan").unwrap().contains("/cluster/firewall/aliases/lan"));
    }

    #[test]
    fn guards_host_level_changes_with_a_rollback() {
        let cmd = rule_create_cmd(&scope("cluster"), &rule()).unwrap();
        assert_eq!(run_script(&scope("guest"), "pve1", "1", 90, "true").unwrap(), "true 2>&1 && echo @@ok");
        let s = run_script(&scope("cluster"), "pve1", "1700000000", 90, &cmd).unwrap();
        assert!(s.contains("D=/var/lib/nightgrid/pve-fw-rollback/1700000000; fresh=0; if [ ! -d \"$D\" ]"));
        assert!(s.contains("/etc/pve/nodes/pve1/host.fw"));
        assert!(s.contains("--on-active=90s"));
        assert!(s.contains(&format!("if o=$({cmd} 2>&1)")));
        assert!(run_script(&scope("node"), "pve1", "x", 90, "true").is_none());
        assert!(run_script(&scope("node"), "bad node", "1", 90, "true").is_none());
        assert!(parse_run("@@armed systemd\n\n@@ok").unwrap());
        assert!(!parse_run("\n@@ok").unwrap());
        assert!(parse_run("@@armed systemd\nrule: invalid\n@@disarmed\n@@failed")
            .unwrap_err()
            .contains("nothing was changed"));
        assert!(confirm_script("1700000000").unwrap().contains("rm -rf -- \"$D\""));
        assert!(rollback_script("1700000000").unwrap().contains("sh \"$D/restore.sh\""));
        // The generated shell parses.
        for script in [
            s,
            confirm_script("1").unwrap(),
            rollback_script("1").unwrap(),
            firewall_script(&scope("cluster")).unwrap(),
        ] {
            let ok = std::process::Command::new("/bin/sh").arg("-n").arg("-c").arg(&script).status().unwrap().success();
            assert!(ok, "{script}");
        }
    }
}
