# Kernel settings with sysctl

The handful of kernel settings worth changing on a home server, why, and how to make them stick. Most defaults are fine; these fix real problems: inotify limits for sync tools, forwarding for VPN routers, memory map limits for search engines, and network buffers for fast links.

## What you end up with

- Your changes in one file under `/etc/sysctl.d/`, applied at boot
- Only settings with a reason, each with a comment
- A way to see and undo what you changed

## Part 1 · How it works

- [ ] Read a value: `sysctl fs.inotify.max_user_watches`. Change it until reboot: `sudo sysctl -w name=value`.
- [ ] Make it permanent in `/etc/sysctl.d/90-homelab.conf`, then `sudo sysctl --system` to load all files.
- [ ] Files load in name order; a higher number wins. Distros and packages put their own in `/usr/lib/sysctl.d/`.

## Part 2 · Settings that solve real problems

```ini
# Syncthing, Nextcloud, IDEs, Plex library scans: "too many open files / inotify watches"
fs.inotify.max_user_watches = 524288
fs.inotify.max_user_instances = 1024

# VPN servers and subnet routers (Tailscale, WireGuard, NetBird) that forward traffic
net.ipv4.ip_forward = 1
net.ipv6.conf.all.forwarding = 1

# OpenSearch, Elasticsearch, Graylog, Wazuh, SonarQube refuse to start without it
vm.max_map_count = 262144

# Less eager swapping on servers with enough RAM
vm.swappiness = 10

# Redis and some databases warn without it
vm.overcommit_memory = 1
```

- [ ] Only add the lines you need. `overcommit_memory = 1` suits a Redis host; leave it at the default elsewhere.

## Part 3 · Fast networks (2.5–10 GbE)

```ini
net.core.rmem_max = 67108864
net.core.wmem_max = 67108864
net.ipv4.tcp_rmem = 4096 87380 33554432
net.ipv4.tcp_wmem = 4096 65536 33554432
net.core.default_qdisc = fq
net.ipv4.tcp_congestion_control = bbr
```

- [ ] Measure before and after with iperf3; on a 1 GbE LAN these change nothing. BBR needs the `tcp_bbr` module (in all current distro kernels).

#### Check

- [ ] Reboot, then `sysctl vm.max_map_count net.ipv4.ip_forward` shows your values.

## In Nightgrid

- [System](view:system) → **config** → **Compare with** another host shows sysctl files that differ between servers that should match.
- [Fleet](view:fleet) → **Speed** is the before-and-after test for Part 3.
- Docker sets `net.ipv4.ip_forward = 1` itself when it starts; that's expected on Docker hosts.

## Limits and gotchas

- Settings for a module that isn't loaded yet (bridge netfilter, for example) fail at boot. Load the module in `/etc/modules-load.d/` first.
- Containers share the host kernel: an app's "set vm.max_map_count" requirement is a host change, not a compose option.
- Copying long "tuning" lists from the internet often makes things worse. Change one thing, measure, keep it or revert.

## Related

- [Swap and zram](kb:linux/swap-zram)
- [Find slow links with iperf3](kb:howto/speed-test)
- [Why is this server slow?](kb:linux/slow-server)
