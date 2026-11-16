# Frigate NVR

Record IP cameras locally with real-time object detection (people, cars, animals), using a Coral, an Intel iGPU or an NVIDIA GPU, and send events to Home Assistant.

## What you end up with

- Continuous or event recording on your own disk
- Object detection that doesn't send video anywhere
- Clips and snapshots in Home Assistant

## Before you start

- Cameras with RTSP streams; a low-resolution substream for detection and the main stream for recording.
- A detector: Google Coral (USB or M.2), an Intel iGPU with OpenVINO, or an NVIDIA GPU.
- Fast storage with room: 1080p continuous recording is roughly 10–20 GB per camera per day.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `frigate`:

```yaml
services:
  frigate:
    image: ghcr.io/blakeblackshear/frigate:stable
    restart: unless-stopped
    shm_size: "256mb"
    devices:
      - /dev/dri/renderD128:/dev/dri/renderD128
    volumes:
      - ./config:/config
      - /srv/frigate:/media/frigate
      - type: tmpfs
        target: /tmp/cache
        tmpfs:
          size: 1000000000
    ports:
      - "8971:8971"
      - "8554:8554"
      - "8555:8555/tcp"
      - "8555:8555/udp"
    environment:
      FRIGATE_RTSP_PASSWORD: ${RTSP_PASSWORD}
```

- [ ] USB Coral: add `- /dev/bus/usb:/dev/bus/usb` under `devices`.
- [ ] Camera passwords go in the **Vault** and are referenced as `{FRIGATE_RTSP_PASSWORD}` in the config.

## Part 2 · Config

- [ ] `./config/config.yml` for one camera with an OpenVINO detector on an Intel iGPU:

```yaml
mqtt:
  enabled: false
detectors:
  ov:
    type: openvino
    device: GPU
model:
  path: /openvino-model/ssdlite_mobilenet_v2.xml
  width: 300
  height: 300
  input_tensor: nhwc
  input_pixel_format: bgr
  labelmap_path: /openvino-model/coco_91cl_bkgr.txt
cameras:
  driveway:
    ffmpeg:
      hwaccel_args: preset-vaapi
      inputs:
        - path: rtsp://admin:{FRIGATE_RTSP_PASSWORD}@192.168.1.50:554/stream2
          roles: [detect]
        - path: rtsp://admin:{FRIGATE_RTSP_PASSWORD}@192.168.1.50:554/stream1
          roles: [record]
    detect:
      width: 640
      height: 360
record:
  enabled: true
  retain:
    days: 7
    mode: motion
```

- [ ] Open port 8971 (the authenticated UI); the admin password is printed in the logs on first start.

## Part 3 · In Nightgrid

- [ ] [Fleet](view:fleet) → **Disks** forecasts when `/srv/frigate` fills; adjust retention before it does.
- [ ] Monitor GPU load (AMD and NVIDIA show in the host monitor) and the container's CPU in [Docker](view:docker).
- [ ] Home Assistant: install the Frigate integration (via HACS) and enable MQTT in Frigate with your broker from [Zigbee2MQTT and Mosquitto](kb:apps/zigbee2mqtt).

#### Check

- [ ] Walking past a camera creates a "person" event with a clip.

## Limits and gotchas

- Put cameras on a separate VLAN without internet access; many phone home.
- Detection on the main high-resolution stream wastes CPU; always detect on the substream.
- Model and detector settings change between Frigate versions; check its docs for yours.

## Related

- [Home Assistant in Docker](kb:apps/home-assistant)
- [Smart home hub](kb:guides/smart-home-hub)
- [Keep an eye on disks and pools](kb:howto/disks)
