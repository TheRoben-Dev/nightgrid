//! "What's growing" (D-200): the biggest folders and files under a mount,
//! what changed since the last look, and the files that usually explain a
//! growing disk (VM disk images, checkpoints, Docker, the journal).
//!
//! One `du -x -a` pass (stays on that filesystem, so nested mounts aren't
//! counted twice) keeps folders two levels down plus anything of 256 MiB or
//! more. The result is saved **on the host** in `~/.nightgrid/growth/`
//! (mode 700) so the next scan can say what grew: nothing about the files
//! is kept on this computer. Portable POSIX sh + awk: GNU and BSD `du`,
//! `find` and `cksum` (macOS CI runs it). Windows hosts get the same output
//! from PowerShell ([`crate::win::growth`]).

use serde::Serialize;

use super::sudo::{self, SudoAuth};
use super::{SystemError, SystemResult};
use crate::files::path::shell_quote;
use crate::ssh::client::Connection;

/// Files at least this big (KiB) are listed wherever they are.
pub const BIG_KIB: u64 = 262_144;
/// Folders this many levels below the scanned one are listed.
pub const DEPTH: usize = 2;
/// Lines kept per scan (biggest first).
pub const KEEP: usize = 400;
/// Where scans are kept on the host, under `$HOME`.
pub const DIR: &str = ".nightgrid/growth";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GrowthEntry {
    /// Path under the scanned folder, `/`-separated (`vms/win11.vhdx`).
    pub path: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub size: u64,
    pub dir: bool,
    /// A file modified within the last day.
    pub recent: bool,
    /// Change since the previous scan, bytes; `None` when it wasn't listed
    /// then (or there was no previous scan).
    #[cfg_attr(feature = "specta", specta(type = Option<i32>))]
    pub delta: Option<i64>,
    /// A VM disk image or VM bundle: the growth is inside that VM.
    pub vm_image: bool,
    /// What usually makes this grow and what to do, for well-known paths.
    pub hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GrowthReport {
    /// The folder scanned, as given.
    pub root: String,
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub at: i64,
    /// When the previous scan of this folder ran, if one was saved.
    #[cfg_attr(feature = "specta", specta(type = Option<u32>))]
    pub prev_at: Option<i64>,
    /// Everything under the folder on its filesystem, bytes.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub total: u64,
    #[cfg_attr(feature = "specta", specta(type = Option<i32>))]
    pub total_delta: Option<i64>,
    /// Biggest first; the folder itself left out.
    pub entries: Vec<GrowthEntry>,
    /// Folders or files that couldn't be read (permissions).
    pub denied: u32,
    /// The folder doesn't exist or can't be entered.
    pub missing: bool,
}

/// The scan for `root`, run as the login (or root through `sudo`).
pub fn command(root: &str) -> String {
    format!(
        r#"LC_ALL=C; export LC_ALL
umask 077
m={root}
G="$HOME/{DIR}"
now=$(date +%s)
echo "@@now $now"
if [ ! -d "$m" ] || ! cd "$m" 2>/dev/null; then echo "@@missing"; exit 0; fi
k=$(printf '%s' "$m" | cksum | awk '{{print $1}}')
f="$G/$k.tsv"
if [ -r "$f" ]; then echo "@@prev"; cat "$f"; fi
t="${{TMPDIR:-/tmp}}/ng-growth.$$"
du -x -k -a . 2>"$t.err" | awk -F'\t' -v big={BIG_KIB} -v depth={DEPTH} '{{ p=$2; n=gsub(/\//,"/",p); if (n<=depth || $1+0>=big) print }}' | sort -rn | head -n {KEEP} > "$t"
echo "@@scan"
tab=$(printf '\t')
while IFS="$tab" read -r s p; do
  ty=f; [ -d "$p" ] && ty=d
  r=0
  if [ "$ty" = f ] && [ -n "$(find "$p" -prune -mtime -1 2>/dev/null)" ]; then r=1; fi
  printf '%s\t%s\t%s\t%s\n' "$s" "$ty" "$r" "$p"
done < "$t"
echo "@@denied $(wc -l < "$t.err" | tr -d ' ')"
if mkdir -p "$G" 2>/dev/null && [ -w "$G" ]; then
  {{ echo "@at $now"; cut -f1,2 "$t"; }} > "$f.new" 2>/dev/null && mv -f "$f.new" "$f" 2>/dev/null
fi
rm -f "$t" "$t.err"
"#,
        root = shell_quote(root),
    )
}

/// `./a/b` → `a/b`; `.` → ``.
fn rel(p: &str) -> String {
    let p = p.trim_end_matches(['\r']);
    p.strip_prefix("./").unwrap_or(if p == "." { "" } else { p }).replace('\\', "/")
}

/// Parses the scan's output (shared by the POSIX and PowerShell scans).
pub fn parse(root: &str, out: &str) -> GrowthReport {
    let mut r = GrowthReport {
        root: root.to_owned(),
        at: 0,
        prev_at: None,
        total: 0,
        total_delta: None,
        entries: Vec::new(),
        denied: 0,
        missing: false,
    };
    let mut prev: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut section = "";
    for line in out.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(v) = line.strip_prefix("@@now ") {
            r.at = v.trim().parse().unwrap_or(0);
        } else if line == "@@missing" {
            r.missing = true;
        } else if line == "@@prev" || line == "@@scan" {
            section = if line == "@@prev" { "prev" } else { "scan" };
        } else if let Some(v) = line.strip_prefix("@@denied ") {
            r.denied = v.trim().parse().unwrap_or(0);
        } else if section == "prev" {
            if let Some(v) = line.strip_prefix("@at ") {
                r.prev_at = v.trim().parse().ok();
            } else if let Some((s, p)) = line.split_once('\t') {
                if let Ok(kib) = s.trim().parse::<u64>() {
                    prev.insert(rel(p), kib * 1024);
                }
            }
        } else if section == "scan" {
            let mut f = line.splitn(4, '\t');
            let (Some(s), Some(ty), Some(recent), Some(p)) = (f.next(), f.next(), f.next(), f.next()) else { continue };
            let Ok(kib) = s.trim().parse::<u64>() else { continue };
            let path = rel(p);
            let size = kib * 1024;
            let delta = if r.prev_at.is_some() { prev.get(&path).map(|&b| size as i64 - b as i64) } else { None };
            if path.is_empty() {
                r.total = size;
                r.total_delta = delta;
                continue;
            }
            let dir = ty == "d";
            let vm_image = vm_image(&path, dir);
            r.entries.push(GrowthEntry {
                hint: hint(&path, dir, vm_image),
                path,
                size,
                dir,
                recent: recent == "1",
                delta,
                vm_image,
            });
        }
    }
    r.entries.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
    r
}

const VM_FILES: [&str; 10] = [".vhdx", ".vhd", ".avhdx", ".avhd", ".qcow2", ".qcow", ".vmdk", ".vdi", ".hdd", ".qed"];
const VM_BUNDLES: [&str; 4] = [".pvm", ".vmwarevm", ".utm", ".vbox-prev"];

/// A VM disk image (or a VM bundle folder on macOS).
pub fn vm_image(path: &str, dir: bool) -> bool {
    let p = path.to_ascii_lowercase();
    if dir {
        return VM_BUNDLES.iter().any(|e| p.ends_with(e));
    }
    VM_FILES.iter().any(|e| p.ends_with(e))
        // Raw images are only flagged where they usually live.
        || ((p.ends_with(".img") || p.ends_with(".raw")) && (p.contains("libvirt") || p.contains("images") || p.contains("vm")))
}

fn hint(path: &str, dir: bool, vm: bool) -> Option<String> {
    let p = path.to_ascii_lowercase();
    if !dir && (p.ends_with(".avhdx") || p.ends_with(".avhd")) {
        return Some("A Hyper-V checkpoint: every write to the VM lands here until the checkpoint is deleted (which merges it back).".into());
    }
    if vm {
        return Some("A VM disk: the growth is inside that VM. Add the VM as a host and run What's growing there to find the files. Images don't shrink when files are deleted inside, so compact it afterwards (Optimize-VHD, qemu-img convert, vmware-vdiskmanager -k or the hypervisor's own compact).".into());
    }
    let seg = |s: &str| {
        p == s || p.starts_with(&format!("{s}/")) || p.ends_with(&format!("/{s}")) || p.contains(&format!("/{s}/"))
    };
    if seg("lib/docker") || seg("docker/overlay2") || seg("docker/volumes") {
        return Some("Docker images, containers and volumes: Fleet → UPDATES and Docker show what's unused, or run docker system df and docker system prune.".into());
    }
    if seg("log/journal") {
        return Some(
            "The systemd journal: journalctl --vacuum-size=500M trims it, SystemMaxUse= in journald.conf caps it."
                .into(),
        );
    }
    if seg("containerd") || seg("lib/containers") {
        return Some("Container images and layers (containerd/Podman): prune unused images.".into());
    }
    None
}

/// Scans `root` on a POSIX host. `auth` reads as root (folders the login
/// can't enter).
pub async fn scan(conn: &Connection, auth: &SudoAuth, root: &str) -> SystemResult<GrowthReport> {
    check_root(root)?;
    let o = sudo::run(conn, auth, &command(root)).await?;
    Ok(parse(root, &o.stdout))
}

/// A path to scan: absolute, no control characters.
pub fn check_root(root: &str) -> SystemResult<()> {
    let windows = root.as_bytes().get(1) == Some(&b':');
    if !(root.starts_with('/') || windows) || root.len() > 1024 || root.chars().any(char::is_control) {
        return Err(SystemError::other("pick a folder by its full path"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_compares() {
        let out = "@@now 1700086400\n@@prev\n@at 1700000000\n100\t.\n40\t./vms\n30\t./vms/win.vhdx\n10\t./logs\n\
                   @@scan\n2000\td\t0\t.\n1500\td\t0\t./vms\n1490\tf\t1\t./vms/win.vhdx\n300\td\t0\t./var/lib/docker\n\
                   12\td\t0\t./new\n@@denied 2\n";
        let r = parse("/mnt/h", out);
        assert_eq!(
            (r.at, r.prev_at, r.total, r.denied, r.missing),
            (1_700_086_400, Some(1_700_000_000), 2000 * 1024, 2, false)
        );
        assert_eq!(r.total_delta, Some(1900 * 1024));
        assert_eq!(
            r.entries.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(),
            ["vms", "vms/win.vhdx", "var/lib/docker", "new"]
        );
        let img = &r.entries[1];
        assert!(img.vm_image && img.recent && !img.dir);
        assert_eq!(img.delta, Some(1460 * 1024));
        assert!(img.hint.as_deref().unwrap().contains("inside that VM"));
        assert!(r.entries[2].hint.as_deref().unwrap().starts_with("Docker"));
        assert_eq!(r.entries[3].delta, None, "not in the previous scan");
        let first = parse("/x", "@@now 5\n@@scan\n8\td\t0\t.\n4\td\t0\t./a\n@@denied 0\n");
        assert_eq!((first.prev_at, first.entries[0].delta), (None, None));
        assert!(parse("/x", "@@now 5\n@@missing\n").missing);
    }

    #[test]
    fn images_and_roots() {
        assert!(vm_image("Hyper-V/Virtual Hard Disks/dc01.vhdx", false));
        assert!(vm_image("var/lib/libvirt/images/pihole.img", false));
        assert!(vm_image("Parallels/Win 11.pvm", true));
        assert!(!vm_image("photos/2024.img", false));
        assert!(hint("vms/a.avhdx", false, true).unwrap().starts_with("A Hyper-V checkpoint"));
        assert!(hint("var/log/journal", true, false).is_some());
        assert!(check_root("/mnt/h").is_ok() && check_root("H:\\").is_ok());
        assert!(check_root("relative").is_err() && check_root("/a\nb").is_err());
    }

    /// The real script on this machine (GNU on Linux CI, BSD on macOS CI).
    #[cfg(unix)]
    #[test]
    fn script_runs_here() {
        let base = std::env::temp_dir().join(format!("ng-growth-test-{}", std::process::id()));
        let home = base.join("home");
        let root = base.join("data");
        std::fs::create_dir_all(root.join("vms/deep/er")).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(root.join("vms/disk.qcow2"), vec![1u8; 300 * 1024]).unwrap();
        std::fs::write(root.join("vms/deep/er/x.bin"), vec![1u8; 64 * 1024]).unwrap();
        let run = || {
            let o = std::process::Command::new("sh")
                .arg("-c")
                .arg(command(root.to_str().unwrap()))
                .env("HOME", &home)
                .output()
                .unwrap();
            assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
            parse(root.to_str().unwrap(), &String::from_utf8_lossy(&o.stdout))
        };
        let a = run();
        assert!(a.prev_at.is_none() && a.at > 0 && !a.missing, "{a:?}");
        let paths: Vec<&str> = a.entries.iter().map(|e| e.path.as_str()).collect();
        assert!(
            paths.contains(&"vms") && paths.contains(&"vms/disk.qcow2") && paths.contains(&"vms/deep"),
            "{paths:?}"
        );
        assert!(!paths.contains(&"vms/deep/er"), "three levels down and small");
        let img = a.entries.iter().find(|e| e.path == "vms/disk.qcow2").unwrap();
        assert!(img.vm_image && img.recent && !img.dir);
        std::fs::write(root.join("vms/more.bin"), vec![1u8; 200 * 1024]).unwrap();
        let b = run();
        assert!(b.prev_at.is_some());
        let vms = b.entries.iter().find(|e| e.path == "vms").unwrap();
        assert!(vms.delta.unwrap() >= 150 * 1024, "{vms:?}");
        assert!(
            parse(
                "/nope",
                &String::from_utf8_lossy(
                    &std::process::Command::new("sh")
                        .arg("-c")
                        .arg(command("/nonexistent/ng"))
                        .output()
                        .unwrap()
                        .stdout
                )
            )
            .missing
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
