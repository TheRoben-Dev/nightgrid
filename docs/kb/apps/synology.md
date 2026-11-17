# A Synology NAS alongside Nightgrid

Add a Synology NAS (DSM 7) as a Nightgrid host: a terminal, file browsing over SFTP, Docker containers from Container Manager, and service checks, while DSM keeps doing the NAS work. DSM is Linux, but with its own rules about users, packages and Docker; this article covers what to change and what to leave alone.

![The layers of a NAS, as DSM builds them.](fig:app-nas-os)

## What you end up with

- SSH and SFTP on the NAS for an admin user with a key
- Nightgrid's terminal, Files and service checks on it
- Container Manager's Docker visible in Nightgrid, if you choose to allow it

## Part 1 · SSH and SFTP

- [ ] **Control Panel → Terminal & SNMP → Enable SSH service**. Change the port if you like.
- [ ] **Control Panel → File Services → FTP → SFTP → Enable SFTP service**. Nightgrid's [Files](view:files) view uses SFTP, which DSM controls separately from SSH.
- [ ] Only users in the **administrators** group can log in with SSH. Use your own admin account, not `admin` (disable that one).
- [ ] **Control Panel → User & Group → Advanced → Enable user home service**, so your user has a home folder for `~/.ssh`.

## Part 2 · Key login

- [ ] Add the host in Nightgrid (**+**), connect once with your password, then put your key on it from **Settings → Connections → SSH keys** (**CHECK HOSTS**, **+**).
- [ ] If the key is refused, DSM's home permissions are too open. In the NAS terminal:

```bash
chmod 755 ~
chmod 700 ~/.ssh
chmod 600 ~/.ssh/authorized_keys
```

## Part 3 · Docker (optional)

- [ ] Container Manager's Docker socket belongs to root. Nightgrid's [Docker](view:docker) view can use it if your user can reach the socket. A common way is a `docker` group:

```bash
sudo synogroup --add docker your-user
sudo chown root:docker /var/run/docker.sock
```

- [ ] DSM updates and Container Manager restarts can reset the socket's ownership; repeat the `chown` (or a Task Scheduler boot task that runs it as root).
- [ ] Membership in that group is root-equivalent on the NAS; see [The docker group](kb:basics/docker-group).

#### Check

- [ ] Nightgrid opens a terminal on the NAS without a password prompt, [Files](view:files) browses `/volume1`, and the Docker view lists Container Manager's containers.

## In Nightgrid

- [Services](view:services): add DSM's web UI (`https://nas:5001`) and the apps you publish from it.
- [Fleet](view:fleet) → **Updates** doesn't apply to DSM; update DSM and packages from its own Control Panel.
- Shared folders live under `/volume1/<share>`. Mount them on other hosts with NFS or SMB (see [Mount a Windows or NAS share on Linux](kb:linux/mount-smb)).

## Limits and gotchas

- Don't install software with `apt` or change system files: DSM has neither apt nor a stable root filesystem, and updates overwrite changes.
- Some commands Nightgrid's checks use may be missing from DSM's BusyBox tools; those cards show less, not wrong data.
- Keep SSH reachable on the LAN or tailnet only. DSM's own firewall (**Control Panel → Security → Firewall**) can restrict it.

## Related

- [The docker group](kb:basics/docker-group)
- [Mount a Windows or NAS share on Linux](kb:linux/mount-smb)
- [Run a TrueNAS box alongside Nightgrid](kb:apps/truenas)
