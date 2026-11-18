# Restart a sick container by itself

A container that goes unhealthy at night shouldn't wait for you to wake up. Let [Nightgrid Node](kb:basics/nightgrid-node) restart it after a few minutes, under strict limits, with every run logged, and only after a dry run proved the rule does what you expect.

![How a self-healing rule decides to act.](fig:ng-node-heal)

## What you end up with

- A self-healing rule: "unhealthy for 5 minutes → restart", capped per hour and per day, that gives up and tells you when restarting doesn't help
- Optional rules for down containers and full disks (prune dangling images)
- A tamper-evident record of every action, real or dry

## Before you start

- The node installed with `allow docker` ([Install Nightgrid Node](kb:howto/node-install))
- The container has a healthcheck if you want the "unhealthy" condition (`docker inspect -f '{{json .State.Health}}' <name>` shows it). Without one, use the "down" condition.

## Part 1 · Allow the action

- [ ] Edit `/etc/nightgrid-node/config.toml` as root. In `[actions]`, set `allow = ["container.restart"]` (add `"images.prune"` if you want the disk rule). Keep `max_per_hour` low (the default 20 covers every rule and manual run together).
- [ ] `sudo systemctl restart nightgrid-node`.
- [ ] `nightgrid-node query actions.policy | jq`: `allowed` lists what you just added. Nothing you send over the socket can widen this list.
- [ ] Rather not edit `config.toml`? `sudo nightgrid-node fix` offers to allow a fix or list a unit, and saves it in `/etc/nightgrid-node/fixes.toml`. That file only adds to `[actions] allow` and `units`.

## Part 2 · Write the rule, dry run first

- [ ] Rules are sent as a whole list. Start with one, enabled but in dry-run mode:
  `nightgrid-node query heal.rules.set '{"rules":[{"id":"web-unhealthy","enabled":true,"dryRun":true,"when":"containerUnhealthy","target":"web","forMinutes":5,"action":"container.restart","maxPerHour":3,"maxPerDay":10,"giveUpAfter":3}]}'`
- [ ] `when` is `containerUnhealthy`, `containerDown` (the container-down alert is active and it isn't running) or `diskFull` (the disk-full alert is active for a mount). `target` is the container name, a mount, or `*` for any.
- [ ] Wait for the condition (or cause it in a test container) and read `nightgrid-node query actions.log '{"after":0}' | jq`: you should see "dry run: would restart web" from source `rule web-unhealthy`.
- [ ] `nightgrid-node query heal.status | jq` shows what each rule is tracking, since when, and how many attempts it made.

## Part 3 · Go live

- [ ] Send the same rule with `"dryRun":false`.
- [ ] How it behaves: once the condition has held `forMinutes`, it restarts; then waits `forMinutes` again before another try; stays within `maxPerHour` and `maxPerDay` (which survive the condition flapping); and after `giveUpAfter` runs without the problem clearing, it stops and adds a warning: "self-healing gave up on web". When the condition clears, the count starts over.
- [ ] Every run is an `action` event in the timeline and a record in the audit log. `verified: true` from `actions.log` means nobody edited or deleted records.

## Part 4 · More rules worth having

- [ ] **Down, not just unhealthy**: `{"id":"web-down","when":"containerDown","target":"web","action":"container.restart",...}` for containers without a healthcheck. Prefer Docker's own `restart: unless-stopped` for plain crashes; this rule catches what that policy doesn't.
- [ ] **Disk full of old images**: `{"id":"prune","when":"diskFull","target":"/","action":"images.prune","forMinutes":10,...}` removes dangling images only, never ones in use.
- [ ] Up to 32 rules; ids are 1–40 letters, digits, `-` and `_`.

## Part 5 · Units, reboots and safe edits (optional)

- [ ] Restarting a systemd service instead of a container needs the root helper: `sudo nightgrid-node allow helper`, the unit by full name in `[actions] units` (for example `units = ["nginx.service"]`), and `unit.restart` in `allow`. These run from manual requests only; self-healing rules can't use unit or power actions.
- [ ] Run one by hand: `nightgrid-node query actions.run '{"action":"unit.restart","target":"nginx.service","dryRun":false}'`.
- [ ] Risky file edits can be wrapped in a rollback: list the paths in `[rollback] paths` (keep it narrow, for example your compose folder), add `files.rollback` to `allow`, and run `sudo nightgrid-node allow helper` again. Before editing, arm it: `nightgrid-node query changes.begin '{"paths":["/etc/ssh/sshd_config"],"confirmWithinSecs":120,"note":"move SSH to port 2222","onRevert":{"action":"unit.restart","target":"ssh.service"}}'`. Make the edit, test a fresh login, then `changes.confirm '{"id":"<id>"}'`. If you get locked out and never confirm, the node puts the file back (even across a reboot) and restarts SSH (that needs `unit.restart` allowed and `ssh.service` in `[actions] units`).

## Part 6 · Host-local presets (optional)

The rules above come from the app or `heal.rules.set`. Root can also switch on ready-made presets on the host itself, next to them.

- [ ] `sudo nightgrid-node fix auto` opens the presets menu. Or switch one directly: `sudo nightgrid-node fix auto NAME on|preview|off`.
- [ ] The presets: `services` (restart failed units root listed), `containers` (restart unhealthy containers), `stopped` (start containers that stopped), and `journal`, `packages` and `images` (when a disk fills).
- [ ] Start with `preview`: it only logs what the preset would do. Read it with `nightgrid-node audit`, where runs show from source `autofix NAME`.
- [ ] Each preset waits 5 to 10 minutes, is capped per hour and per day, and gives up until the condition clears. All start off.

## If it misbehaves

- **"refused" in the log**: the action isn't in `[actions] allow`, the container name is wrong, or the hourly cap was reached. `detail` says which.
- **It restarts but the problem comes back**: that's what `giveUpAfter` is for. Read the container's logs; a restart loop hides the real fault.
- **Turn it all off at once**: `heal.rules.set '{"rules":[]}'`, or empty `[actions] allow` and restart the node.
