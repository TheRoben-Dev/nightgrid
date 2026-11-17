# First homelab weekend

![One machine, a few services, and the habits that keep it alive.](fig:guide-starter-homelab)

Go from one spare computer to a monitored, backed-up home server running DNS filtering, a password manager and a reverse proxy, managed entirely from Nightgrid. Each part builds on the last; stop after any of them and you still have something useful.

## What you end up with

- A Debian server with key-only SSH, a firewall and automatic security updates
- Docker with Caddy in front, Pi-hole for the network and Vaultwarden for passwords
- Service checks, alerts on your phone, nightly backups with test restores
- Remote access through Tailscale, nothing forwarded on the router

## Before you start

- A mini PC or old desktop (4 GB RAM, an SSD), wired to the router.
- A USB stick with the Debian netinst image.
- Nightgrid on this computer with **Standard** features (Settings → **FEATURES**).

## Part 1 · Install and connect

- [ ] Install Debian with only "SSH server" and "standard system utilities" selected. Create your user.
- [ ] Reserve its IP in the router's DHCP (or follow [Give a server a static IP](kb:linux/static-ip)).
- [ ] [Add your first host](kb:howto/first-host) and put your key on it from **Settings → Connections → SSH keys**.

#### Check

- [ ] The server shows live numbers on the [Overview](view:overview).

## Part 2 · Lock it down

- [ ] Follow the fixes in [System](view:system) → **security**: key-only SSH ([Hardening sshd](kb:basics/sshd-hardening)), ufw with SSH allowed from the LAN ([Change a firewall safely](kb:howto/firewall)), [Automatic security updates](kb:apps/unattended-upgrades).
- [ ] Save a baseline: [System](view:system) → **config** → **Save baseline**.

#### Check

- [ ] The security score is A or B.

## Part 3 · Docker and a proxy

- [ ] [Prepare a Linux Docker host](kb:apps/docker-host).
- [ ] [Caddy reverse proxy](kb:apps/caddy) with `tls internal` for LAN names, or a real domain if you have one.

## Part 4 · DNS filtering

- [ ] [Pi-hole](kb:apps/pihole) (or [AdGuard Home](kb:apps/adguard-home)), then point the router's DHCP DNS at it.
- [ ] Add local names for your server in its local DNS records, and the Pi-hole integration in [Services](view:services).

## Part 5 · Passwords

- [ ] [Vaultwarden password manager](kb:apps/vaultwarden) behind Caddy. Close sign-ups once your accounts exist.

## Part 6 · Watch it

- [ ] [Services](view:services) → **Add service** for each app.
- [ ] Alerts to your phone: Settings → Alerts → **WEBHOOK** → ntfy ([Tune alerts so only real problems notify](kb:howto/alerts)).

## Part 7 · Back it up

- [ ] [restic backups with systemd timers](kb:apps/restic) of `/etc`, `/opt/stacks` and the Vaultwarden data, to a USB disk or another machine.
- [ ] Add the job in [Fleet](view:fleet) → **Backups** and run **Test restore**.
- [ ] Export your Nightgrid profile ([Back up and move your Nightgrid setup](kb:howto/profiles)).

## Part 8 · Away from home

- [ ] Tailscale on the server as a subnet router ([Tailscale subnet router](kb:apps/tailscale-subnet-router)) and Nightgrid's built-in client ([Reach your servers away from home](kb:howto/remote-access)).

#### Check

- [ ] On mobile data: Vaultwarden syncs, Nightgrid connects, and the router forwards nothing.

## When Nightgrid is closed

- Keeps running: every container, Pi-hole, backups on their timer, security updates, Tailscale.
- Needs Nightgrid open (or in the tray): alerts, service checks, uptime history.

## Limits and gotchas

- One server is one point of failure: when it's down, DNS goes with it. Keep the router as a fallback DNS until you add a second Pi-hole.
- Vaultwarden data is the crown jewel. Test its restore before you rely on it.

## Related

- [Prepare a Linux Docker host](kb:apps/docker-host)
- [Build a DIY NAS](kb:guides/diy-nas)
- [Raise a host's security score](kb:howto/security-score)
