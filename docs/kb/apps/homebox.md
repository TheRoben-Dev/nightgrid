# Homebox home inventory

A lighter inventory for home: what you own, where it is, receipts and manuals attached, warranty dates. One container with SQLite, no database server.

## What you end up with

- Items by location ("Garage › Shelf 2") with photos, receipts and manuals
- Warranty and purchase info you can find when something breaks
- QR labels that open an item or a whole box

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `homebox`:

```yaml
services:
  homebox:
    image: ghcr.io/sysadminsmedia/homebox:latest
    restart: unless-stopped
    environment:
      HBOX_OPTIONS_ALLOW_REGISTRATION: "true"
      TZ: Etc/UTC
    volumes:
      - ./data:/data
    ports:
      - "3100:7745"
```

- [ ] Open port 3100, register the first account, then set `HBOX_OPTIONS_ALLOW_REGISTRATION` to `"false"` and redeploy. Invite family members from the app instead.

## Part 2 · Organise

- [ ] Create **Locations** as a tree (House › Office › Closet), then **Labels** for cross-cutting groups (Electronics, Tools, Spare parts).
- [ ] Add items with photos. Attach the receipt PDF and the manual: that's the part you'll thank yourself for.
- [ ] Fill **Purchase** and **Warranty** fields for anything expensive.
- [ ] Print labels: Tools → Label generator, for boxes and drawers.

#### Check

- [ ] Search for a manual by item name and open its PDF from your phone.

## In Nightgrid

- Everything lives in `./data`: one folder to back up with the rest of the stack (see [Monitor backups and test restores](kb:howto/backups)).
- Add it to [Services](view:services) so you know when it's down.

## Limits and gotchas

- The original Homebox project was archived; `sysadminsmedia/homebox` is the maintained fork. Old `hay-kot` images still run but get no fixes.
- Attachments make the data folder grow quickly with photos; resize big phone pictures before upload if space is tight.

## Related

- [Snipe-IT asset inventory](kb:apps/snipe-it)
- [Paperless-ngx documents](kb:apps/paperless-ngx)
- [Grocy household stock](kb:apps/grocy)
