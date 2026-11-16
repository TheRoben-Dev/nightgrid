//! Proxmox VE part 2 (M10r): snapshots of a guest, and vzdump backups (jobs,
//! files on backup storages, guests no job covers, back up now, restore,
//! protect, delete). Same transport as part 1: `pvesh` over SSH as root.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{b, explain, f, get, opt_s, s, sections, valid_kind, valid_node, valid_vmid};
use crate::files::path::shell_quote;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveSnapshot {
    pub name: String,
    pub description: String,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub time: Option<i64>,
    /// The VM's RAM was saved too (a running VM resumes where it was).
    pub vmstate: bool,
    pub parent: Option<String>,
    /// The guest's current state was taken from this snapshot.
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveBackupJob {
    pub id: String,
    pub enabled: bool,
    /// Proxmox calendar event: `02:00`, `sun 03:00`, `*-*-1 04:00`…
    pub schedule: String,
    pub storage: String,
    /// snapshot, suspend or stop.
    pub mode: String,
    /// Guests in the job ("all" when every guest is).
    pub guests: String,
    pub exclude: String,
    pub comment: String,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub next_run: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveBackupFile {
    pub volid: String,
    pub storage: String,
    pub vmid: u32,
    /// qemu or lxc.
    pub kind: String,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub time: i64,
    pub size: f64,
    pub format: String,
    pub notes: String,
    pub protected: bool,
    /// ok, failed, or unset when never verified.
    pub verified: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveUncovered {
    pub vmid: u32,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveBackups {
    pub jobs: Vec<PveBackupJob>,
    /// Newest first.
    pub files: Vec<PveBackupFile>,
    /// Guests no backup job includes.
    pub uncovered: Vec<PveUncovered>,
    pub notes: Vec<String>,
}

// ---------------------------------------------------------- validation

/// Proxmox snapshot names: a letter, then letters, digits, `_` or `-`; 2–40 long.
pub fn valid_snapname(n: &str) -> bool {
    (2..=40).contains(&n.len())
        && n.starts_with(|c: char| c.is_ascii_alphabetic())
        && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && n != "current"
}

/// A storage id: a letter, then letters, digits, `-`, `_` or `.`.
pub fn valid_storage(n: &str) -> bool {
    (1..=64).contains(&n.len())
        && n.starts_with(|c: char| c.is_ascii_alphabetic())
        && n.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
}

/// `storage:backup/vzdump-qemu-100-….vma.zst` (no `..`, no spaces).
pub fn valid_volid(v: &str) -> bool {
    let Some((st, path)) = v.split_once(':') else { return false };
    valid_storage(st)
        && !path.is_empty()
        && v.len() <= 512
        && !path.split('/').any(|p| p.is_empty() || p == "..")
        && path.chars().all(|c| c.is_ascii_alphanumeric() || "-_./+".contains(c))
}

/// Free text for descriptions and notes: one line, no control characters.
pub fn valid_text(t: &str) -> bool {
    t.len() <= 500 && !t.chars().any(char::is_control)
}

fn guest(node: &str, kind: &str, vmid: u32) -> Option<String> {
    (valid_node(node) && valid_kind(kind) && valid_vmid(vmid)).then(|| format!("/nodes/{node}/{kind}/{vmid}"))
}

fn create(path: &str, params: &[(&str, String)]) -> String {
    let mut c = format!("pvesh create {}", shell_quote(path));
    for (k, v) in params {
        c.push_str(&format!(" --{k} {}", shell_quote(v)));
    }
    c.push_str(" --output-format json 2>&1");
    c
}

// ------------------------------------------------------------ snapshots

pub fn snapshots_script(node: &str, kind: &str, vmid: u32) -> Option<String> {
    Some(get(&format!("{}/snapshot", guest(node, kind, vmid)?), ""))
}

pub fn parse_snapshots(out: &str) -> Result<Vec<PveSnapshot>, String> {
    let (_, body) = sections(out).into_iter().next().ok_or_else(|| explain(out))?;
    let v: Value = serde_json::from_str(body.trim()).map_err(|_| explain(&body))?;
    let items = v.as_array().ok_or_else(|| explain(&body))?;
    let here = items.iter().find(|i| s(i, "name") == "current").and_then(|i| opt_s(i, "parent"));
    let mut out: Vec<PveSnapshot> = items
        .iter()
        .filter(|i| s(i, "name") != "current")
        .map(|i| {
            let name = s(i, "name");
            PveSnapshot {
                current: here.as_deref() == Some(name.as_str()),
                description: s(i, "description").trim().to_owned(),
                time: Some(f(i, "snaptime") as i64).filter(|t| *t > 0),
                vmstate: b(i, "vmstate"),
                parent: opt_s(i, "parent"),
                name,
            }
        })
        .collect();
    out.sort_by_key(|s| s.time.unwrap_or(0));
    Ok(out)
}

pub fn snapshot_create_script(
    node: &str,
    kind: &str,
    vmid: u32,
    name: &str,
    description: &str,
    vmstate: bool,
) -> Option<String> {
    let base = guest(node, kind, vmid)?;
    if !valid_snapname(name) || !valid_text(description) {
        return None;
    }
    let mut p = vec![("snapname", name.to_owned())];
    if !description.trim().is_empty() {
        p.push(("description", description.trim().to_owned()));
    }
    // Only VMs can save RAM; containers have no such option.
    if vmstate && kind == "qemu" {
        p.push(("vmstate", "1".into()));
    }
    Some(create(&format!("{base}/snapshot"), &p))
}

/// Rolls back to `name`; the guest's state since then is lost.
pub fn snapshot_rollback_script(node: &str, kind: &str, vmid: u32, name: &str, start: bool) -> Option<String> {
    let base = guest(node, kind, vmid)?;
    if !valid_snapname(name) {
        return None;
    }
    let p = if start { vec![("start", "1".to_owned())] } else { vec![] };
    Some(create(&format!("{base}/snapshot/{name}/rollback"), &p))
}

pub fn snapshot_delete_script(node: &str, kind: &str, vmid: u32, name: &str) -> Option<String> {
    let base = guest(node, kind, vmid)?;
    valid_snapname(name)
        .then(|| format!("pvesh delete {} --output-format json 2>&1", shell_quote(&format!("{base}/snapshot/{name}"))))
}

// -------------------------------------------------------------- backups

/// A storage as seen from one node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PveStorageRef {
    pub node: String,
    pub storage: String,
}

/// Jobs, guests no job covers, and the backup files on each storage.
pub fn backups_script(targets: &[PveStorageRef]) -> Option<String> {
    if targets.len() > 64 || !targets.iter().all(|t| valid_node(&t.node) && valid_storage(&t.storage)) {
        return None;
    }
    let mut out = format!("{}\n{}\n", get("/cluster/backup", ""), get("/cluster/backup-info/not-backed-up", ""));
    for t in targets {
        out.push_str(&get(&format!("/nodes/{}/storage/{}/content", t.node, t.storage), " --content backup"));
        out.push('\n');
    }
    Some(out)
}

fn job(j: &Value) -> PveBackupJob {
    PveBackupJob {
        id: s(j, "id"),
        // Jobs are enabled unless `enabled` says 0.
        enabled: j.get("enabled").map_or(true, |_| b(j, "enabled")),
        schedule: opt_s(j, "schedule").or_else(|| opt_s(j, "starttime")).unwrap_or_default(),
        storage: s(j, "storage"),
        mode: opt_s(j, "mode").unwrap_or_else(|| "snapshot".into()),
        guests: if b(j, "all") {
            "all".into()
        } else {
            opt_s(j, "vmid").or_else(|| opt_s(j, "pool").map(|p| format!("pool {p}"))).unwrap_or_default()
        },
        exclude: s(j, "exclude"),
        comment: s(j, "comment"),
        next_run: Some(f(j, "next-run") as i64).filter(|t| *t > 0),
    }
}

pub fn parse_backups(out: &str) -> PveBackups {
    let mut r = PveBackups::default();
    for (path, body) in sections(out) {
        let Ok(v) = serde_json::from_str::<Value>(body.trim()) else {
            r.notes.push(format!("{path}: {}", explain(&body)));
            continue;
        };
        let items = v.as_array().cloned().unwrap_or_default();
        if path == "/cluster/backup" {
            r.jobs = items.iter().map(job).collect();
        } else if path == "/cluster/backup-info/not-backed-up" {
            r.uncovered = items
                .iter()
                .map(|i| PveUncovered { vmid: f(i, "vmid") as u32, name: s(i, "name"), kind: s(i, "type") })
                .collect();
        } else if let Some(st) = path.split('/').nth(4) {
            for i in &items {
                let volid = s(i, "volid");
                if !valid_volid(&volid) {
                    continue;
                }
                let kind = opt_s(i, "subtype").unwrap_or_else(|| {
                    if volid.contains("-lxc-") {
                        "lxc".into()
                    } else {
                        "qemu".into()
                    }
                });
                r.files.push(PveBackupFile {
                    storage: st.to_owned(),
                    vmid: f(i, "vmid") as u32,
                    kind,
                    time: f(i, "ctime") as i64,
                    size: f(i, "size"),
                    format: s(i, "format"),
                    notes: s(i, "notes"),
                    protected: b(i, "protected"),
                    verified: opt_s(&i["verification"], "state"),
                    volid,
                });
            }
        }
    }
    // A shared storage answers the same files on every node: keep one.
    r.files.sort_by(|a, b| b.time.cmp(&a.time).then(a.volid.cmp(&b.volid)));
    r.files.dedup_by(|a, b| a.volid == b.volid);
    r.uncovered.sort_by_key(|u| u.vmid);
    r
}

/// Backup modes vzdump knows.
pub const MODES: &[&str] = &["snapshot", "suspend", "stop"];

pub fn backup_now_script(node: &str, vmid: u32, storage: &str, mode: &str, notes: &str) -> Option<String> {
    if !valid_node(node) || !valid_vmid(vmid) || !valid_storage(storage) || !MODES.contains(&mode) || !valid_text(notes)
    {
        return None;
    }
    let mut p = vec![
        ("vmid", vmid.to_string()),
        ("storage", storage.to_owned()),
        ("mode", mode.to_owned()),
        ("compress", "zstd".to_owned()),
    ];
    if !notes.trim().is_empty() {
        p.push(("notes-template", notes.trim().to_owned()));
    }
    Some(create(&format!("/nodes/{node}/vzdump"), &p))
}

/// Restores `volid` as guest `vmid` on `storage`. `force` overwrites an
/// existing guest with that id (its disks are replaced).
pub fn restore_script(
    node: &str,
    kind: &str,
    vmid: u32,
    volid: &str,
    storage: &str,
    force: bool,
    start: bool,
) -> Option<String> {
    if !valid_node(node) || !valid_kind(kind) || !valid_vmid(vmid) || !valid_volid(volid) || !valid_storage(storage) {
        return None;
    }
    let mut p = vec![("vmid", vmid.to_string())];
    if kind == "qemu" {
        p.push(("archive", volid.to_owned()));
    } else {
        p.push(("ostemplate", volid.to_owned()));
        p.push(("restore", "1".to_owned()));
    }
    p.push(("storage", storage.to_owned()));
    if force {
        p.push(("force", "1".to_owned()));
    }
    if start {
        p.push(("start", "1".to_owned()));
    }
    Some(create(&format!("/nodes/{node}/{kind}"), &p))
}

/// The volume id as one API path segment (`/` encoded).
fn seg(volid: &str) -> String {
    volid.replace('/', "%2F")
}

pub fn backup_protect_script(node: &str, volid: &str, protected: bool) -> Option<String> {
    let (st, _) = volid.split_once(':')?;
    if !valid_node(node) || !valid_volid(volid) {
        return None;
    }
    let path = format!("/nodes/{node}/storage/{st}/content/{}", seg(volid));
    Some(format!(
        "pvesh set {} --protected {} --output-format json 2>&1",
        shell_quote(&path),
        if protected { 1 } else { 0 }
    ))
}

/// Deletes a backup file. `pvesm free` takes the volume id as is.
pub fn backup_delete_script(volid: &str) -> Option<String> {
    valid_volid(volid).then(|| format!("pvesm free {} 2>&1 && echo @@ok", shell_quote(volid)))
}

#[cfg(test)]
mod tests {
    use super::super::MARK;
    use super::*;

    const SNAPS: &str = include_str!("fixtures/snapshots.json");
    const CONTENT: &str = include_str!("fixtures/backup_content.json");
    const JOBS: &str = include_str!("fixtures/backup_jobs.json");
    const UNCOVERED: &str = include_str!("fixtures/not_backed_up.json");
    const VOL: &str = "nas-backup:backup/vzdump-qemu-100-2026_11_16-02_00_01.vma.zst";

    fn out(parts: &[(&str, &str)]) -> String {
        parts.iter().map(|(p, b)| format!("{MARK} {p}\n{b}\n")).collect()
    }

    #[test]
    fn validates_input() {
        assert!(valid_snapname("pre_upgrade") && valid_snapname("a1-b"));
        assert!(
            !valid_snapname("1abc") && !valid_snapname("a") && !valid_snapname("current") && !valid_snapname("a b")
        );
        assert!(valid_storage("nas-backup") && !valid_storage("-x") && !valid_storage("a;b"));
        assert!(valid_volid(VOL) && valid_volid("local:backup/vzdump-lxc-200.tar.zst"));
        assert!(
            !valid_volid("nas:../etc/passwd")
                && !valid_volid("nas:backup/a b")
                && !valid_volid("nobackup")
                && !valid_volid("nas:")
        );
        assert!(valid_text("before upgrade") && !valid_text("a\nb"));
    }

    #[test]
    fn builds_quoted_scripts() {
        assert_eq!(
            snapshot_create_script("pve1", "qemu", 100, "pre_upgrade", "before 'upgrade'", true).unwrap(),
            "pvesh create '/nodes/pve1/qemu/100/snapshot' --snapname 'pre_upgrade' --description 'before '\\''upgrade'\\''' --vmstate '1' --output-format json 2>&1"
        );
        assert!(!snapshot_create_script("pve1", "lxc", 200, "s1", "", true).unwrap().contains("vmstate"));
        assert!(snapshot_create_script("pve1", "qemu", 100, "bad name", "", false).is_none());
        assert!(snapshot_rollback_script("pve1", "qemu", 100, "s1", true)
            .unwrap()
            .contains("/snapshot/s1/rollback' --start '1'"));
        assert_eq!(
            snapshot_delete_script("pve1", "lxc", 200, "s1").unwrap(),
            "pvesh delete '/nodes/pve1/lxc/200/snapshot/s1' --output-format json 2>&1"
        );
        let b = backup_now_script("pve1", 100, "nas-backup", "snapshot", "{{guestname}} manual").unwrap();
        assert!(b.contains("'/nodes/pve1/vzdump' --vmid '100' --storage 'nas-backup' --mode 'snapshot' --compress 'zstd' --notes-template '{{guestname}} manual'"));
        assert!(backup_now_script("pve1", 100, "nas-backup", "live", "").is_none());
        let r =
            restore_script("pve1", "lxc", 205, "nas-backup:backup/vzdump-lxc-200-x.tar.zst", "local-lvm", false, true)
                .unwrap();
        assert!(r.contains("'/nodes/pve1/lxc' --vmid '205' --ostemplate 'nas-backup:backup/vzdump-lxc-200-x.tar.zst' --restore '1' --storage 'local-lvm' --start '1'"));
        let q = restore_script("pve1", "qemu", 100, VOL, "local-lvm", true, false).unwrap();
        assert!(q.contains("--archive") && q.contains("--force '1'") && !q.contains("--start"));
        assert!(backup_protect_script("pve1", VOL, true)
            .unwrap()
            .contains("'/nodes/pve1/storage/nas-backup/content/nas-backup:backup%2Fvzdump-qemu-100-2026_11_16-02_00_01.vma.zst' --protected 1"));
        assert_eq!(backup_delete_script(VOL).unwrap(), format!("pvesm free '{VOL}' 2>&1 && echo @@ok"));
        assert!(backup_delete_script("x:../y").is_none());
        let r = |node: &str, storage: &str| PveStorageRef { node: node.into(), storage: storage.into() };
        let l = backups_script(&[r("pve1", "nas-backup"), r("pve2", "local")]).unwrap();
        assert!(l.contains("'/nodes/pve2/storage/local/content' --output-format json --content backup"));
        assert!(backups_script(&[r("pve1", "a;b")]).is_none());
    }

    #[test]
    fn parses_snapshots() {
        let v = parse_snapshots(&out(&[("/nodes/pve1/qemu/100/snapshot", SNAPS)])).unwrap();
        assert_eq!(v.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["clean_install", "jellyfin", "pre_upgrade"]);
        let pre = &v[2];
        assert!(pre.current && pre.vmstate && pre.description == "before upgrade to 24.04");
        assert!(!v[0].current && !v[0].vmstate && v[0].parent.is_none());
        assert_eq!(v[1].parent.as_deref(), Some("clean_install"));
        assert!(parse_snapshots(&out(&[(
            "/x",
            "Configuration file 'nodes/pve1/qemu-server/999.conf' does not exist"
        )]))
        .is_err());
    }

    #[test]
    fn parses_backups() {
        let r = parse_backups(&out(&[
            ("/cluster/backup", JOBS),
            ("/cluster/backup-info/not-backed-up", UNCOVERED),
            ("/nodes/pve1/storage/nas-backup/content", CONTENT),
            // The shared storage answers again from the other node: no duplicates.
            ("/nodes/pve2/storage/nas-backup/content", CONTENT),
            ("/nodes/pve1/storage/local/content", "storage 'local' is not online"),
        ]));
        assert_eq!(r.jobs.len(), 2);
        assert_eq!(
            (r.jobs[0].schedule.as_str(), r.jobs[0].guests.as_str(), r.jobs[0].enabled),
            ("02:00", "100,200,201", true)
        );
        assert_eq!((r.jobs[1].guests.as_str(), r.jobs[1].enabled, r.jobs[1].exclude.as_str()), ("all", false, "9000"));
        assert_eq!(r.jobs[0].next_run, Some(1794880800));
        assert_eq!(r.uncovered.iter().map(|u| u.vmid).collect::<Vec<_>>(), [101, 102]);
        assert_eq!(r.files.len(), 4);
        assert_eq!(r.files[0].vmid, 200);
        let newest_media = r.files.iter().find(|f| f.vmid == 100).unwrap();
        assert!(newest_media.protected && newest_media.verified.as_deref() == Some("ok"));
        assert_eq!((newest_media.storage.as_str(), newest_media.kind.as_str()), ("nas-backup", "qemu"));
        assert_eq!(r.notes.len(), 1);
        assert!(r.notes[0].contains("not online"));
    }
}
