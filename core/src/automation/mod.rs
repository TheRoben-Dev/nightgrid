//! Automation (M8): saved snippets, simple playbooks and the pieces a
//! multi-host run is built from.
//!
//! - **Snippets**: a command with `{{variables}}` (see [`template`]),
//!   scoped to every host, one host or one group, optionally run as root.
//! - **Playbooks**: an ordered list of [`Step`]s (command, upload a file,
//!   wait for a port, Docker action), run with stop-on-error.
//! - [`danger`] spots destructive commands so the UI asks first.
//! - [`run`] executes one step on one connection; the app runs hosts in
//!   parallel and streams the output.

pub mod danger;
pub mod run;
pub mod template;

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::docker::valid_container_ref;
use crate::hosts::{GroupId, Host, HostId};
use crate::store::Store;
use crate::{Error, Result};

pub type SnippetId = i32;
pub type PlaybookId = i32;

/// Where a snippet shows up: everywhere, on one host, or on a group's hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SnippetScope {
    #[default]
    Global,
    Host {
        host_id: HostId,
    },
    Group {
        group_id: GroupId,
    },
}

impl SnippetScope {
    pub fn applies_to(self, h: &Host) -> bool {
        match self {
            Self::Global => true,
            Self::Host { host_id } => h.id == host_id,
            Self::Group { group_id } => h.group_id == Some(group_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    pub id: SnippetId,
    pub name: String,
    pub command: String,
    pub description: String,
    pub scope: SnippetScope,
    /// Run as root through sudo (password typed per run or remembered per host).
    pub sudo: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct SnippetInput {
    pub name: String,
    pub command: String,
    pub description: String,
    pub scope: SnippetScope,
    pub sudo: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum DockerAction {
    Start,
    Stop,
    Restart,
    /// `docker pull <image>`.
    Pull,
}

impl DockerAction {
    pub fn verb(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Pull => "pull",
        }
    }
}

/// One playbook step. Text fields may use `{{variables}}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Step {
    Command {
        command: String,
        #[serde(default)]
        sudo: bool,
    },
    /// Copies a file from this computer over SFTP, as the login user.
    Upload {
        local_path: String,
        remote_path: String,
        /// Permissions for the uploaded file, e.g. 0o644.
        #[serde(default)]
        mode: Option<u32>,
    },
    /// Waits until a TCP port accepts connections, as seen from the host.
    WaitPort {
        #[serde(default = "localhost")]
        host: String,
        port: u16,
        #[serde(default = "default_wait")]
        timeout_secs: u32,
    },
    Docker {
        action: DockerAction,
        /// Container name/id, or image for pull.
        target: String,
    },
}

fn localhost() -> String {
    "127.0.0.1".into()
}
fn default_wait() -> u32 {
    60
}

impl Step {
    /// Text fields that may hold placeholders.
    pub fn texts(&self) -> Vec<&str> {
        match self {
            Self::Command { command, .. } => vec![command],
            Self::Upload { local_path, remote_path, .. } => vec![local_path, remote_path],
            Self::WaitPort { host, .. } => vec![host],
            Self::Docker { target, .. } => vec![target],
        }
    }

    /// Short label for progress lists: "docker restart web", "upload → /etc/x".
    pub fn label(&self) -> String {
        match self {
            Self::Command { command, sudo } => {
                let first = command.lines().next().unwrap_or("").trim();
                let more = if command.trim().lines().count() > 1 { " …" } else { "" };
                format!("{}{first}{more}", if *sudo { "# " } else { "$ " })
            }
            Self::Upload { remote_path, .. } => format!("upload → {remote_path}"),
            Self::WaitPort { host, port, timeout_secs } => format!("wait for {host}:{port} ({timeout_secs}s)"),
            Self::Docker { action, target } => format!("docker {} {target}", action.verb()),
        }
    }

    /// What this step would destroy or overwrite (empty if harmless).
    pub fn destructive(&self) -> Vec<String> {
        match self {
            Self::Command { command, .. } => danger::destructive(command),
            Self::Upload { remote_path, .. } => vec![format!("overwrite {remote_path}")],
            Self::Docker { action: a @ (DockerAction::Stop | DockerAction::Restart), target } => {
                vec![format!("docker {} {target}", a.verb())]
            }
            _ => Vec::new(),
        }
    }

    pub fn needs_sudo(&self) -> bool {
        matches!(self, Self::Command { sudo: true, .. })
    }

    fn validate(&self) -> Result<()> {
        let bad = |field: &'static str, reason: &str| Err(Error::Invalid { field, reason: reason.into() });
        match self {
            Self::Command { command, .. } if command.trim().is_empty() => bad("command", "a step needs a command"),
            Self::Upload { local_path, remote_path, .. }
                if local_path.trim().is_empty() || remote_path.trim().is_empty() =>
            {
                bad("upload", "choose a local file and a remote path")
            }
            Self::Upload { mode: Some(m), .. } if *m > 0o7777 => bad("mode", "permissions are at most 7777"),
            Self::WaitPort { port: 0, .. } => bad("port", "port must be 1–65535"),
            Self::WaitPort { timeout_secs, .. } if !(1..=3600).contains(timeout_secs) => {
                bad("timeout", "wait 1 to 3600 seconds")
            }
            Self::WaitPort { host, .. } if host.trim().is_empty() || host.contains(char::is_whitespace) => {
                bad("host", "enter a host name or address")
            }
            Self::Docker { action, target }
                if target.trim().is_empty()
                    || (!target.contains("{{") && !valid_docker_target(*action, target.trim())) =>
            {
                bad("target", "enter a container (or image) name")
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Playbook {
    pub id: PlaybookId,
    pub name: String,
    pub description: String,
    pub steps: Vec<Step>,
    pub stop_on_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", default)]
pub struct PlaybookInput {
    pub name: String,
    pub description: String,
    pub steps: Vec<Step>,
    pub stop_on_error: bool,
}

impl Default for PlaybookInput {
    fn default() -> Self {
        Self { name: String::new(), description: String::new(), steps: Vec::new(), stop_on_error: true }
    }
}

impl Playbook {
    pub fn variables(&self) -> Vec<template::Variable> {
        template::variables(self.steps.iter().flat_map(Step::texts))
    }
}

/// Container names for start/stop/restart; image references for pull.
pub fn valid_docker_target(action: DockerAction, s: &str) -> bool {
    match action {
        DockerAction::Pull => {
            !s.is_empty()
                && s.len() <= 255
                && !s.starts_with('-')
                && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | '/' | ':' | '@'))
        }
        _ => valid_container_ref(s),
    }
}

pub const MAX_STEPS: usize = 50;
const MAX_COMMAND: usize = 64 * 1024;

fn check_name(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 80 {
        return Err(Error::Invalid { field: "name", reason: "give it a name (up to 80 characters)".into() });
    }
    Ok(n.to_owned())
}

fn check_command(c: &str) -> Result<()> {
    if c.trim().is_empty() {
        return Err(Error::Invalid { field: "command", reason: "enter a command".into() });
    }
    if c.len() > MAX_COMMAND {
        return Err(Error::Invalid { field: "command", reason: "commands are limited to 64 KB".into() });
    }
    Ok(())
}

pub fn normalize_snippet(i: &SnippetInput) -> Result<SnippetInput> {
    check_command(&i.command)?;
    Ok(SnippetInput {
        name: check_name(&i.name)?,
        command: i.command.trim_end().to_owned(),
        description: i.description.trim().to_owned(),
        scope: i.scope,
        sudo: i.sudo,
    })
}

pub fn normalize_playbook(i: &PlaybookInput) -> Result<PlaybookInput> {
    if i.steps.is_empty() || i.steps.len() > MAX_STEPS {
        return Err(Error::Invalid { field: "steps", reason: format!("a playbook has 1 to {MAX_STEPS} steps") });
    }
    for s in &i.steps {
        if let Step::Command { command, .. } = s {
            check_command(command)?;
        }
        s.validate()?;
    }
    Ok(PlaybookInput {
        name: check_name(&i.name)?,
        description: i.description.trim().to_owned(),
        steps: i.steps.clone(),
        stop_on_error: i.stop_on_error,
    })
}

const SNIPPET_COLS: &str = "id, name, command, description, scope, host_id, group_id, sudo";

fn snippet_row(r: &Row<'_>) -> rusqlite::Result<Snippet> {
    let scope: String = r.get(4)?;
    let host_id: Option<HostId> = r.get(5)?;
    let group_id: Option<GroupId> = r.get(6)?;
    let scope = match (scope.as_str(), host_id, group_id) {
        ("host", Some(host_id), _) => SnippetScope::Host { host_id },
        ("group", _, Some(group_id)) => SnippetScope::Group { group_id },
        _ => SnippetScope::Global,
    };
    Ok(Snippet { id: r.get(0)?, name: r.get(1)?, command: r.get(2)?, description: r.get(3)?, scope, sudo: r.get(7)? })
}

fn scope_cols(s: SnippetScope) -> (&'static str, Option<HostId>, Option<GroupId>) {
    match s {
        SnippetScope::Global => ("global", None, None),
        SnippetScope::Host { host_id } => ("host", Some(host_id), None),
        SnippetScope::Group { group_id } => ("group", None, Some(group_id)),
    }
}

const PLAYBOOK_COLS: &str = "id, name, description, steps, stop_on_error";

fn playbook_row(r: &Row<'_>) -> rusqlite::Result<Playbook> {
    let steps: String = r.get(3)?;
    Ok(Playbook {
        id: r.get(0)?,
        name: r.get(1)?,
        description: r.get(2)?,
        // A step type a newer version wrote is dropped rather than failing the list.
        steps: serde_json::from_str::<Vec<serde_json::Value>>(&steps)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect(),
        stop_on_error: r.get(4)?,
    })
}

impl Store {
    pub fn list_snippets(&self) -> Result<Vec<Snippet>> {
        let mut stmt =
            self.conn().prepare(&format!("SELECT {SNIPPET_COLS} FROM snippets ORDER BY name COLLATE NOCASE, id"))?;
        let rows = stmt.query_map([], snippet_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn get_snippet(&self, id: SnippetId) -> Result<Snippet> {
        self.conn()
            .query_row(&format!("SELECT {SNIPPET_COLS} FROM snippets WHERE id = ?1"), [id], snippet_row)
            .optional()?
            .ok_or(Error::NotFound("snippet"))
    }

    pub fn create_snippet(&mut self, input: &SnippetInput) -> Result<Snippet> {
        let n = normalize_snippet(input)?;
        let (scope, host_id, group_id) = scope_cols(n.scope);
        self.conn().execute(
            "INSERT INTO snippets (name, command, description, scope, host_id, group_id, sudo)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![n.name, n.command, n.description, scope, host_id, group_id, n.sudo],
        )?;
        let id = self.conn().last_insert_rowid() as SnippetId;
        self.get_snippet(id)
    }

    pub fn update_snippet(&mut self, id: SnippetId, input: &SnippetInput) -> Result<Snippet> {
        let n = normalize_snippet(input)?;
        let (scope, host_id, group_id) = scope_cols(n.scope);
        let changed = self.conn().execute(
            "UPDATE snippets SET name = ?2, command = ?3, description = ?4, scope = ?5, host_id = ?6,
                    group_id = ?7, sudo = ?8, updated_at = unixepoch()
             WHERE id = ?1",
            params![id, n.name, n.command, n.description, scope, host_id, group_id, n.sudo],
        )?;
        if changed == 0 {
            return Err(Error::NotFound("snippet"));
        }
        self.get_snippet(id)
    }

    pub fn delete_snippet(&mut self, id: SnippetId) -> Result<()> {
        if self.conn().execute("DELETE FROM snippets WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("snippet"));
        }
        Ok(())
    }

    pub fn list_playbooks(&self) -> Result<Vec<Playbook>> {
        let mut stmt =
            self.conn().prepare(&format!("SELECT {PLAYBOOK_COLS} FROM playbooks ORDER BY name COLLATE NOCASE, id"))?;
        let rows = stmt.query_map([], playbook_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn get_playbook(&self, id: PlaybookId) -> Result<Playbook> {
        self.conn()
            .query_row(&format!("SELECT {PLAYBOOK_COLS} FROM playbooks WHERE id = ?1"), [id], playbook_row)
            .optional()?
            .ok_or(Error::NotFound("playbook"))
    }

    pub fn create_playbook(&mut self, input: &PlaybookInput) -> Result<Playbook> {
        let n = normalize_playbook(input)?;
        self.conn().execute(
            "INSERT INTO playbooks (name, description, steps, stop_on_error) VALUES (?1, ?2, ?3, ?4)",
            params![n.name, n.description, serde_json::to_string(&n.steps)?, n.stop_on_error],
        )?;
        let id = self.conn().last_insert_rowid() as PlaybookId;
        self.get_playbook(id)
    }

    pub fn update_playbook(&mut self, id: PlaybookId, input: &PlaybookInput) -> Result<Playbook> {
        let n = normalize_playbook(input)?;
        let changed = self.conn().execute(
            "UPDATE playbooks SET name = ?2, description = ?3, steps = ?4, stop_on_error = ?5, updated_at = unixepoch()
             WHERE id = ?1",
            params![id, n.name, n.description, serde_json::to_string(&n.steps)?, n.stop_on_error],
        )?;
        if changed == 0 {
            return Err(Error::NotFound("playbook"));
        }
        self.get_playbook(id)
    }

    pub fn delete_playbook(&mut self, id: PlaybookId) -> Result<()> {
        if self.conn().execute("DELETE FROM playbooks WHERE id = ?1", [id])? == 0 {
            return Err(Error::NotFound("playbook"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::HostInput;

    fn cmd(c: &str) -> Step {
        Step::Command { command: c.into(), sudo: false }
    }

    #[test]
    fn snippets_crud_and_scope() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&HostInput { name: "server01".into(), hostname: "10.0.0.1".into(), ..Default::default() })
            .unwrap();
        let a = s
            .create_snippet(&SnippetInput {
                name: "  Tail logs ".into(),
                command: "docker logs --tail {{lines=100}} {{container}}\n".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            (a.name.as_str(), a.command.as_str(), a.scope),
            ("Tail logs", "docker logs --tail {{lines=100}} {{container}}", SnippetScope::Global)
        );
        let b = s
            .create_snippet(&SnippetInput {
                name: "Restart app".into(),
                command: "systemctl restart app".into(),
                scope: SnippetScope::Host { host_id: h.id },
                sudo: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(b.scope, SnippetScope::Host { host_id: h.id });
        assert!(b.scope.applies_to(&h) && a.scope.applies_to(&h));
        assert!(!SnippetScope::Group { group_id: 9 }.applies_to(&h));
        assert_eq!(s.list_snippets().unwrap().len(), 2);
        let b2 = s
            .update_snippet(b.id, &SnippetInput { name: "R".into(), command: "x".into(), ..Default::default() })
            .unwrap();
        assert_eq!((b2.scope, b2.sudo), (SnippetScope::Global, false));
        assert!(s.create_snippet(&SnippetInput { name: "x".into(), ..Default::default() }).is_err());
        assert!(s.create_snippet(&SnippetInput { command: "x".into(), ..Default::default() }).is_err());
        // Host-scoped snippets go with their host.
        s.update_snippet(
            b.id,
            &SnippetInput {
                name: "R".into(),
                command: "x".into(),
                scope: SnippetScope::Host { host_id: h.id },
                sudo: false,
                description: String::new(),
            },
        )
        .unwrap();
        s.delete_host(h.id).unwrap();
        assert_eq!(s.list_snippets().unwrap().len(), 1);
        s.delete_snippet(a.id).unwrap();
        assert!(s.delete_snippet(a.id).is_err());
    }

    #[test]
    fn playbooks_crud_and_validation() {
        let mut s = Store::open_in_memory().unwrap();
        let steps = vec![
            cmd("cd /opt/app && git pull"),
            Step::Upload { local_path: "/tmp/app.env".into(), remote_path: "/opt/app/.env".into(), mode: Some(0o600) },
            Step::Docker { action: DockerAction::Restart, target: "{{container}}".into() },
            Step::WaitPort { host: "127.0.0.1".into(), port: 8080, timeout_secs: 30 },
        ];
        let p = s
            .create_playbook(&PlaybookInput { name: "Deploy".into(), steps: steps.clone(), ..Default::default() })
            .unwrap();
        assert_eq!((p.steps.clone(), p.stop_on_error), (steps, true));
        assert_eq!(p.variables().iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), ["container"]);
        assert_eq!(p.steps[1].destructive(), ["overwrite /opt/app/.env"]);
        assert_eq!(p.steps[2].destructive(), ["docker restart {{container}}"]);
        assert!(p.steps[3].destructive().is_empty());
        assert_eq!(p.steps[3].label(), "wait for 127.0.0.1:8080 (30s)");

        let mut bad = |steps: Vec<Step>| {
            s.create_playbook(&PlaybookInput { name: "x".into(), steps, ..Default::default() }).is_err()
        };
        assert!(bad(vec![]));
        assert!(bad(vec![cmd(" ")]));
        assert!(bad(vec![Step::WaitPort { host: "x".into(), port: 0, timeout_secs: 5 }]));
        assert!(bad(vec![Step::Docker { action: DockerAction::Stop, target: "a b".into() }]));
        assert!(bad(vec![Step::Upload { local_path: "".into(), remote_path: "/x".into(), mode: None }]));

        let p2 = s
            .update_playbook(
                p.id,
                &PlaybookInput {
                    name: "Deploy 2".into(),
                    steps: vec![cmd("true")],
                    stop_on_error: false,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!((p2.name.as_str(), p2.steps.len(), p2.stop_on_error), ("Deploy 2", 1, false));
        // Unknown step kinds (from a newer version) are skipped, not fatal.
        s.conn()
            .execute("UPDATE playbooks SET steps = ?1", [r#"[{"type":"teleport"},{"type":"command","command":"ls"}]"#])
            .unwrap();
        assert_eq!(s.get_playbook(p.id).unwrap().steps, vec![cmd("ls")]);
        s.delete_playbook(p.id).unwrap();
        assert!(s.list_playbooks().unwrap().is_empty());
    }

    #[test]
    fn step_json_shape() {
        let j = serde_json::to_value(Step::WaitPort { host: "db".into(), port: 5432, timeout_secs: 10 }).unwrap();
        assert_eq!(j, serde_json::json!({"type": "waitPort", "host": "db", "port": 5432, "timeoutSecs": 10}));
        let s: Step = serde_json::from_value(serde_json::json!({"type": "waitPort", "port": 80})).unwrap();
        assert_eq!(s, Step::WaitPort { host: "127.0.0.1".into(), port: 80, timeout_secs: 60 });
    }
}
