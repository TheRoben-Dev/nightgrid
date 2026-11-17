# Lightweight containers with systemd-nspawn

Run a full second Linux system (Debian, Ubuntu, Fedora, Arch) inside a folder on the host, with its own init, users and services, using only systemd. Lighter than a VM, more "real machine" than a Docker container, and nothing to install beyond `debootstrap`.

![An nspawn container and machinectl.](fig:lx-nspawn)

## What you end up with

- A Debian container in `/var/lib/machines/`, started at boot
- Its own network namespace, or the host's network if you prefer
- `machinectl` to start, stop, log in and copy files

## Part 1 · Build the container

```bash
sudo apt install systemd-container debootstrap
sudo debootstrap --include=systemd,dbus,openssh-server stable /var/lib/machines/lab1 http://deb.debian.org/debian
```

- [ ] Set a root password and hostname from inside, before the first boot:

```bash
sudo systemd-nspawn -D /var/lib/machines/lab1 passwd
echo lab1 | sudo tee /var/lib/machines/lab1/etc/hostname
```

## Part 2 · Settings and boot

- [ ] `/etc/systemd/nspawn/lab1.nspawn`:

```ini
[Exec]
Boot=yes

[Network]
VirtualEthernet=yes
```

- [ ] With `VirtualEthernet=yes` the container gets a private link to the host; enable `systemd-networkd` both on the host and inside the container for DHCP and NAT to work out of the box. For an address on your LAN instead, use `Bridge=br0` (an existing bridge) or simply share the host's network by removing the `[Network]` section and setting `VirtualEthernet=no`.
- [ ] Start it now and at boot:

```bash
sudo machinectl enable lab1
sudo machinectl start lab1
machinectl list
```

## Part 3 · Day to day

- [ ] `sudo machinectl login lab1` gives a console (leave with Ctrl+] three times); `sudo machinectl shell lab1` gives a root shell directly.
- [ ] `sudo machinectl copy-to lab1 /home/you/file /root/file` copies files in.
- [ ] Inside, it's normal Debian: `apt install`, `systemctl enable`, users, SSH.
- [ ] `sudo machinectl poweroff lab1` shuts it down cleanly.

#### Check

- [ ] `machinectl list` shows `lab1` running, and services inside start at its boot (`machinectl shell lab1 /usr/bin/systemctl --failed` is empty).

## In Nightgrid

- With its own address and `openssh-server` running, the container can be a Nightgrid host of its own: **+** in the sidebar, its IP, a user you created inside.
- On the host, [System](view:system) → **services** lists it as `systemd-nspawn@lab1`, with start, stop and restart.

## Limits and gotchas

- nspawn containers are not a strong security boundary for untrusted code. Use a VM for that.
- Docker inside nspawn needs extra capabilities and is fiddly; run Docker on the host or in a VM.
- Backups: the whole container is the folder. Stop it (or snapshot the filesystem) before copying it.

## Related

- [Incus containers and VMs](kb:apps/incus)
- [Write your own systemd service](kb:linux/systemd-units)
- [Podman containers as systemd services (Quadlets)](kb:linux/podman-quadlets)
