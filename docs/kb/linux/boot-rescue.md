# Fix a server that won't boot

A kernel update, a bad fstab line or a full disk can leave a server stuck at boot. Most of these are fixed from the console in minutes without reinstalling.

## What you end up with

- A way into the system from the GRUB menu or a live USB
- The usual causes checked in order
- The fix, and a note on how to avoid it next time

## Before you start

- Console access: a monitor and keyboard, IPMI/iDRAC, or the Proxmox console for a VM
- A live USB of the same distro, for the cases GRUB can't help

## Part 1 · Read where it stops

- [ ] Remove `quiet splash` from the kernel line (press `e` at the GRUB menu) to see boot messages.
- [ ] **"You are in emergency mode"**: usually an `/etc/fstab` entry for a disk that's missing. Log in as root and look at `journalctl -xb | grep -i -E "fail|mount"`.
- [ ] **Stuck at "A start job is running"**: a mount or network wait; it times out after 90 seconds, then shows which unit.
- [ ] **Kernel panic** or a black screen after a kernel update: pick the previous kernel under "Advanced options" in GRUB.

## Part 2 · fstab problems

- [ ] In emergency mode, remount root writable and fix the line:

```sh
mount -o remount,rw /
nano /etc/fstab
```

- [ ] Add `nofail` to data disks so a missing one never stops boot again: `UUID=... /srv/data ext4 defaults,nofail 0 2`.
- [ ] `systemctl daemon-reload && mount -a` checks the file before rebooting.

## Part 3 · Full root disk

- [ ] Services fail randomly and logins hang. Free space from the emergency shell:

```sh
df -h /
journalctl --vacuum-size=200M
apt clean
du -xh --max-depth=2 /var | sort -h | tail
```

## Part 4 · Chroot from a live USB

- [ ] For a broken GRUB, initramfs or package state, boot the live USB and enter the installed system:

```sh
sudo mount /dev/sdX2 /mnt
sudo mount /dev/sdX1 /mnt/boot/efi
for d in dev proc sys run; do sudo mount --rbind /$d /mnt/$d; done
sudo chroot /mnt
```

- [ ] Inside: `update-initramfs -u -k all`, `grub-install` and `update-grub` (Debian/Ubuntu), or `dracut -f` and `grub2-mkconfig -o /boot/grub2/grub.cfg` (Fedora), or finish a broken upgrade with `dpkg --configure -a`.
- [ ] Root on LVM or LUKS: open it first with `cryptsetup open` and `vgchange -ay`, then mount the logical volume.

#### Check

- [ ] The server boots to a login without the console, and `systemctl --failed` lists nothing.

## In Nightgrid

- An unbootable server just shows as offline; the [Overview](view:overview) and the incident timeline in [System](view:system) show when it went away, which often lines up with an update or reboot.
- Before risky updates, take a Proxmox snapshot of a VM from [Proxmox](view:proxmox), or check that backups in [Fleet](view:fleet) → **Backups** are fresh.
- Write down what fixed it as a [Knowledge](view:notes) note; it'll happen again.

## Limits and gotchas

- Keep at least one older kernel installed; `apt autoremove` keeps the running one and one more by default.
- Secure Boot can refuse a self-built or DKMS module (ZFS, NVIDIA) after an update; enroll the MOK key when asked.
- A failed disk under the root filesystem needs the disk replaced, not this guide: see [Recover data from a failing drive](kb:linux/data-recovery).

## Related

- [Add a disk: partition, format and mount](kb:linux/disks-fstab)
- [Upgrade Debian or Ubuntu to a new release](kb:linux/release-upgrade)
- [Manage a server with IPMI](kb:linux/ipmi)
