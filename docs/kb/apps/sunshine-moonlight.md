# Stream games with Sunshine and Moonlight

Play games running on your gaming PC or a GPU server from a laptop, TV, phone or handheld anywhere in the house, at low latency and up to 4K/120. Sunshine is the host (it captures the screen and encodes on the GPU); Moonlight is the free client for almost every device.

## What you end up with

- Sunshine on the gaming machine, paired with your devices by PIN
- Hardware encoding on NVIDIA (NVENC), AMD (AMF/VA-API) or Intel (Quick Sync)
- Streaming at home, and away from home over your tailnet

## Before you start

- A host with a supported GPU and a desktop session. Windows or Linux both work; on Linux, Wayland and X11 are both supported but KMS capture needs extra permissions.
- Wired Ethernet on the host. Wi-Fi on the client is fine on 5 or 6 GHz.

## Part 1 · Install Sunshine

- [ ] **Windows**: run the installer from the Sunshine releases page; it installs a service that starts at boot.
- [ ] **Linux**: install the package for your distro or the Flatpak (`dev.lizardbyte.app.Sunshine`). For KMS screen capture, follow the post-install step that grants `cap_sys_admin` to the binary, then start it as a user service:

```bash
systemctl --user enable --now sunshine
```

- [ ] Open `https://localhost:47990` on the host, accept the self-signed certificate and create the admin login.

## Part 2 · Pair a client

- [ ] Install **Moonlight** on the client (Windows, macOS, Linux, Android, iOS, Apple TV, Steam Deck and most handhelds, many smart TVs).
- [ ] The host appears automatically on the LAN. Select it: Moonlight shows a 4-digit PIN.
- [ ] Enter the PIN in Sunshine's web UI under **PIN**. The client is now trusted.
- [ ] Launch **Desktop** or **Steam Big Picture**. Add more apps under **Applications** with their own commands and resolutions.

## Part 3 · Tune

- [ ] Moonlight settings: resolution and FPS that match the client's screen, bitrate 20–50 Mbps for 1080p60, 80–150 Mbps for 4K. Enable **HEVC** or **AV1** if both ends support it.
- [ ] Sunshine **Audio/Video**: pick the encoder explicitly if it guesses wrong (logs show which one it uses).
- [ ] A headless host needs a display: a dummy HDMI plug, or a virtual display driver, so the GPU renders at your chosen resolution.

## Part 4 · Away from home

- [ ] With Tailscale (or another VPN) on both ends, add the host by its tailnet address in Moonlight: **+** → address. No ports are opened on the router.
- [ ] Lower the bitrate to what your upload allows (10–20 Mbps for most home connections).

#### Check

- [ ] A game starts on the client within seconds, and Moonlight's stats overlay (Ctrl+Alt+Shift+S on PC) shows decode and network latency under 10 ms on the LAN.

## In Nightgrid

- Sunshine's web UI listens on the host's LAN by default. To reach it from elsewhere, use [Tunnels](view:tunnels) → **OPEN WEB UI VIA** with `localhost:47990` on that host.
- A Linux streaming host can be a Nightgrid host like any other: **WAKE** from [System](view:system) before you start playing, and a power schedule to turn it off at night.

## Limits and gotchas

- Games with anti-cheat may block virtual input devices; most work.
- Don't port-forward Sunshine's ports to the internet. Use a VPN.
- On Linux, HDR streaming and some capture methods depend on the desktop environment and driver version; X11 is the most predictable.

## Related

- [GPU passthrough to a VM](kb:apps/gpu-passthrough)
- [Power schedules, Wake-on-LAN and group shutdown](kb:howto/power)
- [Tailscale subnet router](kb:apps/tailscale-subnet-router)
