# Borg backups with borgmatic

Back up with BorgBackup driven by borgmatic: one YAML file for sources, retention, consistency checks and database dumps, on a systemd timer.

## What you end up with

- Encrypted, deduplicated archives on a local disk or a remote borg server
- Database dumps taken by borgmatic as part of each run
- A timer Nightgrid's backup board reads, with test restores

## Before you start

- A repository location: a local disk or `ssh://backup@backup-host/./borg/myhost` with borg installed on the remote.
- Root's SSH key authorized on the backup host if remote.

## Part 1 · Install

- [ ] `sudo apt install -y borgbackup borgmatic`. Debian's borgmatic ships a systemd service and timer.
- [ ] Generate a starting config: `sudo borgmatic config generate` (writes `/etc/borgmatic/config.yaml`).

## Part 2 · Configure

- [ ] A minimal `/etc/borgmatic/config.yaml`:

```yaml
source_directories:
  - /etc
  - /opt/stacks
  - /srv/data
repositories:
  - path: ssh://backup@backup-host/./borg/myhost
    label: remote
exclude_caches: true
encryption_passcommand: cat /etc/borgmatic/passphrase
keep_daily: 7
keep_weekly: 4
keep_monthly: 6
checks:
  - name: repository
    frequency: 2 weeks
  - name: archives
    frequency: 1 month
postgresql_databases:
  - name: all
    hostname: 127.0.0.1
    username: postgres
    password: "{credential file /etc/borgmatic/pgpass}"
```

- [ ] Create the passphrase file (mode 600) and save a copy in your password manager.
- [ ] Validate: `sudo borgmatic config validate`.
- [ ] Create the repository: `sudo borgmatic repo-create --encryption repokey-blake2` (older borgmatic: `borgmatic init --encryption repokey-blake2`).
- [ ] Export the key too: `sudo borg key export <repo> /root/borg-key.txt`, and store it with the passphrase.

## Part 3 · Run on a timer

- [ ] `sudo systemctl enable --now borgmatic.timer` (daily by default; override with `systemctl edit borgmatic.timer`).
- [ ] First run by hand with output: `sudo borgmatic --verbosity 1 --list --stats`.

## Part 4 · In Nightgrid

- [ ] [Fleet](view:fleet) → **Backups** → **Find jobs** picks up `borgmatic.timer`.
- [ ] **Test restore** works for borg jobs and runs as the unit's user.

#### Check

- [ ] `sudo borgmatic list` shows today's archive, and the board is green.

## When Nightgrid is closed

- Everything runs on the host.

## Limits and gotchas

- The database hook needs the client tools (`pg_dump`, `mariadb-dump`) on the host, or use borgmatic's container-aware options; check its docs for your version.
- A repository is locked while in use. A crashed run can leave a stale lock: `borg break-lock` once you're sure nothing is running.
- Borg 2 changes the repository format; stay on 1.x until borgmatic and your tools support it.

## Related

- [restic backups with systemd timers](kb:apps/restic)
- [Monitor backups and test restores](kb:howto/backups)
- [Backups](kb:basics/backups)
