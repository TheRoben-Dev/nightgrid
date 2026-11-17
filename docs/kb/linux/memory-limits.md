# Memory limits and OOM kills

Stop one runaway service or container from eating the whole server's RAM, and find out what the kernel killed when memory ran out. Uses the cgroup limits that systemd and Docker already build on, no extra tools.

## What you end up with

- Hard and soft memory limits on the services and containers that tend to balloon
- A quick way to tell an OOM kill from an ordinary crash
- Important services protected so the kernel kills something else first

## Part 1 · Find the culprit

- [ ] [System](view:system) → **timeline** on the host lists OOM kills with the process name and time.
- [ ] Or in a [Terminal](view:terminal) tab: `journalctl -k | grep -i -E "out of memory|oom-kill"` shows each kill and which cgroup (service or container) it was in.
- [ ] Containers: `docker inspect --format '{{.State.OOMKilled}}' <name>` says `true` when the container died from its own limit or the kernel.
- [ ] `systemd-cgtop -m` sorts services, user sessions and containers by live memory use; `docker stats --no-stream` does the same for containers alone.

## Part 2 · Limit a systemd service

- [ ] `sudo systemctl edit <service>` and add:

```ini
[Service]
MemoryHigh=1500M
MemoryMax=2G
```

- [ ] `MemoryHigh` slows the service and makes it reclaim memory above the line; `MemoryMax` is the hard ceiling where the kernel kills inside that service only. Restart the service to apply.
- [ ] Check it took effect: `systemctl show <service> -p MemoryMax -p MemoryCurrent`.

## Part 3 · Limit a container

- [ ] In the stack's `compose.yaml`, per service:

```yaml
    mem_limit: 2g
    memswap_limit: 2g
```

- [ ] Equal `mem_limit` and `memswap_limit` means no extra swap for that container. Redeploy, then `docker stats` shows the limit in the MEM USAGE / LIMIT column.
- [ ] Java, Node and databases size their caches from the memory they see. Also set their own limit (`-Xmx`, `--max-old-space-size`, database cache settings) a bit under the container limit, or they get killed instead of trimming caches.

## Part 4 · Protect what matters

- [ ] For sshd or your reverse proxy, `sudo systemctl edit <service>` with `OOMScoreAdjust=-500` makes the kernel prefer other victims.
- [ ] For containers, `oom_score_adj: -500` in compose does the same. Don't use `-1000` (never kill) on anything that can leak.
- [ ] On desktops-turned-servers, `systemd-oomd` (Fedora, Ubuntu desktop) may kill whole slices early under memory pressure; `oomctl` shows what it watches.

#### Check

- [ ] Run a memory hog inside a limited container (`docker run --rm -m 256m alpine sh -c 'tail /dev/zero'`): it dies with exit code 137 while the host stays responsive.

## In Nightgrid

- In [Fleet](view:fleet) → **Logs**, a saved search for `Out of memory` in the journal across all hosts, with a **LOG MATCH** alert, tells you when any host starts killing.
- The host's memory chart on the [Overview](view:overview) shows whether usage climbs slowly (a leak) or jumps (one big job).

## Limits and gotchas

- A limit only moves the kill from "random big process" to "this one"; fix the cause (a leak, a too-large cache) as well.
- cgroup v1 hosts (very old distributions) use different file names; `MemoryMax` needs cgroup v2, standard since Debian 11, Ubuntu 22.04 and Fedora 31.
- Swap hides memory pressure until everything gets slow; see [Swap and zram](kb:linux/swap-zram).

## Related

- [Why is this server slow?](kb:linux/slow-server)
- [Write your own systemd service](kb:linux/systemd-units)
- [Swap and zram](kb:linux/swap-zram)
