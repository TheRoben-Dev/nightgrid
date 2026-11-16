# Users, groups and sudo

Create admin and service accounts properly, give sudo only where needed, and allow the few passwordless commands Nightgrid's unattended features need.

## What you end up with

- A personal admin user with key login and sudo
- Service users that can't log in
- Narrow passwordless rules for shutdown or upgrades, if you want unattended power actions

## Part 1 · Your admin user

```sh
sudo adduser alice
sudo usermod -aG sudo alice      # Debian/Ubuntu (wheel on Fedora/RHEL)
```

- [ ] Put your key on it with **Settings → SSH keys → CHECK HOSTS** in Nightgrid, then log in as it and stop using root.

## Part 2 · Service users

```sh
sudo useradd --system --no-create-home --shell /usr/sbin/nologin backup
```

- [ ] Containers: match `PUID`/`PGID` to a real user so files on the host have a known owner (`id alice`).

## Part 3 · Narrow passwordless sudo

- [ ] Nightgrid's UPS group shutdown and scheduled power actions skip hosts whose sudo needs a password it doesn't remember. Allow only the commands needed, with `sudo visudo -f /etc/sudoers.d/nightgrid`:

```
alice ALL=(root) NOPASSWD: /usr/bin/systemctl poweroff, /usr/bin/systemctl reboot, /usr/bin/systemctl suspend
```

- [ ] `visudo` checks the syntax before saving; a broken sudoers file can lock you out of sudo entirely.
- [ ] Alternatively, tick **remember** on the host's sudo prompt in Nightgrid; the password stays in the OS keychain.

## Part 4 · Review

- [ ] [Fleet](view:fleet) → **Access** lists users with a login shell, sudo rules and authorized keys per host.

#### Check

- [ ] `sudo -l` as your user shows exactly the rules you expect.

## Limits and gotchas

- `NOPASSWD: ALL` makes any compromise of your account a root compromise. Keep rules to specific commands.
- Membership of `docker` or `lxd` is root-equivalent too (see [The docker group](kb:basics/docker-group)).

## Related

- [Audit who can log in](kb:howto/access-audit)
- [Hardening sshd](kb:basics/sshd-hardening)
- [UPS monitoring with NUT](kb:howto/ups)
