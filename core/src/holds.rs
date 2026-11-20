//! Update holds (D-195): what the user keeps out of updates.
//!
//! - A **host** hold skips the host in batch upgrades and silences its
//!   pending count, for a while (a snooze) or until released.
//! - A **package** hold here is only for hosts that can't hold a package
//!   themselves (pacman, dnf without versionlock): Nightgrid's upgrades pass
//!   it as an exclude. Hosts that can (apt-mark hold, dnf versionlock,
//!   hidden Windows updates) keep their holds on the host, so other tools
//!   like unattended-upgrades respect them too.
//! - An **image** hold keeps one image of a stack where it is: update
//!   checks still say whether a newer one exists, but it isn't counted and
//!   Pull / Update leave its services alone.
//! - A **stack** hold does that for every image of the stack.
//!
//! Nothing is held by default.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::hosts::HostId;
use crate::store::Store;
use crate::{Error, Result};

pub type HoldId = i32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum HoldKind {
    Host,
    Package,
    Image,
    Stack,
}

impl HoldKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Package => "package",
            Self::Image => "image",
            Self::Stack => "stack",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "host" => Self::Host,
            "package" => Self::Package,
            "image" => Self::Image,
            "stack" => Self::Stack,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpdateHold {
    pub id: HoldId,
    pub host_id: HostId,
    pub kind: HoldKind,
    /// Package name, image reference or stack name; empty for a host.
    pub target: String,
    /// The stack an image belongs to; empty otherwise.
    pub stack: String,
    /// What was installed or running when it was held.
    pub version: Option<String>,
    /// Unix seconds the hold ends by itself; `None` = until released.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub until: Option<i64>,
    pub note: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HoldInput {
    pub host_id: HostId,
    pub kind: HoldKind,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub stack: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub until: Option<i64>,
    #[serde(default)]
    pub note: String,
}

/// A package name a package manager accepts: no spaces, quotes or a leading
/// dash (it ends up on a command line, quoted).
pub fn valid_package(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('-')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._+:@~-".contains(&b))
}

/// A compose service name (compose allows `[a-zA-Z0-9._-]`).
pub fn valid_service(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('-')
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

fn invalid(field: &'static str, reason: &str) -> Error {
    Error::Invalid { field, reason: reason.into() }
}

fn clean(i: &HoldInput, now: i64) -> Result<HoldInput> {
    let target = i.target.trim().to_owned();
    let stack = i.stack.trim().to_owned();
    match i.kind {
        HoldKind::Host => {
            if !target.is_empty() || !stack.is_empty() {
                return Err(invalid("target", "a host hold names no package or image"));
            }
        }
        HoldKind::Package => {
            if !valid_package(&target) {
                return Err(invalid("target", "a package name (letters, digits and . _ + : @ ~ -)"));
            }
            if !stack.is_empty() {
                return Err(invalid("stack", "a package hold names no stack"));
            }
        }
        HoldKind::Image => {
            if target.is_empty() || target.len() > 512 || target.chars().any(|c| c.is_whitespace() || c.is_control()) {
                return Err(invalid("target", "an image reference"));
            }
            if stack.is_empty() || stack.len() > 128 {
                return Err(invalid("stack", "the stack the image belongs to"));
            }
        }
        HoldKind::Stack => {
            if target.is_empty() || target.len() > 128 || target.chars().any(|c| c.is_whitespace() || c.is_control()) {
                return Err(invalid("target", "a stack name"));
            }
            if !stack.is_empty() {
                return Err(invalid("stack", "a stack hold names the stack as its target"));
            }
        }
    }
    if let Some(u) = i.until {
        if u <= now {
            return Err(invalid("until", "an end in the future"));
        }
    }
    let version = i.version.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(|v| v.chars().take(200).collect());
    let note = i.note.trim().to_owned();
    if note.chars().count() > 500 {
        return Err(invalid("note", "at most 500 characters"));
    }
    Ok(HoldInput { host_id: i.host_id, kind: i.kind, target, stack, version, until: i.until, note })
}

fn now_unix() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl Store {
    /// Every hold still in force; holds whose end has passed are removed.
    pub fn update_holds(&mut self) -> Result<Vec<UpdateHold>> {
        self.update_holds_at(now_unix())
    }

    pub fn update_holds_at(&mut self, now: i64) -> Result<Vec<UpdateHold>> {
        self.conn().execute("DELETE FROM update_holds WHERE until IS NOT NULL AND until <= ?1", [now])?;
        let mut stmt = self.conn().prepare(
            "SELECT id, host_id, kind, target, stack, version, until, note, created_at FROM update_holds
             ORDER BY host_id, kind, stack, target",
        )?;
        let rows = stmt.query_map([], |r| {
            let kind: String = r.get(2)?;
            Ok(UpdateHold {
                id: r.get(0)?,
                host_id: r.get(1)?,
                kind: HoldKind::parse(&kind).unwrap_or(HoldKind::Host),
                target: r.get(3)?,
                stack: r.get(4)?,
                version: r.get(5)?,
                until: r.get(6)?,
                note: r.get(7)?,
                created_at: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Holds of one host still in force.
    pub fn update_holds_for(&mut self, host_id: HostId) -> Result<Vec<UpdateHold>> {
        Ok(self.update_holds()?.into_iter().filter(|h| h.host_id == host_id).collect())
    }

    /// Adds a hold, or updates the one for the same thing (its end, note
    /// and version).
    pub fn save_update_hold(&mut self, input: &HoldInput) -> Result<UpdateHold> {
        let now = now_unix();
        let i = clean(input, now)?;
        self.conn().execute(
            "INSERT INTO update_holds (host_id, kind, target, stack, version, until, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(host_id, kind, stack, target) DO UPDATE SET
               version = COALESCE(?5, version), until = ?6, note = ?7",
            params![i.host_id, i.kind.as_str(), i.target, i.stack, i.version, i.until, i.note],
        )?;
        self.update_holds_at(now)?
            .into_iter()
            .find(|h| h.host_id == i.host_id && h.kind == i.kind && h.target == i.target && h.stack == i.stack)
            .ok_or(Error::NotFound("hold"))
    }

    pub fn delete_update_hold(&mut self, id: HoldId) -> Result<()> {
        self.conn().execute("DELETE FROM update_holds WHERE id = ?1", [id])?;
        Ok(())
    }
}

/// The host hold among `holds`, if the host is held.
pub fn host_hold(holds: &[UpdateHold], host_id: HostId) -> Option<&UpdateHold> {
    holds.iter().find(|h| h.host_id == host_id && h.kind == HoldKind::Host)
}

/// Package names Nightgrid holds for a host (excluded from its upgrades).
pub fn held_packages(holds: &[UpdateHold], host_id: HostId) -> Vec<String> {
    holds.iter().filter(|h| h.host_id == host_id && h.kind == HoldKind::Package).map(|h| h.target.clone()).collect()
}

/// Whether the whole stack is held.
pub fn stack_held(holds: &[UpdateHold], host_id: HostId, stack: &str) -> bool {
    holds.iter().any(|h| h.host_id == host_id && h.kind == HoldKind::Stack && h.target == stack)
}

/// Whether one image of a stack is held (itself, or by its whole stack).
pub fn image_held(holds: &[UpdateHold], host_id: HostId, stack: &str, image: &str) -> bool {
    stack_held(holds, host_id, stack)
        || holds
            .iter()
            .any(|h| h.host_id == host_id && h.kind == HoldKind::Image && h.stack == stack && h.target == image)
}

/// What a Pull / Update of a stack does about holds: `None` = pull
/// everything as before, `Some(services)` = pull only these (an empty list
/// = nothing to pull, every service is held). `images` is image → its
/// services, from the stack's containers.
pub fn pull_services(
    holds: &[UpdateHold],
    host_id: HostId,
    stack: &str,
    images: &[(String, Vec<String>)],
) -> Option<Vec<String>> {
    let held: Vec<&str> = images
        .iter()
        .filter(|(img, _)| image_held(holds, host_id, stack, img))
        .flat_map(|(_, svcs)| svcs.iter().map(String::as_str))
        .collect();
    if held.is_empty() && !stack_held(holds, host_id, stack) {
        return None;
    }
    let mut free: Vec<String> = images
        .iter()
        .flat_map(|(_, svcs)| svcs.iter())
        .filter(|s| !held.contains(&s.as_str()) && valid_service(s))
        .cloned()
        .collect();
    if stack_held(holds, host_id, stack) {
        free.clear();
    }
    free.sort();
    free.dedup();
    Some(free)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::HostInput;

    fn input(host_id: HostId, kind: HoldKind, target: &str, stack: &str) -> HoldInput {
        HoldInput {
            host_id,
            kind,
            target: target.into(),
            stack: stack.into(),
            version: None,
            until: None,
            note: String::new(),
        }
    }

    #[test]
    fn names() {
        assert!(valid_package("linux-image-6.8.0-49-generic"));
        assert!(valid_package("libstdc++6"));
        assert!(valid_package("python3.12"));
        assert!(!valid_package("-y"));
        assert!(!valid_package("a b"));
        assert!(!valid_package("a;rm"));
        assert!(!valid_package(""));
        assert!(valid_service("web_1.app"));
        assert!(!valid_service("a b"));
    }

    #[test]
    fn save_list_expire_delete_cascade() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&HostInput { name: "server01".into(), hostname: "server01.lan".into(), ..Default::default() })
            .unwrap();
        // Bad inputs are refused.
        assert!(s.save_update_hold(&input(h.id, HoldKind::Package, "a b", "")).is_err());
        assert!(s.save_update_hold(&input(h.id, HoldKind::Host, "x", "")).is_err());
        assert!(s.save_update_hold(&input(h.id, HoldKind::Image, "nginx:1", "")).is_err());
        assert!(s.save_update_hold(&HoldInput { until: Some(1), ..input(h.id, HoldKind::Host, "", "") }).is_err());

        let far = now_unix() + 3600;
        let host = s.save_update_hold(&HoldInput { until: Some(far), ..input(h.id, HoldKind::Host, "", "") }).unwrap();
        assert_eq!(host.until, Some(far));
        let pkg = s
            .save_update_hold(&HoldInput {
                version: Some("6.11.5".into()),
                ..input(h.id, HoldKind::Package, "linux", "")
            })
            .unwrap();
        s.save_update_hold(&input(h.id, HoldKind::Image, "nginx:1.27", "web")).unwrap();
        // Saving the same thing again updates it, keeping the version.
        let again = s
            .save_update_hold(&HoldInput { note: "breaks DKMS".into(), ..input(h.id, HoldKind::Package, "linux", "") });
        let again = again.unwrap();
        assert_eq!(again.id, pkg.id);
        assert_eq!(again.version.as_deref(), Some("6.11.5"));
        assert_eq!(again.note, "breaks DKMS");

        let all = s.update_holds().unwrap();
        assert_eq!(all.len(), 3);
        assert!(host_hold(&all, h.id).is_some());
        assert_eq!(held_packages(&all, h.id), ["linux"]);
        assert!(image_held(&all, h.id, "web", "nginx:1.27"));
        assert!(!image_held(&all, h.id, "web", "redis:7"));
        assert!(!image_held(&all, h.id, "other", "nginx:1.27"));

        // A snooze that ran out is gone.
        assert_eq!(s.update_holds_at(far + 1).unwrap().len(), 2);
        s.delete_update_hold(pkg.id).unwrap();
        assert_eq!(s.update_holds().unwrap().len(), 1);
        s.delete_host(h.id).unwrap();
        assert!(s.update_holds().unwrap().is_empty());
    }

    #[test]
    fn pulls_skip_held_services() {
        let hold = |kind, target: &str, stack: &str| UpdateHold {
            id: 1,
            host_id: 1,
            kind,
            target: target.into(),
            stack: stack.into(),
            version: None,
            until: None,
            note: String::new(),
            created_at: 0,
        };
        let images = vec![
            ("nginx:1.27".to_owned(), vec!["web".to_owned()]),
            ("postgres:16".to_owned(), vec!["db".to_owned(), "db-replica".to_owned()]),
        ];
        assert_eq!(pull_services(&[], 1, "app", &images), None);
        let holds = [hold(HoldKind::Image, "postgres:16", "app")];
        assert_eq!(pull_services(&holds, 1, "app", &images), Some(vec!["web".to_owned()]));
        // Another stack's or host's hold doesn't count.
        assert_eq!(pull_services(&holds, 1, "other", &images), None);
        assert_eq!(pull_services(&holds, 2, "app", &images), None);
        let holds = [hold(HoldKind::Stack, "app", "")];
        assert_eq!(pull_services(&holds, 1, "app", &images), Some(vec![]));
    }
}
