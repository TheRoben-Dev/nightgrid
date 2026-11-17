# Node-RED flows

Wire up automations visually: MQTT messages, HTTP requests, schedules and Home Assistant events, in a browser flow editor.

![A flow from trigger to action.](fig:app-flows)

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `nodered`:

```yaml
services:
  node-red:
    image: nodered/node-red:latest
    restart: unless-stopped
    environment:
      TZ: Etc/UTC
    volumes:
      - ./data:/data
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] `./data` must be writable by uid 1000: `sudo chown -R 1000 data`.
- [ ] Turn on the editor's login: in `./data/settings.js` set `adminAuth` with a bcrypt hash (`docker exec -it nodered-node-red-1 node-red admin hash-pw`), then restart.
- [ ] Route `flows.example.com` to `node-red:1880`.

## Part 2 · Connect things

- [ ] MQTT nodes to your broker from [Zigbee2MQTT and Mosquitto](kb:apps/zigbee2mqtt).
- [ ] Home Assistant: install `node-red-contrib-home-assistant-websocket` from the palette and add a long-lived token from HA.
- [ ] Nightgrid's **JSON POST** webhook (Settings → Alerts) can target an `http in` node to react to alerts.

#### Check

- [ ] A test flow with an inject node and debug output runs.

## Limits and gotchas

- Without `adminAuth`, anyone who reaches the editor can run code on the host. Never skip it.
- Back up `./data` (flows and credentials file; the credentials key is in `settings.js` or `.config.runtime.json`).

## Related

- [Home Assistant in Docker](kb:apps/home-assistant)
- [n8n workflow automation](kb:apps/n8n)
