# Choose what Nightgrid Node may do

Out of the box, [Nightgrid Node](kb:basics/nightgrid-node) only reads what any user can read (metrics, uptime, disk usage, the ARP table, listening ports, file metadata). Everything more is a separate grant that root makes with one command and can take back. Here is what each grant gives, what it costs in trust, and when it's worth it.

![Grants stack on a base that needs none.](fig:ng-node-grants)

## How grants work

- Grant: `sudo nightgrid-node allow <name>`. Take back: `sudo nightgrid-node deny <name>`.
- `sudo nightgrid-node allow` (or `deny`) with no name opens a menu that shows every grant, whether it's on, and toggles it.
- Each grant adds a systemd drop-in, a group, a udev rule or a separate small unit with its own sandbox. The main service never gains network access.
- `nightgrid-node status` lists every grant under `access` as `ok`, `denied` (not granted), `absent` (nothing to read on this host) or `off` (disabled in the config).
- Actions are a second, separate gate: even with the helper installed, the node does nothing that root didn't list in `[actions] allow` in `/etc/nightgrid-node/config.toml` (empty by default).

## The grants

### allow docker
Gives: container states, crashes, restarts, out-of-memory kills and health changes, container-down alerts, container logs for log watching, per-container usage, and the containers behind listening ports. Works with Docker or Podman's compatible socket (`[docker] socket`).
Costs: the service joins the socket's group. Anyone who can use the Docker socket controls the host, so this is **root-equivalent**. The node only sends `GET` requests (list and events) and the rest of its sandbox stays, but a flaw in the node could be used for more. Sockets owned by group `root` are refused.
Worth it: on container hosts where you want crash and restart history while away. Skip it on hosts where Docker access is guarded on purpose.

### allow journal
Gives: SSH logins and failed logins (summed per minute), failed-login burst alerts, auto-block, log watching (error bursts, new kinds of errors, saved searches), and the output of backup and scheduled jobs.
Costs: the service joins the journal's group (`systemd-journal`), which can read every log on the host, and logs sometimes contain secrets programs printed. The node keeps only sshd's login lines and error lines in memory, after dropping anything that looks like a secret.
Worth it: on almost every server. It's what turns "something happened" into "who logged in, and what broke".

### allow netwatch
Gives: DHCP servers on the LAN, a rogue DHCP server next to the real one, and device names from DHCP.
Costs: a second process with `CAP_NET_RAW`, listen-only: a kernel filter passes it only ARP and DHCP, it never sends a packet and has no IP sockets.
Worth it: on one always-on host per LAN segment, if you want to catch a rogue DHCP server (a forgotten router in AP mode is the usual one).

### allow health
Gives: disk SMART, ZFS/mdadm/Btrfs/LVM pools, pending updates and reboot-required, UPS status through NUT (targets in `[health] ups`), and reading root-only files for the file baseline.
Costs: a short-lived, read-only root helper per read, running fixed scripts only, with `CAP_SYS_RAWIO`, `CAP_SYS_ADMIN` and `CAP_DAC_READ_SEARCH`, network only to localhost and private ranges (for a NUT server).
Worth it: on every host with disks you care about, and required for UPS shutdown.

### allow helper
Gives: the root helper for actions: restart, start or stop the systemd units listed in `[actions] units`, reboot and power off (always one minute ahead, cancel with `shutdown -c`), file rollbacks for paths in `[rollback] paths`, and address blocking in the node's own nftables table.
Costs: a socket only the service account can reach; each request starts a short-lived root process that re-reads root's config and does only what's listed there. With rollback paths it also holds file-writing capabilities for those paths only.
Worth it: when you want self-healing on units, UPS shutdown, or blocking. Keep `[actions] units` and `[rollback] paths` short.

### allow power
Gives: power draw in watts from the CPU's energy counters (RAPL) in usage readings.
Costs: makes the energy counters readable by the node's group. The kernel keeps them root-only because very fast sampling can hint at what other programs compute (CVE-2020-8694); the node reads at most once a second.
Worth it: on a home server you want to tune for power. Skip on shared machines.

### allow checks
Gives: HTTP(S) and TCP service checks run from this host every minute, with status codes and certificate expiry, so "down" is told from "unreachable from my laptop".
Costs: a separate runner with IP access only to `[checks] networks` (private ranges, loopback and the tailnet by default), enforced by systemd. Requests are fixed `GET`s that read only the status line.
Worth it: on one host inside each network whose services you check.

### allow peers
Gives: the link between nodes: paired nodes check each other over the tailnet with mutual TLS, and a host can follow another host's UPS shutdown.
Costs: a separate unit listening only on the host's tailnet address (`[peers] listen`), talking only to `[peers] networks`. It answers only what `[peers] serve` lists (`ping` by default) and only to peers whose certificate fingerprint was paired.
Worth it: with two or more always-on hosts on a tailnet. See [Link several servers](kb:guides/node-multi-host).

### allow statuspage
Gives: a read-only status page for your phone, at a secret link printed by the command.
Costs: plain HTTP on the tailnet address (WireGuard encrypts it) or loopback, never LAN or public addresses. Anyone with the link and tailnet access can read the host's name, load, disks, alerts, UPS and linked hosts. `deny statuspage` removes the token, so the link dies.
Worth it: if you want a glance at a host from your phone without the app.

## Starting points

| Server | Grants |
|---|---|
| Docker host at home | journal, docker, health |
| NAS / storage box | journal, health (add helper for UPS shutdown) |
| Internet-facing VPS | journal, helper (for blocking), health |
| Always-on box on the home LAN | journal, netwatch, checks |
| Two or more hosts on a tailnet | peers (and statuspage on one, if you like) |

None of them is required. A node with no grants still records metrics, uptime, disk forecasts and the timeline.

## Part 1 · Grant and check

- [ ] Pick the row closest to your server above, and read each grant's "Costs" once more.
- [ ] Run the `allow` commands, for example `sudo nightgrid-node allow journal`, or pick them from the menu with `sudo nightgrid-node allow`.
- [ ] `nightgrid-node status`: each grant you made shows `ok` under `access`.
- [ ] For actions, list exactly what may run in `[actions] allow` (and `[actions] units` for systemd units), then `sudo systemctl restart nightgrid-node`.
- [ ] `nightgrid-node query actions.policy | jq` confirms what's allowed. Changed your mind? `sudo nightgrid-node deny <name>`.
