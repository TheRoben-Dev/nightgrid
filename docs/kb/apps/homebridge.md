# Homebridge for Apple Home

Bring devices that don't support HomeKit (or Matter) into Apple's Home app and Siri through plugins: Ring, Tuya, Nest, Roborock, many TVs and thousands more.

## What you end up with

- Non-HomeKit devices in the Home app on iPhone, iPad, Mac and Apple TV
- A web UI for plugins and their settings
- Optional child bridges so one crashing plugin doesn't take down the rest

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `homebridge`:

```yaml
services:
  homebridge:
    image: homebridge/homebridge:latest
    restart: unless-stopped
    network_mode: host
    volumes:
      - ./homebridge:/homebridge
```

- [ ] Host networking is required for HomeKit's mDNS discovery. The UI is on port 8581; create the admin login on first open.

## Part 2 · Plugins and pairing

- [ ] Plugins tab: search for your device brand; prefer plugins marked "Verified".
- [ ] Configure each plugin, and in its bridge settings turn on **Child Bridge** for anything flaky.
- [ ] Pair: in the Home app → Add Accessory → scan the QR code on Homebridge's status page.

#### Check

- [ ] A device added through a plugin can be switched from the Home app and from Siri.

## In Nightgrid

- The container log in [Docker](view:docker) shows plugin errors (token expired, device offline) first.
- Back up `./homebridge`: it holds the pairing; losing it means re-adding every accessory in the Home app.

## Limits and gotchas

- If you also run Home Assistant, its HomeKit Bridge integration does the same job; you rarely need both.
- Apple Home on another VLAN or subnet from Homebridge needs mDNS reflection.

## Related

- [Home Assistant in Docker](kb:apps/home-assistant)
- [Scrypted cameras for HomeKit](kb:apps/scrypted)
- [Smart home hub](kb:guides/smart-home-hub)
