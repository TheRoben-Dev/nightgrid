# LXC containers on Proxmox

Use lightweight Proxmox containers (LXC) for Linux services: privileged vs unprivileged, bind mounts, Docker inside, and device access.

## What you end up with

- Unprivileged containers created from Nightgrid with SSH key login
- Host folders bind-mounted with the right ownership
- Docker running inside a container where it makes sense

## Part 1 · Create

- [ ] [Proxmox](view:proxmox) → **GUESTS** → **+ Container**: a Debian template (get one with **STORAGE** → **Get CT template…**), unprivileged, your SSH public key (required: no password is set), disk, cores and memory.
- [ ] Start it and add it as a host in Nightgrid with its LAN address, user `root`.

## Part 2 · Bind mounts

- [ ] On the node (from a [Terminal](view:terminal)):

```sh
pct set <ctid> -mp0 /tank/media,mp=/media
```

- [ ] Unprivileged containers shift uids by 100000: files owned by uid 1000 inside are 101000 on the host. Either `chown -R 101000:101000` the host folder, or map ids in `/etc/pve/lxc/<ctid>.conf` with `lxc.idmap` lines (see the Proxmox wiki's "Unprivileged LXC containers").

## Part 3 · Docker inside

```sh
pct set <ctid> --features nesting=1,keyctl=1
```

- [ ] Restart the container, then install Docker as in [Prepare a Linux Docker host](kb:apps/docker-host).
- [ ] On ZFS storage this needs Proxmox 8.1+ (ZFS 2.2) for overlay2.

## Part 4 · Devices (GPU for transcoding)

- [ ] Proxmox 8.2+ passes devices without config file edits:

```sh
pct set <ctid> -dev0 /dev/dri/renderD128,gid=<render-gid-in-container>
```

#### Check

- [ ] The container shows in Proxmox **GUESTS** and on Nightgrid's [Overview](view:overview), and `ls /media` inside shows the host files.

## Limits and gotchas

- Containers share the host kernel: no custom kernel modules, and a kernel panic takes them all down.
- Privileged containers are root on the host if broken out of; prefer unprivileged.
- Live migration isn't possible for containers; they restart on the other node.

## Related

- [Day-to-day Proxmox from Nightgrid](kb:howto/proxmox)
- [Proxmox media server](kb:guides/proxmox-media-server)
- [File permissions, ownership and ACLs](kb:linux/permissions)
