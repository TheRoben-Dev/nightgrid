# Journald: keep logs, but not forever

Make the systemd journal persistent across reboots, cap its size, and read it efficiently, so Nightgrid's timeline, log search and alerts have history to work with.

![Where journal entries come from and how to read them.](fig:lx-journald)

## What you end up with

- Logs that survive a reboot (needed for "why did it crash last night?")
- A size cap so logs never fill the root disk
- The journal readable by your login without sudo

## Part 1 · Persistent and capped

- [ ] Create `/etc/systemd/journald.conf.d/10-nightgrid.conf`:

```ini
[Journal]
Storage=persistent
SystemMaxUse=1G
SystemKeepFree=2G
MaxRetentionSec=3month
```

- [ ] Apply:

```sh
sudo mkdir -p /var/log/journal
sudo systemctl restart systemd-journald
journalctl --disk-usage
```

## Part 2 · Read access

- [ ] Add your login to the `systemd-journal` group so Nightgrid can read system logs (failed units and OOM kills in the timeline need this):

```sh
sudo usermod -aG systemd-journal $USER
```

- [ ] Log out and in again.

## Part 3 · Commands worth knowing

```sh
journalctl -b -1 -p err           # errors from the previous boot
journalctl -u nginx --since "1 hour ago"
journalctl -k -g "I/O error"      # kernel messages matching text
journalctl --list-boots           # when the machine booted
```

## Part 4 · In Nightgrid

- [ ] [System](view:system) → **journal** for one host, [Fleet](view:fleet) → **Logs** across all of them, [System](view:system) → **timeline** for boots, crashes and OOM kills.

#### Check

- [ ] After a reboot, `journalctl -b -1` shows the previous boot.

## Limits and gotchas

- Container logs aren't in the journal unless Docker's log driver is `journald`; Nightgrid reads them from Docker either way.
- On very small SD cards (Raspberry Pi), keep `SystemMaxUse` low or use `Storage=volatile` to spare the card.

## Related

- [Reading logs](kb:basics/reading-logs)
- [Search logs across all hosts](kb:howto/log-search)
- [Track config drift and host history](kb:howto/config-drift)
