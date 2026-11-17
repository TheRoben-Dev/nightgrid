# Run commands on many hosts with snippets and playbooks

Run one command across a group, save commands with variables as snippets, and chain steps into playbooks you can rerun.

![One run across a group, with a result per host.](fig:ng-automation)

## What you end up with

- Per-host output, exit code and time for every run
- A snippet library with `{{variables}}`
- Playbooks for repeat jobs: prepare a new host, roll out a config file, restart a stack in order

## Part 1 · Ad-hoc runs

- [ ] [Automation](view:automation) (`Mod+8`): type a command, pick hosts (click a group to select all of it), **Run**.
- [ ] **Select failed** reruns only the hosts that failed.
- [ ] **✦ Explain** on a result sends it to DRIFT.

## Part 2 · Snippets

- [ ] Save a command as a snippet. `{{name}}` asks for a value, `{{name=default}}` pre-fills one, and `{{host}}`, `{{hostname}}`, `{{user}}` fill in per host.
- [ ] Values are quoted safely; don't add your own quotes around placeholders.

Useful starters:

```sh
df -h --output=target,pcent,avail -x tmpfs -x devtmpfs
```

```sh
docker compose -f {{stack_dir=/opt/stacks/app}}/compose.yaml pull
```

```sh
systemctl restart {{unit}}
```

## Part 3 · Playbooks

- [ ] Steps run in order: commands, file uploads, wait for a port, Docker start, stop, restart or pull.
- [ ] A "new Debian host" playbook: install basics, upload your `sshd_config` drop-in, restart ssh, wait for port 22.

#### Check

- [ ] Run a harmless snippet (`uptime`) on a group and see every host answer.

## Part 4 · Safety

- [ ] Anything destructive (`rm`, `prune`, `kill`, `compose down`, `reboot`, `upgrade`, `> file`, …) is confirmed with the targets named. On several hosts you also type the group name or the number of hosts.

## Limits and gotchas

- Runs need Nightgrid open; for unattended jobs use a timer on the host (see [Schedule jobs with cron and timers](kb:howto/schedules)).
- For idempotent config management across many machines, Ansible is the better tool; Nightgrid can still launch it (see [Ansible for your homelab](kb:apps/ansible)).

## Related

- [Ansible for your homelab](kb:apps/ansible)
- [Schedule jobs with cron and timers](kb:howto/schedules)
- [Patch every host from one place](kb:howto/fleet-updates)
