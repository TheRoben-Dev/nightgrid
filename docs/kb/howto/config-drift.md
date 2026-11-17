# Track config drift and host history

Save a baseline of a host's configuration, see exactly what changed since, compare two hosts that should match, and read a timeline of everything that happened to a host.

![Baseline versus today, as a diff.](fig:ng-config-drift)

## What you end up with

- Baselines of `/etc`, compose files and the package list, stored on the host itself
- File-by-file diffs with secrets masked
- A per-host incident timeline with boots, crashes, upgrades, deploys and alerts

## Part 1 · Save a baseline

- [ ] [System](view:system) → **config** → **Save baseline** on a host that's working well. It's kept on the host in `~/.nightgrid/baselines/default`, readable only by that login.
- [ ] Save a new baseline after every intended change, so the next comparison only shows surprises.

## Part 2 · Compare

- [ ] **Compare** shows files changed, added or removed, and packages upgraded, downgraded, added or removed.
- [ ] Click a changed file for a line diff. Secret-looking values are masked until **Reveal secrets**.
- [ ] **Compare with** another host compares two machines live, such as two nodes that should be identical. Files meant to differ (hostname, machine-id, host keys, network config) are hidden unless you turn that off.

## Part 3 · Timeline

- [ ] [System](view:system) → **timeline**: newest first, for 24 hours, 7, 30 or 90 days.
- [ ] **NG** entries come from Nightgrid (alerts, power actions, compose runs, deploys, rollbacks, upgrades, automation runs). **HOST** entries are read from the host when you open the tab (boots and downtime, failed units, OOM kills, package transactions, Docker crashes and health changes).
- [ ] **Problems only** filters to what went wrong; **✦ Summarize** has DRIFT explain the period.

#### Check

- [ ] Change a harmless file in `/etc` (add a comment), **Compare**: it shows up. Undo it.

## Limits and gotchas

- Only files your login can read are compared; `.env` files only by checksum.
- Failed units and OOM kills in the timeline need a login in the `systemd-journal` or `adm` group.
- Docker only remembers recent events; older crashes come from the journal.
- For full config management across many hosts, see [Ansible for your homelab](kb:apps/ansible).

## Related

- [Ansible for your homelab](kb:apps/ansible)
- [Reading logs](kb:basics/reading-logs)
- [Search logs across all hosts](kb:howto/log-search)
