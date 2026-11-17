# apt-cacher-ng package cache

Download each Debian or Ubuntu package once and serve it to every other machine on the LAN. Updates across ten hosts or a stack of reinstalls at the bench go at LAN speed and save the internet line.

## What you end up with

- A caching proxy for apt on port 3142
- Every host pointed at it, with a fallback if the cache is down
- Faster `apt upgrade` across the fleet

## Part 1 · Install the cache

- [ ] On an always-on host with some disk space:

```sh
sudo apt install apt-cacher-ng
sudo systemctl enable --now apt-cacher-ng
```

- [ ] The cache lives in `/var/cache/apt-cacher-ng`; give it a few GB.
- [ ] Report page: `http://cache-host:3142/acng-report.html` shows hits and misses.

## Part 2 · Point clients at it

- [ ] On each client, `/etc/apt/apt.conf.d/01proxy`:

```
Acquire::http::Proxy "http://cache-host:3142";
Acquire::https::Proxy "DIRECT";
```

- [ ] HTTPS repositories (Docker's, for example) pass straight through with `DIRECT`; only plain HTTP mirrors are cached. The Debian and Ubuntu main mirrors use HTTP by default, which is fine: packages are signed.
- [ ] Laptops that leave home: use `squid-deb-proxy-client` or an auto-detect script instead of a fixed proxy line, or apt fails away from the LAN.

#### Check

- [ ] Run `sudo apt update && sudo apt install --reinstall -y htop` on two clients; the report page shows a hit for the second.

## In Nightgrid

- Push the `01proxy` file to every Debian/Ubuntu host with a snippet in [Automation](view:automation).
- [Fleet](view:fleet) → **Updates** applies updates across hosts; with the cache in place, the second and later hosts download from the LAN.

## Limits and gotchas

- If the cache host is down, clients with a fixed proxy line can't update. Keep it on a reliable box.
- Fedora and RHEL hosts need a different approach (a local mirror or a generic caching proxy).

## Related

- [Patch every host from one place](kb:howto/fleet-updates)
- [Manage packages with apt and dnf](kb:linux/packages)
- [Run commands on many hosts with snippets and playbooks](kb:howto/automation)
