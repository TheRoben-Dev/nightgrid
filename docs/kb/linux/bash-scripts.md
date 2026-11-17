# Write admin scripts that don't bite

Small Bash scripts run backups, cleanups and checks on every homelab server. A few habits make them stop on errors, log what they did, and never run twice at once.

## What you end up with

- A script template that fails loudly instead of carrying on after an error
- Logging to the journal, so you can read runs later
- Locking, dry runs and a check with ShellCheck

## Part 1 · The template

```bash
#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'

log() { logger -t "$(basename "$0")" -- "$*"; echo "$*"; }
trap 'log "failed at line $LINENO"' ERR

DRY_RUN=${DRY_RUN:-0}
run() { if [[ $DRY_RUN == 1 ]]; then log "dry run: $*"; else "$@"; fi; }

exec 9>/run/lock/"$(basename "$0")".lock
flock -n 9 || { log "already running"; exit 0; }

log "start"
run rsync -a --delete /srv/photos/ /mnt/backup/photos/
log "done"
```

- [ ] `set -e` stops on errors, `-u` on unset variables, `pipefail` on errors inside pipes.
- [ ] `flock` makes a second copy exit instead of running in parallel (a timer firing while the last run is still busy).
- [ ] `DRY_RUN=1 ./script.sh` prints what it would do.

## Part 2 · Habits

- [ ] Quote every variable: `"$dir"`, never `$dir`. Unquoted paths with spaces delete the wrong things.
- [ ] Guard destructive paths: `rm -rf -- "${target:?}/"*` stops if `target` is empty instead of running `rm -rf /*`.
- [ ] Use absolute paths for commands in scripts run by timers; their `PATH` is minimal.
- [ ] Exit codes matter: return non-zero on failure so timers and monitors see it.

## Part 3 · Check it

- [ ] `sudo apt install shellcheck` and run `shellcheck script.sh`: it catches quoting mistakes and common traps before they cost you.

#### Check

- [ ] Break a path in the script on purpose: it stops at that line, and `journalctl -t script.sh` shows the failure.

## In Nightgrid

- Edit scripts in [Files](view:files) on the host, then run them from a [Terminal](view:terminal) tab or schedule them with [System](view:system) → **Schedules** → **+ Timer**. A failed last run shows on the timer.
- Ask DRIFT to explain a command before you put it in a script: it flags risky flags like `rm -rf` on variables.
- Save scripts you use on many hosts as snippets in [Automation](view:automation).

## Limits and gotchas

- `set -e` doesn't trigger inside `if` conditions or in commands followed by `||`; check those results yourself.
- Scripts that grow past a page or two are easier to maintain in Python.

## Related

- [Schedule jobs with cron and timers](kb:howto/schedules)
- [Run commands on many hosts with snippets and playbooks](kb:howto/automation)
- [Write your own systemd service](kb:linux/systemd-units)
