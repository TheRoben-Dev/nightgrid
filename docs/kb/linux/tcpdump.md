# Capture network traffic with tcpdump

See exactly what reaches a server and what it answers: whether DHCP requests arrive, whether a client even tries port 443, why a VPN tunnel is silent. Capture on the server with tcpdump, read it there or open the file in Wireshark on your computer.

## What you end up with

- Quick live views of one host, port or protocol
- Saved `.pcap` files for a closer look in Wireshark
- A few filters that answer the common "is anything arriving?" questions

## Part 1 · Live capture

- [ ] Install: `sudo apt install tcpdump` or `sudo dnf install tcpdump`.
- [ ] List interfaces: `tcpdump -D`, or `ip -br link`.
- [ ] Watch one port without name lookups: `sudo tcpdump -ni eth0 port 443`. Stop with **Ctrl+C**; the summary says how many packets the kernel dropped.
- [ ] One client: `sudo tcpdump -ni eth0 host 192.0.2.50`.
- [ ] Exclude your own SSH session so it doesn't flood the output: add `and not port 22`.

## Part 2 · Filters that answer questions

- [ ] **Is DHCP reaching this box?** `sudo tcpdump -ni eth0 port 67 or port 68`
- [ ] **Are DNS queries arriving at Pi-hole?** `sudo tcpdump -ni eth0 udp port 53`
- [ ] **Does a WireGuard peer send anything?** `sudo tcpdump -ni eth0 udp port 51820`
- [ ] **Connection attempts only (SYNs):** `sudo tcpdump -ni eth0 'tcp[tcpflags] & tcp-syn != 0 and tcp[tcpflags] & tcp-ack == 0'`
- [ ] **Inside Docker:** capture on the bridge (`docker0` or `br-<id>` from `ip -br link`), or on `any` to see everything.
- [ ] Payload as text for plain HTTP: add `-A`; a hex dump: `-X`.

## Part 3 · Save and open in Wireshark

- [ ] `sudo tcpdump -ni eth0 -w /tmp/cap.pcap port 443`, reproduce the problem, **Ctrl+C**.
- [ ] Cap the size on long captures: `-C 100 -W 5` rotates five 100 MB files.
- [ ] Make it readable for download: `sudo chown $USER /tmp/cap.pcap`, then fetch it with [Files](view:files) and open it in Wireshark.

#### Check

- [ ] While capturing `port 53`, run `dig example.com @<server>` from another machine: you see the query and the answer. If you see only the query, the server isn't answering; if nothing, the packet never arrives (firewall, routing, wrong IP).

## In Nightgrid

- Run captures from a [Terminal](view:terminal) tab; a capture that should run while you work elsewhere goes in tmux (see [tmux: keep sessions running](kb:linux/tmux)).
- [Network](view:network) → **Tools** → **Ports** checks from the host whether a port is open; tcpdump is the next step when it says open but the app still fails.

## Limits and gotchas

- Captures can contain passwords and cookies from plain-text protocols. Delete `.pcap` files when done; don't paste them into notes or chats.
- On a busy link without a filter, tcpdump drops packets and the disk fills fast. Always filter, and use `-s 128` (headers only) if you don't need payloads.
- TLS traffic is encrypted; you see who talks to whom and when, not the content.

## Related

- [Find what's using a port, a file or the CPU](kb:linux/ports-processes)
- [Debug DNS on a Linux server](kb:linux/dns-debug)
- [Write nftables rules by hand](kb:linux/nftables)
