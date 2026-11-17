# Scrypted cameras for HomeKit

Bring IP cameras into Apple Home (HomeKit Secure Video), Google Home or Alexa with low-latency streams. Scrypted is the go-to bridge when you want cameras in the Home app with notifications and recordings.

## What you end up with

- RTSP and ONVIF cameras (Reolink, Amcrest, Hikvision, UniFi Protect and more) in the Home app
- Fast stream start and two-way audio where the camera supports it
- Optional motion and object detection on your server

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `scrypted`:

```yaml
services:
  scrypted:
    image: ghcr.io/koush/scrypted:latest
    restart: unless-stopped
    network_mode: host
    volumes:
      - ./volume:/server/volume
```

- [ ] Open `https://host:10443` (self-signed certificate) and create the admin account.
- [ ] For hardware video decoding, add `devices: ["/dev/dri:/dev/dri"]` (Intel or AMD iGPU).

## Part 2 · Cameras

- [ ] Plugins → install the plugin for your camera brand (or **ONVIF** / **RTSP** for generic ones).
- [ ] Add each camera with its address and a local user made on the camera for Scrypted.
- [ ] Install **Rebroadcast** (usually pulled in automatically): one connection to the camera, shared by everything.

## Part 3 · HomeKit

- [ ] Install the **HomeKit** plugin. Each camera gets its own pairing code; add them in the Home app.
- [ ] For HomeKit Secure Video recordings you need a home hub (Apple TV or HomePod) and an iCloud plan.

#### Check

- [ ] The camera's live view opens in the Home app within a couple of seconds.

## In Nightgrid

- Streams show up as steady network traffic on [Overview](view:overview); a camera that drops off shows as a gap there and in the container log.
- Keep cameras on their own VLAN with no internet; Scrypted on the server is the only thing that talks to them.

## Limits and gotchas

- Recording all the time is a job for an NVR like Frigate; Scrypted's own NVR is a paid plugin.
- Many cheap cameras' main stream is too heavy for phones; set a sub-stream for remote viewing.

## Related

- [Frigate NVR](kb:apps/frigate)
- [Homebridge for Apple Home](kb:apps/homebridge)
- [Smart home hub](kb:guides/smart-home-hub)
