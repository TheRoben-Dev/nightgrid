# Reach admin web UIs through SSH tunnels

Keep admin panels (Portainer, qBittorrent, Zigbee2MQTT, Proxmox, router UIs) bound to localhost or the LAN, and open them from anywhere through the host's SSH connection. No ports opened, no reverse proxy entry, nothing public.

## What you end up with

- One-click web UIs through **OPEN WEB UI VIA**
- Saved tunnels that start with Nightgrid and come back after a reconnect
- A SOCKS proxy to browse a remote LAN as if you were there

## Part 1 · A one-off web UI

- [ ] [Tunnels](view:tunnels): next to **OPEN WEB UI VIA**, pick the host and type the address as the host sees it, such as `localhost:8080` or `192.168.1.1:443`, then **Open**.
- [ ] Nightgrid starts a temporary tunnel (marked **TEMP**) and opens it in your browser. It's not saved.

## Part 2 · Saved tunnels

- [ ] **Add tunnel**:
  - **local** (`-L`): reach a port that only listens on the host, or something on the host's LAN. Example: local 8091 → `127.0.0.1:8091` for Z-Wave JS UI.
  - **remote** (`-R`): expose a port on this computer to the host.
  - **SOCKS** (`-D`): a proxy that browses from the host's network. Point a browser profile at it to reach every LAN device's web UI.
- [ ] Turn on auto-start (**AUTO**) for the ones you use daily; they start with Nightgrid and after reconnects.
- [ ] **Browse** opens a local tunnel's address in a Nightgrid window (see [Open app web UIs inside Nightgrid](kb:howto/web-uis)).

## Part 3 · Bind apps to localhost

- [ ] In compose, publish admin ports as `127.0.0.1:8080:8080`. Then only the host itself (and your tunnels) can reach them.
- [ ] [System](view:system) → **security** confirms nothing admin-only listens on all interfaces.

#### Check

- [ ] The admin UI opens through the tunnel, and `nc -vz <host> 8080` from another LAN machine fails.

## When Nightgrid is closed

- Tunnels close with Nightgrid. The tray keeps them running with the window closed, and its menu starts and stops saved tunnels.

## Limits and gotchas

- Listening anywhere other than localhost on this computer is flagged **PUBLIC**: other devices on your current network could use the tunnel.
- Web apps that redirect to their own configured URL (some set an absolute base URL) may jump away from the tunnel address; use the app's LAN URL setting or a SOCKS proxy instead.
- Tunnels go through the host's SSH server; `AllowTcpForwarding no` in sshd blocks them.

## Related

- [Raise a host's security score](kb:howto/security-score)
- [Nginx Proxy Manager](kb:apps/nginx-proxy-manager)
- [Reach your servers away from home](kb:howto/remote-access)
