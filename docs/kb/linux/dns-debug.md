# Debug DNS on a Linux server

"It's always DNS." Find out which resolver a server actually uses, what it answers, and why a name works on your laptop but not on the server (or the other way round).

![The resolver chain to test one step at a time.](fig:lx-dns-debug)

## What you end up with

- The resolver chain on a host, from `/etc/resolv.conf` to the upstream
- Direct queries that bypass caches
- The usual fixes for split DNS, Pi-hole and tailnet names

## Part 1 · Who answers?

```sh
cat /etc/resolv.conf
resolvectl status
```

- [ ] `nameserver 127.0.0.53` means systemd-resolved; `resolvectl status` shows the real upstream servers per interface (Tailscale adds its own on `tailscale0`).
- [ ] Docker containers get a copy of the host's resolvers, but `127.0.0.53` is replaced with the upstreams, since the stub isn't reachable inside a container.

## Part 2 · Ask directly

```sh
dig app.example.lan
dig @192.168.1.2 app.example.lan +short
dig @9.9.9.9 example.com +short
resolvectl query app.example.lan
getent hosts app.example.lan
```

- [ ] `dig` asks DNS only; `getent` goes through `/etc/hosts` and NSS like real programs do. Different answers point at `/etc/hosts` or mDNS.
- [ ] `status: NXDOMAIN` = the server says the name doesn't exist; `SERVFAIL` = it tried and failed (often DNSSEC or an unreachable upstream).
- [ ] Install `dig` with `sudo apt install dnsutils` (Debian) or `bind-utils` (Fedora).

## Part 3 · Common fixes

- [ ] Stale answers: `sudo resolvectl flush-caches`, and restart the app (many cache on their own).
- [ ] Internal names only resolve on some machines: those machines use a different resolver. Point DHCP at your Pi-hole or AdGuard, or add a routing domain: `resolvectl domain eth0 '~example.lan'`.
- [ ] A static resolver on a server (netplan example):

```yaml
      nameservers:
        addresses: [192.168.1.2, 9.9.9.9]
        search: [example.lan]
```

- [ ] A public name that should point at a LAN address from inside: add a local DNS record in Pi-hole or AdGuard instead of `/etc/hosts` on every machine.

#### Check

- [ ] `dig` against your resolver and `getent hosts` give the same address on the server and on your laptop.

## In Nightgrid

- [Network](view:network) → **Tools** → **DNS** runs the lookup from any host (**Run from**), with a **Server** field to ask one resolver; compare answers between hosts to find the odd one.
- Pi-hole and AdGuard local records can be added from [Services](view:services) → the integration's **Manage…** dashboard.
- Hosts reached through the built-in tailnet use tailnet DNS for tailnet names; see [Reach your servers away from home](kb:howto/remote-access).

## Limits and gotchas

- Browsers may use DNS over HTTPS and ignore your resolver; that explains "works in curl, not in the browser" (or the reverse).
- `.local` is for mDNS; using it for a normal DNS zone causes slow or failed lookups. Prefer `home.arpa` or a subdomain of a domain you own.

## Related

- [Pi-hole](kb:apps/pihole)
- [Unbound recursive DNS](kb:apps/unbound)
- [Diagnose network problems with Network → Tools](kb:howto/network-tools)
