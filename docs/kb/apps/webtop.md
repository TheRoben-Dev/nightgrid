# Webtop Linux desktop in a browser

A full Linux desktop (XFCE, KDE, MATE and others on Debian, Ubuntu, Fedora, Arch or Alpine) running in a container and shown in a browser tab. Handy as a disposable desktop, for GUI-only tools on a headless server, or a browser that runs from home while you're away.

## What you end up with

- A Webtop container with a persistent home folder
- The desktop opened safely through an SSH tunnel, never exposed on the LAN
- GPU acceleration where the host has an Intel or AMD iGPU

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `webtop`:

```yaml
services:
  webtop:
    image: lscr.io/linuxserver/webtop:debian-xfce
    restart: unless-stopped
    shm_size: "1gb"
    environment:
      PUID: "1000"
      PGID: "1000"
      TZ: Etc/UTC
    volumes:
      - ./config:/config
    ports:
      - "127.0.0.1:3000:3000"
    labels:
      nightgrid.webui: "3000"
```

- [ ] Pick another flavour by tag: `ubuntu-kde`, `fedora-xfce`, `arch-mate`, `alpine-xfce` and so on. Alpine images are the smallest.
- [ ] `/config` is the desktop user's home: browser profiles, files and settings live there and survive updates.
- [ ] `shm_size` matters: browsers inside the desktop crash with the default 64 MB.

## Part 2 · Open it

- [ ] **⧉** on the container's row in [Docker](view:docker): the port is bound to `127.0.0.1`, so Nightgrid always opens it through a temporary SSH tunnel. Nobody else on the LAN can reach the desktop.
- [ ] The container has **no login of its own by default**. That's why the port stays on loopback. If you must publish it, set `CUSTOM_USER` and `PASSWORD`, and use the HTTPS port 3001 behind a reverse proxy with real authentication.

## Part 3 · Make it useful

- [ ] Hardware-accelerated video and apps on Intel/AMD: add `devices: ["/dev/dri:/dev/dri"]`.
- [ ] Install apps from the desktop's terminal; they're lost when the image updates. Anything permanent goes in a custom init script under `/config/custom-cont-init.d/` or a Docker mod.
- [ ] Mount extra folders (`/srv/media:/media:ro`) to work on server files with GUI tools.

#### Check

- [ ] The desktop opens in a Nightgrid window, a file saved on its desktop is still there after **Restart**, and `curl http://<host-lan-ip>:3000` from another machine fails.

## In Nightgrid

- For a real VM's screen use [Desktops](view:desktops) instead; Webtop is for a lightweight desktop without a hypervisor.
- Add the stack's `config` folder to your backups if the desktop holds anything you care about.

## Limits and gotchas

- Audio and clipboard work in most browsers, but gaming and video calls inside are not smooth.
- Each desktop uses 500 MB–2 GB RAM depending on the flavour and apps running.
- Exposed without authentication, a Webtop is a full shell on your network for anyone who finds it. Keep it on loopback.

## Related

- [Open app web UIs inside Nightgrid](kb:howto/web-uis)
- [Open VM screens and RDP](kb:howto/vm-desktops)
- [Apache Guacamole remote desktop gateway](kb:apps/guacamole)
