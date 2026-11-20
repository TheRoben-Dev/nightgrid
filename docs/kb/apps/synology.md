# A Synology NAS alongside Nightgrid

Add a Synology NAS (DSM 7) to Nightgrid without weakening it: a status card from DSM's web API (2-step verification stays on), an optional read-only SNMP card that needs no DSM account at all, and SSH for the terminal, files and Docker on a key-only login. DSM is Linux, but with its own rules about users, packages and Docker; this article covers what to change and what to leave alone.

![The layers of a NAS, as DSM builds them.](fig:app-nas-os)

## Two doors into DSM

DSM keeps two separate ways in, and each has its own rules:

| | DSM web API (the **Synology DSM** card) | SSH (terminal, Files, Docker) |
|---|---|---|
| Who can use it | Any account with the DSM app allowed | Only members of **administrators** |
| 2-step verification | Asked at sign-in; Nightgrid keeps a trusted-device token | Never asked: SSH has its own keys |
| What protects it | Password + 2-step | Your SSH key (turn password logins off) |

So 2-step verification protects the web door only. An administrator account with 2-step enforced still opens SSH with nothing but its password, unless you turn password logins off. Part 3 does that.

## What you end up with

- A **Synology DSM** card for a dedicated account, with 2-step verification on, entered once
- If you want no administrator anywhere: an **SNMP** card for health, pools, disks and volumes, read-only
- SSH and SFTP for one administrator, key only, for the terminal, [Files](view:files) and [Docker](view:docker)
- Docker run as its own low-rights user, so the SSH login does little

## Part 1 · The DSM card, with 2-step verification on

- [ ] **Control Panel → User & Group → Create** a user just for Nightgrid (say `nightgrid`) with a long random password.
- [ ] Decide whether it's an administrator. DSM shows volumes and disks only to administrators. A regular user's card shows what DSM shares with it and names the parts DSM keeps to administrators, instead of failing. For those parts without an administrator, add SNMP (Part 2) next to or instead of this card.
- [ ] In **Application Privileges**, allow the user **DSM** only if it's a regular user (administrators always have every app). Deny the rest.
- [ ] Sign in to DSM once as that user if 2-step verification is enforced, and set it up there. DSM won't let an API sign-in set it up.
- [ ] In Nightgrid: [Services](view:services) → **Add integration** → **Synology DSM**, the address (`https://nas.lan:5001`), login name and password. **Test** says "Login name and password work. DSM wants a 2-step code" when 2-step is on.
- [ ] Type a current code from your authenticator into **2-step verification** and press **Add** (or **Save**). Nightgrid signs in once with it, DSM hands back a trusted-device token, and Nightgrid keeps that token in the OS keychain. The code itself isn't stored.

#### Check

- [ ] The card shows the NAS, and the integration's settings say **DSM trusts this computer**.
- [ ] In DSM, as that user: **Personal → Security** lists **Nightgrid** among the trusted devices.

If DSM drops the token (you remove it there, it expires, or the password changes), the card says DSM asks for a 2-step code again and shows **Enter 2-step code**. **Forget** in the settings removes Nightgrid's copy; remove it in DSM too. Each computer gets its own token: a profile export doesn't carry it, so a second Mac asks for one code too.

## Part 2 · SNMP, with no DSM account (optional)

SNMP is read-only and has its own user, separate from DSM's accounts, so nothing here can sign in to DSM or change anything.

- [ ] **Control Panel → Terminal & SNMP → SNMP → Enable SNMP service**.
- [ ] Tick **SNMPv3 service**, enter a user name (say `nightgrid`) and a password, protocol **SHA**. Tick **Enable SNMP privacy**, protocol **AES**, and a second password. Leave **SNMPv1, SNMPv2c** off: v2c sends its community name in the clear.
- [ ] **Control Panel → Security → Firewall**: allow UDP 161 from your LAN or tailnet only.
- [ ] In Nightgrid: **Add integration** → **SNMP**, the NAS's address (`nas.lan`), the SNMPv3 user, **SHA + AES**, both passwords.

The card shows DSM's system and power status, fans, temperature, storage pools with their state (degraded and crashed in red), every disk with its health, volumes and how full they are, CPU, memory and uptime, plus a note when a DSM update is out. It reads Synology's own MIB, so other SNMP agents (a Linux server, another NAS) show volumes, CPU and memory only.

SNMP uses UDP. Nightgrid's built-in Tailscale carries TCP only, so from outside the LAN the SNMP card needs the Tailscale app (or another VPN) on this computer.

## Part 3 · SSH, key only

DSM 7 lets only administrators use SSH, and 2-step verification doesn't apply to it. Make the SSH login one administrator that only ever uses a key.

- [ ] **Control Panel → Terminal & SNMP → Enable SSH service**. Change the port if you like.
- [ ] **Control Panel → File Services → FTP → SFTP → Enable SFTP service**. Nightgrid's [Files](view:files) view uses SFTP, which DSM controls separately from SSH.
- [ ] Use a named administrator, not `admin` (disable that one). It can be your own, or one only Nightgrid uses. With 2-step enforced on administrators, its web sign-ins still need a code.
- [ ] **Control Panel → User & Group → Advanced → Enable user home service**, so the user has a home folder for `~/.ssh`.
- [ ] Add the host in Nightgrid (**+**), connect once with the password, then put your key on it from **Settings → Connections → SSH keys** (**CHECK HOSTS**, **+**).
- [ ] If the key is refused, DSM's home permissions are too open. In the NAS terminal:

```bash
chmod 755 ~
chmod 700 ~/.ssh
chmod 600 ~/.ssh/authorized_keys
```

- [ ] Once the key works, turn password logins off for SSH, so the password (the part 2-step protects on the web) can't open SSH:

```bash
sudo sed -i 's/^#\?PasswordAuthentication .*/PasswordAuthentication no/' /etc/ssh/sshd_config
```

  Then turn **Enable SSH service** off and on again in Control Panel, which restarts SSH with the new setting. Keep the Nightgrid terminal open while you do, in case you need to undo it.

- [ ] DSM updates can put `sshd_config` back. After an update, check with `grep PasswordAuthentication /etc/ssh/sshd_config`, or let [System → CONFIG](view:system) watch the file for changes.
- [ ] **Control Panel → Security → Firewall**: allow the SSH port from your LAN or tailnet only. Keep **Security → Protection → Auto block** on.

#### Check

- [ ] Nightgrid opens a terminal on the NAS without a password prompt, and `ssh -o PreferredAuthentications=password you@nas` from another computer is refused.

## Part 4 · Docker as its own user (optional)

Container Manager's Docker socket belongs to root, and access to it is root-equivalent on the NAS (see [The docker group](kb:basics/docker-group)). Rather than giving the SSH login that power, run Docker as a separate account:

- [ ] Create a user for Docker (say `dockerops`) that owns your stacks, and give it the socket with a `docker` group:

```bash
sudo synogroup --add docker dockerops
sudo chown root:docker /var/run/docker.sock
```

- [ ] In the host's settings in Nightgrid, set **Docker user** to `dockerops`. Nightgrid keeps logging in as the SSH administrator and runs every Docker action as `dockerops` through `sudo -u`.
- [ ] The first time, **DOCKER USER NEEDS SUDO** shows the one sudoers rule it needs (the login may run `docker` and `docker-compose` as that user, nothing else). **Fix it for me** adds it after checking it with `visudo`.
- [ ] DSM updates and Container Manager restarts can reset the socket's ownership; the card comes back with only the `chown`. A Task Scheduler boot task that runs it as root saves the trip.

#### Check

- [ ] The [Docker](view:docker) view lists Container Manager's containers with no NO ACCESS card, and `sudo -l` in the NAS terminal shows only the docker rule for `dockerops`.

## In Nightgrid

- [Services](view:services): add DSM's web UI (`https://nas:5001`) and the apps you publish from it.
- [Fleet](view:fleet) → **Updates** doesn't apply to DSM; update DSM and packages from its own Control Panel. The SNMP card notes when a DSM update is out.
- Shared folders live under `/volume1/<share>`. Mount them on other hosts with NFS or SMB (see [Mount a Windows or NAS share on Linux](kb:linux/mount-smb)).

## Limits and gotchas

- Don't install software with `apt` or change system files beyond the ones above: DSM has neither apt nor a stable root filesystem, and updates overwrite changes.
- Some commands Nightgrid's checks use may be missing from DSM's BusyBox tools; those cards show less, not wrong data.
- Keep SSH and SNMP reachable on the LAN or tailnet only.

## Related

- [The docker group](kb:basics/docker-group)
- [Add app cards](kb:howto/integrations)
- [Mount a Windows or NAS share on Linux](kb:linux/mount-smb)
- [Run a TrueNAS box alongside Nightgrid](kb:apps/truenas)
