//! Restore drills (1.1, D-171): a backup only counts once it has been
//! restored. A drill is either a backup job's test restore (restic, borg or
//! a folder copy: one file back, checked, deleted) or a Proxmox guest's
//! newest vzdump backup restored as a new guest that is cut off from the
//! network and tagged as a sandbox (D-162), started, checked and destroyed.
//! Drills run by hand or every N days; each run keeps its steps.

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::backup_jobs::BackupJobId;
use crate::hosts::HostId;
use crate::proxmox::backup::{valid_storage, PveBackupFile};
use crate::proxmox::{valid_kind, valid_node, valid_vmid};
use crate::store::Store;
use crate::{Error, Result};

const DAY: i64 = 86_400;
const KEEP: i64 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum DrillKind {
    /// A backup job's test restore.
    Job,
    /// A Proxmox guest's newest backup into an isolated sandbox.
    Proxmox,
}

impl DrillKind {
    fn as_str(self) -> &'static str {
        match self {
            DrillKind::Job => "job",
            DrillKind::Proxmox => "proxmox",
        }
    }
    fn parse(s: &str) -> Self {
        if s == "proxmox" {
            DrillKind::Proxmox
        } else {
            DrillKind::Job
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Drill {
    /// `None` adds a new one.
    pub id: Option<u32>,
    pub name: String,
    pub kind: DrillKind,
    pub host_id: HostId,
    pub job_id: Option<BackupJobId>,
    pub node: Option<String>,
    /// `qemu` or `lxc`.
    pub guest_kind: Option<String>,
    pub vmid: Option<u32>,
    /// Where the guest's backups are.
    pub storage: Option<String>,
    /// Where the drill restores to (needs room for the guest's disks).
    pub target_storage: Option<String>,
    /// Run every this many days; 0 = only by hand.
    pub every_days: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DrillRun {
    pub id: u32,
    pub drill_id: u32,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub at: i64,
    pub seconds: u32,
    pub ok: bool,
    pub steps: Vec<String>,
    pub error: Option<String>,
    /// What was restored (snapshot, file, backup volume).
    pub detail: String,
}

/// A drill with its last run and when it runs next.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DrillView {
    pub drill: Drill,
    pub last: Option<DrillRun>,
    /// Runs passed out of the last ten.
    pub passed: u32,
    pub runs: u32,
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub next_at: Option<i64>,
}

fn bad(field: &'static str, reason: &str) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

pub fn check(d: &Drill) -> Result<()> {
    if d.name.trim().is_empty() || d.name.chars().count() > 80 {
        return Err(bad("name", "1 to 80 characters"));
    }
    if d.every_days > 365 {
        return Err(bad("everyDays", "at most a year"));
    }
    match d.kind {
        DrillKind::Job => {
            if d.job_id.is_none() {
                return Err(bad("jobId", "pick a backup job"));
            }
        }
        DrillKind::Proxmox => {
            let ok = d.node.as_deref().is_some_and(valid_node)
                && d.guest_kind.as_deref().is_some_and(valid_kind)
                && d.vmid.is_some_and(valid_vmid)
                && d.storage.as_deref().is_some_and(valid_storage)
                && d.target_storage.as_deref().is_some_and(valid_storage);
            if !ok {
                return Err(bad("guest", "pick a node, a guest, its backup storage and a storage to restore to"));
            }
        }
    }
    Ok(())
}

/// When a scheduled drill runs next: `every_days` after its last run, or
/// now when it never ran.
pub fn next_at(d: &Drill, last: Option<i64>, now: i64) -> Option<i64> {
    if !d.enabled || d.every_days == 0 {
        return None;
    }
    Some(last.map_or(now, |l| l + i64::from(d.every_days) * DAY))
}

/// The newest backup of a guest among the files listed.
pub fn newest_backup<'a>(files: &'a [PveBackupFile], vmid: u32, kind: &str) -> Option<&'a PveBackupFile> {
    files.iter().filter(|f| f.vmid == vmid && f.kind == kind).max_by_key(|f| f.time)
}

/// `pvesh get /cluster/nextid` prints `"105"` (or a bare number).
pub fn parse_nextid(out: &str) -> Option<u32> {
    out.lines().map(|l| l.trim().trim_matches('"')).find_map(|l| l.parse().ok()).filter(|&n| valid_vmid(n))
}

/// `2026-11-18 14:05 UTC`.
pub fn utc(t: i64) -> String {
    let (days, secs) = (t.div_euclid(DAY), t.rem_euclid(DAY));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", secs / 3600, secs % 3600 / 60)
}

/// One drill's runs as a Markdown report, newest first.
pub fn report(d: &Drill, host: &str, runs: &[DrillRun]) -> String {
    let what = match d.kind {
        DrillKind::Job => "test restore of a backup job".to_owned(),
        DrillKind::Proxmox => format!(
            "{} {} restored from {} into an isolated sandbox on {}",
            d.guest_kind.as_deref().unwrap_or("guest"),
            d.vmid.unwrap_or(0),
            d.storage.as_deref().unwrap_or("?"),
            d.target_storage.as_deref().unwrap_or("?")
        ),
    };
    let ok = runs.iter().filter(|r| r.ok).count();
    let mut md = format!(
        "# Restore drill: {}\n\nHost: {host}. Drill: {what}. Schedule: {}.\n\n{ok} of {} runs passed.\n",
        d.name,
        if d.every_days == 0 { "by hand".to_owned() } else { format!("every {} days", d.every_days) },
        runs.len()
    );
    for r in runs {
        let when = utc(r.at);
        md.push_str(&format!("\n## {when}: {} ({} s)\n\n", if r.ok { "passed" } else { "FAILED" }, r.seconds));
        if !r.detail.is_empty() {
            md.push_str(&format!("{}\n\n", r.detail));
        }
        for s in &r.steps {
            md.push_str(&format!("- {s}\n"));
        }
        if let Some(e) = &r.error {
            md.push_str(&format!("\n**Error:** {e}\n"));
        }
    }
    md
}

fn row_drill(r: &rusqlite::Row<'_>) -> rusqlite::Result<Drill> {
    let kind: String = r.get(2)?;
    Ok(Drill {
        id: Some(r.get(0)?),
        name: r.get(1)?,
        kind: DrillKind::parse(&kind),
        host_id: r.get(3)?,
        job_id: r.get(4)?,
        node: r.get(5)?,
        guest_kind: r.get(6)?,
        vmid: r.get(7)?,
        storage: r.get(8)?,
        target_storage: r.get(9)?,
        every_days: r.get(10)?,
        enabled: r.get(11)?,
    })
}

fn row_run(r: &rusqlite::Row<'_>) -> rusqlite::Result<DrillRun> {
    let steps: String = r.get(5)?;
    Ok(DrillRun {
        id: r.get(0)?,
        drill_id: r.get(1)?,
        at: r.get(2)?,
        seconds: r.get(3)?,
        ok: r.get(4)?,
        steps: serde_json::from_str(&steps).unwrap_or_default(),
        error: r.get(6)?,
        detail: r.get(7)?,
    })
}

const DRILL_COLS: &str =
    "id, name, kind, host_id, job_id, node, guest_kind, vmid, storage, target_storage, every_days, enabled";
const RUN_COLS: &str = "id, drill_id, at, seconds, ok, steps, error, detail";

impl Store {
    pub fn drills(&self) -> Result<Vec<Drill>> {
        let mut st = self.conn().prepare(&format!("SELECT {DRILL_COLS} FROM drills ORDER BY name"))?;
        let rows = st.query_map([], row_drill)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn drill(&self, id: u32) -> Result<Drill> {
        self.conn()
            .query_row(&format!("SELECT {DRILL_COLS} FROM drills WHERE id = ?1"), [id], row_drill)
            .optional()?
            .ok_or(Error::NotFound("drill"))
    }

    pub fn drill_save(&self, d: &Drill) -> Result<u32> {
        check(d)?;
        let c = self.conn();
        let (job, pve) = match d.kind {
            DrillKind::Job => (d.job_id, false),
            DrillKind::Proxmox => (None, true),
        };
        let p = |o: &Option<String>| if pve { o.clone() } else { None };
        let vmid = if pve { d.vmid } else { None };
        let (node, gk, st, ts) = (p(&d.node), p(&d.guest_kind), p(&d.storage), p(&d.target_storage));
        let vals = params![
            d.name.trim(),
            d.kind.as_str(),
            d.host_id,
            job,
            node,
            gk,
            vmid,
            st,
            ts,
            d.every_days,
            d.enabled,
            d.id
        ];
        match d.id {
            Some(_) => {
                let n = c.execute(
                    "UPDATE drills SET name = ?1, kind = ?2, host_id = ?3, job_id = ?4, node = ?5, guest_kind = ?6, vmid = ?7,
                     storage = ?8, target_storage = ?9, every_days = ?10, enabled = ?11 WHERE id = ?12",
                    vals,
                )?;
                if n == 0 {
                    return Err(Error::NotFound("drill"));
                }
                Ok(d.id.unwrap_or_default())
            }
            None => {
                c.execute(
                    "INSERT INTO drills (name, kind, host_id, job_id, node, guest_kind, vmid, storage, target_storage, every_days, enabled)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    &vals[..11],
                )?;
                Ok(c.last_insert_rowid() as u32)
            }
        }
    }

    pub fn drill_delete(&self, id: u32) -> Result<()> {
        self.conn().execute("DELETE FROM drills WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Keeps a run (and the last 50 per drill).
    #[allow(clippy::too_many_arguments)]
    pub fn drill_run_save(
        &self,
        drill_id: u32,
        at: i64,
        seconds: u32,
        ok: bool,
        steps: &[String],
        error: Option<&str>,
        detail: &str,
    ) -> Result<u32> {
        let c = self.conn();
        c.execute(
            "INSERT INTO drill_runs (drill_id, at, seconds, ok, steps, error, detail) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![drill_id, at, seconds, ok, serde_json::to_string(steps).unwrap_or_default(), error, detail],
        )?;
        let id = c.last_insert_rowid() as u32;
        c.execute(
            "DELETE FROM drill_runs WHERE drill_id = ?1 AND id NOT IN
             (SELECT id FROM drill_runs WHERE drill_id = ?1 ORDER BY at DESC, id DESC LIMIT ?2)",
            params![drill_id, KEEP],
        )?;
        Ok(id)
    }

    /// Newest first.
    pub fn drill_runs(&self, drill_id: u32, limit: u32) -> Result<Vec<DrillRun>> {
        let mut st = self.conn().prepare(&format!(
            "SELECT {RUN_COLS} FROM drill_runs WHERE drill_id = ?1 ORDER BY at DESC, id DESC LIMIT ?2"
        ))?;
        let rows = st.query_map(params![drill_id, limit], row_run)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn drill_views(&self, now: i64) -> Result<Vec<DrillView>> {
        let mut out = Vec::new();
        for d in self.drills()? {
            let id = d.id.unwrap_or(0);
            let runs = self.drill_runs(id, 10)?;
            let last = runs.first().cloned();
            let next = next_at(&d, last.as_ref().map(|r| r.at), now);
            out.push(DrillView {
                passed: runs.iter().filter(|r| r.ok).count() as u32,
                runs: runs.len() as u32,
                drill: d,
                last,
                next_at: next,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pve() -> Drill {
        Drill {
            id: None,
            name: "Home Assistant VM".into(),
            kind: DrillKind::Proxmox,
            host_id: 1,
            job_id: None,
            node: Some("pve".into()),
            guest_kind: Some("qemu".into()),
            vmid: Some(101),
            storage: Some("backup-nfs".into()),
            target_storage: Some("local-lvm".into()),
            every_days: 7,
            enabled: true,
        }
    }

    #[test]
    fn checks_and_schedules() {
        assert!(check(&pve()).is_ok());
        assert!(check(&Drill { node: Some("bad node".into()), ..pve() }).is_err());
        assert!(check(&Drill { kind: DrillKind::Job, ..pve() }).is_err());
        let now = 1_800_000_000;
        assert_eq!(next_at(&pve(), None, now), Some(now));
        assert_eq!(next_at(&pve(), Some(now - DAY), now), Some(now + 6 * DAY));
        assert_eq!(next_at(&Drill { every_days: 0, ..pve() }, None, now), None);
        assert_eq!(parse_nextid("\"105\"\n"), Some(105));
        assert_eq!(parse_nextid("oops"), None);
        assert_eq!(utc(1_795_000_000), "2026-11-18 11:06 UTC");
        assert_eq!(utc(951_782_400), "2000-02-29 00:00 UTC");
    }

    #[test]
    fn picks_the_newest_backup() {
        let f = |vmid, time, kind: &str| PveBackupFile {
            volid: format!("s:backup/{vmid}-{time}"),
            storage: "s".into(),
            vmid,
            kind: kind.into(),
            time,
            size: 1.0,
            format: "vma.zst".into(),
            notes: String::new(),
            protected: false,
            verified: None,
        };
        let files = vec![f(101, 10, "qemu"), f(101, 30, "qemu"), f(102, 50, "qemu"), f(101, 40, "lxc")];
        assert_eq!(newest_backup(&files, 101, "qemu").map(|b| b.time), Some(30));
        assert!(newest_backup(&files, 103, "qemu").is_none());
    }

    #[test]
    fn keeps_runs_and_reports() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&crate::hosts::HostInput {
                name: "pve".into(),
                hostname: "pve.lan".into(),
                ..Default::default()
            })
            .unwrap();
        let id = s.drill_save(&Drill { host_id: h.id, ..pve() }).unwrap();
        s.drill_run_save(id, 100, 42, true, &["Restored".into(), "Booted".into()], None, "vzdump-qemu-101").unwrap();
        s.drill_run_save(id, 200, 10, false, &["Restore failed".into()], Some("no space"), "").unwrap();
        let v = s.drill_views(300).unwrap();
        assert_eq!((v[0].runs, v[0].passed, v[0].last.as_ref().map(|r| r.ok)), (2, 1, Some(false)));
        let md = report(&s.drill(id).unwrap(), "pve", &s.drill_runs(id, 50).unwrap());
        assert!(md.contains("1 of 2 runs passed"));
        assert!(md.contains("**Error:** no space"));
        s.drill_delete(id).unwrap();
        assert!(s.drills().unwrap().is_empty());
    }
}
