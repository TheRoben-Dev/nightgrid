# Errors: networking and DNS

Messages from ip, ping, curl, wget, NetworkManager, netplan, systemd-networkd, resolved and DNS tools. See also [DNS debugging](kb:linux/dns-debug), [Static IP](kb:linux/static-ip) and [Ports and processes](kb:linux/ports-processes).

### Network is unreachable
Means: ENETUNREACH: there's no route for that address family at all, typically no default gateway, or IPv6 without IPv6 connectivity.
Fix: `ip route` (and `ip -6 route`). Add the gateway, fix the interface config, or force IPv4 (`curl -4`, `ping -4`).

### connect: Network is unreachable
Means: ping or another tool found no route to the destination.
Fix: `ip route get <ip>`; check the interface is up with an address (`ip -br addr`).

### Destination Host Unreachable
Means: ping got an ICMP reply (from your host or a router) saying the target didn't answer ARP on its subnet: it's off, on another VLAN, or the address is wrong.
Fix: Check the device is on and its IP; `ip neigh` shows FAILED/INCOMPLETE entries.

### Destination Net Unreachable
Means: A router on the path has no route to the destination network.
Fix: Check routes on your gateway or VPN, especially for subnets behind a tailnet subnet router.

### Request timeout for icmp_seq <n>
Means: No ping reply: the host is down, filtering ICMP, or the path is broken.
Fix: Many hosts block ping; test the actual service port (`nc -vz <host> <port>`).

### ping: <host>: Temporary failure in name resolution
Means: DNS lookup failed: no DNS server reachable, or resolv.conf/resolved misconfigured.
Fix: `resolvectl status`, `cat /etc/resolv.conf`, `ping -c1 <dns-ip>`. Try `dig @1.1.1.1 <host>` to separate DNS from network.

### ping: <host>: Name or service not known
Means: The name doesn't exist in DNS or /etc/hosts (typo, missing local record, or wrong search domain).
Fix: `getent hosts <host>`, `dig <host>`; add a local DNS record (Pi-hole/AdGuard) or an /etc/hosts entry.

### ping: socktype: SOCK_RAW
Means: ping lacks permission to open raw sockets (missing capability or ping_group_range).
Fix: Use sudo, or `sudo setcap cap_net_raw+ep $(command -v ping)`. In containers add the NET_RAW capability.

### curl: (6) Could not resolve host: <host>
Means: curl couldn't resolve the name.
Fix: As for any DNS failure: check the resolver. In containers, check the container's /etc/resolv.conf and Docker's DNS.

### curl: (7) Failed to connect to <host> port <port> after <n> ms: Connection refused
Means: Nothing listens on that address and port, or a firewall rejects.
Fix: On the host: `ss -ltnp | grep :<port>`. A service bound to 127.0.0.1 can't be reached from elsewhere.

### curl: (7) Failed to connect to <host> port <port> after <n> ms: Couldn't connect to server
Means: Generic connect failure (refused, unreachable or no route).
Fix: `curl -v` shows which address it tried; test with `nc -vz <host> <port>`.

### curl: (28) Connection timed out after <n> milliseconds
Means: No answer: a firewall drops packets, the host is down, or IPv6 is broken and curl tried it first.
Fix: Try `curl -4`. Check firewalls on the host and cloud security groups.

### curl: (28) Operation timed out after <n> milliseconds with <n> bytes received
Means: The connection worked but the response didn't finish in time (a slow app or a stalled transfer).
Fix: Check the app's load and logs; raise `--max-time` if it's a big download.

### curl: (35) OpenSSL SSL_connect: SSL_ERROR_SYSCALL in connection to <host>:<port>
Means: The TLS handshake was cut off: wrong port (talking TLS to plain HTTP), a firewall or proxy interfering, or the server crashed.
Fix: Check the port really serves HTTPS; `openssl s_client -connect <host>:<port> -servername <host>`.

### curl: (52) Empty reply from server
Means: The server accepted the connection then closed it without sending anything: the app crashed, or plain HTTP to an HTTPS port.
Fix: Check the server's logs; try `https://` vs `http://`.

### curl: (56) Recv failure: Connection reset by peer
Means: The server or something in between reset the connection mid-transfer.
Fix: Check the app's log, proxies and MTU (VPNs with too-large MTU often cause resets on big responses).

### curl: (60) SSL certificate problem: unable to get local issuer certificate
Means: curl can't build a chain to a trusted root: a self-signed or private CA, or the server doesn't send its intermediate certificate.
Fix: Add your CA to the system store (`update-ca-certificates`), or fix the server to send the full chain. `-k` only for testing.

### curl: (60) SSL certificate problem: certificate has expired
Means: The server's certificate (or one in its chain) is past its expiry date, or your clock is wrong.
Fix: Check `date`. Renew the certificate (Caddy/Traefik/certbot logs). Nightgrid's expiry watch flags these early.

### curl: (60) SSL: no alternative certificate subject name matches target host name '<host>'
Means: The certificate isn't for the name you used.
Fix: Use the name on the certificate, or issue one that includes this name (SAN).

### curl: (22) The requested URL returned error: <n>
Means: With `-f`, curl treats HTTP errors (4xx/5xx) as failures.
Fix: Drop `-f` and add `-i` to see the response body and headers.

### curl: (3) URL using bad/illegal format or missing URL
Means: The URL is malformed (spaces, an unexpanded variable, missing scheme).
Fix: Quote the URL; `echo` the variable first.

### wget: unable to resolve host address '<host>'
Means: DNS failed for that name.
Fix: Check the resolver and the name.

### ERROR: cannot verify <host>'s certificate
Means: wget couldn't verify the TLS certificate.
Fix: Install ca-certificates, fix the clock, or add your private CA.

### RTNETLINK answers: File exists
Means: `ip addr add` or `ip route add` for something that already exists.
Fix: Use `ip route replace` or check with `ip route show`.

### RTNETLINK answers: Network is unreachable
Means: `ip route add ... via <gw>`: the gateway isn't on any directly connected subnet.
Fix: Add an address on the gateway's subnet first, or add an `onlink` route.

### RTNETLINK answers: Operation not permitted
Means: Changing network settings needs root (or CAP_NET_ADMIN in containers).
Fix: Use sudo; in a container, add NET_ADMIN.

### Cannot find device "<iface>"
Means: No interface by that name (names changed to enp*/eno* or the device isn't there).
Fix: `ip -br link` lists real names.

### Error: Nexthop has invalid gateway.
Means: The route's gateway isn't reachable on a connected network.
Fix: Check the interface address and netmask cover the gateway.

### Error: Connection activation failed: No suitable device found for this connection
Means: NetworkManager's profile is bound to an interface name or MAC that doesn't match any device.
Fix: `nmcli con show <name>`: fix `connection.interface-name` or the MAC, or create a new profile.

### Error: Connection activation failed: IP configuration could not be reserved (no available address, timeout, etc.)
Means: NetworkManager got no DHCP lease (no DHCP server answering, wrong VLAN, cable).
Fix: Check the link (`ethtool <iface>`), the DHCP server, and the switch port's VLAN.

### Error: Could not create NMClient object: Could not connect: No such file or directory.
Means: NetworkManager isn't running (the system uses networkd or ifupdown instead).
Fix: Use the right tool for the system: netplan/networkd on Ubuntu Server, /etc/network/interfaces on Debian/Proxmox.

### Invalid YAML at <file> line <n> column <n>: <error>
Means: netplan's YAML is broken, usually tabs or wrong indentation.
Fix: Spaces only, consistent indentation. `sudo netplan generate` checks it; `sudo netplan try` applies with automatic rollback.

### Error in network definition: <details>
Means: netplan understood the YAML but a setting is invalid.
Fix: The message names the key; `man netplan` for valid values.

### ** (generate:<pid>): WARNING **: Permissions for <file> are too open. Netplan configuration should NOT be accessible by others.
Means: A netplan file is world-readable (it may contain Wi-Fi passwords).
Fix: `sudo chmod 600 /etc/netplan/*.yaml`.

### systemd-networkd-wait-online: Timeout occurred while waiting for network connectivity.
Means: Some interface networkd manages never got configured, so boot waited.
Fix: Mark unused ones `optional: true` in netplan, or configure them. See the systemd entry "Failed to start Wait for Network to be Configured".

### ifup: failed to bring up <iface>
Means: ifupdown (Debian/Proxmox) couldn't configure that interface.
Fix: Check /etc/network/interfaces for typos; `ifup -v <iface>` shows the failing step. On Proxmox, `ifreload -a` applies changes.

### error: <iface>: cmd '<cmd>' failed: returned 1 (Cannot find device "<iface>")
Means: ifupdown2 (Proxmox) references an interface that doesn't exist, often after a NIC rename.
Fix: Update /etc/network/interfaces with the new name (`ip link`), then `ifreload -a` from the console.

### dhclient: No DHCPOFFERS received.
Means: No DHCP server answered on that interface.
Fix: Check the cable, link speed, VLAN and that the DHCP server is up.

### IPv4 address conflict detected
Means: Another device already uses this IP (NetworkManager or ARP probing detected it).
Fix: Find it with `arping -D <ip>` or the router's lease table. Use DHCP reservations instead of static IPs inside the DHCP range.

### Temporary failure in name resolution
Means: The resolver couldn't reach any DNS server, or got no answer in time.
Fix: `resolvectl status` shows which servers are used per link. Test them: `dig @<server> example.com`.

### ;; connection timed out; no servers could be reached
Means: dig/nslookup got no answer from the DNS server.
Fix: Is the server up? Is UDP/TCP 53 blocked? Try another server to compare.

### ;; communications error to <ip>#53: connection refused
Means: Nothing listens on port 53 at that address (DNS service down, or listening on another address).
Fix: On the DNS host: `ss -lunp | grep :53`. Pi-hole or AdGuard may be bound to another interface.

### status: NXDOMAIN
Means: The name doesn't exist according to that DNS server.
Fix: Check spelling and the zone. For local names, add the record on your local DNS; public resolvers won't know `.lan` names.

### status: SERVFAIL
Means: The DNS server failed to answer: upstream unreachable, DNSSEC validation failure, or a broken zone.
Fix: Try `dig +cd <name>` (skip DNSSEC checking); if that works, it's a DNSSEC problem. Check the server's logs.

### status: REFUSED
Means: The DNS server refuses to answer you: not allowed to recurse for your address, or a zone it doesn't serve.
Fix: Allow your subnet on the server (Pi-hole "Permit all origins" when needed, BIND allow-recursion), or query the right server.

### ** server can't find <name>: NXDOMAIN
Means: nslookup's wording of "the name doesn't exist".
Fix: As for NXDOMAIN.

### resolvectl: Failed to resolve hostname: Name '<name>' not found
Means: systemd-resolved got no answer for that name.
Fix: Check per-link DNS servers and search domains; `resolvectl query <name>` shows which link it used.

### Failed to resolve '<name>': Unknown error -<n>
Means: A program's DNS lookup failed without a specific reason (often resolved returning SERVFAIL).
Fix: Test with `resolvectl query <name>` and `dig`.

### systemd-resolved[<pid>]: Server returned error NXDOMAIN, mitigating potential DNS violation DVE-2018-0001, retrying transaction with reduced feature level UDP.
Means: resolved got NXDOMAIN and retried in a simpler mode, a workaround for broken DNS servers; often harmless.
Fix: If lookups fail, the upstream DNS server is misbehaving: use another one.

### Name or service not known
Means: getaddrinfo found no address for the name (EAI_NONAME).
Fix: Check the name, /etc/hosts and DNS.

### Address family for hostname not supported
Means: The name has only IPv6 (or only IPv4) addresses and the program asked for the other family.
Fix: Use the matching family, or add the missing record.

### Cannot assign requested address
Means: EADDRNOTAVAIL: binding to an address this host doesn't have, or running out of local ephemeral ports.
Fix: Bind to an address in `ip addr`, or 0.0.0.0. For outbound exhaustion: many TIME_WAIT sockets (`ss -s`); reuse connections.

### Address already in use
Means: EADDRINUSE: another process listens on that port.
Fix: `sudo ss -ltnp 'sport = :<port>'` shows which. Stop it or choose another port.

### Connection reset by peer
Means: ECONNRESET: the other side (or a middlebox) aborted the connection.
Fix: Check the server's log at that time; idle timeouts on NAT/firewalls and proxies are common causes.

### Protocol not available
Means: ENOPROTOOPT: a socket option isn't supported (often IPv6 options on a kernel with IPv6 disabled).
Fix: Configure the app for IPv4 only, or re-enable IPv6.

### Address family not supported by protocol
Means: EAFNOSUPPORT: the program tried IPv6 (or another family) that's disabled in the kernel or container.
Fix: Disable IPv6 in the app's config, or enable IPv6 (`sysctl net.ipv6.conf.all.disable_ipv6=0`).

### Network dropped connection on reset
Means: ENETRESET: the network reset the connection (rare; a router or VPN restart).
Fix: Retry; check the VPN/router logs.

### No buffer space available
Means: ENOBUFS: the kernel ran out of socket buffers or the neighbour table is full.
Fix: Look for floods; raise neighbour table thresholds (`gc_thresh`) on large networks.

### Message too long
Means: EMSGSIZE: a packet is bigger than the path MTU (common over VPNs and with jumbo frames on part of the path).
Fix: Lower the MTU on the interface or VPN; make jumbo frames consistent end to end. Test with `ping -M do -s 1472 <host>`.

### ping: local error: message too long, mtu=<n>
Means: The ping size exceeds the interface MTU with don't-fragment set.
Fix: Expected when probing MTU; lower `-s` until it passes to find the path MTU.

### arping: Interface "<iface>" is not ARPable
Means: The interface doesn't use ARP (loopback, a WireGuard/tun interface).
Fix: Use arping on an Ethernet interface.

### iperf3: error - unable to connect to server: Connection refused
Means: No iperf3 server is running on the target (or a firewall blocks 5201).
Fix: Start `iperf3 -s` on the target and open TCP 5201. Nightgrid's Fleet → **Speed** starts it for you.

### iperf3: error - the server is busy running a test. try again later
Means: iperf3's server handles one test at a time.
Fix: Wait, or start a second server on another port (`-p`).

### nc: connect to <host> port <port> (tcp) failed: Connection refused
Means: netcat found nothing listening there.
Fix: Check the service and its bind address.

### wg-quick: '<iface>' already exists
Means: The WireGuard interface is already up.
Fix: `wg-quick down <iface>` first, or use `wg syncconf` to apply changes.

### RTNETLINK answers: Operation not supported
Means: The kernel lacks the module for that link type (WireGuard, VLAN, bonding), often in a container or an old kernel.
Fix: `modprobe <module>` on the host; in LXC, load it on the Proxmox host.

### tailscale: failed to connect to local tailscaled; it doesn't appear to be running
Means: The Tailscale daemon isn't running.
Fix: `sudo systemctl enable --now tailscaled`, then `tailscale up`.

### Health check: Tailscale could not connect to any relay server.
Means: tailscaled can't reach DERP relays or the coordination server (outbound blocked, captive portal).
Fix: Allow outbound HTTPS and UDP 3478/41641; check `tailscale netcheck`.

### Warning: UDP is blocked, Tailscale will use a DERP relay
Means: Direct connections aren't possible, so traffic is relayed (slower).
Fix: Allow outbound UDP; on the router, enable UPnP/NAT-PMP or forward UDP 41641 to the host.

### Some peers are advertising routes but --accept-routes is false
Means: Subnet routes exist on the tailnet but this machine ignores them.
Fix: `sudo tailscale set --accept-routes` (Linux) if you want to reach those subnets. See [Tailscale subnet router](kb:apps/tailscale-subnet-router).

### dnsmasq: failed to create listening socket for port 53: Address already in use
Means: Port 53 is taken, typically by systemd-resolved's stub listener (or another DNS server).
Fix: Set `DNSStubListener=no` in /etc/systemd/resolved.conf and restart resolved, then point resolv.conf at a real server. Pi-hole and AdGuard need the same.

### Error starting userland proxy: listen tcp4 0.0.0.0:53: bind: address already in use
Means: A container wants host port 53 but systemd-resolved (or dnsmasq) already listens there.
Fix: Free port 53 as above, or publish DNS on a specific IP.
