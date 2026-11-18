# Plan capacity and replay incidents

Know what still fits before you add a VM, a game server or another stack, see what each app is and whether it's backed up, and step through the last outage minute by minute to see what really happened.

## What you end up with

- Every host's size and its busiest CPU and memory on [Fleet](view:fleet) → **CAPACITY**
- An answer to "where does this go?" and a size for a new server
- A catalog of every app with its URL, version and backup on **CATALOG**
- A replay of any outage or alert burst on **REPLAY**

## Before you start

- Hosts monitored for a few days: capacity uses the hourly history Nightgrid keeps while a host's metrics are polled.

## Part 1 · Read the headroom

- [ ] [Fleet](view:fleet) → **CAPACITY**. For each host: cores and memory, CPU and memory as average and **p95** (the level it reaches on its busiest hours), and the fullest disk with when it fills up.
- [ ] Switch between 7, 30 and 90 days. A host that's fine over 30 days but tight over 7 is getting busier.
- [ ] A host whose memory stays above 90% or CPU above 85% at p95 for a week sends a notice.

## Part 2 · Will it fit?

- [ ] Under **What if I add…**, set how many of each workload you're planning: a Windows VM, two Linux VMs, a game server.
- [ ] **FITS?** in the table says per host: **FITS**, **TIGHT** (it would run hot at its busiest) or **NO**, and why.
- [ ] **A new server for these** sizes a box for the same set with 30% headroom: a starting point for a new Proxmox host.

## Part 3 · Catalog what runs where

- [ ] **CATALOG** → **Build catalog** reads every server's containers with image, version, published ports and data folders, the URLs your service checks and reverse-proxy routes use, and which data folders a backup job's command line names.
- [ ] Tick **only without a full backup** to see what would be lost. Add those folders to a backup job, then **Read again**.
- [ ] **Export Markdown** saves the catalog as a document for your notes or a disaster-recovery binder.

## Part 4 · Replay an incident

- [ ] **REPLAY** lists outages (from uptime) and bursts of alerts from the last 14 days.
- [ ] Pick one and drag the scrubber, or press **Play**: CPU and memory, alerts, container restarts, OOM kills, deploys, file edits and notices line up on one timeline from 30 minutes before to 30 minutes after.
- [ ] **Logs for this window** reads the host's journal and container warnings and errors for that time (up to 7 days back).
- [ ] **Export** saves the timeline as Markdown for a write-up, or ask DRIFT what it makes of it.

## Limits and gotchas

- Workload sizes are typical values, not measurements. A transcode on a GPU costs far less CPU than the preset.
- Backup coverage is matched on folder paths in the job's systemd unit. A job that reads its folders from a file (`--files-from`) shows as not covering them.
- Replay shows what Nightgrid recorded. Time when the app was closed is missing unless Nightgrid Node filled it in.

## Related

- [Monitor backups and test restores](kb:howto/backups)
- [Prove your backups restore](kb:howto/restore-drills)
- [Uptime history and monthly reports](kb:howto/uptime-reports)
