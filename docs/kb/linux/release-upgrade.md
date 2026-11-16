# Upgrade Debian or Ubuntu to a new release

Move a server to the next Debian or Ubuntu release with a backup, a checklist and a way back, instead of hoping.

## Before you start

- Read the release notes for the new version (Debian's "Issues to be aware of", Ubuntu's release notes).
- Snapshot or back up first: Proxmox → **SNAPSHOTS** for a guest, LVM or btrfs snapshot for a physical box, plus your normal backups.
- In Nightgrid, [System](view:system) → **config** → **Save baseline** so you can compare afterwards.

## Part 1 · Get current

```sh
sudo apt update && sudo apt full-upgrade -y
sudo apt autoremove --purge
sudo reboot
```

- [ ] Remove or disable third-party repositories you don't need; they're the most common source of breakage.

## Part 2 · Ubuntu

- [ ] Run inside `tmux` so a dropped SSH session doesn't kill the upgrade:

```sh
sudo apt install -y tmux && tmux
sudo do-release-upgrade
```

- [ ] LTS servers only see the next LTS once its first point release is out (or use `-d` to go earlier, at your own risk).

## Part 3 · Debian

- [ ] Change the codename in apt sources (for example `bookworm` to `trixie`) in `/etc/apt/sources.list` and files under `/etc/apt/sources.list.d/`, then in `tmux`:

```sh
sudo apt update
sudo apt upgrade --without-new-pkgs
sudo apt full-upgrade
sudo reboot
```

- [ ] Proxmox has its own upgrade guide and checker script (`pve8to9` and similar); follow that instead on a Proxmox node.

## Part 4 · After

- [ ] [System](view:system) → **config** → **Compare**: review changed configs (`*.dpkg-dist` files are the new defaults you didn't take).
- [ ] [System](view:system) → **security** and **services** for anything that failed to start.

#### Check

- [ ] `cat /etc/os-release` shows the new version, and every service check in [Services](view:services) is green.

## Limits and gotchas

- Skipping a release isn't supported on Debian; go one at a time.
- Kernel and ZFS module mismatches are the classic failure on ZFS hosts; keep the old kernel in the boot menu until the new one boots.

## Related

- [Patch every host from one place](kb:howto/fleet-updates)
- [Track config drift and host history](kb:howto/config-drift)
- [LVM: grow and snapshot volumes](kb:linux/lvm)
