# Proxmox Backup Server

Back up Proxmox VMs and containers incrementally with deduplication, verify them, and restore single files, using a separate Proxmox Backup Server (PBS).

## What you end up with

- PBS on its own machine (or a VM on different hardware) with a datastore
- Proxmox backup jobs that send only changed blocks
- Scheduled verify and prune, and backups you can browse file by file

## Before you start

- A machine or VM with a disk for the datastore. Don't put PBS on the same disks as the guests it protects.

## Part 1 · Install PBS

- [ ] Install from the PBS ISO, or on Debian 12 add the PBS repository and install `proxmox-backup-server`.
- [ ] Add it as a host in Nightgrid and connect as root once.
- [ ] Create a datastore (in PBS's web UI on port 8007, or the shell):

```sh
proxmox-backup-manager datastore create store1 /mnt/datastore/store1
```

- [ ] Create a user for Proxmox VE with the DatastoreBackup role on that datastore, and note the server's fingerprint: `proxmox-backup-manager cert info | grep Fingerprint`.

## Part 2 · Connect Proxmox VE

- [ ] On the PVE node (from a Nightgrid [Terminal](view:terminal)):

```sh
pvesm add pbs pbs-store1 --server pbs.example.lan --datastore store1 --username backup@pbs --fingerprint <fingerprint> --password
```

- [ ] Create a nightly job to it:

```sh
pvesh create /cluster/backup --schedule "02:00" --storage pbs-store1 --all 1 --mode snapshot --enabled 1
```

## Part 3 · Keep it healthy

- [ ] In PBS: Datastore → Prune & GC (keep-daily 7, keep-weekly 4, keep-monthly 6) and a weekly Verify job.
- [ ] Encrypt backups: add an encryption key to the PBS storage in PVE and store the key's paper backup safely.

## Part 4 · In Nightgrid

- [ ] [Proxmox](view:proxmox) → **BACKUPS** lists the PBS backups with ages, flags uncovered guests, and restores (as new or over the guest).
- [ ] [Fleet](view:fleet) → **Disks** on the PBS host forecasts when the datastore fills.

#### Check

- [ ] A guest's newest backup in Proxmox → BACKUPS is from last night, and a verify job shows OK in PBS.

## Limits and gotchas

- PBS backups are deduplicated chunks; copying the datastore offsite needs a second PBS (sync job) or a tape/S3 target in newer versions.
- Restoring single files from a VM backup is done in PBS's or PVE's UI; Nightgrid restores whole guests.

## Related

- [Day-to-day Proxmox from Nightgrid](kb:howto/proxmox)
- [Proxmox media server, run from Nightgrid](kb:guides/proxmox-media-server)
- [Backups](kb:basics/backups)
