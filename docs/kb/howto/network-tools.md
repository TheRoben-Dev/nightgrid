# Diagnose network problems with Network → Tools

Ping, trace, look up DNS, test ports, check a certificate and see what a device runs, from whichever machine has the right view of the network. "Can the NAS reach the printer?" is answered from the NAS.

## What you end up with

- Everyday network checks without opening a terminal
- Results run from a host or this computer, whichever you pick
- Risky services (Telnet, open Docker APIs, unprotected databases) flagged on the map

## Part 1 · Pick where to run from

- [ ] [Network](view:network) → **Tools** (`Mod+Shift+N`). Under **Run from**, pick a host or this computer (macOS and Linux).
- [ ] Choose the machine that sits where the problem is: a host on the LAN for LAN questions, this computer for "is it my laptop?".

## Part 2 · The tools

- [ ] **Ping**: 1, 4 or 10 packets with loss and round trip.
- [ ] **Path**: every router on the way (tracepath, traceroute or mtr, whichever is installed).
- [ ] **DNS**: A, AAAA, CNAME, MX, TXT, NS, SOA, SRV, PTR and CAA records; **Server** asks a specific DNS server, so you can compare Pi-hole with a public resolver.
- [ ] **Ports**: a list or ranges like `22,80,8000-8010` (up to 64).
- [ ] **HTTP / TLS**: one request; status, server, redirect target, and for https the certificate's issuer, expiry and whether that machine trusts it.
- [ ] **Wake**: send a Wake-on-LAN packet from that machine.
- [ ] **Subnet** and **MAC lookup** work offline.

## Part 3 · What does this device run?

- [ ] **Services** connects to common ports (or the ones you list) and reads what answers: SSH version, web server, mail or FTP greeting.
- [ ] Risky ones are flagged in red or amber with the reason, and the device gets a ⚠ on the map.
- [ ] **Add as service** on a web port opens the service editor with the address filled in.

## Part 4 · From the map

- [ ] Select a device, the router or a host on the Map and use its **Tools** buttons; the address is filled in and the tool runs from the machine that scanned it.

#### Check

- [ ] DNS for one of your service names returns the same address from the Pi-hole and from the host that runs the service.

## Common answers

- [ ] Ping works, Ports fails: a firewall or the service is down.
- [ ] DNS differs between servers: a stale local record or split DNS.
- [ ] HTTP / TLS says "not trusted" from a host but fine in your browser: the host lacks your internal CA (see [Private certificate authority with step-ca](kb:apps/step-ca)).

## Limits and gotchas

- A missing command on the chosen machine shows as a note. Installing `mtr-tiny`, `dnsutils` (or `bind-utils`), `netcat-openbsd` and `python3` covers everything.
- Service reading needs python3 on the scanning machine; with only nc you get open ports without names.

## Related

- [Map your network and find exposed ports](kb:howto/network-map)
- [When a host won't connect](kb:howto/troubleshoot-connection)
- [Find slow links with iperf3](kb:howto/speed-test)
