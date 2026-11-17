# Kopia backups

Fast, encrypted, deduplicated backups with snapshot policies, compression and a built-in UI, to local disks, SFTP, S3 or a Kopia repository server.

## Part 1 · Install on a host

- [ ] Install from Kopia's apt repository (see its docs) or the release binary.
- [ ] Create a repository and connect:

```sh
sudo kopia repository create filesystem --path /mnt/backup/kopia
sudo kopia policy set --global --keep-daily 7 --keep-weekly 4 --keep-monthly 6 --compression zstd
sudo kopia snapshot create /etc /opt/stacks /srv/data
```

- [ ] Save the repository password in your password manager.

## Part 2 · Schedule

- [ ] A [timer](kb:howto/schedules) named `kopia-backup` running `kopia snapshot create /etc /opt/stacks /srv/data` nightly (as root, so the repository config in `/root/.config/kopia` is used).
- [ ] Add it in [Fleet](view:fleet) → **Backups** as a timer job.

## Part 3 · Restore

```sh
sudo kopia snapshot list /srv/data
sudo kopia restore <snapshot-id>/path/to/file /tmp/restore/
```

#### Check

- [ ] `kopia snapshot list` shows last night's snapshot.

## Limits and gotchas

- Nightgrid's one-click **Test restore** understands restic and borg; for Kopia, restore a file by hand monthly.
- `kopia server` adds a web UI and multi-client repository; keep it behind TLS and a login.

## Related

- [restic backups with systemd timers](kb:apps/restic)
- [Backups](kb:basics/backups)
