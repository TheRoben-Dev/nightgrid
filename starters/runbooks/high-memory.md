---
id: high-memory
title: "Runbook: high memory use"
summary: Find what's using the memory, and stop the kernel killing things at random.
tags: [runbook, starter, memory]
library: [linux/memory-limits, linux/slow-server, linux/swap-zram]
---
The host is short on memory: it's slow, swapping hard, or the kernel's out-of-memory killer is ending processes. Nightgrid shows memory per host on Overview and in System; there's no alert kind for it yet, so this runbook isn't offered on alerts. Open it from Knowledge, or tie it to a host in Edit.

- [ ] Remember that Linux uses free memory as cache: look at "available", not "free":

```sh
free -h
```

- [ ] See the biggest processes and containers:

```sh
ps aux --sort=-%mem | head -12
docker stats --no-stream --format 'table {{.Name}}\t{{.MemUsage}}\t{{.MemPerc}}'
```

- [ ] Check whether the kernel already killed something:

```sh
journalctl -k -b --no-pager | grep -i -E 'out of memory|oom-kill' | tail -5
```

- [ ] One container growing without end is usually a leak: restart it now, then set a limit (`mem_limit: 1g` in compose) so it can't take the whole host.
- [ ] No swap at all? Add zram or a small swap file so a spike slows the host instead of killing processes.
- [ ] Always near the limit: move a stack to another host, or add memory.

More in [Memory limits](kb:linux/memory-limits) and [Swap and zram](kb:linux/swap-zram).
