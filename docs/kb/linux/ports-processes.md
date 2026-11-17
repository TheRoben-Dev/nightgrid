# Find what's using a port, a file or the CPU

"Address already in use", "device is busy", a fan at full speed: three questions with the same answer, find the process. These commands work on any distro.

![From a port to its owner.](fig:lx-port-owner)

## What you end up with

- The process behind any listening port
- The process holding a file or mount open
- The process eating CPU, memory or disk I/O

## Part 1 · Ports

```sh
sudo ss -ltnup
sudo ss -ltnp 'sport = :8080'
```

- [ ] `-l` listening, `-t` TCP, `-u` UDP, `-n` numbers, `-p` process. Root is needed to see other users' processes.
- [ ] A port held by `docker-proxy` belongs to a container: `docker ps --format '{{.Names}} {{.Ports}}' | grep 8080`.
- [ ] Established connections to a service: `ss -tn state established '( sport = :22 )'`.

## Part 2 · Files and mounts

- [ ] "target is busy" when unmounting:

```sh
sudo fuser -vm /mnt/usb
sudo lsof +f -- /mnt/usb
```

- [ ] Deleted files still taking space (the disk is full but `du` doesn't add up):

```sh
sudo lsof +L1 | sort -k7 -n | tail
```

- [ ] Restarting the process that holds them frees the space.

## Part 3 · CPU, memory and I/O

```sh
top -o %CPU
ps aux --sort=-%mem | head
sudo iotop -oPa
```

- [ ] `iotop` (`sudo apt install iotop`) shows which process is hammering the disk. `-o` only active ones, `-a` totals since start.
- [ ] A process in state `D` in `top` is waiting on disk or NFS; lots of them means storage trouble, not CPU.
- [ ] Which container a process belongs to: `cat /proc/<pid>/cgroup`.

## Part 4 · What a process is doing

```sh
sudo strace -f -p <pid> -e trace=network,file 2>&1 | head -50
sudo ls -l /proc/<pid>/cwd /proc/<pid>/exe
```

#### Check

- [ ] For the port you were chasing, you can name the process, its user and (if any) its container.

## In Nightgrid

- [System](view:system) → **security** flags databases or the Docker API listening on every interface, and the [Network](view:network) map's **Audit ports** marks which listening ports the internet can reach.
- Run these commands in a [Terminal](view:terminal) tab; DRIFT can explain a command's flags before you run it.
- [Docker](view:docker) shows each container's published ports, so a clash is visible before you start a stack.

## Limits and gotchas

- Inside a container, `ss` only sees the container's own network namespace. Run it on the host.
- Ports below 1024 need root or `CAP_NET_BIND_SERVICE`; "permission denied" on bind is that, not a clash.
- `lsof` on a big NFS mount can hang; prefer `fuser -m`.

## Related

- [Why is this server slow?](kb:linux/slow-server)
- [Map your network and find exposed ports](kb:howto/network-map)
- [Docker networks: bridge, shared proxy network and macvlan](kb:linux/docker-networks)
