//! The feature store (D-206): optional components a user installs from
//! Settings → STORE instead of every view shipping switched on.
//!
//! The core app keeps the standard homelab essentials (hosts, overview,
//! terminal, files, Docker, services, one host's system view, the fleet's
//! updates, disks, backups and uptime, tunnels, network tools, alerts, notes
//! and the Library). Everything else is a component. A component that is not
//! installed is off in the strongest sense: its view is hidden and never
//! loaded, its pollers and checks don't run (it reads as a turned-off feature
//! everywhere through [`crate::settings::AppSettings::feature`]), and its data
//! is kept, so installing it again brings everything back.
//!
//! The UI mirrors this list in `ui/src/lib/storeCatalog.ts` (a test keeps them
//! in step). See docs/FEATURE_STORE.md for the design and the later parts
//! (signed per-component packs on their own update channel).

use serde::Serialize;

/// One installable component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Component {
    /// Store id (`proxmox`, `ups` …).
    pub id: &'static str,
    /// Feature ids (Settings → Features) it brings, with their children.
    pub features: &'static [&'static str],
}

/// Every component in the store.
pub const COMPONENTS: &[Component] = &[
    Component { id: "proxmox", features: &["proxmox"] },
    Component { id: "desktops", features: &["desktops"] },
    Component { id: "security", features: &["security"] },
    Component { id: "netmap", features: &["network"] },
    Component { id: "automation", features: &["automation"] },
    Component { id: "drift", features: &["drift"] },
    Component { id: "expiry", features: &["services.expiry"] },
    Component { id: "ups", features: &["fleet.ups"] },
    Component { id: "logsearch", features: &["fleet.logs"] },
    Component { id: "access", features: &["fleet.access"] },
    Component { id: "speed", features: &["fleet.speed"] },
    Component { id: "journal", features: &["fleet.journal"] },
    Component { id: "blueprints", features: &["fleet.blueprints"] },
    Component { id: "power", features: &["fleet.power"] },
    Component { id: "node", features: &["fleet.node"] },
    Component { id: "catalog", features: &["fleet.catalog"] },
    Component { id: "drills", features: &["fleet.drills"] },
    Component { id: "capacity", features: &["fleet.capacity"] },
    Component { id: "replay", features: &["fleet.replay"] },
    Component { id: "sound", features: &["sound"] },
];

pub fn by_id(id: &str) -> Option<&'static Component> {
    COMPONENTS.iter().find(|c| c.id == id)
}

/// `id` is `root` or one of its children (`fleet.ups.x` under `fleet.ups`).
fn under(id: &str, root: &str) -> bool {
    id == root || id.strip_prefix(root).is_some_and(|rest| rest.starts_with('.'))
}

/// The component a feature id belongs to (None: part of the core app).
pub fn owner(feature: &str) -> Option<&'static Component> {
    COMPONENTS.iter().find(|c| c.features.iter().any(|f| under(feature, f)))
}

/// Whether a feature can run with these components installed. `None` means
/// the list was never chosen, which reads as everything installed.
pub fn installed_for(installed: Option<&[String]>, feature: &str) -> bool {
    match (installed, owner(feature)) {
        (Some(list), Some(c)) => list.iter().any(|i| i == c.id),
        _ => true,
    }
}

/// What an existing user starts with when the store arrives: every component
/// with at least one of its features switched on. A feature they had turned
/// off isn't installed (its data is still kept), so nothing they used goes
/// missing and nothing they had hidden comes back.
pub fn seed_from(disabled: &[String]) -> Vec<String> {
    let off = |f: &str| disabled.iter().any(|d| under(f, d));
    COMPONENTS.iter().filter(|c| c.features.iter().any(|f| !off(f))).map(|c| c.id.to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::FEATURES;

    #[test]
    fn every_component_feature_is_a_known_feature() {
        for c in COMPONENTS {
            for f in c.features {
                assert!(FEATURES.contains(f), "{} brings unknown feature {f}", c.id);
            }
        }
        let mut ids: Vec<_> = COMPONENTS.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), COMPONENTS.len(), "duplicate component id");
    }

    #[test]
    fn owners() {
        assert_eq!(owner("proxmox.sandbox").map(|c| c.id), Some("proxmox"));
        assert_eq!(owner("fleet.ups").map(|c| c.id), Some("ups"));
        assert_eq!(owner("fleet"), None, "the fleet view itself is core");
        assert_eq!(owner("fleet.disks"), None);
        assert_eq!(owner("docker"), None);
        assert_eq!(owner("services.expiry").map(|c| c.id), Some("expiry"));
        assert_eq!(owner("services"), None);
        assert_eq!(owner("networkx"), None, "a prefix is not a parent");
    }

    #[test]
    fn installed_checks() {
        assert!(installed_for(None, "proxmox"));
        let list = vec!["ups".to_owned()];
        assert!(installed_for(Some(&list), "fleet.ups"));
        assert!(!installed_for(Some(&list), "proxmox.sandbox"));
        assert!(installed_for(Some(&list), "terminal"));
        assert!(installed_for(Some(&[]), "fleet"));
    }

    #[test]
    fn seeding_keeps_what_was_on() {
        assert_eq!(seed_from(&[]).len(), COMPONENTS.len());
        let s = seed_from(&["proxmox".into(), "fleet".into(), "sound".into()]);
        assert!(!s.contains(&"proxmox".to_owned()));
        assert!(!s.contains(&"ups".to_owned()), "fleet off turns its children off");
        assert!(!s.contains(&"sound".to_owned()));
        assert!(s.contains(&"desktops".to_owned()) && s.contains(&"drift".to_owned()));
    }

    #[test]
    fn ui_list_matches() {
        let ui = include_str!("../../ui/src/lib/storeCatalog.ts");
        for c in COMPONENTS {
            assert!(ui.contains(&format!("id: \"{}\"", c.id)), "{} missing in storeCatalog.ts", c.id);
            let feats = c.features.iter().map(|f| format!("\"{f}\"")).collect::<Vec<_>>().join(", ");
            assert!(ui.contains(&format!("features: [{feats}]")), "{} features differ in storeCatalog.ts", c.id);
        }
        assert_eq!(ui.matches(", features: [").count(), COMPONENTS.len(), "storeCatalog.ts has ids the backend lacks");
    }
}
