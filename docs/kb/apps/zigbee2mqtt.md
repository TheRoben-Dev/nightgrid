# Zigbee2MQTT and Mosquitto

Run your Zigbee network independently of Home Assistant: Mosquitto as the MQTT broker and Zigbee2MQTT talking to the USB coordinator.

![From a Zigbee device to Home Assistant through Zigbee2MQTT and MQTT.](fig:app-ha-zigbee)

## What you end up with

- An MQTT broker with a password
- Zigbee2MQTT with its frontend, pairing devices from a browser
- Devices discovered automatically in Home Assistant through MQTT

## Before you start

- A Zigbee coordinator (a current Sonoff, SMLIGHT or similar). Put it on a USB extension cable away from USB 3 ports and Wi-Fi.
- Its path: `ls -l /dev/serial/by-id/`.

## Part 1 · Mosquitto

- [ ] Create `./mosquitto/config/mosquitto.conf` in a stack folder `zigbee`:

```
listener 1883
allow_anonymous false
password_file /mosquitto/config/passwd
persistence true
persistence_location /mosquitto/data/
```

- [ ] Create a user (after the stack exists, in a [Terminal](view:terminal)):

```sh
docker compose -f /opt/stacks/zigbee/compose.yaml run --rm mosquitto mosquitto_passwd -c /mosquitto/config/passwd mqtt
```

## Part 2 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `zigbee`:

```yaml
services:
  mosquitto:
    image: eclipse-mosquitto:2
    restart: unless-stopped
    ports:
      - "1883:1883"
    volumes:
      - ./mosquitto/config:/mosquitto/config
      - ./mosquitto/data:/mosquitto/data
  zigbee2mqtt:
    image: ghcr.io/koenkk/zigbee2mqtt:latest
    restart: unless-stopped
    depends_on: [mosquitto]
    ports:
      - "127.0.0.1:8080:8080"
    environment:
      TZ: Etc/UTC
    volumes:
      - ./z2m:/app/data
      - /run/udev:/run/udev:ro
    devices:
      - /dev/serial/by-id/usb-<your-stick>:/dev/ttyACM0
```

- [ ] `./z2m/configuration.yaml` (essentials):

```yaml
mqtt:
  server: mqtt://mosquitto:1883
  user: mqtt
  password: "!secret mqtt_password"
serial:
  port: /dev/ttyACM0
frontend:
  enabled: true
homeassistant:
  enabled: true
advanced:
  network_key: GENERATE
```

- [ ] Put `mqtt_password: <password>` in `./z2m/secret.yaml`.
- [ ] Some coordinators need `adapter: ember` (Silicon Labs) or `zstack` (TI) under `serial:`; Zigbee2MQTT's docs list yours.

## Part 3 · Pair and connect

- [ ] Open the frontend through a saved tunnel in [Tunnels](view:tunnels) to port 8080, press **Permit join**, and reset each device to pair it.
- [ ] Home Assistant: add the MQTT integration with the broker's address and the `mqtt` login. Devices appear by themselves.

#### Check

- [ ] A paired sensor's state changes show in both the frontend and Home Assistant.

## Limits and gotchas

- Back up `./z2m` (it holds the network key and device database). Losing it means re-pairing every device.
- Mains-powered devices act as routers; add a few before battery sensors far from the coordinator.

## Related

- [Home Assistant in Docker](kb:apps/home-assistant)
- [Smart home hub](kb:guides/smart-home-hub)
