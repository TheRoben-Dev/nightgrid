//! Turns stored hosts (plus their ProxyJump chain) into connection targets.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::client::{Auth, Target};
use crate::hosts::{AuthMethod, Host, HostId};
use crate::secrets::{Secret, SecretKind};
use crate::store::Store;
use crate::{Error, Result};

/// One hop plus the stored host it came from (if any).
#[derive(Debug, Clone)]
pub struct Hop {
    pub host_id: Option<HostId>,
    pub target: Target,
}

/// Looks up a secret for a hop: `(host id, kind, target label)`.
pub type SecretLookup<'a> = dyn Fn(Option<HostId>, SecretKind, &str) -> Option<Arc<Secret>> + 'a;

pub fn local_username() -> String {
    std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "root".into())
}

pub fn expand_home(p: &str, home: Option<&Path>) -> PathBuf {
    match (p.strip_prefix("~/").or_else(|| p.strip_prefix("~\\")), home) {
        (Some(rest), Some(h)) => h.join(rest),
        _ if p == "~" => home.map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from(p)),
        _ => PathBuf::from(p),
    }
}

/// Parses `[user@]host[:port]` (IPv6 as `[::1]:22`).
pub fn parse_jump(spec: &str) -> Option<(Option<String>, String, u16)> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }
    let (user, rest) = match spec.rsplit_once('@') {
        Some((u, r)) => (Some(u.to_owned()), r),
        None => (None, spec),
    };
    let (host, port) = if let Some(r) = rest.strip_prefix('[') {
        let (h, after) = r.split_once(']')?;
        (h.to_owned(), after.strip_prefix(':').and_then(|p| p.parse().ok()).unwrap_or(22))
    } else if let Some((h, p)) = rest.rsplit_once(':').filter(|(h, _)| !h.contains(':')) {
        (h.to_owned(), p.parse().ok()?)
    } else {
        (rest.to_owned(), 22)
    };
    if host.is_empty() {
        return None;
    }
    Some((user, host, port))
}

fn label(user: &str, host: &str, port: u16) -> String {
    if port == 22 {
        format!("{user}@{host}")
    } else {
        format!("{user}@{host}:{port}")
    }
}

fn hop_for_host(h: &Host, home: Option<&Path>, secrets: &SecretLookup<'_>) -> Hop {
    let user = h.username.clone().unwrap_or_else(local_username);
    let lbl = label(&user, &h.hostname, h.port);
    let identity = h.identity_file.as_deref().map(|p| expand_home(p, home));
    let auth = match h.auth_method {
        AuthMethod::Agent => {
            Auth::Agent { identity_file: identity, passphrase: secrets(Some(h.id), SecretKind::KeyPassphrase, &lbl) }
        }
        AuthMethod::Key => Auth::Key {
            path: identity.unwrap_or_default(),
            passphrase: secrets(Some(h.id), SecretKind::KeyPassphrase, &lbl),
        },
        AuthMethod::Password => Auth::Password { password: secrets(Some(h.id), SecretKind::HostPassword, &lbl) },
    };
    Hop { host_id: Some(h.id), target: Target { host: h.hostname.clone(), port: h.port, user, auth } }
}

/// Resolves a stored host and its ProxyJump chain. Jump entries that name
/// another stored host (by ssh alias or name) use that host's settings;
/// anything else is parsed as `[user@]host[:port]` with agent auth.
pub fn resolve(store: &Store, id: HostId, home: Option<&Path>, secrets: &SecretLookup<'_>) -> Result<(Hop, Vec<Hop>)> {
    let host = store.get_host(id)?.ok_or(Error::NotFound("host"))?;
    let mut jumps = Vec::new();
    if let Some(pj) = &host.proxy_jump {
        for item in pj.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            if let Some(h) = store.find_host_by_name_or_alias(item)? {
                if h.id == id {
                    return Err(Error::Invalid {
                        field: "proxyJump",
                        reason: "host cannot jump through itself".into(),
                    });
                }
                jumps.push(hop_for_host(&h, home, secrets));
            } else {
                let (user, hostname, port) = parse_jump(item)
                    .ok_or_else(|| Error::Invalid { field: "proxyJump", reason: format!("cannot parse {item:?}") })?;
                let user = user.unwrap_or_else(local_username);
                let lbl = label(&user, &hostname, port);
                jumps.push(Hop {
                    host_id: None,
                    target: Target {
                        host: hostname,
                        port,
                        user,
                        auth: Auth::Agent {
                            identity_file: None,
                            passphrase: secrets(None, SecretKind::KeyPassphrase, &lbl),
                        },
                    },
                });
            }
        }
    }
    Ok((hop_for_host(&host, home, secrets), jumps))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosts::HostInput;

    #[test]
    fn parses_jump_specs() {
        assert_eq!(parse_jump("bastion"), Some((None, "bastion".into(), 22)));
        assert_eq!(parse_jump("me@b.lan:2200"), Some((Some("me".into()), "b.lan".into(), 2200)));
        assert_eq!(parse_jump("[::1]:2222"), Some((None, "::1".into(), 2222)));
        assert_eq!(parse_jump("fe80::1"), Some((None, "fe80::1".into(), 22)));
        assert_eq!(parse_jump(""), None);
        assert_eq!(parse_jump("h:notaport"), None);
    }

    #[test]
    fn resolves_chain_and_secrets() {
        let mut s = Store::open_in_memory().unwrap();
        let bastion = s
            .create_host(&HostInput {
                name: "Bastion".into(),
                hostname: "bastion.example".into(),
                username: Some("jump".into()),
                ssh_alias: Some("bastion".into()),
                auth_method: AuthMethod::Password,
                ..Default::default()
            })
            .unwrap();
        let inner = s
            .create_host(&HostInput {
                name: "Inner".into(),
                hostname: "10.0.0.5".into(),
                port: Some(2222),
                username: Some("roben".into()),
                identity_file: Some("~/.ssh/id_ed25519".into()),
                proxy_jump: Some("bastion, ops@edge.example:2200".into()),
                ..Default::default()
            })
            .unwrap();
        let lookup = |id: Option<HostId>, kind: SecretKind, _l: &str| {
            (id == Some(bastion.id) && kind == SecretKind::HostPassword).then(|| Arc::new(Secret::new("pw")))
        };
        let home = PathBuf::from("/home/r");
        let (target, jumps) = resolve(&s, inner.id, Some(&home), &lookup).unwrap();
        assert_eq!(target.target.label(), "roben@10.0.0.5:2222");
        match &target.target.auth {
            Auth::Agent { identity_file, .. } => {
                assert_eq!(identity_file.as_deref(), Some(home.join(".ssh/id_ed25519").as_path()))
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(jumps.len(), 2);
        assert_eq!(jumps[0].host_id, Some(bastion.id));
        assert!(matches!(&jumps[0].target.auth, Auth::Password { password: Some(p) } if p.expose() == "pw"));
        assert_eq!(jumps[1].target.label(), "ops@edge.example:2200");
        assert_eq!(jumps[1].host_id, None);
    }

    #[test]
    fn rejects_self_jump() {
        let mut s = Store::open_in_memory().unwrap();
        let h = s
            .create_host(&HostInput {
                name: "loop".into(),
                hostname: "loop.lan".into(),
                proxy_jump: Some("loop".into()),
                ..Default::default()
            })
            .unwrap();
        assert!(resolve(&s, h.id, None, &|_, _, _| None).is_err());
    }
}
