//! Security posture (1.1, D-168): the System → security audit (M10d) for
//! every host at once, plus two checks it doesn't do (container images
//! older than six months, security updates waiting), kept as a history so
//! a drop can notify. Each failed check links to the Library guide whose
//! quest step verifies the fix. Read only: nothing is changed on a host.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::hosts::HostId;
use crate::quest::{QuestResult, QuestState};
use crate::store::Store;
use crate::system::security::{self, Check, CheckStatus, SecurityReport};
use crate::Result;

/// Runs kept per host.
pub const KEEP_RUNS: i64 = 90;
/// A score this many points below the last run notifies.
pub const DROP_NOTIFY: u32 = 10;
/// Quest checks run beside the audit.
pub const EXTRA_CHECKS: [&str; 2] = ["images-fresh", "security-updates"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PostureStatus {
    Pass,
    Warn,
    Fail,
    Unknown,
}

impl From<CheckStatus> for PostureStatus {
    fn from(s: CheckStatus) -> Self {
        match s {
            CheckStatus::Pass => PostureStatus::Pass,
            CheckStatus::Warn => PostureStatus::Warn,
            CheckStatus::Fail => PostureStatus::Fail,
            CheckStatus::Unknown | CheckStatus::Info => PostureStatus::Unknown,
        }
    }
}

/// One scored check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PostureCheck {
    pub id: String,
    pub title: String,
    pub status: PostureStatus,
    pub detail: String,
    pub fix: Option<String>,
    pub weight: u32,
    /// The Library article that fixes it (`howto/firewall`).
    pub article: String,
}

/// The Library article for a check.
pub fn article(id: &str) -> &'static str {
    match id {
        "ssh-root" | "ssh-password" => "basics/sshd-hardening",
        "firewall" => "howto/firewall",
        "intrusion" => "apps/fail2ban-crowdsec",
        "auto-updates" => "apps/unattended-upgrades",
        "uid0" => "howto/access-audit",
        "security-updates" => "howto/fleet-updates",
        _ => "guides/security-posture",
    }
}

/// A host's score from one run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PostureRun {
    pub host_id: HostId,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub at: i64,
    /// 0–100; none when the host couldn't be read.
    pub score: Option<u32>,
    /// `A` … `F`.
    pub grade: Option<String>,
    pub checks: Vec<PostureCheck>,
    /// Ports on every interface (from the audit), for the summary.
    pub open_ports: u32,
    pub error: Option<String>,
}

fn extra(r: &QuestResult) -> Option<Check> {
    let status = match r.state {
        QuestState::Pass => CheckStatus::Pass,
        QuestState::Fail if r.id == "images-fresh" => CheckStatus::Warn,
        QuestState::Fail => CheckStatus::Fail,
        QuestState::Unknown => CheckStatus::Unknown,
    };
    let (title, fix, weight) = match r.id.as_str() {
        "images-fresh" => (
            "Container images are under six months old",
            "Pull and redeploy the oldest stacks (Docker → STACKS → Update, which takes a snapshot first), or Fleet → UPDATES → Check images.",
            10,
        ),
        "security-updates" => (
            "No security updates waiting",
            "Install them from System → UPDATES (or Fleet → UPDATES for every host), then reboot if asked.",
            15,
        ),
        _ => return None,
    };
    Some(Check {
        id: r.id.clone(),
        title: title.into(),
        status,
        detail: r.detail.clone(),
        fix: (status != CheckStatus::Pass && status != CheckStatus::Unknown).then(|| fix.to_owned()),
        weight,
    })
}

/// The audit and the extra checks as one scored run.
pub fn combine(host_id: HostId, at: i64, report: &SecurityReport, extras: &[QuestResult]) -> PostureRun {
    let mut checks: Vec<Check> = report.checks.iter().filter(|c| c.status != CheckStatus::Info).cloned().collect();
    checks.extend(extras.iter().filter_map(extra));
    let (score, grade) = security::score(&checks);
    let open_ports = report.listeners.iter().filter(|l| l.scope == security::Scope::All).count() as u32;
    PostureRun {
        host_id,
        at,
        score: Some(score),
        grade: Some(grade),
        checks: checks
            .into_iter()
            .map(|c| PostureCheck {
                article: article(&c.id).to_owned(),
                id: c.id,
                title: c.title,
                status: c.status.into(),
                detail: c.detail,
                fix: c.fix,
                weight: c.weight,
            })
            .collect(),
        open_ports,
        error: None,
    }
}

pub fn failed(host_id: HostId, at: i64, error: String) -> PostureRun {
    PostureRun { host_id, at, score: None, grade: None, checks: Vec::new(), open_ports: 0, error: Some(error) }
}

/// The fixes worth doing first: failed, then warned, heaviest first.
pub fn todo(r: &PostureRun) -> Vec<&PostureCheck> {
    let mut v: Vec<&PostureCheck> =
        r.checks.iter().filter(|c| matches!(c.status, PostureStatus::Fail | PostureStatus::Warn)).collect();
    v.sort_by_key(|c| (c.status != PostureStatus::Fail, std::cmp::Reverse(c.weight)));
    v
}

fn grade_rank(g: &str) -> u8 {
    match g {
        "A" => 0,
        "B" => 1,
        "C" => 2,
        "D" => 3,
        _ => 4,
    }
}

/// Whether going from `prev` to `now` deserves a notice: a drop of
/// [`DROP_NOTIFY`] points or more, or a worse grade.
pub fn dropped(prev: Option<&PostureRun>, now: &PostureRun) -> bool {
    match (prev.and_then(|p| p.score.zip(p.grade.as_deref())), now.score.zip(now.grade.as_deref())) {
        (Some((p, pg)), Some((n, ng))) => p >= n + DROP_NOTIFY || grade_rank(ng) > grade_rank(pg),
        _ => false,
    }
}

/// What got worse between two runs, for the notice.
pub fn newly_failing(prev: &PostureRun, now: &PostureRun) -> Vec<String> {
    now.checks
        .iter()
        .filter(|c| matches!(c.status, PostureStatus::Fail | PostureStatus::Warn))
        .filter(|c| !prev.checks.iter().any(|p| p.id == c.id && p.status == c.status))
        .map(|c| c.title.clone())
        .collect()
}

/// A host's latest run with the score before it and the recent trend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PostureHost {
    pub latest: PostureRun,
    pub previous: Option<u32>,
    /// Scores of the last 30 runs that read the host, oldest first.
    pub trend: Vec<u32>,
}

impl Store {
    pub fn posture_save(&self, r: &PostureRun) -> Result<()> {
        let checks = serde_json::to_string(&r.checks)?;
        let c = self.conn();
        c.execute(
            "INSERT INTO posture_runs (host_id, at, score, grade, checks, open_ports, error) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![r.host_id, r.at, r.score, r.grade, checks, r.open_ports, r.error],
        )?;
        c.execute(
            "DELETE FROM posture_runs WHERE host_id = ?1 AND id NOT IN
               (SELECT id FROM posture_runs WHERE host_id = ?1 ORDER BY at DESC, id DESC LIMIT ?2)",
            params![r.host_id, KEEP_RUNS],
        )?;
        Ok(())
    }

    /// Runs of one host, newest first.
    pub fn posture_runs(&self, host_id: HostId, limit: u32) -> Result<Vec<PostureRun>> {
        let mut st = self.conn().prepare(
            "SELECT at, score, grade, checks, open_ports, error FROM posture_runs
             WHERE host_id = ?1 ORDER BY at DESC, id DESC LIMIT ?2",
        )?;
        let rows = st.query_map(params![host_id, limit], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<u32>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, u32>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (at, score, grade, checks, open_ports, error) = row?;
            out.push(PostureRun {
                host_id,
                at,
                score,
                grade,
                checks: serde_json::from_str(&checks).unwrap_or_default(),
                open_ports,
                error,
            });
        }
        Ok(out)
    }

    /// The last run that read the host.
    pub fn posture_last_good(&self, host_id: HostId) -> Result<Option<PostureRun>> {
        Ok(self.posture_runs(host_id, 10)?.into_iter().find(|r| r.error.is_none()))
    }

    /// Every scored host's latest run.
    pub fn posture_hosts(&self) -> Result<Vec<PostureHost>> {
        let ids: Vec<HostId> = {
            let mut st = self.conn().prepare("SELECT DISTINCT host_id FROM posture_runs ORDER BY host_id")?;
            let rows = st.query_map([], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let mut out = Vec::new();
        for id in ids {
            let mut runs = self.posture_runs(id, 30)?;
            if runs.is_empty() {
                continue;
            }
            let trend: Vec<u32> = runs.iter().rev().filter_map(|r| r.score).collect();
            let previous = runs.iter().skip(1).find_map(|r| r.score);
            out.push(PostureHost { latest: runs.remove(0), previous, trend });
        }
        Ok(out)
    }

    /// When each host was last scored (read or not).
    pub fn posture_last_at(&self) -> Result<Vec<(HostId, i64)>> {
        let mut st = self.conn().prepare("SELECT host_id, MAX(at) FROM posture_runs GROUP BY host_id")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audit(fw: bool) -> SecurityReport {
        let out = format!(
            "@@uid\n1000\n@@ss\ntcp LISTEN 0 4096 0.0.0.0:22 0.0.0.0:*\n@@sshd\n#file /etc/ssh/sshd_config\nPasswordAuthentication no\nKbdInteractiveAuthentication no\nPermitRootLogin no\n\
@@services\nufw {} enabled\nfirewalld inactive disabled\nnftables inactive disabled\nfail2ban active enabled\ncrowdsec unknown unknown\nunattended-upgrades active enabled\napt-daily-upgrade.timer active enabled\ndnf-automatic.timer unknown unknown\ndnf-automatic-install.timer unknown unknown\n\
@@ufw\nENABLED={}\n@@aptauto\nAPT::Periodic::Unattended-Upgrade \"1\";\n@@reboot\n@@uid0\nroot\n@@docker\n@@end\n",
            if fw { "active" } else { "inactive" },
            if fw { "yes" } else { "no" }
        );
        security::parse_audit(&out)
    }

    fn q(id: &str, state: QuestState) -> QuestResult {
        QuestResult { id: id.into(), state, detail: "d".into() }
    }

    #[test]
    fn combines_audit_and_extras() {
        let good = combine(
            1,
            100,
            &audit(true),
            &[q("images-fresh", QuestState::Pass), q("security-updates", QuestState::Pass)],
        );
        assert_eq!(good.score, Some(100));
        assert_eq!(good.grade.as_deref(), Some("A"));
        assert_eq!(good.open_ports, 1);
        assert!(good.checks.iter().any(|c| c.id == "security-updates" && c.article == "howto/fleet-updates"));
        assert!(todo(&good).is_empty());
        let bad = combine(
            1,
            200,
            &audit(false),
            &[q("images-fresh", QuestState::Fail), q("security-updates", QuestState::Fail)],
        );
        // Firewall off is a warning (7 of 15), security updates fail (15), old images warn (5 of 10).
        assert_eq!(bad.score, Some(73));
        assert_eq!(
            todo(&bad).iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            vec!["security-updates", "firewall", "images-fresh"]
        );
        assert!(bad.checks.iter().find(|c| c.id == "images-fresh").unwrap().fix.is_some());
        assert!(dropped(Some(&good), &bad));
        assert_eq!(newly_failing(&good, &bad).len(), 3);
        assert!(!dropped(Some(&bad), &good));
        assert!(!dropped(None, &bad));
        // Unknown extras don't cost points.
        let unk = combine(1, 300, &audit(true), &[q("security-updates", QuestState::Unknown)]);
        assert_eq!(unk.score, Some(100));
    }

    #[test]
    fn articles_exist_and_have_quest_steps() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/kb");
        for id in [
            "ssh-root",
            "firewall",
            "intrusion",
            "auto-updates",
            "uid0",
            "security-updates",
            "images-fresh",
            "exposed-services",
        ] {
            let a = article(id);
            assert!(root.join(format!("{a}.md")).exists(), "{a}");
        }
        let guide = std::fs::read_to_string(root.join("guides/security-posture.md")).unwrap();
        for c in ["ssh-no-password", "firewall", "no-exposed-db", "no-docker-tcp", "security-updates", "images-fresh"] {
            assert!(guide.contains(&format!("{{check: {c}}}")), "{c}");
        }
    }

    #[test]
    fn keeps_runs_per_host() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&crate::hosts::HostInput {
                name: "server01".into(),
                hostname: "10.0.0.2".into(),
                ..Default::default()
            })
            .unwrap();
        s.posture_save(&combine(h.id, 100, &audit(true), &[])).unwrap();
        s.posture_save(&failed(h.id, 150, "timed out".into())).unwrap();
        s.posture_save(&combine(h.id, 200, &audit(false), &[])).unwrap();
        let hosts = s.posture_hosts().unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].latest.score, Some(93));
        assert_eq!(hosts[0].previous, Some(100));
        assert_eq!(hosts[0].trend, vec![100, 93]);
        assert_eq!(s.posture_last_good(h.id).unwrap().unwrap().at, 200);
        assert_eq!(s.posture_last_at().unwrap(), vec![(h.id, 200)]);
        s.delete_host(h.id).unwrap();
        assert!(s.posture_hosts().unwrap().is_empty());
    }
}
