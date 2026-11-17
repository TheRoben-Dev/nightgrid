# Guard an internet-facing server

A VPS or a server with ports open to the internet gets probed all day. [Nightgrid Node](kb:basics/nightgrid-node) records every login and failed login, alerts on bursts, can block brute-forcers for a while by itself, notices when a new port starts listening, and tracks changes to the files attackers touch first.

## What you end up with

- Logins and failed logins in the timeline, with a burst alert
- Optional auto-block of addresses that keep failing, in a firewall table of the node's own that expires by itself
- Alerts when a socket starts listening on every interface, and which container published it
- A baseline of sensitive files (SSH, sudoers, PAM, cron, `authorized_keys`, compose files) with changes recorded, never contents

## Before you start

- Key-only SSH already set up: see [SSH keys](kb:howto/ssh-keys). Blocking is a seatbelt, not a lock.
- Nightgrid Node installed ([Install Nightgrid Node](kb:howto/node-install))

## Part 1 · Logins

- [ ] `sudo nightgrid-node allow journal`.
- [ ] Turn on the burst alert: read `nightgrid-node query alerts.rules.get`, add `"loginBurst":true` and send the whole object back with `alerts.rules.set`.
- [ ] The threshold is root's: `[logins] burst = 20` failed logins in 5 minutes (critical at ten times that). Lower it on a quiet server.
- [ ] Logins appear as "alice logged in from 10.0.0.5" (`detail`: "ssh, publickey"); failures are summed per minute, "12 failed logins from 3 addresses", with the top addresses in the detail.

## Part 2 · Block brute-forcers (optional)

- [ ] `sudo nightgrid-node allow helper` (blocking goes through the root helper).
- [ ] In `/etc/nightgrid-node/config.toml`: add `"firewall.block"` to `[actions] allow`; under `[block]` set `auto = true`, `threshold = 10` (failures from one address in 5 minutes), `minutes = 60`.
- [ ] Add your own fixed addresses to `[block] never` (for example your office's range). Private, link-local, CGNAT and Tailscale addresses are never blocked unless you set `private = true`, and an address that logged in successfully in the last day is never auto-blocked.
- [ ] `sudo systemctl restart nightgrid-node`, then `nightgrid-node query block.list | jq` shows the settings and current blocks.
- [ ] Block one by hand: `nightgrid-node query actions.run '{"action":"firewall.block","target":"203.0.113.7","dryRun":false}'`.
- [ ] Blocks live in the node's own nftables table, `inet nightgrid_node`, with kernel timeouts, so they end even if the node stops. Nothing else in your firewall is touched. Already running fail2ban or CrowdSec ([setup](kb:apps/fail2ban-crowdsec))? Pick one of them for auto-blocking to keep things simple.

## Part 3 · New listening ports

- [ ] Nothing to grant: listening sockets come from `/proc/net`. With `allow docker`, the node also names the container that published the port.
- [ ] The timeline gets "new listener 8080/tcp (http-alt)", as a warning when it's reachable from the network (bound to every interface or a public address), with a note for databases and caches.
- [ ] Review what listens now: `nightgrid-node query ports.list | jq`. Anything on `0.0.0.0` that should be private: bind it to `127.0.0.1` or the tailnet address, or firewall it.

## Part 4 · Sensitive files

- [ ] On by default: `[files] watch` lists sshd_config, passwd, shadow, group, sudoers, crontab, cron.d, hosts, fstab and `/etc/systemd/system`; `[files] baseline = true` adds more of `/etc` (PAM, `ld.so.preload`, profile, sysctl, package sources), every login user's and root's `authorized_keys`, and the compose files of running containers.
- [ ] `sudo nightgrid-node allow health` lets the read-only helper hash files the service can't read (root's crontab, other users' homes).
- [ ] A change reads "changed /etc/ssh/sshd_config" with size, short hash, mode and owner; warnings for SSH, sudoers, passwd, shadow, cron, PAM and `ld.so.preload`. Changes made while the node was stopped are reported at its next start.
- [ ] Add your own: append paths to `[files] watch` (for example `/etc/caddy/Caddyfile`) and restart the node.

## Part 5 · Review it

- [ ] Daily: `nightgrid-node query summary.daily | jq -r '.facts[]'` has `Failed logins`, `Files changed` and `Listening ports` lines when there's something to say.
- [ ] In Nightgrid, [Fleet](view:fleet) → **Access** runs a full audit of users, keys, sudo and sshd settings on demand; the node covers the time in between.
- [ ] An unexpected `authorized_keys` change or a new uid-0 account: treat the host as compromised until you know why.
