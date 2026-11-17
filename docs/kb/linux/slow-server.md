# Why is this server slow?

A short, ordered checklist to find what's slowing a Linux server: CPU, memory, disk or network, and which process or container is responsible.

![The four resources to check, in order.](fig:lx-slow-server)

## Part 1 · Look at the trend first

- [ ] Click the host on the [Overview](view:overview) for history charts. A sudden step (a deploy, an update) points elsewhere than a slow climb (a leak, a filling disk). Drag across the spike to zoom in.
- [ ] [System](view:system) → **timeline** for that time: deploys, restarts, OOM kills, upgrades.

## Part 2 · CPU and load

```sh
uptime
top -o %CPU
```

- [ ] Load above the core count with low CPU use means processes are waiting on disk or network (`wa` in `top` high = disk).
- [ ] [Docker](view:docker) shows CPU per container.

## Part 3 · Memory

```sh
free -h
ps aux --sort=-rss | head
```

- [ ] Little "available" memory and active swapping (`vmstat 1`, columns `si`/`so`) means memory pressure.

## Part 4 · Disk

```sh
sudo apt install -y sysstat iotop
iostat -xz 2
sudo iotop -o
```

- [ ] `%util` near 100 or `await` in the tens of ms on an SSD means the disk is the bottleneck. A failing disk often shows huge `await` and kernel I/O errors: `journalctl -k -g "I/O error"`.
- [ ] Full disks slow down too. [Fleet](view:fleet) → **Disks**.

## Part 5 · Network

- [ ] [Fleet](view:fleet) → **Speed** between this host and another; a link stuck at 100 Mbit/s is flagged.
- [ ] `ss -s` for connection counts; thousands in TIME-WAIT or SYN-RECV point at a flood or a misbehaving client.

## Part 6 · Ask DRIFT

- [ ] Select `top` or `iostat` output in the terminal and press `Mod+J`. DRIFT explains it with the host's live state attached.

#### Check

- [ ] You can name the resource and the process. Fix that, then watch the chart return to normal.

## Limits and gotchas

- High load on a NAS during a scrub or resilver is normal; [Fleet](view:fleet) → **Disks** shows scrub progress.
- Thermal throttling looks like slowness with "normal" usage. The host monitor shows temperatures.

## Related

- [Swap and zram](kb:linux/swap-zram)
- [Keep an eye on disks and pools](kb:howto/disks)
- [Find slow links with iperf3](kb:howto/speed-test)
