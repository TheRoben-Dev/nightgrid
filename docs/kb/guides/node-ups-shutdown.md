# Shut down cleanly on a power cut

A UPS only buys minutes. With [Nightgrid Node](kb:basics/nightgrid-node) on each server, every host powers itself off in the right order during a long outage, with the app closed and the laptop asleep: compute first, storage last, and hosts with no UPS cable follow the one that has it.

![From a power cut to a clean power-off.](fig:ng-node-ups)

## What you end up with

- Each host with its own limits (minutes on battery, charge left, low-battery flag), so storage and the NUT server go last
- A countdown announced in the timeline, cancelled by itself when power returns
- Hosts that can't read the UPS following a paired host's shutdown over the tailnet
- A dry-run mode to rehearse it all without powering anything off

## Before you start

- A UPS connected by USB to one host, with NUT serving it (`upsc <name>@localhost` prints readings). Set NUT up first: see [NUT](kb:apps/nut) and [the UPS tab](kb:howto/ups), which reads the same NUT server.
- Nightgrid Node on every host on that UPS ([Install Nightgrid Node](kb:howto/node-install))
- For hosts without the USB cable: Tailscale on all of them (the link runs over the tailnet)

## Part 1 · The host with the UPS cable

- [ ] `sudo nightgrid-node allow health` and `sudo nightgrid-node allow helper`.
- [ ] In `/etc/nightgrid-node/config.toml`: `[health] ups = ["<ups>@localhost"]`, and in `[actions]` add `"power.poweroff"` to `allow`.
- [ ] In `[ups]`: `shutdown = true`, `dry_run = true` for now, and limits. For the storage box or NUT server use the longest: for example `on_battery_minutes = 15`, `below_percent = 30`, `low_battery = true`, `countdown_secs = 60`.
- [ ] `sudo systemctl restart nightgrid-node`, then `nightgrid-node query ups.guard | jq`: `problem` must be `null`, `phase` is `idle`. If `problem` names something ("power.poweroff isn't in [actions] allow"), fix that first.

## Part 2 · Other hosts on the same UPS

Either give each host its own NUT client reading the NUT server (best: each decides alone from the real reading), or let it follow the UPS host over the link.

- [ ] **Own reading**: on each host `[health] ups = ["<ups>@<nut-server-ip>"]` (the health helper may reach private LAN ranges), `allow health`, `allow helper`, `"power.poweroff"` in `allow`, `[ups] shutdown = true` with shorter limits than storage (for example `on_battery_minutes = 3`).
- [ ] **Follow a peer** (no reading of its own): `sudo nightgrid-node allow peers` and `allow helper` on both hosts; on the follower, `"ups"` in `[peers] serve`, `"power.poweroff"` in `[actions] allow`, `[ups] shutdown = true`. Pair the two hosts as in [Link several servers](kb:guides/node-multi-host), and on the follower mark the UPS host's entry with `"followUps": true`.
- [ ] `nightgrid-node query ups.guard | jq` on each: `problem` is `null` everywhere.

## Part 3 · Rehearse

- [ ] Leave `dry_run = true` on every host. Pull the UPS's wall plug (with nothing critical running, or at a quiet time).
- [ ] Watch on the UPS host: `nightgrid-node watch events`. You'll see the on-battery alert, then "UPS shutdown in 60 s" when its limit is reached; followers get the same through the link (`ledBy` in their `ups.guard` names the UPS host).
- [ ] Plug it back in: "UPS shutdown cancelled: power is back". The action log has dry-run power-offs, nothing real.
- [ ] Check the order is what you want: compute hosts' countdowns start first, storage last.

## Part 4 · Arm it

- [ ] Set `dry_run = false` on every host and restart the node.
- [ ] During a real outage, a countdown can be cancelled from the shell with `nightgrid-node query ups.cancel`. That stops this host's shutdown for the rest of the outage; power returning cancels it by itself. A scheduled power-off can also be stopped within its minute with `sudo shutdown -c`.
- [ ] Set the hosts' firmware to power on after AC returns ("Restore on AC power loss: Power On") so they come back without you.

## How it decides

- `phase` goes `idle` → `onBattery` → `countdown` (power-off at `deadline`) → `shuttingDown` (the power-off was requested, a minute ahead, and logged with source `ups`). `cancelled` holds for the rest of that outage.
- While on battery the UPS is read every 10 s instead of 30 s.
- An unreadable UPS never starts a countdown. Nothing from the socket can start a shutdown or change the limits; it can only read and cancel.

## If it misbehaves

- **Never counts down**: `problem` in `ups.guard` says why. Also check `nightgrid-node query health.latest | jq .ups`: no reading means NUT isn't reachable.
- **Follower doesn't follow**: `peers.latest` on it must show the UPS host reachable, the UPS host's entry needs `"followUps": true`, and `"ups"` must be in the follower's `[peers] serve`.
