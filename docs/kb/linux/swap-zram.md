# Swap and zram

Give a server a safety margin against memory spikes with a swap file or compressed RAM swap (zram), and tune how eagerly it's used.

![Memory pressure, from page cache to zram to disk swap.](fig:lx-zram)

## What you end up with

- zram swap (fast, no disk wear) or a swap file on disk, or both
- Fewer surprise OOM kills, and a warning sign in Nightgrid when memory keeps climbing

## Part 1 · zram (recommended for most)

- [ ] Debian/Ubuntu: `sudo apt install -y systemd-zram-generator` (Fedora has it by default), then `/etc/systemd/zram-generator.conf`:

```ini
[zram0]
zram-size = ram / 2
compression-algorithm = zstd
```

- [ ] `sudo systemctl daemon-reload && sudo systemctl start systemd-zram-setup@zram0`, then `swapon --show`.

## Part 2 · A swap file

```sh
sudo fallocate -l 4G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
echo '/swapfile none swap sw,pri=10 0 0' | sudo tee -a /etc/fstab
```

- [ ] On btrfs, use `sudo btrfs filesystem mkswapfile --size 4G /swapfile` instead. ZFS shouldn't hold swap.
- [ ] With both, zram has higher priority automatically, so the disk is only the overflow.

## Part 3 · Tuning

- [ ] `vm.swappiness=10` for a disk swap file on a server that should stay in RAM; leave it at 100 or more with zram only:

```sh
echo 'vm.swappiness=10' | sudo tee /etc/sysctl.d/99-swap.conf && sudo sysctl --system
```

## Part 4 · In Nightgrid

- [ ] The host monitor shows memory; the forecast adds a line when memory keeps climbing towards 95 %, which often means a leak in one container.
- [ ] [System](view:system) → **timeline** lists OOM kills with the process that died.

#### Check

- [ ] `free -h` shows swap, and `cat /proc/swaps` lists it with priorities.

## Limits and gotchas

- Swap hides a real shortage. If swap is always used, add RAM or limit containers (`mem_limit` in compose).
- Kubernetes (k3s) expects swap off unless configured otherwise.

## Related

- [Why is this server slow?](kb:linux/slow-server)
- [Track config drift and host history](kb:howto/config-drift)
