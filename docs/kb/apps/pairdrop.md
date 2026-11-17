# PairDrop local file sharing

AirDrop for every device: open the page on a phone, a Windows PC and a Linux laptop, and they see each other and can send files and text straight between them. Files go device to device over WebRTC; the server only introduces them.

## What you end up with

- PairDrop on your LAN (and optionally on a public name)
- Devices on the same network finding each other automatically
- Paired devices that can share across networks

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `pairdrop`:

```yaml
services:
  pairdrop:
    image: lscr.io/linuxserver/pairdrop:latest
    restart: unless-stopped
    environment:
      PUID: "1000"
      PGID: "1000"
      TZ: Etc/UTC
      RATE_LIMIT: "false"
      WS_FALLBACK: "false"
    ports:
      - "3000:3000"
```

- [ ] Browsers only allow some features (installing as an app, clipboard paste) on HTTPS. Put it behind your reverse proxy with a name like `drop.example.com`, and set `RATE_LIMIT` to `1` when the proxy is the only client.

## Part 2 · Use it

- [ ] Open the page on two devices on the same network: each shows the other with a random name. Click a device to send files, right-click (long-press on phones) to send text.
- [ ] **Pair device**: shows a code or QR so two devices find each other even on different networks, permanently.
- [ ] **Public rooms**: a temporary room code for one-off sharing with someone who isn't paired.
- [ ] On phones, **Add to Home Screen** makes it behave like an app with a share-sheet target on Android.

#### Check

- [ ] A photo sent from a phone arrives on a laptop in a few seconds, without either leaving the LAN.

## In Nightgrid

- Add the page to [Services](view:services). PairDrop keeps no files, so there's nothing to back up beyond the compose file.

## Limits and gotchas

- Devices find each other by public IP; on networks where every device has its own IPv6 address, pair them once instead.
- Strict corporate or guest networks block WebRTC; `WS_FALLBACK: "true"` relays through the server instead (uses its bandwidth).
- Transfers stop if a screen locks on the sending phone; keep it awake for big files.

## Related

- [Syncthing](kb:apps/syncthing)
- [File Browser web file manager](kb:apps/filebrowser)
- [Samba file shares](kb:apps/samba)
