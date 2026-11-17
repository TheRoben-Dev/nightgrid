# Open app web UIs inside Nightgrid

Open Portainer, qBittorrent, Home Assistant or any container's web page in its own Nightgrid window, from home or away, without publishing ports or adding proxy routes.

![A web UI window dials the app directly, or through a temporary SSH tunnel.](fig:ng-web-ui-window)

## What you end up with

- **⧉** on a container's row that opens its web page in a window
- Services that open in a Nightgrid window with one click
- The right port and path for containers that publish several

## Part 1 · From Docker

- [ ] [Docker](view:docker): on a running container's row, **⧉** opens its web interface. Nightgrid picks published ports that look like web pages (8080, 443, 3000 and so on) and skips databases, DNS and game ports.
- [ ] A container with several web ports: open its detail panel; **Web UI** opens the main one, and there's a button for each other web port.
- [ ] Nightgrid connects straight from this computer when it can reach the port, otherwise through a temporary SSH tunnel to the host. Ports published only on `127.0.0.1` always go through the tunnel, which makes loopback-only admin pages safe and still easy to reach.

## Part 2 · Tell it which port

- [ ] Add a `nightgrid.webui` label to the service in its compose file, then redeploy the stack:

```yaml
    labels:
      nightgrid.webui: "https:8443/admin"
```

- [ ] The value can be a port (`8080`), a scheme, port and path (`https:8443/admin`), or a path alone (`/ui`).

## Part 3 · From Services and Tunnels

- [ ] [Services](view:services): click a service's name to open it in a Nightgrid window (through its host by SSH tunnel when this computer can't reach it directly). **↗** still opens it in your browser.
- [ ] [Tunnels](view:tunnels): **Browse** on a running local tunnel opens it in a window too.

#### Check

- [ ] Publish a container's port on `127.0.0.1` only (`"127.0.0.1:9000:9000"`), then **⧉**: the page opens through the tunnel, and the port is still closed to the LAN.

## When Nightgrid is closed

- Nothing keeps running: the window and its temporary tunnel close with the app. The containers themselves are unaffected.

## Limits and gotchas

- HTTPS pages with a self-signed certificate can't be shown in the window. Nightgrid says so and offers **Open in browser**; or give the app a certificate from your own CA.
- The window is an ordinary web page with no access to Nightgrid or your hosts. Links that open a new window go to your browser.
- Apps that check the `Host` header or redirect to a fixed URL (some *arr apps behind a base URL, Nextcloud's trusted domains) may need their settings to allow the address Nightgrid uses.

## Related

- [Reach admin web UIs through SSH tunnels](kb:howto/tunnels)
- [Manage reverse proxy routes](kb:howto/proxy-routes)
- [Private certificate authority with step-ca](kb:apps/step-ca)
