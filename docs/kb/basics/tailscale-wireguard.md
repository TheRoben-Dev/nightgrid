# Tailscale and WireGuard

A VPN lets you reach home servers from anywhere as if you were on the LAN, without opening ports on your router. **WireGuard** is the fast, simple VPN protocol underneath; **Tailscale** builds a managed mesh on top of it.

![Devices join the tailnet directly; a subnet router covers LAN gear that can't.](fig:tailnet-subnet)

## WireGuard on its own

You run one WireGuard server (often the router or a small VM), forward one UDP port to it, and give each device a key pair and a config file. It's fast and has no outside dependency, but you manage keys, addresses and the port forward yourself, and every device connects through that one server.

## Tailscale

Every device runs the Tailscale client and logs in. Devices find each other through Tailscale's coordination service and connect directly when they can (through NAT, without port forwards), falling back to Tailscale's relays (DERP) when they can't. Traffic is always end-to-end WireGuard; the coordination service only sees keys and metadata.

Useful features for a homelab:

- **MagicDNS**: reach machines by name (`nas`, `lab`).
- **Subnet routes**: one machine advertises your LAN (`--advertise-routes=192.168.1.0/24`) so devices without Tailscale (a printer, a NAS) are reachable too. Approve the route in the admin console.
- **Exit node**: send all your traffic through home when on public Wi-Fi.
- **Tailscale SSH**: SSH authorised by your tailnet identity instead of keys, optionally with a browser check.
- **ACLs**: which devices may reach which.

Headscale is a self-hosted replacement for the coordination service if you'd rather not depend on Tailscale's.

## In Nightgrid

- Nightgrid has a **built-in Tailscale client** (Settings → **CONNECTIONS** → **TAILNET**), opt-in, so tailnet hosts work even when the system app isn't running.
- A host can have a **tailnet fallback address**; Nightgrid switches between LAN and tailnet automatically.
- Tailscale SSH hosts work, including the browser approval step.
- The network map shows tailnet peers and whether each connection is direct or relayed.

## Things that trip people up

- Two sites that both use `192.168.1.0/24` can't both be subnet routes. Change one LAN's subnet.
- Linux subnet routers need IP forwarding on (`net.ipv4.ip_forward=1`).
- A peer that's idle often shows as relayed until traffic sets up a direct path.
- Key expiry: servers should have key expiry turned off in the admin console, or they drop off every few months.

## Related

- [Hardening sshd](kb:basics/sshd-hardening): with a VPN, SSH needn't face the internet at all.
- [Reverse proxies](kb:basics/reverse-proxies): certificates for names only your tailnet uses.
