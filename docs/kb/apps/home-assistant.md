# Home Assistant in Docker

Run Home Assistant Container on an existing Linux server, with USB radios passed in, a reverse proxy in front and backups that restore.

## What you end up with

- Home Assistant at `ha.example.com` on host networking (so discovery works)
- Zigbee or Z-Wave sticks passed through by stable path
- Automatic backups copied off the host

## Before you start

- Container vs HA OS: the container has no add-on store. If you want add-ons with one click, run HA OS as a VM instead (see [Smart home hub](kb:guides/smart-home-hub)). Everything an add-on does can also run as a separate container.
- USB radio paths: `ls -l /dev/serial/by-id/`.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `homeassistant`:

```yaml
services:
  homeassistant:
    image: ghcr.io/home-assistant/home-assistant:stable
    restart: unless-stopped
    network_mode: host
    environment:
      TZ: Etc/UTC
    volumes:
      - ./config:/config
      - /etc/localtime:/etc/localtime:ro
      - /run/dbus:/run/dbus:ro
```

- [ ] Open port 8123 on the host and run onboarding.
- [ ] Bluetooth works through the `/run/dbus` mount with BlueZ on the host.

## Part 2 · Behind a reverse proxy

- [ ] Add to `./config/configuration.yaml` (edit in [Files](view:files)):

```yaml
http:
  use_x_forwarded_for: true
  trusted_proxies:
    - 127.0.0.1
    - 172.16.0.0/12
```

- [ ] Route `ha.example.com` to the host's LAN IP on port 8123 (host networking means there's no container name to use).
- [ ] Restart Home Assistant from its UI or the container in Docker.

## Part 3 · Radios

- [ ] Zigbee: run [Zigbee2MQTT and Mosquitto](kb:apps/zigbee2mqtt) as their own containers, or pass the stick to HA for ZHA:

```yaml
    devices:
      - /dev/serial/by-id/usb-<your-stick>:/dev/ttyUSB0
```

## Part 4 · Backups and updates

- [ ] Settings → System → Backups: automatic daily backups, keep 7. They land in `./config/backups`; include that folder in restic or borg.
- [ ] Read the monthly release notes' "backward-incompatible changes" before updating; Nightgrid's **Updates** and stack snapshot make rollback quick.
- [ ] [Services](view:services) → **Add service** for `ha.example.com`.

#### Check

- [ ] Restart the host: HA comes back by itself and devices reconnect.

## Limits and gotchas

- Host networking is needed for mDNS discovery (HomeKit, Chromecast, many integrations).
- The database in `./config` grows; set `recorder: purge_keep_days: 10` if it gets large.

## Related

- [Smart home hub](kb:guides/smart-home-hub)
- [Zigbee2MQTT and Mosquitto](kb:apps/zigbee2mqtt)
- [Frigate NVR](kb:apps/frigate)
