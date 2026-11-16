# File permissions, ownership and ACLs

Fix the "permission denied" errors that come up with containers, shares and shared folders: users and groups, the setgid bit, umask and ACLs.

## Part 1 · Read what you have

```sh
ls -la /srv/data
stat /srv/data/file
id
namei -l /srv/data/media/movies
```

- [ ] `namei -l` shows every folder on the path; one folder without `x` for your user blocks everything below it.

## Part 2 · A shared folder for a group

```sh
sudo groupadd -f media
sudo chgrp -R media /srv/media
sudo chmod -R g+rwX /srv/media
sudo find /srv/media -type d -exec chmod g+s {} +
```

- [ ] `g+s` on folders makes new files inherit the group. Add users and containers (`PGID`) to `media`.
- [ ] Set `UMASK=002` in containers that support it so new files are group-writable.

## Part 3 · ACLs when groups aren't enough

```sh
sudo setfacl -R -m u:jellyfin:rx /srv/media
sudo setfacl -R -d -m u:jellyfin:rx /srv/media
getfacl /srv/media
```

- [ ] `-d` sets the default for new files. ZFS needs `acltype=posixacl` on the dataset.

## Part 4 · Containers

- [ ] Rootless or user-mapped containers (unprivileged Proxmox LXC) shift uids: uid 1000 inside is 101000 on the host.
- [ ] `:ro` on volume mounts for media a container only reads.

#### Check

- [ ] `sudo -u <user> touch /srv/media/test && rm /srv/media/test` works for each user that should write.

## Limits and gotchas

- `chmod -R 777` makes the error go away and every account able to change your files. Fix the owner or group instead.
- NFS checks numeric ids; names don't matter across machines.

## Related

- [Users, groups and sudo](kb:linux/users-sudo)
- [NFS shares](kb:apps/nfs)
- [Samba file shares](kb:apps/samba)
