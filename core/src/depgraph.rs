//! Service dependency graph (M10s): the user says what depends on what (a
//! media server behind a NAS, everything behind a router), and alerts about
//! things downstream of an outage are folded under the alert for its cause
//! instead of each notifying on its own.
//!
//! Nodes are hosts and services. Besides the edges the user adds, every
//! service checked on a host depends on that host, and every alert about a
//! host (a container, a disk, a backup) depends on the host being up. All of
//! this is pure: the alerts loop asks [`fold`] which active alerts have an
//! upstream cause, and which should wait because their cause may still fire.

use std::collections::{HashMap, HashSet, VecDeque};

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::alerts::{Alert, AlertKind, Condition};
use crate::hosts::HostId;
use crate::services::ServiceId;
use crate::store::Store;
use crate::{Error, Result};

pub type DepId = i32;

/// Most edges kept; far more than a homelab needs.
pub const MAX_DEPS: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum Node {
    Host(HostId),
    Service(ServiceId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub id: DepId,
    /// The thing that breaks when `parent` is down.
    pub child: Node,
    pub parent: Node,
}

fn split(n: Node) -> (Option<HostId>, Option<ServiceId>) {
    match n {
        Node::Host(h) => (Some(h), None),
        Node::Service(s) => (None, Some(s)),
    }
}

fn join(h: Option<i32>, s: Option<i32>) -> Option<Node> {
    match (h, s) {
        (Some(h), None) => Some(Node::Host(h)),
        (None, Some(s)) => Some(Node::Service(s)),
        _ => None,
    }
}

impl Store {
    pub fn deps_list(&self) -> Result<Vec<Dependency>> {
        let mut st = self.conn().prepare(
            "SELECT id, child_host_id, child_service_id, parent_host_id, parent_service_id FROM alert_deps ORDER BY id",
        )?;
        let rows =
            st.query_map([], |r| Ok((r.get::<_, DepId>(0)?, join(r.get(1)?, r.get(2)?), join(r.get(3)?, r.get(4)?))))?;
        let mut out = Vec::new();
        for row in rows {
            if let (id, Some(child), Some(parent)) = row? {
                out.push(Dependency { id, child, parent });
            }
        }
        Ok(out)
    }

    /// Adds "`child` depends on `parent`". Refuses a node depending on
    /// itself and anything that would make a loop; an existing edge is
    /// returned as it is.
    pub fn deps_add(&self, child: Node, parent: Node) -> Result<Dependency> {
        let list = self.deps_list()?;
        if let Some(d) = list.iter().find(|d| d.child == child && d.parent == parent) {
            return Ok(d.clone());
        }
        check_edge(&list, child, parent)?;
        let exists = |n: Node| -> Result<bool> {
            let (sql, id) = match n {
                Node::Host(h) => ("SELECT 1 FROM hosts WHERE id = ?1", h),
                Node::Service(s) => ("SELECT 1 FROM services WHERE id = ?1", s),
            };
            Ok(self.conn().query_row(sql, [id], |_| Ok(())).optional()?.is_some())
        };
        if !exists(child)? || !exists(parent)? {
            return Err(Error::NotFound("host or service"));
        }
        let (ch, cs) = split(child);
        let (ph, ps) = split(parent);
        self.conn().execute(
            "INSERT INTO alert_deps (child_host_id, child_service_id, parent_host_id, parent_service_id)
             VALUES (?1, ?2, ?3, ?4)",
            params![ch, cs, ph, ps],
        )?;
        Ok(Dependency { id: self.conn().last_insert_rowid() as DepId, child, parent })
    }

    pub fn deps_delete(&self, id: DepId) -> Result<()> {
        self.conn().execute("DELETE FROM alert_deps WHERE id = ?1", [id])?;
        Ok(())
    }
}

/// Whether `child → parent` can be added to `list`.
pub fn check_edge(list: &[Dependency], child: Node, parent: Node) -> Result<()> {
    let invalid = |reason: &str| Error::Invalid { field: "dependency", reason: reason.into() };
    if child == parent {
        return Err(invalid("a host or service can't depend on itself"));
    }
    if list.len() >= MAX_DEPS {
        return Err(invalid("too many dependencies"));
    }
    // A loop: parent already depends (through user edges) on child.
    let g = Graph::new(list, &HashMap::new());
    if parent_reaches(&g, parent, child) {
        return Err(invalid("that would make a loop: the other side already depends on this one"));
    }
    Ok(())
}

fn parent_reaches(g: &Graph, from: Node, to: Node) -> bool {
    g.ancestors(from).iter().any(|(n, _)| *n == to)
}

/// Parents of every node, the user's edges plus "a service depends on its host".
#[derive(Debug, Default, Clone)]
pub struct Graph {
    parents: HashMap<Node, Vec<Node>>,
}

impl Graph {
    pub fn new(deps: &[Dependency], service_hosts: &HashMap<ServiceId, HostId>) -> Self {
        let mut parents: HashMap<Node, Vec<Node>> = HashMap::new();
        for d in deps {
            parents.entry(d.child).or_default().push(d.parent);
        }
        for (s, h) in service_hosts {
            let e = parents.entry(Node::Service(*s)).or_default();
            if !e.contains(&Node::Host(*h)) {
                e.push(Node::Host(*h));
            }
        }
        Self { parents }
    }

    /// Everything `n` depends on, directly or not, with its distance.
    /// Each node once, at its shortest distance; loops are harmless.
    pub fn ancestors(&self, n: Node) -> Vec<(Node, usize)> {
        let mut out = Vec::new();
        let mut seen = HashSet::from([n]);
        let mut queue = VecDeque::from([(n, 0usize)]);
        while let Some((cur, d)) = queue.pop_front() {
            for p in self.parents.get(&cur).into_iter().flatten() {
                if seen.insert(*p) {
                    out.push((*p, d + 1));
                    queue.push_back((*p, d + 1));
                }
            }
        }
        out
    }
}

fn svc_id(key: &str) -> Option<ServiceId> {
    let mut parts = key.split(':');
    (parts.next() == Some("svc")).then_some(())?;
    parts.next()?.parse().ok()
}

/// The node an alert (or condition) is about.
pub fn subject(key: &str, host: Option<HostId>) -> Option<Node> {
    svc_id(key).map(Node::Service).or(host.map(Node::Host))
}

/// The node an alert says is down: a host offline or a service down.
pub fn down_node(key: &str, kind: AlertKind, host: Option<HostId>) -> Option<Node> {
    match kind {
        AlertKind::HostOffline => host.map(Node::Host),
        AlertKind::ServiceDown => svc_id(key).map(Node::Service),
        _ => None,
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Folding {
    /// Alert id → the id of the active alert for its root cause.
    pub caused_by: HashMap<u32, u32>,
    /// Alerts with no active cause yet, but something upstream is in its
    /// grace period: notifying now could be noise a minute later.
    pub waiting: HashSet<u32>,
}

/// Which active alerts are explained by an outage upstream of them. The
/// cause is the furthest-up node that is down, so a chain (router → NAS →
/// media server) folds everything under the router.
pub fn fold(active: &[Alert], pending: &[Condition], graph: &Graph) -> Folding {
    let down: HashMap<Node, u32> =
        active.iter().filter_map(|a| down_node(&a.key, a.kind, a.host_id).map(|n| (n, a.id))).collect();
    let pending_down: HashSet<Node> = pending.iter().filter_map(|c| down_node(&c.key, c.kind, c.host_id)).collect();
    let mut out = Folding::default();
    for a in active {
        let Some(s) = subject(&a.key, a.host_id) else { continue };
        let own = down_node(&a.key, a.kind, a.host_id);
        let mut candidates: Vec<(Node, usize)> = Vec::new();
        // A container or disk alert on a host that is offline is explained
        // by the host itself; the host's own offline alert is not.
        if own != Some(s) {
            candidates.push((s, 0));
        }
        candidates.extend(graph.ancestors(s));
        let cause = candidates
            .iter()
            .filter_map(|(n, d)| down.get(n).filter(|id| **id != a.id).map(|id| (*d, *id)))
            .max_by_key(|(d, id)| (*d, std::cmp::Reverse(*id)));
        if let Some((_, id)) = cause {
            out.caused_by.insert(a.id, id);
        } else if candidates.iter().any(|(n, _)| pending_down.contains(n)) {
            out.waiting.insert(a.id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alerts::Severity;

    fn alert(id: u32, key: &str, kind: AlertKind, host: Option<HostId>) -> Alert {
        Alert {
            id,
            key: key.into(),
            kind,
            severity: Severity::Critical,
            title: key.into(),
            detail: String::new(),
            host_id: host,
            since: 0,
            resolved: None,
            caused_by: None,
            silenced: None,
        }
    }

    fn dep(child: Node, parent: Node) -> Dependency {
        Dependency { id: 0, child, parent }
    }

    #[test]
    fn folds_downstream_alerts_under_the_root_cause() {
        // Router (service 9) ← NAS (host 2) ← Plex (service 5, on host 3).
        let deps = [dep(Node::Host(2), Node::Service(9)), dep(Node::Service(5), Node::Host(2))];
        let g = Graph::new(&deps, &HashMap::from([(5, 3)]));
        let active = [
            alert(1, "svc:9", AlertKind::ServiceDown, None),
            alert(2, "host:2", AlertKind::HostOffline, Some(2)),
            alert(3, "svc:5", AlertKind::ServiceDown, Some(3)),
            alert(4, "docker:2:abc", AlertKind::ContainerDown, Some(2)),
            alert(5, "disk:7:/", AlertKind::DiskFull, Some(7)),
        ];
        let f = fold(&active, &[], &g);
        assert_eq!(f.caused_by.get(&2), Some(&1));
        assert_eq!(f.caused_by.get(&3), Some(&1));
        assert_eq!(f.caused_by.get(&4), Some(&1));
        assert_eq!(f.caused_by.get(&1), None);
        assert_eq!(f.caused_by.get(&5), None);
        assert!(f.waiting.is_empty());
    }

    #[test]
    fn host_alerts_fold_under_the_host_being_offline() {
        let g = Graph::new(&[], &HashMap::from([(5, 3)]));
        let active = [
            alert(1, "host:3", AlertKind::HostOffline, Some(3)),
            alert(2, "svc:5", AlertKind::ServiceDown, Some(3)),
            alert(3, "svc:5:cert", AlertKind::CertExpiring, Some(3)),
            alert(4, "backup:3:1", AlertKind::BackupStale, Some(3)),
        ];
        let f = fold(&active, &[], &g);
        assert_eq!(f.caused_by, HashMap::from([(2, 1), (3, 1), (4, 1)]));
    }

    #[test]
    fn waits_while_the_cause_is_in_its_grace_period() {
        let g = Graph::new(&[], &HashMap::from([(5, 3)]));
        let active = [alert(2, "svc:5", AlertKind::ServiceDown, Some(3))];
        let pending = [crate::alerts::host_offline(3, "nas", None)];
        let f = fold(&active, &pending, &g);
        assert!(f.caused_by.is_empty());
        assert_eq!(f.waiting, HashSet::from([2]));
        assert!(fold(&active, &[], &g).waiting.is_empty());
    }

    #[test]
    fn loops_are_refused_and_harmless() {
        let a = Node::Host(1);
        let b = Node::Service(2);
        let c = Node::Host(3);
        let list = [dep(a, b), dep(b, c)];
        assert!(check_edge(&list, c, a).is_err());
        assert!(check_edge(&list, a, a).is_err());
        assert!(check_edge(&list, a, c).is_ok());
        // A loop through the implicit service → host edge still terminates.
        let g = Graph::new(&[dep(Node::Host(3), Node::Service(2))], &HashMap::from([(2, 3)]));
        assert_eq!(g.ancestors(Node::Host(3)), vec![(Node::Service(2), 1)]);
    }

    #[test]
    fn store_round_trip() {
        let mut s = Store::open_in_memory().unwrap();
        let host =
            |n: &str| crate::hosts::HostInput { name: n.into(), hostname: format!("{n}.lan"), ..Default::default() };
        let h = s.create_host(&host("nas")).unwrap();
        let r = s.create_host(&host("router")).unwrap();
        let d = s.deps_add(Node::Host(h.id), Node::Host(r.id)).unwrap();
        assert_eq!(s.deps_add(Node::Host(h.id), Node::Host(r.id)).unwrap(), d);
        assert!(s.deps_add(Node::Host(r.id), Node::Host(h.id)).is_err());
        assert!(s.deps_add(Node::Host(h.id), Node::Service(99)).is_err());
        assert_eq!(s.deps_list().unwrap(), vec![d.clone()]);
        s.delete_host(r.id).unwrap();
        assert!(s.deps_list().unwrap().is_empty());
    }
}
