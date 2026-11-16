# Add Plex, AMP, Uptime Kuma and Portainer cards

Bring other tools' live status into Nightgrid's Services board: what's playing on Plex, game servers in AMP, monitors from Uptime Kuma, and links into Portainer. Tokens stay in the OS keychain.

## Part 1 · Plex

- [ ] [Services](view:services) → **Add integration** → **Plex**: the server address (port 32400) and a Plex token.
- [ ] Find the token: open any item in Plex Web, "Get Info" → "View XML", and copy the `X-Plex-Token` value from the address.
- [ ] The card shows current streams. Server setup: [Plex Media Server](kb:apps/plex).

## Part 2 · AMP

- [ ] **Add integration** → **AMP**: the panel address and a login.
- [ ] The card lists instances with start, stop and restart (stop and restart confirm by name). More in [Game server host](kb:guides/game-server-host).

## Part 3 · Uptime Kuma

- [ ] **Add integration** → **Uptime Kuma** with a status page's URL. The card lists that page's monitors.
- [ ] Kuma watches 24/7 from a server; Nightgrid adds history and alert folding. See [Uptime Kuma](kb:apps/uptime-kuma).

## Part 4 · Portainer

- [ ] **Add integration** → **Portainer** for "Open in Portainer" links (see [Portainer alongside Nightgrid](kb:apps/portainer)).

## Part 5 · DNS filters

- [ ] Pi-hole v6 and AdGuard Home have their own guide: [Control Pi-hole and AdGuard Home from Nightgrid](kb:howto/dns-dashboard).

#### Check

- [ ] Each card shows live data; removing an integration also removes its token from the keychain.

## Limits and gotchas

- Integration tokens are included in profile exports only when you tick **Include saved passwords and tokens**.
- Cards refresh while Services is open; they don't alert by themselves. Add a plain service check for alerts.

## Related

- [Watch services and expiry dates](kb:howto/service-checks)
- [Control Pi-hole and AdGuard Home from Nightgrid](kb:howto/dns-dashboard)
