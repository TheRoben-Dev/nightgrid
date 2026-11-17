# Automatic security updates

Let each server install security fixes by itself every day, reboot at a quiet time when a kernel needs it, and tell you what it did.

![What unattended-upgrades does on its own.](fig:app-unattended)

## What you end up with

- Daily security updates on Debian/Ubuntu (unattended-upgrades) or Fedora/RHEL (dnf-automatic)
- Optional automatic reboots at a fixed time
- The finding cleared in Nightgrid's security score

## Part 1 · Debian and Ubuntu

- [ ] Install and enable:

```sh
sudo apt install -y unattended-upgrades apt-listchanges
sudo dpkg-reconfigure -plow unattended-upgrades
```

- [ ] Choose reboots in `/etc/apt/apt.conf.d/50unattended-upgrades` (uncomment and set):

```
Unattended-Upgrade::Automatic-Reboot "true";
Unattended-Upgrade::Automatic-Reboot-Time "04:30";
Unattended-Upgrade::Remove-Unused-Kernel-Packages "true";
```

- [ ] Test without changing anything:

```sh
sudo unattended-upgrade --dry-run --debug
```

## Part 2 · Fedora, RHEL, Alma, Rocky

- [ ] Fedora 41+ (dnf5):

```sh
sudo dnf install -y dnf5-plugin-automatic
sudo cp /usr/share/dnf5/dnf5-plugins/automatic.conf /etc/dnf/automatic.conf
```

- [ ] RHEL/Alma/Rocky 9: `sudo dnf install -y dnf-automatic`.
- [ ] In `/etc/dnf/automatic.conf` set `upgrade_type = security` and `apply_updates = yes` (and `reboot = when-needed` if you want it), then enable the timer: `dnf5-automatic.timer` on Fedora 41+, `dnf-automatic.timer` on RHEL 9 and its rebuilds.

```sh
sudo systemctl enable --now dnf5-automatic.timer
```

## Part 3 · In Nightgrid

- [ ] [System](view:system) → **security** stops flagging automatic updates.
- [ ] [Fleet](view:fleet) → **Updates** still shows non-security updates for you to apply in batches.
- [ ] A reboot-needed flag on a host means the automatic reboot is off or hasn't happened yet.
- [ ] System → **timeline** shows each apt or dnf transaction.

#### Check

- [ ] `systemctl list-timers | grep -E "apt-daily|dnf"` shows the next run.

## When Nightgrid is closed

- Everything here runs on the hosts by themselves.

## Limits and gotchas

- Proxmox nodes: leave unattended-upgrades for Debian security only; do Proxmox's own updates by hand with `apt full-upgrade`.
- Automatic reboots on a hypervisor or NAS take everything else down too. Stagger reboot times, or reboot those by hand.
- Containers aren't covered. See [Update containers without fear](kb:howto/container-updates).
- Mail reports need a working sendmail (see [Send mail from your servers](kb:apps/mail-relay)).

## Related

- [Patch every host from one place](kb:howto/fleet-updates)
- [Raise a host's security score](kb:howto/security-score)
- [Send mail from your servers](kb:apps/mail-relay)
