# NFS shares

Export folders from a Linux server or NAS over NFS to other Linux hosts and containers, and mount them so a NAS reboot doesn't hang the client.

## What you end up with

- An NFSv4 export limited to your LAN
- Clients that mount on first access and recover after the server returns
- The mounts visible on Nightgrid's network map

## Part 1 · Server

- [ ] `sudo apt install -y nfs-kernel-server`.
- [ ] Add to `/etc/exports`:

```
/srv/media 192.168.1.0/24(rw,sync,no_subtree_check,root_squash)
```

- [ ] Apply: `sudo exportfs -ra` and check with `sudo exportfs -v`.
- [ ] Allow TCP 2049 from the LAN in [System](view:system) → **FIREWALL**.

## Part 2 · Client

- [ ] `sudo apt install -y nfs-common`.
- [ ] Add to `/etc/fstab`:

```
nas.example.lan:/srv/media /mnt/media nfs4 _netdev,x-systemd.automount,x-systemd.mount-timeout=10,noatime,nofail 0 0
```

- [ ] `sudo mkdir -p /mnt/media && sudo systemctl daemon-reload && ls /mnt/media`.

## Part 3 · Permissions

- [ ] NFS checks numeric uid/gid. Make the files' owner on the server match the uid the client's apps run as (for containers, `PUID`/`PGID`, often 1000).
- [ ] `root_squash` maps client root to `nobody`, so containers running as root can't write. Run them as a normal uid instead of turning it off.

## Part 4 · In Nightgrid

- [ ] Add a dependency in **Settings → Alerts → Dependencies**: the client host depends on the NAS, so a NAS outage folds the client's alerts.
- [ ] The [Network](view:network) map shows the share and its mount point on the client.

#### Check

- [ ] Reboot the NAS: the client's `ls /mnt/media` waits, then works again without a remount.

## Limits and gotchas

- NFS has no passwords; access is by client address. Keep it on a trusted LAN or tailnet.
- `sync` is safe but slower; `async` is faster and can lose recent writes if the server crashes.
- Docker volumes can mount NFS directly (`driver_opts: type: nfs`), which avoids depending on the host's fstab.

## Related

- [Samba file shares](kb:apps/samba)
- [ZFS pools, datasets and snapshots](kb:apps/zfs)
- [Tune alerts so only real problems notify](kb:howto/alerts)
