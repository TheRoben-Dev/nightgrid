# Nightgrid Node: the optional server agent

Nightgrid works without anything installed on your servers: it does everything over SSH, but only while the app is open. Nightgrid Node is an optional, tiny service you can put on a Linux server so it keeps watching when the app is closed, and hands everything over when the app comes back.

![Where Nightgrid Node runs and how clients reach it.](fig:ng-node-arch)

## Why it exists

Close the laptop and nobody is watching. A container that crash-loops at 3 AM, a disk that fills overnight or a power cut while you're away only show up later as a gap in the graphs. The node runs on the server itself and closes those gaps:

- **Metric history with no holes**: CPU, memory, swap, load, disks, network, temperatures and GPUs every 10 s, kept in the same tiers as the app (1-minute points for 14 days, 15-minute for 90 days, hourly for 400 days).
- **Uptime that knows what happened**: a heartbeat every minute tells a crash or power loss ("Down") from a clean reboot ("Restarted") and from a suspend ("Suspended").
- **Alerts while you're away**: disk full, disk filling up, container down, failing disks, degraded pools, power cuts, failed backups, services down. They are kept and delivered when the app reconnects.
- **A timeline**: alerts firing and clearing, outages, boots, container crashes and restarts, logins, file changes, new devices on the LAN, numbered so a client pulls only what's new.

## What it is (and isn't)

- **Optional.** Every Nightgrid feature works without it. It makes things gap-free and faster, never required.
- **Tiny.** One static binary of about 2 MB for x86_64 and aarch64, about 2.5 MB of RAM, no measurable idle CPU.
- **Offline by design.** The main service has no network access at all (`IPAddressDeny=any`, unix sockets only). No open ports, no outbound connections, no telemetry, no cloud. Clients reach it through a unix socket, `/run/nightgrid-node/node.sock`, over the SSH connection they already have.
- **Least privilege.** It runs as its own `nightgrid-node` system account under a heavily sandboxed systemd unit. Anything beyond the basics (containers, the journal, health checks, actions) is a separate grant that root makes with `nightgrid-node allow <name>` and can take back with `deny`.
- **Off by default for anything that acts.** Self-healing, blocking addresses, UPS shutdown, the link between nodes and the phone status page all start switched off.
- **Several clients at once.** The node keeps no per-client state, so more than one computer can sync from the same node.

## How it fits together

```
your computer ──SSH──> server: sshd ──> nightgrid-node bridge ──> /run/nightgrid-node/node.sock
                                                                    │
                                          nightgrid-node run (user nightgrid-node)
                                            ├ sampler: /proc, /sys, statvfs
                                            ├ engine: uptime, disk usage, alerts
                                            ├ containers (only if root allowed it)
                                            └ SQLite history in /var/lib/nightgrid-node
```

Clients speak a small, versioned JSON-lines protocol to the socket. The first request, `hello`, lists the node's capabilities, and a client only uses what's listed. On the server you talk to it with the same binary: `nightgrid-node status`, `nightgrid-node query <method>` and `nightgrid-node watch <stream>`.

## What it can do, by grant

| Works out of the box | Needs a grant from root |
|---|---|
| Metrics, uptime, disk usage and forecasts | `allow docker`: watch containers (root-equivalent access, read-only use) |
| Disk-full and filling-up alerts | `allow journal`: SSH logins, failed logins, log watching, job output |
| Timeline of events | `allow netwatch`: DHCP servers, rogue DHCP, device names (listen-only) |
| File change watch (metadata only) | `allow health`: SMART, ZFS/mdadm/Btrfs pools, pending updates, UPS via NUT |
| LAN inventory from the ARP table | `allow helper`: restart/start/stop listed units, reboot, power off, file rollbacks, blocking |
| Listening ports | `allow power`: power draw from the CPU's energy counters |
| Usage per service and container | `allow checks`: HTTP(S)/TCP service checks from inside the LAN |
| Daily summary facts for DRIFT | `allow peers`: nodes check each other over the tailnet |
| | `allow statuspage`: a read-only status page for your phone on the tailnet |

Read [Choose what Nightgrid Node may do](kb:howto/node-grants) before granting anything, then [Install Nightgrid Node](kb:howto/node-install).

## Where the app stands

The node is developed as its own project and published separately. Since Nightgrid 0.10.88 the app uses it where it runs: **Fleet → NODE** installs, updates and removes it per host, and after a reconnect the node's history fills the app's charts, uptime bars, disk forecasts, timeline and alerts; while connected, events arrive as they happen and live tiles take the node's samples instead of polling over SSH. Its other abilities (containers, logins and file changes, the LAN watch, self-healing, service checks, status page and so on) are set up on the server for now ([Use Nightgrid Node from the command line](kb:howto/node-cli)) and come into the app later. Nightgrid's views keep working over SSH exactly as before, with or without the node.

## Scenarios

- [Keep watching with the laptop closed](kb:guides/node-always-watching): one server, history and alerts while you're away.
- [Restart a sick container by itself](kb:guides/node-self-healing): self-healing rules, dry run first.
- [Shut down cleanly on a power cut](kb:guides/node-ups-shutdown): UPS limits per host, storage last, hosts without a UPS cable follow.
- [Guard an internet-facing server](kb:guides/node-security-watch): failed logins, auto-block, file baseline, new listening ports.
- [Watch the home network](kb:guides/node-lan-watch): new devices, IP conflicts, rogue DHCP, a gateway that changes its MAC.
- [Link several servers and check from inside](kb:guides/node-multi-host): node-to-node checks, service checks from the LAN, a phone status page.
