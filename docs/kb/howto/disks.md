# Keep an eye on disks and pools

Catch failing drives, degraded pools and disks that will be full next month, across every host, before they turn into an outage.

![What the Disks tab collects and what it warns about.](fig:ng-disk-forecast)

## What you end up with

- ZFS, mdraid and btrfs health with scrub and resync progress on one page
- SMART health per drive
- A "full in about N days" forecast for every mount, with alerts

## Part 1 · Look at the fleet

- [ ] [Fleet](view:fleet) → **Disks**, **Check all**.
- [ ] Each host lists its pools and arrays with degraded devices, error counters and scrub, resilver or resync progress. None of this needs root.
- [ ] SMART shows where sudo needs no prompt. For a host that asks for a password, read it once from [System](view:system) → **disks** with **remember** ticked.

## Part 2 · Forecasts

- [ ] Every mount shows growth per day and **full in about N days**, from usage recorded while the host is monitored (one point an hour, kept 90 days).
- [ ] The host's monitor chart also shows a 14-day trend line for the fullest disk.

## Part 3 · Alerts

- [ ] **Settings → Alerts**: **Disk almost full** (pick **DISK FULL AT**), **Disk full soon (forecast)** (pick **FULL WITHIN**, 14 days by default), **Storage pool degraded** and **SMART warning**.

## Part 4 · Scrubs and SMART tests on the host

Nightgrid reads results; the scheduled work runs on the host so it happens with Nightgrid closed.

- [ ] ZFS: Debian and Ubuntu's `zfsutils-linux` scrubs every pool monthly by itself. Check with `systemctl list-timers | grep -i zfs` or `cat /etc/cron.d/zfsutils-linux`.
- [ ] mdraid: Debian's `mdadm` package runs a monthly check (`checkarray`).
- [ ] SMART self-tests: install `smartmontools` and let `smartd` run them. A line in `/etc/smartd.conf` that tests all disks (short daily at 02:00, long on Saturdays at 03:00):

```
DEVICESCAN -a -o on -S on -s (S/../.././02|L/../../6/03) -m root
```

- [ ] Restart it: `sudo systemctl restart smartd` (the unit is `smartmontools` on some distros).

#### Check

- [ ] Fleet → Disks shows a recent scrub date for every pool and SMART **PASSED** for every drive.

## Limits and gotchas

- USB enclosures often hide SMART. `smartctl -d sat -a /dev/sdX` sometimes gets through.
- SMART PASSED isn't a promise. Rising reallocated or pending sector counts matter more than the overall verdict.
- The forecast needs about a day of data, and a one-off big copy skews it for a while.

## Related

- [ZFS pools, datasets and snapshots](kb:apps/zfs)
- [mergerfs and SnapRAID](kb:apps/mergerfs-snapraid)
- [Tune alerts so only real problems notify](kb:howto/alerts)
- [Send mail from your servers](kb:apps/mail-relay)
