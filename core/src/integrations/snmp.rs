//! **SNMP** (read-only): health, disks and volumes from any SNMP agent, with
//! Synology's own MIB when the agent has it (system, fans, power, disks with
//! health, storage pools) and the standard HOST-RESOURCES / UCD tables
//! otherwise (D-199). The no-admin way to watch a NAS: DSM's SNMP user isn't
//! a DSM account at all.
//!
//! Address: `snmp://host[:port]` (161 by default). Login name empty = v2c
//! with the community in the secret field; a login name = v3 with the
//! authentication password, and optionally a newline and the privacy
//! password, in the secret. `slug` holds the v3 protocols (`sha-aes`, ...).

use std::collections::HashMap;
use std::time::Duration;

use super::app::{bytes, cap_rows, duration, plural, AppHealth, AppRow, AppRowState, AppStatus};
use super::Integration;
use crate::snmp::{oid, oid_str, AuthProto, Client, Oid, PrivProto, Security, Value};

/// v3 protocol choices, as stored in `slug`. The first is the default.
pub const PROTOCOLS: [&str; 6] = ["sha-aes", "sha-des", "sha", "md5-aes", "md5-des", "md5"];

/// `host[:port]`, `snmp://host[:port]` → `snmp://host[:port]`.
pub fn normalize_address(raw: &str) -> Result<String, &'static str> {
    let t = raw.trim().trim_end_matches('/');
    let rest = match t.split_once("://") {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("snmp") => rest,
        Some(_) => return Err("use snmp://host or just the host name"),
        None => t,
    };
    if rest.is_empty() {
        return Err("missing host");
    }
    if rest.contains(['/', '@', '?', '#']) || rest.chars().any(char::is_whitespace) {
        return Err("just the host name or address, with an optional :port");
    }
    let (host, port) = split_host_port(rest)?;
    Ok(if port == 161 { format!("snmp://{host}") } else { format!("snmp://{host}:{port}") })
}

fn split_host_port(rest: &str) -> Result<(String, u16), &'static str> {
    if let Some(v6) = rest.strip_prefix('[') {
        let (h, after) = v6.split_once(']').ok_or("unclosed [")?;
        let port = match after.strip_prefix(':') {
            Some(p) => p.parse().map_err(|_| "the port must be a number")?,
            None => 161,
        };
        return Ok((format!("[{h}]"), port));
    }
    match rest.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') => Ok((h.to_owned(), p.parse().map_err(|_| "the port must be a number")?)),
        _ => Ok((rest.to_owned(), 161)),
    }
}

/// How to log in, from the saved fields.
pub fn security(i: &Integration, secret: &str) -> Result<Security, String> {
    if i.username.is_empty() {
        let community = secret.trim();
        if community.is_empty() {
            return Err("enter the SNMP community name".into());
        }
        return Ok(Security::V2c { community: community.to_owned() });
    }
    let mut parts = secret.splitn(2, '\n');
    let auth_pass = parts.next().unwrap_or("").trim().to_owned();
    let priv_pass = parts.next().unwrap_or("").trim().to_owned();
    if auth_pass.is_empty() {
        return Err("enter the SNMPv3 password".into());
    }
    let proto = if i.slug.is_empty() { PROTOCOLS[0] } else { i.slug.as_str() };
    let (auth, privacy) = proto.split_once('-').unwrap_or((proto, ""));
    let auth = if auth == "md5" { AuthProto::Md5 } else { AuthProto::Sha };
    let privacy = match privacy {
        "" => None,
        p => {
            let pass = if priv_pass.is_empty() { auth_pass.clone() } else { priv_pass };
            Some((if p == "des" { PrivProto::Des } else { PrivProto::Aes }, pass))
        }
    };
    Ok(Security::V3 { user: i.username.clone(), auth, auth_pass, privacy })
}

const SYS_DESCR: &str = "1.3.6.1.2.1.1.1.0";
const SYS_UPTIME: &str = "1.3.6.1.2.1.1.3.0";
const SYS_NAME: &str = "1.3.6.1.2.1.1.5.0";
const SYNO_STATUS: &str = "1.3.6.1.4.1.6574.1.1.0";
const SYNO_TEMP: &str = "1.3.6.1.4.1.6574.1.2.0";
const SYNO_POWER: &str = "1.3.6.1.4.1.6574.1.3.0";
const SYNO_SYS_FAN: &str = "1.3.6.1.4.1.6574.1.4.1.0";
const SYNO_CPU_FAN: &str = "1.3.6.1.4.1.6574.1.4.2.0";
const SYNO_MODEL: &str = "1.3.6.1.4.1.6574.1.5.1.0";
const SYNO_VERSION: &str = "1.3.6.1.4.1.6574.1.5.3.0";
const SYNO_UPGRADE: &str = "1.3.6.1.4.1.6574.1.5.4.0";
const CPU_IDLE: &str = "1.3.6.1.4.1.2021.11.11.0";
const MEM_TOTAL: &str = "1.3.6.1.4.1.2021.4.5.0";
const MEM_AVAIL: &str = "1.3.6.1.4.1.2021.4.6.0";
const MEM_BUFFER: &str = "1.3.6.1.4.1.2021.4.14.0";
const MEM_CACHED: &str = "1.3.6.1.4.1.2021.4.15.0";
const SCALARS: [&str; 16] = [
    SYS_DESCR,
    SYS_UPTIME,
    SYS_NAME,
    SYNO_STATUS,
    SYNO_TEMP,
    SYNO_POWER,
    SYNO_SYS_FAN,
    SYNO_CPU_FAN,
    SYNO_MODEL,
    SYNO_VERSION,
    SYNO_UPGRADE,
    CPU_IDLE,
    MEM_TOTAL,
    MEM_AVAIL,
    MEM_BUFFER,
    MEM_CACHED,
];
const SYNO_DISKS: &str = "1.3.6.1.4.1.6574.2.1.1";
const SYNO_RAID: &str = "1.3.6.1.4.1.6574.3.1.1";
const HR_STORAGE: &str = "1.3.6.1.2.1.25.2.3.1";
const HR_FIXED_DISK: &str = "1.3.6.1.2.1.25.2.1.4";

/// A walked table: row index → column → value.
pub type Table = HashMap<u32, HashMap<u32, Value>>;

/// Groups `root.col.index` rows (single-number indexes only).
pub fn table(root: &str, rows: &[(Oid, Value)]) -> Table {
    let root = oid(root);
    let mut t: Table = HashMap::new();
    for (o, v) in rows {
        if o.len() == root.len() + 2 && o.starts_with(&root) {
            t.entry(o[root.len() + 1]).or_default().insert(o[root.len()], v.clone());
        }
    }
    t
}

fn sorted(t: &Table) -> Vec<(&u32, &HashMap<u32, Value>)> {
    let mut v: Vec<_> = t.iter().collect();
    v.sort_by_key(|(k, _)| **k);
    v
}

fn col_text(r: &HashMap<u32, Value>, c: u32) -> String {
    r.get(&c).map(Value::text).unwrap_or_default()
}

fn col_num(r: &HashMap<u32, Value>, c: u32) -> Option<f64> {
    r.get(&c).and_then(Value::as_f64)
}

fn failed(v: Option<&Value>) -> bool {
    v.and_then(Value::as_i64) == Some(2)
}

const RAID_STATES: [&str; 22] = [
    "",
    "normal",
    "repairing",
    "migrating",
    "expanding",
    "deleting",
    "creating",
    "syncing",
    "parity checking",
    "assembling",
    "canceling",
    "degraded",
    "crashed",
    "data scrubbing",
    "deploying",
    "undeploying",
    "mount cache",
    "unmount cache",
    "expanding unfinished SHR",
    "converting SHR to pool",
    "migrating SHR1 to SHR2",
    "unknown",
];

/// Builds the card from what the agent shared.
pub fn build(scalars: &HashMap<String, Value>, disks: &Table, raids: &Table, storage: &Table) -> AppStatus {
    let get = |k: &str| scalars.get(k).filter(|v| !matches!(v, Value::NoSuch | Value::Null | Value::EndOfMib));
    let synology = get(SYNO_STATUS).is_some() || !disks.is_empty() || !raids.is_empty();
    let mut rows: Vec<AppRow> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut health = AppHealth::Ok;

    for (idx, r) in sorted(raids) {
        let name = Some(col_text(r, 2)).filter(|n| !n.is_empty()).unwrap_or_else(|| format!("Pool {idx}"));
        let code = col_num(r, 3).unwrap_or(0.0) as usize;
        let status = RAID_STATES.get(code).copied().unwrap_or("unknown");
        let (free, total) = (col_num(r, 4).unwrap_or(0.0), col_num(r, 5).unwrap_or(0.0));
        let state = match code {
            1 | 13 => AppRowState::Ok,
            11 | 12 => AppRowState::Bad,
            _ => AppRowState::Warn,
        };
        if state != AppRowState::Ok {
            notes.push(format!("{name}: {status}"));
            health = health.max(if state == AppRowState::Bad { AppHealth::Bad } else { AppHealth::Warn });
        }
        rows.push(usage(format!("raid{idx}"), name, total - free, total, status, state));
    }

    // Volumes from HOST-RESOURCES: on DSM the /volumeN mounts; elsewhere
    // every fixed disk except boot and system mounts.
    let fixed: Vec<(&u32, &HashMap<u32, Value>)> = sorted(storage)
        .into_iter()
        .filter(|(_, r)| matches!(r.get(&2), Some(Value::Oid(o)) if oid_str(o) == HR_FIXED_DISK))
        .collect();
    let has_volumes = fixed.iter().any(|(_, r)| col_text(r, 3).starts_with("/volume"));
    for (idx, r) in fixed {
        let path = col_text(r, 3);
        let keep = if has_volumes {
            path.starts_with("/volume") && !path.contains("/@")
        } else {
            !path.is_empty() && !["/boot", "/sys", "/proc", "/dev", "/run", "/snap"].iter().any(|p| path.starts_with(p))
        };
        if !keep {
            continue;
        }
        let unit = col_num(r, 4).unwrap_or(1.0).max(1.0);
        let (total, used) = (col_num(r, 5).unwrap_or(0.0) * unit, col_num(r, 6).unwrap_or(0.0) * unit);
        if total <= 0.0 {
            continue;
        }
        let pct = used / total * 100.0;
        let state = if pct >= 95.0 {
            AppRowState::Bad
        } else if pct >= 90.0 {
            AppRowState::Warn
        } else {
            AppRowState::Ok
        };
        if state != AppRowState::Ok {
            notes.push(format!("{path} is {pct:.0} % full"));
            health = health.max(AppHealth::Warn);
        }
        rows.push(usage(format!("hr{idx}"), path, used, total, "", state));
    }

    let mut bad_disks = 0u64;
    for (_, r) in sorted(disks) {
        let name = Some(col_text(r, 2)).filter(|n| !n.is_empty()).unwrap_or_else(|| col_text(r, 12));
        let status = col_num(r, 5).unwrap_or(1.0) as i64;
        let health_code = col_num(r, 13).unwrap_or(1.0) as i64;
        let problem = match (status, health_code) {
            (4, _) => Some("system partition failed".to_owned()),
            (5, _) => Some("crashed".to_owned()),
            (_, 2) => Some("health warning".to_owned()),
            (_, 3) => Some("health critical".to_owned()),
            (_, 4) => Some("failing".to_owned()),
            _ => None,
        };
        if let Some(p) = problem {
            bad_disks += 1;
            let model = col_text(r, 3);
            notes.push(if model.is_empty() { format!("{name}: {p}") } else { format!("{name} ({model}): {p}") });
            health =
                health.max(if matches!(status, 4 | 5) || health_code >= 3 { AppHealth::Bad } else { AppHealth::Warn });
        }
    }

    let summary = if synology {
        format!(
            "{} · {}{}",
            if raids.is_empty() { plural(rows.len() as u64, "volume") } else { plural(raids.len() as u64, "pool") },
            plural(disks.len() as u64, "disk"),
            if bad_disks > 0 { format!(" ({bad_disks} with problems)") } else { String::new() }
        )
    } else {
        plural(rows.len() as u64, "volume")
    };
    let mut st = AppStatus::new(summary);
    if synology {
        st = st.tile("Disks", disks.len());
        if let Some(t) = get(SYNO_TEMP).and_then(Value::as_f64) {
            st = st.tile("Temp", format!("{t:.0} °C"));
        }
        if failed(get(SYNO_STATUS)) {
            notes.insert(0, "DSM reports a system failure".into());
            health = AppHealth::Bad;
        }
        if failed(get(SYNO_POWER)) {
            notes.insert(0, "power supply failed".into());
            health = AppHealth::Bad;
        }
        for (k, what) in [(SYNO_SYS_FAN, "system fan"), (SYNO_CPU_FAN, "CPU fan")] {
            if failed(get(k)) {
                notes.push(format!("{what} failed"));
                health = health.max(AppHealth::Warn);
            }
        }
        if get(SYNO_UPGRADE).and_then(Value::as_i64) == Some(1) {
            notes.push("a DSM update is available".into());
        }
    }
    if let Some(idle) = get(CPU_IDLE).and_then(Value::as_f64) {
        st = st.tile("CPU", format!("{:.0} %", (100.0 - idle).clamp(0.0, 100.0)));
    }
    if let (Some(total), Some(avail)) = (get(MEM_TOTAL).and_then(Value::as_f64), get(MEM_AVAIL).and_then(Value::as_f64))
    {
        let cache = get(MEM_BUFFER).and_then(Value::as_f64).unwrap_or(0.0)
            + get(MEM_CACHED).and_then(Value::as_f64).unwrap_or(0.0);
        if total > 0.0 {
            let used = (total - avail - cache).max(0.0);
            st = st.tile("Memory", format!("{:.0} %", used / total * 100.0));
        }
    }
    if let Some(up) = get(SYS_UPTIME).and_then(Value::as_f64) {
        st = st.tile("Up", duration(up / 100.0));
    }
    let model = get(SYNO_MODEL).map(Value::text).unwrap_or_default();
    let version = get(SYNO_VERSION).map(Value::text).unwrap_or_default();
    st.version = match (version.is_empty(), model.is_empty()) {
        (false, false) => Some(format!("{version} ({model})")),
        (false, true) => Some(version),
        (true, false) => Some(model),
        (true, true) => get(SYS_DESCR).map(|d| d.text().chars().take(60).collect()),
    };
    st.health = health;
    st.notes = notes;
    st.more = cap_rows(&mut rows);
    st.rows_title = if raids.is_empty() { "Volumes".into() } else { "Storage".into() };
    st.rows = rows;
    st
}

fn usage(id: String, name: String, used: f64, total: f64, status: &str, state: AppRowState) -> AppRow {
    let pct = if total > 0.0 { used / total * 100.0 } else { 0.0 };
    let mut meta = Vec::new();
    if !status.is_empty() {
        meta.push(status.to_owned());
    }
    if total > 0.0 {
        meta.push(format!("{} of {} used ({pct:.0} %)", bytes(used), bytes(total)));
    }
    AppRow::new(id, name, meta.join(" · "), state).progress((total > 0.0).then_some(pct))
}

pub async fn status(i: &Integration, secret: &str) -> Result<AppStatus, String> {
    let addr = i.base_url.strip_prefix("snmp://").ok_or("the address must look like snmp://nas.lan")?;
    let (host, port) = split_host_port(addr).map_err(str::to_owned)?;
    let host = host.trim_matches(['[', ']']).to_owned();
    let sec = security(i, secret)?;
    let mut c = Client::connect(&host, port, sec, Duration::from_secs(3)).await.map_err(|e| udp_hint(&host, e))?;
    let oids: Vec<Oid> = SCALARS.iter().map(|s| oid(s)).collect();
    let got = c.get(&oids).await.map_err(|e| udp_hint(&host, e))?;
    let scalars: HashMap<String, Value> = got.into_iter().map(|(o, v)| (oid_str(&o), v)).collect();
    if scalars.values().all(|v| matches!(v, Value::NoSuch)) {
        return Err("the SNMP agent answered but shares nothing Nightgrid reads (check its view settings)".into());
    }
    let synology = scalars.get(SYNO_STATUS).is_some_and(|v| !matches!(v, Value::NoSuch));
    let (disks, raids) = if synology {
        let d = c.walk(&oid(SYNO_DISKS), 2000).await.unwrap_or_default();
        let r = c.walk(&oid(SYNO_RAID), 1000).await.unwrap_or_default();
        (table(SYNO_DISKS, &d), table(SYNO_RAID, &r))
    } else {
        (Table::new(), Table::new())
    };
    let hr = c.walk(&oid(HR_STORAGE), 3000).await.unwrap_or_default();
    Ok(build(&scalars, &disks, &raids, &table(HR_STORAGE, &hr)))
}

/// SNMP is UDP: the built-in Tailscale client carries TCP only.
fn udp_hint(host: &str, e: String) -> String {
    if e.starts_with("no SNMP answer") && crate::tailnet::route(host) != crate::tailnet::Route::Direct {
        format!(
            "{e}. SNMP uses UDP, which Nightgrid's built-in Tailscale can't carry: use the LAN or the Tailscale app"
        )
    } else {
        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::IntegrationKind;

    fn row(root: &str, col: u32, idx: u32, v: Value) -> (Oid, Value) {
        (oid(&format!("{root}.{col}.{idx}")), v)
    }

    #[test]
    fn ui_lists_the_same_protocols() {
        let ts = include_str!("../../../ui/src/lib/stores/integrations.svelte.ts");
        for p in PROTOCOLS {
            assert!(ts.contains(&format!("{{ id: \"{p}\"")), "{p}");
        }
    }

    #[test]
    fn addresses() {
        assert_eq!(normalize_address("nas.lan").unwrap(), "snmp://nas.lan");
        assert_eq!(normalize_address("SNMP://nas.lan:161/").unwrap(), "snmp://nas.lan");
        assert_eq!(normalize_address("10.0.0.5:1161").unwrap(), "snmp://10.0.0.5:1161");
        assert_eq!(normalize_address("[fd00::5]:162").unwrap(), "snmp://[fd00::5]:162");
        assert!(normalize_address("http://nas").is_err());
        assert!(normalize_address("nas:abc").is_err());
        assert!(normalize_address("user@nas").is_err());
    }

    #[test]
    fn security_from_fields() {
        let mut i = crate::integrations::app::testing::integration(IntegrationKind::Snmp, "snmp://nas");
        assert!(matches!(security(&i, "public").unwrap(), Security::V2c { .. }));
        assert!(security(&i, " ").is_err());
        i.username = "nightgrid".into();
        match security(&i, "authpass1\nprivpass1").unwrap() {
            Security::V3 { auth, privacy, .. } => {
                assert_eq!(auth, AuthProto::Sha);
                assert_eq!(privacy.unwrap(), (PrivProto::Aes, "privpass1".to_string()));
            }
            _ => panic!(),
        }
        i.slug = "md5".into();
        assert!(matches!(security(&i, "authpass1").unwrap(), Security::V3 { auth: AuthProto::Md5, privacy: None, .. }));
        i.slug = "sha-des".into();
        // No separate privacy password: the same one, as DSM allows.
        assert!(matches!(security(&i, "authpass1").unwrap(), Security::V3 { privacy: Some((PrivProto::Des, _)), .. }));
    }

    #[test]
    fn synology_card() {
        let scalars: HashMap<String, Value> = [
            (SYNO_STATUS, Value::Int(1)),
            (SYNO_TEMP, Value::Int(41)),
            (SYNO_POWER, Value::Int(1)),
            (SYNO_SYS_FAN, Value::Int(2)),
            (SYNO_MODEL, Value::Str(b"DS920+".to_vec())),
            (SYNO_VERSION, Value::Str(b"DSM 7.2.2-72806".to_vec())),
            (SYNO_UPGRADE, Value::Int(2)),
            (CPU_IDLE, Value::Int(93)),
            (MEM_TOTAL, Value::Int(4_000_000)),
            (MEM_AVAIL, Value::Int(1_000_000)),
            (MEM_CACHED, Value::Int(1_000_000)),
            (SYS_UPTIME, Value::Uint(8_640_000)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        let disks = table(
            SYNO_DISKS,
            &[
                row(SYNO_DISKS, 2, 0, Value::Str(b"Drive 1".to_vec())),
                row(SYNO_DISKS, 5, 0, Value::Int(1)),
                row(SYNO_DISKS, 13, 0, Value::Int(1)),
                row(SYNO_DISKS, 2, 1, Value::Str(b"Drive 2".to_vec())),
                row(SYNO_DISKS, 3, 1, Value::Str(b"WD40EFRX".to_vec())),
                row(SYNO_DISKS, 5, 1, Value::Int(1)),
                row(SYNO_DISKS, 13, 1, Value::Int(4)),
            ],
        );
        let raids = table(
            SYNO_RAID,
            &[
                row(SYNO_RAID, 2, 0, Value::Str(b"Storage Pool 1".to_vec())),
                row(SYNO_RAID, 3, 0, Value::Int(11)),
                row(SYNO_RAID, 4, 0, Value::Uint(1_000)),
                row(SYNO_RAID, 5, 0, Value::Uint(4_000)),
            ],
        );
        let fixed = Value::Oid(oid(HR_FIXED_DISK));
        let storage = table(
            HR_STORAGE,
            &[
                row(HR_STORAGE, 2, 31, fixed.clone()),
                row(HR_STORAGE, 3, 31, Value::Str(b"/volume1".to_vec())),
                row(HR_STORAGE, 4, 31, Value::Int(4096)),
                row(HR_STORAGE, 5, 31, Value::Int(1000)),
                row(HR_STORAGE, 6, 31, Value::Int(960)),
                row(HR_STORAGE, 2, 32, fixed.clone()),
                row(HR_STORAGE, 3, 32, Value::Str(b"/volume1/@docker".to_vec())),
                row(HR_STORAGE, 5, 32, Value::Int(1000)),
                row(HR_STORAGE, 2, 1, fixed),
                row(HR_STORAGE, 3, 1, Value::Str(b"/".to_vec())),
                row(HR_STORAGE, 5, 1, Value::Int(1000)),
            ],
        );
        let st = build(&scalars, &disks, &raids, &storage);
        assert_eq!(st.summary, "1 pool · 2 disks (1 with problems)");
        assert_eq!(st.health, AppHealth::Bad);
        assert_eq!(st.version.as_deref(), Some("DSM 7.2.2-72806 (DS920+)"));
        assert_eq!(st.rows.len(), 2, "pool + /volume1, not / or @docker");
        assert_eq!(st.rows[0].title, "Storage Pool 1");
        assert_eq!(st.rows[0].state, AppRowState::Bad);
        assert_eq!(st.rows[1].title, "/volume1");
        assert_eq!(st.rows[1].state, AppRowState::Bad);
        assert!(st.notes.iter().any(|n| n == "Drive 2 (WD40EFRX): failing"), "{:?}", st.notes);
        assert!(st.notes.iter().any(|n| n == "system fan failed"));
        let tiles: HashMap<_, _> = st.tiles.iter().map(|t| (t.label.as_str(), t.value.as_str())).collect();
        assert_eq!(tiles["CPU"], "7 %");
        assert_eq!(tiles["Memory"], "50 %");
        assert_eq!(tiles["Temp"], "41 °C");
    }

    #[test]
    fn generic_agent_card() {
        let scalars: HashMap<String, Value> =
            [(SYNO_STATUS.to_string(), Value::NoSuch), (SYS_DESCR.to_string(), Value::Str(b"Linux nas 6.1".to_vec()))]
                .into_iter()
                .collect();
        let fixed = Value::Oid(oid(HR_FIXED_DISK));
        let storage = table(
            HR_STORAGE,
            &[
                row(HR_STORAGE, 2, 1, fixed.clone()),
                row(HR_STORAGE, 3, 1, Value::Str(b"/".to_vec())),
                row(HR_STORAGE, 4, 1, Value::Int(1024)),
                row(HR_STORAGE, 5, 1, Value::Int(100)),
                row(HR_STORAGE, 6, 1, Value::Int(10)),
                row(HR_STORAGE, 2, 2, fixed),
                row(HR_STORAGE, 3, 2, Value::Str(b"/boot/efi".to_vec())),
                row(HR_STORAGE, 5, 2, Value::Int(100)),
            ],
        );
        let st = build(&scalars, &Table::new(), &Table::new(), &storage);
        assert_eq!(st.summary, "1 volume");
        assert_eq!(st.health, AppHealth::Ok);
        assert_eq!(st.version.as_deref(), Some("Linux nas 6.1"));
    }

    /// `NG_SNMP_TEST=127.0.0.1:16161` against net-snmp (see snmp::tests::live_agent).
    #[tokio::test]
    #[ignore]
    async fn live_agent_card() {
        let Ok(addr) = std::env::var("NG_SNMP_TEST") else { return };
        let i = crate::integrations::app::testing::integration(IntegrationKind::Snmp, &format!("snmp://{addr}"));
        let st = status(&i, "public123").await.unwrap();
        assert!(st.summary.ends_with("volume") || st.summary.ends_with("volumes"), "{}", st.summary);
        assert!(st.tiles.iter().any(|t| t.label == "Up"), "{:?}", st.tiles);
    }
}
