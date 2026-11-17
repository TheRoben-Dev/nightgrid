# evcc solar EV charging

Charge the car from your own solar surplus: evcc reads your grid meter and inverter, then sets the wallbox's charging current so the car uses what the house would otherwise export. Modes for "solar only", "solar plus minimum" and "fast", plus departure-time plans and cheap-tariff charging.

## What you end up with

- evcc on a home server, talking to your charger, meters and car
- A web page and app showing house, solar, grid, battery and car in one view
- Charging that follows the sun, with a minimum to make sure the car is ready

## Before you start

- A supported **charger** (go-e, Easee, Wallbox, OpenWB, KEBA, Tesla Wall Connector, many OCPP chargers and more), a **grid meter** (inverter with a smart meter, Shelly 3EM, Tibber Pulse, a Modbus meter) and optionally your **car's** API login.
- Check evcc's device list for your exact models before buying hardware.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `evcc`. Host networking lets it find devices with mDNS and reach Modbus/TCP gear:

```yaml
services:
  evcc:
    image: evcc/evcc:latest
    restart: unless-stopped
    network_mode: host
    volumes:
      - ./data:/root/.evcc
      - ./evcc.yaml:/etc/evcc.yaml
```

- [ ] Create an empty `evcc.yaml` first (`touch evcc.yaml`) so Docker mounts a file, not a folder.
- [ ] Open `http://<host>:7070`. Recent versions configure meters, chargers and vehicles in the web UI (**Configuration**); older guides use the `evcc configure` wizard or a hand-written `evcc.yaml`. Use one or the other, not both.

## Part 2 · Configure

- [ ] **Grid meter** first: evcc needs to know import and export, otherwise it can't compute surplus.
- [ ] **PV meter(s)** and **battery** if you have them, so the house view is complete.
- [ ] **Charger**, then a **loadpoint** that ties the charger to a vehicle, with minimum current (6 A is the usual floor) and phases.
- [ ] **Vehicle** with its API login for state of charge, or none (evcc then charges blind but still follows surplus).

## Part 3 · Use it

- [ ] **Solar** mode charges only from surplus; **Min+Solar** charges at the minimum and adds surplus; **Fast** charges at full power.
- [ ] **Plan**: "80 % by 7:00" picks the cheapest or sunniest hours to get there.
- [ ] The page installs as an app on phones; there's also a Home Assistant integration.

#### Check

- [ ] On a sunny day in **Solar** mode, grid export stays near zero while the car charges, and charging pauses when a cloud passes.

## In Nightgrid

- Add `http://<host>:7070` to [Services](view:services). evcc only controls charging while it runs, so an alert on it matters.
- Back up `data` (the database with configuration and history) and `evcc.yaml`.

## Limits and gotchas

- Many cars and chargers only switch between 1 and 3 phases with specific hardware; without that, the minimum on three phases is about 4.1 kW, so small solar systems rarely reach it.
- Cloud APIs for cars rate-limit; don't poll more often than the defaults.
- A wrong meter sign (import shown as export) makes evcc charge from the grid. Watch the first day closely.

## Related

- [Home Assistant in Docker](kb:apps/home-assistant)
- [TeslaMate car data logger](kb:apps/teslamate)
- [InfluxDB and Telegraf metrics](kb:apps/influxdb-telegraf)
