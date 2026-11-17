# Mount a Windows or NAS share on Linux

Mount an SMB share (a Windows PC, Synology, TrueNAS, Samba) on a Linux server so containers and scripts can use it like a local folder, with the password kept out of fstab.

## What you end up with

- A share mounted at boot with the right owner for your containers
- Credentials in a root-only file
- A mount that waits for the network and doesn't hang boot if the NAS is off

## Before you start

- `sudo apt install cifs-utils` (Fedora: `dnf install cifs-utils`)
- A user on the NAS with access to the share

## Part 1 · Credentials file

```sh
sudo install -m 600 /dev/null /etc/smb-nas.cred
sudoedit /etc/smb-nas.cred
```

```
username=mediauser
password=<the password>
domain=WORKGROUP
```

## Part 2 · Try it by hand

```sh
sudo mkdir -p /mnt/nas/media
sudo mount -t cifs //192.168.1.20/media /mnt/nas/media \
  -o credentials=/etc/smb-nas.cred,uid=1000,gid=1000,vers=3.1.1
ls /mnt/nas/media
```

- [ ] `uid`/`gid` make files appear owned by that user (match your containers' `PUID`/`PGID`). SMB doesn't carry Linux ownership by default.
- [ ] Errors: `Permission denied` = wrong user or password; `Host is down` = wrong protocol version or SMB1-only device (`vers=1.0` works but is insecure).

## Part 3 · Mount at boot

- [ ] Add one line to `/etc/fstab`:

```
//192.168.1.20/media /mnt/nas/media cifs credentials=/etc/smb-nas.cred,uid=1000,gid=1000,vers=3.1.1,_netdev,nofail,x-systemd.automount 0 0
```

- [ ] `x-systemd.automount` mounts on first access, so boot never waits on the NAS; `nofail` keeps boot going if it's off.
- [ ] `sudo systemctl daemon-reload && ls /mnt/nas/media` triggers it.

#### Check

- [ ] Reboot with the NAS off: the server boots normally, and the share mounts once the NAS is back and something opens the folder.

## In Nightgrid

- Containers using the share: bind-mount `/mnt/nas/media` in the stack; if the NAS is down, the container sees an empty folder or errors, which shows in its logs in [Docker](view:docker).
- The mount appears in [Fleet](view:fleet) → **Disks** with the NAS's free space.
- For Linux-to-Linux, NFS is usually faster and keeps ownership: [NFS shares](kb:apps/nfs).

## Limits and gotchas

- SQLite databases (Plex, *arr apps) on SMB shares corrupt easily; keep app config on local disk and only media on the share.
- inotify doesn't work across SMB, so apps that watch folders for changes need a periodic scan instead.

## Related

- [NFS shares](kb:apps/nfs)
- [Samba file shares](kb:apps/samba)
- [Add a disk: partition, format and mount](kb:linux/disks-fstab)
