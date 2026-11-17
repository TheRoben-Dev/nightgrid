# Reset a forgotten root or sudo password

Get back into a Linux server when nobody remembers the admin password: boot into a root shell with a kernel parameter, set a new password, and boot normally. Needs the physical console (or IPMI, PiKVM, the hypervisor's console), not SSH.

![Resetting a forgotten root password.](fig:lx-reset-root)

## What you end up with

- A new password on root or your admin account
- SELinux labels fixed on Fedora/RHEL, so the next boot isn't broken
- A note of where the password lives now, so this doesn't happen again

## Before you start

- If the disk is LUKS-encrypted, you still need the disk passphrase; this only bypasses the login password.
- On a VM: Proxmox **Console**, Nightgrid's [Desktops](view:desktops) or the hypervisor's own console all work.

## Part 1 · Debian, Ubuntu, Proxmox

- [ ] Reboot. At the GRUB menu press **e** on the default entry (hold **Shift** or tap **Esc** during boot if the menu is hidden).
- [ ] On the line starting with `linux`, replace `ro` with `rw` and add `init=/bin/bash` at the end. **Ctrl+X** to boot.
- [ ] At the `#` prompt: `passwd root` or `passwd <admin user>`, enter the new password twice.
- [ ] `sync`, then `exec /sbin/init` to boot normally, or force a reboot with `reboot -f`.

## Part 2 · Fedora, RHEL, Rocky, Alma

- [ ] At the GRUB menu press **e**, add `rd.break` to the end of the `linux` line, **Ctrl+X**.
- [ ] `mount -o remount,rw /sysroot`, then `chroot /sysroot`.
- [ ] `passwd <user>`, then `touch /.autorelabel` so SELinux relabels the files at next boot (this takes a few minutes; let it finish).
- [ ] `exit` twice to continue booting.

## Part 3 · No GRUB menu access

- [ ] Boot a live USB or ISO (or [netboot.xyz network boot menu](kb:apps/netbootxyz)), mount the root filesystem, `chroot` into it and run `passwd`. On Fedora/RHEL also create `/.autorelabel`.

#### Check

- [ ] After the normal boot, log in on the console with the new password and run `sudo -v` (for an admin user). Then log in over SSH as usual.

## In Nightgrid

- Nightgrid logs in with SSH keys where you've set them up, so a lost password often doesn't stop it. Use a [Terminal](view:terminal) tab and `sudo passwd <user>` if you still have key access and a working sudo rule.
- Store the new password in your password manager, and record which hosts it applies to in a [Knowledge](view:notes) note.

## Limits and gotchas

- Anyone with the console can do this. Physical security (or a BIOS/GRUB password) is what protects a server that's not encrypted.
- `init=/bin/bash` skips all services, so networking and other mounts are absent. Do only the password change there.
- Some cloud images lock root entirely; reset your admin user's password instead and use sudo.

## Related

- [Fix a server that won't boot](kb:linux/boot-rescue)
- [Users, groups and sudo](kb:linux/users-sudo)
- [Encrypt a disk with LUKS](kb:linux/luks)
