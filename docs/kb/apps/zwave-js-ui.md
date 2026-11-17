# Z-Wave JS UI

Run a Z-Wave network with Z-Wave JS UI: pair devices, manage security keys and expose everything to Home Assistant.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `zwave`:

```yaml
services:
  zwave-js-ui:
    image: zwavejs/zwave-js-ui:latest
    restart: unless-stopped
    tty: true
    stop_signal: SIGINT
    environment:
      TZ: Etc/UTC
      SESSION_SECRET: ${SESSION_SECRET}
    devices:
      - /dev/serial/by-id/usb-<your-stick>:/dev/zwave
    volumes:
      - ./store:/usr/src/app/store
    ports:
      - "127.0.0.1:8091:8091"
      - "3000:3000"
```

- [ ] Open the UI (port 8091) through a saved tunnel in [Tunnels](view:tunnels).
- [ ] Settings → Z-Wave: serial port `/dev/zwave`, generate all security keys (S0, S2), save. Copy the keys into your password manager.
- [ ] Settings → Home Assistant: enable the WS server (port 3000) and in HA add the Z-Wave integration pointing at `ws://<host>:3000`.

#### Check

- [ ] An included device shows "Alive" and appears in Home Assistant.

## Limits and gotchas

- Losing the security keys means re-including every secure device. Back up `./store`.
- Allow port 3000 only from the Home Assistant host.

## Related

- [Home Assistant in Docker](kb:apps/home-assistant)
- [Zigbee2MQTT and Mosquitto](kb:apps/zigbee2mqtt)
