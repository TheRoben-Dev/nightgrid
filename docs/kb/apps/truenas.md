# Run a TrueNAS box alongside Nightgrid

TrueNAS SCALE (now "TrueNAS Community Edition") is a full NAS operating system built on ZFS. It wants to own its config, so treat it as an appliance: manage it from its web UI and let Nightgrid watch it over SSH.

![The ZFS layout TrueNAS builds for you.](fig:zfs-layout)

## What you end up with

- ZFS pools, datasets, snapshots and replication from TrueNAS's web UI
- The box visible in Nightgrid with pool health, SMART and load
- Clear lines between what TrueNAS manages and what you touch

## Part 1 · SSH for Nightgrid

- [ ] TrueNAS: Credentials → Users → your admin user → add your public key (Nightgrid's from Settings → **Connections** → **SSH keys**).
- [ ] System → Services → SSH: start it, start automatically. Leave password login off.
- [ ] In Nightgrid, add the host with that user. Root-only reads (SMART) need sudo: allow passwordless sudo for that user in TrueNAS only if you accept that.

## Part 2 · What to watch from Nightgrid

- [ ] [Fleet](view:fleet) → **Disks**: pool health, scrub progress and the full-in-N-days forecast per dataset mount.
- [ ] [Overview](view:overview): CPU, memory (ZFS ARC makes "used" look high; that's normal), network and temperatures.
- [ ] [Services](view:services): add the TrueNAS web UI and any apps you run on it.
- [ ] [Services](view:services) → **Add integration** → **TrueNAS** with an API key (user menu → API Keys): pool health and fill, TrueNAS's own alerts, load and memory on a card, no sudo needed (see [Add app cards](kb:howto/integrations)).

## Part 3 · Leave alone

- [ ] Don't install packages with apt, edit files under `/etc`, or change ZFS properties by hand: TrueNAS overwrites them, and updates replace the system.
- [ ] Updates come through TrueNAS's own update page, not [Fleet](view:fleet) → **Updates**.
- [ ] Apps (Docker-based since 24.10) are managed in TrueNAS's Apps page. Don't start, stop or redeploy them from Nightgrid's [Docker](view:docker) view: TrueNAS keeps its own record of each app.

#### Check

- [ ] Fleet → Disks lists the TrueNAS pools as ONLINE with the last scrub date.

## Limits and gotchas

- TrueNAS needs a boot drive of its own; data pools on separate disks.
- ECC memory is recommended for ZFS but not required; good backups matter more.
- For a NAS that's also a general server you shape yourself, plain Debian with ZFS or openmediavault fits better.

## Related

- [ZFS pools, datasets and snapshots](kb:apps/zfs)
- [Keep an eye on disks and pools](kb:howto/disks)
- [openmediavault NAS on Debian](kb:apps/openmediavault)
