//! What kind of storage a mount really is (D-200): a disk of this host, a
//! share from another machine (NFS, SMB, SSHFS), a folder or drive passed
//! in from the machine this host runs on (WSL's Windows drives, VM shared
//! folders), or a dataset of a pool whose free space it shares with others.
//!
//! The disk forecast uses this to count growth once, on the machine that
//! owns the disk, and to explain the rest instead of raising a second
//! alert. Decided from the filesystem type, the mount point and the device
//! column of `/proc/mounts` alone: nothing extra runs on the host.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum MountKind {
    /// A filesystem on this host's own disks.
    Local,
    /// A share served by another machine over the network.
    Network,
    /// A drive or folder of the machine this host runs on (WSL, a VM's
    /// shared folder): its disks and SMART belong to that machine.
    Passthrough,
    /// A dataset or volume sharing its free space with others in one pool
    /// (ZFS pool, APFS container), forecast as the pool.
    Pool,
}

/// What a mount is, with where it comes from when it isn't local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountInfo {
    pub kind: MountKind,
    /// Short words for a chip: "NFS · nas", "Windows drive H:", "ZFS tank".
    pub label: String,
    /// The serving machine of a network share (`nas`, `192.168.0.129`).
    pub server: Option<String>,
    /// The path or share on that machine (`/volume1/plex`, `media`).
    pub export: Option<String>,
    /// Pool, container or Windows drive letter the mount belongs to.
    pub group: Option<String>,
}

const NETWORK: [&str; 14] = [
    "nfs",
    "nfs4",
    "cifs",
    "smb3",
    "smbfs",
    "afpfs",
    "webdav",
    "davfs",
    "glusterfs",
    "ceph",
    "lustre",
    "beegfs",
    "gpfs",
    "ocfs2",
];
const NETWORK_FUSE: [&str; 7] = ["sshfs", "rclone", "davfs2", "glusterfs", "ceph", "s3fs", "smbnetfs"];
const PASSTHROUGH: [&str; 9] = ["9p", "v9fs", "drvfs", "wslfs", "virtiofs", "vboxsf", "vmhgfs", "prl_fs", "hgfs"];
const PASSTHROUGH_FUSE: [&str; 2] = ["vmhgfs-fuse", "prl_fsd"];

fn fuse_sub(fstype: &str) -> Option<&str> {
    fstype.strip_prefix("fuse.").or_else(|| fstype.strip_prefix("fuse-"))
}

/// The protocol word shown on a share's chip.
fn proto(fstype: &str) -> &'static str {
    match fstype {
        "nfs" | "nfs4" => "NFS",
        "cifs" | "smb3" | "smbfs" => "SMB",
        "afpfs" => "AFP",
        t if t.contains("sshfs") => "SSHFS",
        t if t.contains("rclone") => "RCLONE",
        t if t.contains("dav") => "WEBDAV",
        t if t.contains("s3fs") => "S3",
        _ => "SHARE",
    }
}

/// `nas:/volume1/plex`, `[fd00::2]:/srv`, `//nas/media/sub`,
/// `\\nas\media`, `user@box:/data`, `remote:bucket` → (server, export).
pub fn parse_remote(device: &str) -> Option<(String, String)> {
    let d = device.trim();
    if let Some(rest) = d.strip_prefix("//").or_else(|| d.strip_prefix("\\\\")) {
        let rest = rest.replace('\\', "/");
        let (server, share) = rest.split_once('/').unwrap_or((&rest, ""));
        let server = server.rsplit_once('@').map_or(server, |(_, s)| s);
        return (!server.is_empty()).then(|| (server.to_ascii_lowercase(), share.trim_end_matches('/').to_owned()));
    }
    let (server, export) = if let Some(rest) = d.strip_prefix('[') {
        let (ip, tail) = rest.split_once(']')?;
        (ip, tail.strip_prefix(':')?)
    } else {
        d.split_once(':')?
    };
    let server = server.rsplit_once('@').map_or(server, |(_, s)| s);
    if server.is_empty() || server.contains('/') {
        return None;
    }
    let export = if export.len() > 1 { export.trim_end_matches('/') } else { export };
    Some((server.to_ascii_lowercase(), export.to_owned()))
}

/// WSL's `/mnt/c` style mount of a Windows drive: its letter.
fn wsl_drive(mount: &str, device: &str) -> Option<char> {
    let dev = device.as_bytes();
    if dev.len() >= 2 && dev[1] == b':' && dev[0].is_ascii_alphabetic() {
        return Some(dev[0].to_ascii_uppercase() as char);
    }
    let rest = mount.strip_prefix("/mnt/")?;
    let mut c = rest.chars();
    match (c.next(), c.next()) {
        (Some(l), None) if l.is_ascii_alphabetic() => Some(l.to_ascii_uppercase()),
        _ => None,
    }
}

/// Classifies one mount from `/proc/mounts` (or `df`) columns.
pub fn classify(mount: &str, device: &str, fstype: &str) -> MountInfo {
    let t = fstype.to_ascii_lowercase();
    let t = t.as_str();
    let local = || MountInfo { kind: MountKind::Local, label: String::new(), server: None, export: None, group: None };

    let network = NETWORK.contains(&t)
        || t.starts_with("nfs")
        || fuse_sub(t).is_some_and(|s| NETWORK_FUSE.iter().any(|n| s.starts_with(n)));
    if network {
        let (server, export) = match parse_remote(device) {
            Some((s, e)) => (Some(s), Some(e)),
            None => (None, None),
        };
        let label = match &server {
            Some(s) => format!("{} · {s}", proto(t)),
            None => proto(t).to_owned(),
        };
        return MountInfo { kind: MountKind::Network, label, server, export, group: None };
    }

    let passthrough = PASSTHROUGH.contains(&t) || fuse_sub(t).is_some_and(|s| PASSTHROUGH_FUSE.contains(&s));
    if passthrough {
        // WSL mounts Windows drives as 9p (WSL 2) or drvfs (WSL 1).
        if matches!(t, "9p" | "drvfs" | "wslfs" | "v9fs") {
            if let Some(l) = wsl_drive(mount, device) {
                return MountInfo {
                    kind: MountKind::Passthrough,
                    label: format!("Windows drive {l}:"),
                    server: None,
                    export: None,
                    group: Some(format!("{l}:")),
                };
            }
        }
        return MountInfo {
            kind: MountKind::Passthrough,
            label: "shared from the VM host".into(),
            server: None,
            export: None,
            group: None,
        };
    }

    match t {
        // ZFS: `tank/media` → pool `tank`.
        "zfs" if !device.starts_with('/') => {
            let pool = device.split('/').next().unwrap_or(device);
            MountInfo {
                kind: MountKind::Pool,
                label: format!("ZFS {pool}"),
                server: None,
                export: None,
                group: Some(format!("zfs:{pool}")),
            }
        }
        // APFS: `/dev/disk3s5` → container `disk3`.
        "apfs" => {
            let dev = device.strip_prefix("/dev/").unwrap_or(device);
            match dev
                .rfind('s')
                .filter(|&i| i > 4 && dev[i + 1..].chars().all(|c| c.is_ascii_digit()) && i + 1 < dev.len())
            {
                Some(i) => MountInfo {
                    kind: MountKind::Pool,
                    label: format!("APFS {}", &dev[..i]),
                    server: None,
                    export: None,
                    group: Some(format!("apfs:{}", &dev[..i])),
                },
                None => local(),
            }
        }
        _ => local(),
    }
}

/// Whether `path` is `base` or inside it (`/volume1/plex` in `/volume1`).
pub fn within(path: &str, base: &str) -> bool {
    let base = if base.len() > 1 { base.trim_end_matches('/') } else { base };
    path == base || base == "/" && path.starts_with('/') || path.strip_prefix(base).is_some_and(|r| r.starts_with('/'))
}

/// Lower-case names a host may be called by on the network: each one and
/// its first label (`nas.lan` → `nas`).
pub fn host_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in names {
        let n = n.trim().trim_end_matches('.').to_ascii_lowercase();
        if n.is_empty() {
            continue;
        }
        let short = if n.parse::<std::net::IpAddr>().is_ok() { None } else { n.split('.').next().map(str::to_owned) };
        for v in std::iter::once(n.clone()).chain(short) {
            if !out.contains(&v) {
                out.push(v);
            }
        }
    }
    out
}

/// Whether a share's `server` names a host known by `names` ([`host_names`]).
pub fn server_is(server: &str, names: &[String]) -> bool {
    let s = server.trim_end_matches('.').to_ascii_lowercase();
    let short =
        if s.parse::<std::net::IpAddr>().is_ok() { s.clone() } else { s.split('.').next().unwrap_or(&s).to_owned() };
    names.iter().any(|n| *n == s || *n == short)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_sources() {
        assert_eq!(parse_remote("nas:/volume1/plex"), Some(("nas".into(), "/volume1/plex".into())));
        assert_eq!(
            parse_remote("192.168.0.129:/volume1/plex/"),
            Some(("192.168.0.129".into(), "/volume1/plex".into()))
        );
        assert_eq!(parse_remote("[fd00::2]:/srv"), Some(("fd00::2".into(), "/srv".into())));
        assert_eq!(parse_remote("//NAS/media/sub"), Some(("nas".into(), "media/sub".into())));
        assert_eq!(parse_remote("\\\\nas\\media"), Some(("nas".into(), "media".into())));
        assert_eq!(parse_remote("me@box:/data"), Some(("box".into(), "/data".into())));
        assert_eq!(parse_remote("/dev/sda1"), None);
        assert_eq!(parse_remote("tank/media"), None);
    }

    #[test]
    fn kinds() {
        let nfs = classify("/mnt/plex", "192.168.0.129:/volume1/plex", "nfs4");
        assert_eq!((nfs.kind, nfs.label.as_str()), (MountKind::Network, "NFS · 192.168.0.129"));
        assert_eq!(nfs.export.as_deref(), Some("/volume1/plex"));
        assert_eq!(classify("/mnt/media", "//nas/media", "cifs").label, "SMB · nas");
        assert_eq!(classify("/mnt/box", "me@box:/", "fuse.sshfs").label, "SSHFS · box");
        let h = classify("/mnt/h", "H:\\134", "9p");
        assert_eq!((h.kind, h.label.as_str()), (MountKind::Passthrough, "Windows drive H:"));
        assert_eq!(classify("/mnt/h", "drvfs", "9p").label, "Windows drive H:");
        assert_eq!(classify("/mnt/c", "C:\\", "drvfs").group.as_deref(), Some("C:"));
        assert_eq!(classify("/srv/share", "share", "virtiofs").kind, MountKind::Passthrough);
        assert_eq!(classify("/media/sf_x", "x", "vboxsf").kind, MountKind::Passthrough);
        let z = classify("/tank/media", "tank/media", "zfs");
        assert_eq!((z.kind, z.group.as_deref()), (MountKind::Pool, Some("zfs:tank")));
        assert_eq!(classify("/", "rpool/ROOT/ubuntu", "zfs").group.as_deref(), Some("zfs:rpool"));
        let a = classify("/System/Volumes/Data", "/dev/disk3s5", "apfs");
        assert_eq!((a.kind, a.group.as_deref()), (MountKind::Pool, Some("apfs:disk3")));
        assert_eq!(classify("/", "/dev/sda2", "ext4").kind, MountKind::Local);
        assert_eq!(classify("C:\\", "C:", "NTFS").kind, MountKind::Local);
        assert_eq!(classify("/", "/dev/mapper/vg-root", "xfs").kind, MountKind::Local);
    }

    #[test]
    fn names_and_paths() {
        let n = host_names(["Oracle", "oracle.lan", "100.64.0.9", ""]);
        assert_eq!(n, ["oracle", "oracle.lan", "100.64.0.9"]);
        assert!(server_is("ORACLE.lan", &n));
        assert!(server_is("oracle.tail1234.ts.net", &n));
        assert!(server_is("100.64.0.9", &n));
        assert!(!server_is("192.168.0.129", &n));
        assert!(within("/volume1/plex", "/volume1"));
        assert!(within("/volume1", "/volume1/"));
        assert!(within("/x", "/"));
        assert!(!within("/volume10", "/volume1"));
    }
}
