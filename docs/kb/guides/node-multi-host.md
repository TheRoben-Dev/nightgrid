# Link several servers and check from inside

With [Nightgrid Node](kb:basics/nightgrid-node) on two or more hosts, they can watch each other over the tailnet, check your services from inside the LAN every minute, and give you a read-only status page on your phone, all with the app closed and the laptop off.

![Linked nodes, inside checks and a phone page.](fig:ng-node-link)

## What you end up with

- Paired nodes that check each other over mutual TLS, so "the NAS is down" is told from "the path to the NAS is broken"
- HTTP(S) and TCP service checks with certificate expiry, run from inside the network
- A bookmarkable status page per host for your phone on the tailnet

## Before you start

- Nightgrid Node on each host ([Install Nightgrid Node](kb:howto/node-install))
- Tailscale running on each host that will link or serve a status page (`tailscale ip -4` prints an address). See [Tailscale subnet router](kb:apps/tailscale-subnet-router) for the basics.
- `jq` on the hosts makes the JSON easier to read

## Part 1 · Turn on the link

- [ ] On each host: `sudo nightgrid-node allow peers`. It installs `nightgrid-node-peer.service`, which listens only on the host's tailnet address (port 7419 by default) and talks only to tailnet ranges.
- [ ] `nightgrid-node query peers.latest | jq '{fingerprint, listening}'` on each host. Note both values, and the node id from `nightgrid-node status | jq -r .node.nodeId`. `listening` is `null` until Tailscale is up there.

## Part 2 · Pair two hosts

Pairing is done on both sides, each given the other's id, address and certificate fingerprint. There's no trust on first use: a node accepts only the certificate you gave it.

- [ ] On host A: `nightgrid-node query peers.set '{"peers":[{"id":"<B nodeId>","name":"nas","address":"<B listening>","fingerprint":"<B fingerprint>"}]}'`.
- [ ] On host B: the same with A's values.
- [ ] Within about 10 seconds, `nightgrid-node query peers.latest | jq .readings` on A shows B with `ok: true` and a round-trip time.
- [ ] `peers.set` replaces the whole list, so with three or more hosts send every peer each time (up to 64).
- [ ] What a peer may ask is root's `[peers] serve`: `["ping"]` by default (version, host name, uptime). Add `"ups"` only on hosts that should follow another host's UPS shutdown ([Shut down cleanly on a power cut](kb:guides/node-ups-shutdown)). `sudo nightgrid-node serve` with no arguments opens a menu of `ping`, `ups` and `sync` to switch each on or off; the named forms (`serve add sync`) still work.

## Part 3 · What linking tells you

- [ ] When a peer stops answering twice in a row, the timeline gets "peer nas unreachable" (warning) on every host that lost it, and "peer nas reachable again" when it's back.
- [ ] Read the `error`: "connection refused (is the link on there?)", "timed out", "not allowed by [peers] networks", or "the peer's certificate isn't the paired one". The last one appears after a purged reinstall gave the peer a new key: check why, then pair again with the new fingerprint.
- [ ] If every host lost one peer, that host is down. If only one host lost it, that path is broken.

## Part 4 · Service checks from inside

- [ ] On one host per network: `sudo nightgrid-node allow checks`. The runner may reach only `[checks] networks` (private ranges, loopback and the tailnet by default).
- [ ] Give it your checks: `nightgrid-node query checks.set '{"checks":[{"id":"jellyfin","name":"Jellyfin","url":"http://192.168.1.20:8096/health"},{"id":"proxy","name":"Proxy","url":"https://home.example.com/","expectStatus":"200-399"},{"id":"nas-ssh","name":"NAS SSH","url":"tcp://192.168.1.10:22"}]}'`.
- [ ] Results every minute: `nightgrid-node query checks.latest | jq`. A check down for 90 s raises `serviceDown`; a certificate within `certDays` (14 by default) raises `certExpiring`.
- [ ] Self-signed internal services: add `"allowUntrusted":true` so the certificate is shown but doesn't count as down.
- [ ] Checking something on the internet? Add its address to `[checks] networks` and run `allow checks` again. A name resolving outside the list fails with "outside [checks] networks".

## Part 5 · A status page for your phone

- [ ] On a host: `sudo nightgrid-node allow statuspage`. It prints the page's address, something like `http://100.x.y.z:7420/<secret>`.
- [ ] Open it on your phone with Tailscale on and bookmark it. It shows the host's state, load, disks, active alerts, the UPS guard and linked hosts, and refreshes by itself. No scripts, nothing it can change.
- [ ] Treat the link like a password: anyone with it and tailnet access can read that host's status. `sudo nightgrid-node deny statuspage` kills the link; turning it on again makes a new one.
- [ ] Prefer HTTPS? Set `[statuspage] listen` to a loopback `ip:port` and put `tailscale serve` (or your reverse proxy) in front.

## Tips

- Three hosts: pair every pair, so each one's view tells a dead host from a broken path.
- Keep versions close: `peers.latest` readings include each peer's version.
- Everything here is off unless you turned it on; `deny peers`, `deny checks` and `deny statuspage` undo each part.
