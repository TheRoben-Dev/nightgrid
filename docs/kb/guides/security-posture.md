# Raise your fleet's security score

Take every host from whatever grade it has now to an A, one fix at a time, and let Nightgrid verify each step on the real host. Nothing here is changed by Nightgrid itself: you run the fixes, the checks only read.

## What you end up with

- Every host on [Security](view:security) → **POSTURE** at A or B, with a trend you can watch
- SSH by key only, a firewall on, and nothing sensitive listening on every address
- Security updates installed, current container images, and a notice when a score drops

## Before you start

- Each host added in Nightgrid and reachable over SSH.
- Pick a host at the top of this guide so the **{check}** steps run against it.
- Keep a second SSH session open while you change SSH or the firewall, so a mistake can't lock you out.

## Part 1 · Read the scores

- [ ] Open [Security](view:security) → **POSTURE** and press **Check all**. Each host gets a score from 0 to 100 and a grade.
- [ ] Click a host to see its checks, worst first. Each one says what's wrong, gives the fix to run, and links back here.

## Part 2 · SSH

- [ ] Put your key on the host first: **Settings → Connections → SSH keys** ([Set up SSH keys](kb:howto/ssh-keys)). {check: ssh-key}
- [ ] Turn password logins off: `PasswordAuthentication no` and `KbdInteractiveAuthentication no` in a file under `/etc/ssh/sshd_config.d/`, then `sudo sshd -t && sudo systemctl reload ssh` ([Hardening sshd](kb:basics/sshd-hardening)). {check: ssh-no-password}
- [ ] Root can't log in with a password: `PermitRootLogin no` (or `prohibit-password`). {check: ssh-no-root-password}
- [ ] Repeated failed logins get banned: fail2ban or CrowdSec ([fail2ban and CrowdSec](kb:apps/fail2ban-crowdsec)). {check: fail2ban}

#### Check

- [ ] A new SSH login with your key works before you close the old session.

## Part 3 · What can be reached

- [ ] A firewall is on, with SSH allowed first ([Change a firewall safely](kb:howto/firewall)). {check: firewall}
- [ ] No database, cache or search engine listens on every address. In compose, change `5432:5432` to `127.0.0.1:5432:5432`, or leave the port out and use a Docker network. {check: no-exposed-db}
- [ ] Docker's API isn't on TCP 2375. Remove `-H tcp://0.0.0.0:2375` from the service or `"hosts"` from `/etc/docker/daemon.json`; Nightgrid talks to Docker over SSH and never needs it. {check: no-docker-tcp}
- [ ] Docker publishes ports around ufw: publish only what must be reachable, or use the `DOCKER-USER` chain ([Raise a host's security score](kb:howto/security-score)).

## Part 4 · Stay patched

- [ ] Install waiting security updates from [Fleet](view:fleet) → **UPDATES** ([Update every host](kb:howto/fleet-updates)). {check: security-updates}
- [ ] Turn on automatic security updates ([Automatic security updates](kb:apps/unattended-upgrades)). {check: auto-updates}
- [ ] Reboot if a new kernel is waiting. {check: no-reboot-pending}
- [ ] Update stacks whose images are more than six months old: [Docker](view:docker) → **STACKS** → **Update** takes a snapshot first ([Keep containers up to date](kb:howto/container-updates)). {check: images-fresh}
- [ ] The clock is in sync, so certificates and logs make sense ([Time sync](kb:linux/time-sync)). {check: time-sync}

#### Check

- [ ] **Check again** on the host shows A or B.

## Part 5 · Keep it there

- [ ] Leave **Check connected hosts once a day** on in [Security](view:security) → **POSTURE**: Nightgrid re-scores hosts it's already connected to and tells you when a score drops by 10 points or a grade.
- [ ] Choose where those notices go in **Settings → Alerts → Notifications** (the inbox always has them).

## Limits and gotchas

- The score is hygiene, not a penetration test. It can't see a weak password in a web app or a hole in your router.
- Checks run as your login, without sudo. Anything only root can read shows as unknown and doesn't count either way.
- "Every address" means `0.0.0.0` or `::`. A port bound to one LAN address can still be reachable from that LAN.

## Related

- [Raise a host's security score](kb:howto/security-score)
- [Audit who can log in](kb:howto/access-audit)
- [Rotate secrets and certificates](kb:howto/secret-rotation)
