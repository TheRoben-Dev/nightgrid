# Use Nightgrid Node from the command line

Everything [Nightgrid Node](kb:basics/nightgrid-node) records is available from the server's shell with three commands: `status`, `query` and `watch`. They talk to the node's socket like any client, so they need a session in the `nightgrid-node` group (no sudo). Use them in a [Terminal](view:terminal) tab, or save the ones you like as command snippets.

## What you end up with

- Quick answers on the server itself: what happened while you were away, what's alerting, what's filling up
- Live views of events and metrics
- Settings the node takes over the socket (alert rules, checks, backup jobs, self-healing) set by hand

## Part 1 · The three commands

- [ ] `nightgrid-node status` (or `--json`): version, node id, history coverage per tier, memory and database size, and the state of every grant.
- [ ] `nightgrid-node query <method> ['<json params>']`: one request, the answer as JSON. Pipe it to `jq` to read it comfortably.
- [ ] `nightgrid-node watch events metrics containers usage`: live frames for the streams you name; `--count N` stops after N frames.

## Part 2 · Everyday questions

- [ ] What's wrong right now: `nightgrid-node query alerts.active | jq`.
- [ ] What happened since yesterday, as short facts: `nightgrid-node query summary.daily '{"hours":24}' | jq -r '.facts[]'`. The same facts are meant for DRIFT: paste them into the DRIFT panel and ask what to look at first.
- [ ] The timeline: `nightgrid-node query events.sync '{"after":0,"limit":50}' | jq`. Each event has a growing `seq`; pass the last one you saw as `after` next time.
- [ ] Disks filling up: `nightgrid-node query disk.sync '{"after":0}' | jq` for hourly usage per mount.
- [ ] Containers: `nightgrid-node query containers.list | jq` (with `allow docker`).
- [ ] Who logged in or failed to (with `allow journal`): the `login` events in `events.sync`.
- [ ] Devices on the LAN: `nightgrid-node query net.devices | jq`; DHCP servers seen: `net.dhcp` (with `allow netwatch`).
- [ ] Listening sockets and the containers that publish them: `nightgrid-node query ports.list | jq`.
- [ ] Busiest services, containers and processes: `nightgrid-node query usage.latest | jq`.
- [ ] Disk health, pools, updates and UPS (with `allow health`): `nightgrid-node query health.latest | jq`.
- [ ] Blocked addresses: `nightgrid-node query block.list | jq`.

## Part 3 · Watch it live

- [ ] `nightgrid-node watch events` prints every new event as it's written: alerts, container crashes, logins, outages. Leave it in a tmux pane while you work on the host.
- [ ] `nightgrid-node watch metrics` pushes a sample every 2 seconds while you watch; the node samples faster only while someone subscribes.
- [ ] `nightgrid-node watch usage` shows per-service and per-container use every second.

## Part 4 · Change what it watches

These replace the whole setting each time: read the current value with the `.get` method first, edit it, and send it back.

- [ ] **Alert rules**: `nightgrid-node query alerts.rules.get`, then for example `nightgrid-node query alerts.rules.set '{"enabled":true,"diskFull":true,"diskPercent":85,"diskForecast":true,"forecastDays":14,"loginBurst":true}'`. `diskPercent` must be 50–99 and `forecastDays` 1–90. Missing fields reset to defaults.
- [ ] **Service checks** (with `allow checks`): `nightgrid-node query checks.set '{"checks":[{"id":"grafana","name":"Grafana","url":"https://grafana.lan/","expectStatus":"200-399","allowUntrusted":true},{"id":"nas-ssh","name":"NAS SSH","url":"tcp://nas.lan:22"}]}'`. Results: `checks.latest`.
- [ ] **Backup jobs**: `nightgrid-node query jobs.set '{"backups":[{"id":1,"name":"nightly","tool":"restic","source":"unit","target":"restic-backup.timer","staleHours":26}]}'`. `tool` is restic, borg, rsync or rclone; `source` is `unit` (a systemd unit) or `path` (a folder whose newest file counts). Results: `jobs.latest`.
- [ ] **Self-healing rules**: `heal.rules.get` / `heal.rules.set`; see [Restart a sick container by itself](kb:guides/node-self-healing).
- [ ] **Saved log searches**: `logs.searches.get` / `logs.searches.set`.

When Nightgrid's own integration with the node arrives, it sends these same settings from its views; until then, what you set here stays on the host and survives restarts and upgrades.

## Part 5 · Actions by hand

- [ ] See what root allows: `nightgrid-node query actions.policy | jq`.
- [ ] Try one without doing it: `nightgrid-node query actions.run '{"action":"container.restart","target":"web","dryRun":true}'`.
- [ ] Run it: the same with `"dryRun":false`. Unit actions take the full unit name as `target`; `images.prune` and `power.*` take none.
- [ ] Every request, dry run and refusal is in the hash-chained audit log: `nightgrid-node query actions.log '{"after":0}' | jq`. `verified: true` means the chain is intact.

## Errors you may see

- **`hello_required`** or **`unsupported_proto`**: you're talking to the socket with something other than `nightgrid-node query`; use the CLI.
- **`capability_disabled`**: that feature is off in `/etc/nightgrid-node/config.toml`.
- **`bad_request`**: the JSON params don't fit; the message says which field.
- **`unknown_method`**: the node is older than the method; check `nightgrid-node --version`.
- **`forbidden`**: an internal method only the node's own helpers may call.
