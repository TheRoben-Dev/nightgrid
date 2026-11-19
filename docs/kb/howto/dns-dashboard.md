# Control Pi-hole and AdGuard Home from Nightgrid

Add your DNS filter as an integration to see its stats on a card, pause blocking for a few minutes, and manage local DNS records and block lists without opening its web UI.

![What the DNS integration reads and controls.](fig:ng-dns-integration)

## What you end up with

- Queries, blocked percentage and top blocked domains on the Services board
- Pause blocking for 5 min, 30 min, 1 h or until you turn it on
- Local DNS records (`nas.home.lan` → `192.168.1.20`) managed in one place

## Before you start

- Pi-hole version 6 or AdGuard Home running (see [Pi-hole](kb:apps/pihole) or [AdGuard Home](kb:apps/adguard-home)).

## Part 1 · Add the integration

- [ ] [Services](view:services) → **Add integration** → **Pi-hole** or **AdGuard Home**.
- [ ] Pi-hole: the web password or, better, an app password (Pi-hole: Settings → Web interface / API → Configure app password).
- [ ] AdGuard Home: its login name and password.
- [ ] Credentials go to the OS keychain.

## Part 2 · Manage

- [ ] **Manage…** on the card opens the dashboard.
- [ ] Pause blocking when a site breaks, then come back to whitelist the domain.
- [ ] Add local DNS records for your servers, so you use names instead of IPs everywhere (including in Nightgrid's hosts).
- [ ] Turn block and allow lists on or off, add or remove lists and download them again.

#### Check

- [ ] `nslookup nas.home.lan <dns-ip>` from any host returns the record you added.

## Limits and gotchas

- Run two DNS servers (a second Pi-hole or AdGuard, or the router as backup) so a reboot doesn't take the whole network offline. Keep their records in sync with Nebula Sync or AdGuardHome-Sync.
- Pi-hole 5 and older aren't supported by this integration.
- Saving a Pi-hole or AdGuard Home integration failed with a database error before Nightgrid 1.7.2. Update Nightgrid and add it again; nothing else needs changing.

## Related

- [Pi-hole](kb:apps/pihole)
- [AdGuard Home](kb:apps/adguard-home)
- [Unbound recursive DNS](kb:apps/unbound)
