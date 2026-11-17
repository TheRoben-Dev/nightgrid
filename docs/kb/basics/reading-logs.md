# Reading logs

When something breaks, the answer is almost always in a log. The trick is knowing where to look and how to cut it down to the lines that matter.

## Where logs live

- **systemd journal** (most Linux services): `journalctl`.
- **Docker containers**: whatever the app prints, via `docker logs`.
- **Files** under `/var/log` for older or non-systemd services (`/var/log/syslog`, nginx's `access.log` and `error.log`).
- **The kernel**: `dmesg` or `journalctl -k` (disks, USB, out-of-memory kills).

## journalctl essentials

```sh
journalctl -u ssh -n 100            # last 100 lines of one unit
journalctl -u nginx -f              # follow live
journalctl -p err -b                # errors and worse since this boot
journalctl --since "1 hour ago"
journalctl -b -1 -e                 # end of the previous boot (after a crash)
```

## Docker logs

```sh
docker logs --tail 200 -f app
docker logs --since 30m app 2>&1 | grep -i error
docker compose logs -f              # every service in the stack
```

## How to read them

1. **Find the first error, not the last.** One failure causes a cascade; the earliest error is usually the cause.
2. **Read the line before it.** "Connection refused" says little; the line before often names the address and port.
3. **Match times.** Note when the problem started and look there. Clocks matter: check the host's time zone.
4. **Look for the usual suspects**: `permission denied` (ownership, user IDs in containers), `address already in use` (two things on one port), `no space left on device`, `OOM` or `Killed` (memory), `certificate` (expiry, wrong name), `connection refused` vs `timed out` (nothing listening vs a firewall).
5. **Raise the log level** temporarily if the app allows it, then turn it back down.

## In Nightgrid

- System → **JOURNAL** follows units with filters; [Docker](view:docker) shows container logs with search.
- Fleet → **LOGS** searches the journal and container logs across all hosts at once, and saved searches can raise alerts.
- Log alerts spot error bursts and new error patterns; DRIFT can explain the lines in front of you and suggest what to check, offline.
- System → **TIMELINE** lines up alerts, restarts and deploys, which often answers "what changed?".

## Related

- [Docker and compose basics](kb:basics/docker-compose): `docker compose logs` per stack.
- [Hardening sshd](kb:basics/sshd-hardening): failed logins in the journal and what to do about them.
