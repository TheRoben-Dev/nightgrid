# Use Nightgrid Node from the command line and its fix menu

Everything [Nightgrid Node](kb:basics/nightgrid-node) records is available from the server's shell. Run `nightgrid-node` on its own for a menu, `nightgrid-node fix` to find and fix common problems, or use the three plain commands: `status`, `query` and `watch`. They talk to the node's socket like any client, so they need a session in the `nightgrid-node` group (no sudo). Use them in a [Terminal](view:terminal) tab, or save the ones you like as command snippets.

## What you end up with

- Quick answers on the server itself: what happened while you were away, what's alerting, what's filling up
- Live views of events and metrics
- Settings the node takes over the socket (alert rules, checks, backup jobs, self-healing) set by hand
- A fix menu that lists what looks wrong on the host and fixes it after asking

## Part 1 · The menu and the three commands

- [ ] `nightgrid-node` with no command, in a terminal, opens a menu. Move with the arrow keys or `j`/`k`, pick with a number key or Enter, leave with `q`. It uses the app's NIGHTGRID colours and glyphs.
- [ ] Piped output, `NO_COLOR` or `NIGHTGRID_PLAIN=1` gives a plain numbered list instead, which suits scripts and screen readers.
- [ ] On a hub, `nightgrid-hub` on its own opens the hub's menu, and `sudo nightgrid-hub allow` (or `deny`) with no name opens a menu for the collector.
- [ ] As root, `sudo nightgrid-node allow` (or `deny`) and `sudo nightgrid-node serve` with no name open menus too: every grant and whether it's on, and what the link serves (`ping`, `ups`, `sync`). See [Choose what Nightgrid Node may do](kb:howto/node-grants).

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

The easy way is `nightgrid-node fix` (Part 6): it shows what's allowed and asks before each step. Keep `query actions.run` for scripting.

- [ ] See what root allows: `nightgrid-node query actions.policy | jq`.
- [ ] Try one without doing it: `nightgrid-node query actions.run '{"action":"container.restart","target":"web","dryRun":true}'`.
- [ ] Run it: the same with `"dryRun":false`. Unit actions take the full unit name as `target`; `images.prune` and `power.*` take none.
- [ ] Every request, dry run and refusal is in the hash-chained audit log: `nightgrid-node query actions.log '{"after":0}' | jq`. `verified: true` means the chain is intact.

## Part 6 · Fix things from the menu

- [ ] `nightgrid-node fix` opens a menu in the style of `ujust`. First it lists what looks wrong on the host: failed units, unhealthy or stopped containers, disks at 90% or more, a journal over 2 GB, a pending reboot and loose `~/.ssh` permissions. Then every fix, grouped by kind, then automation.
- [ ] Each fix explains what it does and asks before it runs. A reboot needs the word `reboot` typed.
- [ ] Short forms skip the menu. Add `-y` to skip the question:
  - `nightgrid-node fix list` (add `--json` for scripts)
  - `nightgrid-node fix restart UNIT|CONTAINER`
  - `nightgrid-node fix reset UNIT` clears a unit's failed state
  - `nightgrid-node fix start CONTAINER`
  - `nightgrid-node fix journal` shrinks the journal (to 500M by default)
  - `nightgrid-node fix logs` runs `logrotate --force`
  - `nightgrid-node fix packages` cleans the package cache
  - `nightgrid-node fix prune` removes unused images
  - `nightgrid-node fix ssh [USER]` tightens `~/.ssh` permissions
  - `nightgrid-node fix reboot`
  - `nightgrid-node fix run RUNBOOK`
- [ ] Automation is root only and off by default. `sudo nightgrid-node fix auto` opens the presets; see [Restart a sick container by itself](kb:guides/node-self-healing).
- [ ] Every fix is in the audit log: `nightgrid-node audit`. Fixes show as `fix.unit-reset`, `fix.journal-vacuum`, `fix.logrotate`, `fix.package-cache`, `fix.ssh-permissions` and `fix.runbook`, from source `menu (uid N)` or `autofix NAME`.

### fixes.toml and runbooks

- [ ] Run with sudo, the fix menu offers to allow a fix or list a unit. It saves that in `/etc/nightgrid-node/fixes.toml`, which only adds to `[actions] allow` and `units`. It never rewrites `config.toml`.
- [ ] Runbooks are `[[runbook]]` entries in that file: `name`, `description`, `run = ["/abs/path", "args"]` (run directly, no shell) and `timeout_secs` (1 to 600). Run one with `nightgrid-node fix run NAME`.

## Errors you may see

- **`hello_required`** or **`unsupported_proto`**: you're talking to the socket with something other than `nightgrid-node query`; use the CLI.
- **`capability_disabled`**: that feature is off in `/etc/nightgrid-node/config.toml`.
- **`bad_request`**: the JSON params don't fit; the message says which field.
- **`unknown_method`**: the node is older than the method; check `nightgrid-node --version`.
- **`forbidden`**: an internal method only the node's own helpers may call.
