# ESPHome devices

Build your own sensors and switches on ESP32/ESP8266 boards from YAML, flash them over the air, and use them in Home Assistant without any cloud.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `esphome`:

```yaml
services:
  esphome:
    image: ghcr.io/esphome/esphome:stable
    restart: unless-stopped
    network_mode: host
    volumes:
      - ./config:/config
      - /etc/localtime:/etc/localtime:ro
```

- [ ] The dashboard is on port 6052 of the host. Host networking lets it find devices with mDNS.
- [ ] Set `USERNAME` and `PASSWORD` environment variables to protect the dashboard.

## Part 2 · First device

- [ ] Flash the first time over USB from a browser with ESPHome's web flasher, or from the dashboard with the board on the host's USB.
- [ ] A temperature sensor example:

```yaml
esphome:
  name: office-sensor
esp32:
  board: esp32dev
wifi:
  ssid: !secret wifi_ssid
  password: !secret wifi_password
api:
  encryption:
    key: !secret api_key
ota:
  - platform: esphome
    password: !secret ota_password
sensor:
  - platform: dht
    pin: GPIO4
    temperature:
      name: Office temperature
    humidity:
      name: Office humidity
```

- [ ] Secrets go in `./config/secrets.yaml`.
- [ ] Home Assistant discovers the device; enter the API key.

#### Check

- [ ] Readings arrive in Home Assistant, and a config change installs over Wi-Fi.

## Limits and gotchas

- Compiling the first time downloads toolchains (a few hundred MB) and takes minutes.
- Put IoT devices on their own VLAN or SSID without internet access if your network supports it.

## Related

- [Home Assistant in Docker](kb:apps/home-assistant)
- [Smart home hub](kb:guides/smart-home-hub)
