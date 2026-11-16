# Unbound recursive DNS

Resolve DNS yourself, starting from the root servers, instead of trusting a public resolver. Pairs with Pi-hole or AdGuard Home as their only upstream.

## What you end up with

- A validating, caching recursive resolver on `127.0.0.1:5335`
- Pi-hole or AdGuard forwarding to it
- No third-party resolver seeing your full query history

## Part 1 · Install on the DNS host

- [ ] `sudo apt install -y unbound`.
- [ ] `/etc/unbound/unbound.conf.d/pi-hole.conf`:

```
server:
    verbosity: 0
    interface: 127.0.0.1
    port: 5335
    do-ip4: yes
    do-udp: yes
    do-tcp: yes
    do-ip6: no
    prefer-ip6: no
    harden-glue: yes
    harden-dnssec-stripped: yes
    use-caps-for-id: no
    edns-buffer-size: 1232
    prefetch: yes
    num-threads: 1
    so-rcvbuf: 1m
    private-address: 192.168.0.0/16
    private-address: 169.254.0.0/16
    private-address: 172.16.0.0/12
    private-address: 10.0.0.0/8
    private-address: fd00::/8
    private-address: fe80::/10
```

- [ ] `sudo systemctl restart unbound`.
- [ ] Test: `dig example.com @127.0.0.1 -p 5335`. The first lookup is slower; repeats are instant.
- [ ] DNSSEC check: `dig fail01.dnssec.works @127.0.0.1 -p 5335` should return SERVFAIL.

## Part 2 · Point the filter at it

- [ ] Pi-hole in Docker can't reach the host's `127.0.0.1`. Either run Pi-hole with `network_mode: host`, or set Unbound's `interface:` to the Docker bridge gateway (`172.17.0.1`) plus `access-control: 172.16.0.0/12 allow`, and use `172.17.0.1#5335` as Pi-hole's upstream.
- [ ] Pi-hole: `FTLCONF_dns_upstreams: 127.0.0.1#5335` (host networking). AdGuard: upstream `127.0.0.1:5335`.

## Part 3 · In Nightgrid

- [ ] [System](view:system) → **services** shows `unbound` with start, stop and restart.
- [ ] A command widget for its cache hit count is easy if your login can run `unbound-control`; otherwise the journal is enough.

#### Check

- [ ] Pi-hole or AdGuard's query log shows answers coming from the new upstream.

## Limits and gotchas

- Debian's `unbound-resolvconf` service may rewrite `/etc/resolv.conf`; disable it if the host itself should use something else: `sudo systemctl disable --now unbound-resolvconf`.
- Some networks (hotel, some ISPs) block outbound port 53; a recursive resolver won't work there.

## Related

- [Pi-hole](kb:apps/pihole)
- [AdGuard Home](kb:apps/adguard-home)
