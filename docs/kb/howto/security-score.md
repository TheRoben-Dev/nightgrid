# Raise a host's security score

Use System → security to go from a D to an A: each finding says what's wrong and gives the command to fix it. Nightgrid never changes anything itself.

## What you end up with

- An A–F score per host from checks that need no root
- Every listening port labelled local, VPN, one address or all interfaces, with its owner
- A side-by-side port map of all hosts

## Part 1 · Read the score

- [ ] [System](view:system) → **security**. The checks: SSH root and password logins, an active firewall, databases or the Docker API on every interface, Docker ports that bypass ufw, fail2ban/CrowdSec, automatic security updates, a pending reboot and extra uid-0 accounts.
- [ ] Each finding has copy buttons for its fix.

## Part 2 · The usual fixes, in order

- [ ] Key-only SSH, no root login: [Hardening sshd](kb:basics/sshd-hardening).
- [ ] Turn on a firewall: [Change a firewall safely](kb:howto/firewall).
- [ ] Databases and admin panels on `127.0.0.1` only: change `5432:5432` to `127.0.0.1:5432:5432` in compose.
- [ ] Ban brute-forcers: [fail2ban and CrowdSec](kb:apps/fail2ban-crowdsec).
- [ ] Automatic security updates: [Automatic security updates](kb:apps/unattended-upgrades).
- [ ] Reboot if one is pending.

## Part 3 · Ports

- [ ] Below the checks, every listening port shows who can reach it and which process or container owns it.
- [ ] **Port map · all hosts** scans every host and lays their reachable ports side by side.
- [ ] Anything on **all interfaces** that you didn't mean to share: bind it to localhost or the LAN, or firewall it.

#### Check

- [ ] The score is A or B, and nothing unexpected listens on all interfaces.

## Part 4 · From the internet's side

- [ ] The [Network](view:network) map marks ways in from the internet in red: router port forwards (UPnP), public addresses, Tailscale Funnel and cloudflared tunnels (see [Map your network and find exposed ports](kb:howto/network-map)).

## Limits and gotchas

- The score is hygiene, not a pentest. It can't see app-level holes (a default admin password in a web app).
- Checks run as your login. A rule only root can read may show as unknown.

## Related

- [Audit who can log in](kb:howto/access-audit)
- [Change a firewall safely](kb:howto/firewall)
- [Hardening sshd](kb:basics/sshd-hardening)
- [Map your network and find exposed ports](kb:howto/network-map)
