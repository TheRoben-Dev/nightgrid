# SFTP-only accounts locked to one folder

Give someone (a family member, a scanner, a camera, a website's backup job) an account that can upload and download files over SFTP, can't open a shell, and can't see anything outside its own folder. Built into OpenSSH; nothing else to install.

## What you end up with

- A `sftponly` group whose members are jailed to `/srv/sftp/<user>`
- A writable `upload` folder inside each jail
- No shell, no port forwarding, no agent forwarding for those accounts

## Part 1 · Group and folders

- [ ] Create the group and a user without a usable shell:

```bash
sudo groupadd sftponly
sudo useradd -M -d /upload -s /usr/sbin/nologin -G sftponly scanner
sudo passwd scanner
```

- [ ] The home `/upload` is relative to the jail, so the account lands in its upload folder after login.
- [ ] Build the jail. The jail folder itself and every folder above it **must be owned by root and not writable by anyone else**, or sshd refuses the login:

```bash
sudo mkdir -p /srv/sftp/scanner/upload
sudo chown root:root /srv/sftp /srv/sftp/scanner
sudo chmod 755 /srv/sftp /srv/sftp/scanner
sudo chown scanner:sftponly /srv/sftp/scanner/upload
```

## Part 2 · sshd

- [ ] Create `/etc/ssh/sshd_config.d/30-sftponly.conf`:

```text
Match Group sftponly
    ChrootDirectory /srv/sftp/%u
    ForceCommand internal-sftp
    AllowTcpForwarding no
    AllowAgentForwarding no
    X11Forwarding no
    PermitTTY no
```

- [ ] `%u` is the user name. `internal-sftp` runs inside sshd, so the jail needs no binaries or libraries.
- [ ] If your main config has `PasswordAuthentication no`, add `PasswordAuthentication yes` inside the `Match` block for accounts that use a password, or give them a key instead (put it in `/etc/ssh/authorized_keys/%u` with `AuthorizedKeysFile /etc/ssh/authorized_keys/%u` in the block, since their home is inside the jail).
- [ ] `sudo sshd -t && sudo systemctl reload ssh`.

## Part 3 · Share data with other users

- [ ] To let a jailed account read a media or backup folder, bind-mount it into the jail rather than symlinking (symlinks can't point out of a chroot):

```text
/srv/media  /srv/sftp/family/media  none  bind,ro  0 0
```

- [ ] Add the line to `/etc/fstab`, create the empty `media` folder, then `sudo mount -a`.

#### Check

- [ ] `sftp scanner@server` logs in, `ls /` shows only `upload`, uploads work, and `ssh scanner@server` closes right away.

## In Nightgrid

- [Fleet](view:fleet) → **Access** lists the accounts and their recent logins, so an SFTP account that logs in at odd hours stands out.
- Nightgrid's own [Files](view:files) view uses your normal account, not these.
- [System](view:system) → **config** → **Compare** shows when someone changes the sshd drop-in.

## Limits and gotchas

- "bad ownership or modes for chroot directory" in the journal means a folder in the path is writable by someone other than root. Fix the ownership, not the error.
- Older clients and devices that only speak FTP need an FTP server (vsftpd with TLS) instead.
- Disk quotas don't come with SFTP; use a separate filesystem or quotas if an account could fill the disk.

## Related

- [Hardening sshd](kb:basics/sshd-hardening)
- [Users, groups and sudo](kb:linux/users-sudo)
- [File permissions, ownership and ACLs](kb:linux/permissions)
