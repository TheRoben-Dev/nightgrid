# Manage packages with apt and dnf

Install, update, hold and clean up software on Debian, Ubuntu and Fedora-family servers, and find which package a file or command comes from.

## What you end up with

- The handful of commands you use every week on both families
- Packages held at a version when an update would break something
- A record of what's installed, to rebuild a server from

## Part 1 · Everyday commands

- **Refresh and upgrade**: `sudo apt update && sudo apt upgrade` · `sudo dnf upgrade --refresh`
- **Install**: `sudo apt install htop` · `sudo dnf install htop`
- **Remove**: `sudo apt purge htop` (with its config) · `sudo dnf remove htop`
- **Search**: `apt search smart` · `dnf search smart`
- **Clean leftovers**: `sudo apt autoremove` · `sudo dnf autoremove`

The first command is Debian and Ubuntu, the second Fedora, RHEL and Rocky.

## Part 2 · Who owns what

```sh
dpkg -S /usr/bin/ss          # Debian: which package has this file
apt-file search bin/iostat    # file from a package you don't have (apt install apt-file; apt-file update)
rpm -qf /usr/bin/ss          # Fedora
dnf provides '*/bin/iostat'   # Fedora, not installed yet
```

## Part 3 · Hold a version

- [ ] Debian/Ubuntu: `sudo apt-mark hold docker-ce` and `apt-mark showhold`; `unhold` to release.
- [ ] Fedora: `sudo dnf install 'dnf-command(versionlock)'` then `sudo dnf versionlock add docker-ce`.
- [ ] Note why you held it. Holds are easy to forget and block security fixes.

## Part 4 · Third-party repositories

- [ ] Prefer repos with a signed key file in `/etc/apt/keyrings/` and a `signed-by=` in their `.list` or `.sources` file. Old `apt-key` instructions are deprecated.
- [ ] List what you've added: `ls /etc/apt/sources.list.d/` or `dnf repolist`.

## Part 5 · Save the package list

```sh
apt-mark showmanual > packages-$(hostname).txt          # Debian
dnf repoquery --userinstalled > packages-$(hostname).txt # Fedora
```

- [ ] Reinstall on a new server with `xargs -a packages.txt sudo apt install -y`.

#### Check

- [ ] `apt list --upgradable` (or `dnf check-update`) shows only what you expect, and `apt-mark showhold` only what you meant to hold.

## In Nightgrid

- [Fleet](view:fleet) → **Updates** shows pending and security updates and reboot-needed for every host, and applies them in batches (see [Patch every host from one place](kb:howto/fleet-updates)).
- [System](view:system) → **config** saves a baseline that includes the package list, so a package that appeared or vanished shows as drift.

## Limits and gotchas

- Mixing distro releases in one sources list ("FrankenDebian") breaks upgrades. Use backports or containers instead.
- `apt upgrade` holds back packages that need new dependencies; `apt full-upgrade` installs them.
- rpm-ostree systems (Fedora Atomic, Bazzite) layer packages differently: see [Atomic Fedora hosts (rpm-ostree)](kb:linux/rpm-ostree).

## Related

- [Automatic security updates](kb:apps/unattended-upgrades)
- [Upgrade Debian or Ubuntu to a new release](kb:linux/release-upgrade)
- [Track config drift and host history](kb:howto/config-drift)
