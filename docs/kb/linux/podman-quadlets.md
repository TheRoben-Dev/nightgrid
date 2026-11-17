# Podman containers as systemd services (Quadlets)

Run containers with Podman, rootless if you like, managed by systemd through Quadlet files: they start at boot, restart on failure, log to the journal and auto-update if you want.

## What you end up with

- A `.container` file per app that systemd turns into a service
- Containers visible in Nightgrid's System → services and journal

## Part 1 · Install

```sh
sudo apt install -y podman     # or: sudo dnf install podman
podman --version               # Quadlets need 4.4 or newer
```

## Part 2 · A Quadlet

- [ ] Rootful: `/etc/containers/systemd/uptime-kuma.container`. Rootless: `~/.config/containers/systemd/uptime-kuma.container`.

```ini
[Unit]
Description=Uptime Kuma

[Container]
Image=docker.io/louislam/uptime-kuma:1
PublishPort=3001:3001
Volume=/srv/uptime-kuma:/app/data:Z
AutoUpdate=registry

[Service]
Restart=always

[Install]
WantedBy=default.target
```

- [ ] Load and start (add `--user` for rootless):

```sh
sudo systemctl daemon-reload
sudo systemctl start uptime-kuma
```

- [ ] Rootless services survive logout only with lingering: `loginctl enable-linger $USER`.

## Part 3 · In Nightgrid

- [ ] [System](view:system) → **services** lists the generated unit; **journal** shows the container's output.
- [ ] Nightgrid's [Docker](view:docker) view talks to Docker; Podman containers show there only if the Podman socket is exposed as Docker's (`podman-docker` package plus `systemctl enable --now podman.socket`). Systemd is the simpler way to manage them.

#### Check

- [ ] `systemctl status uptime-kuma` is active, and a reboot brings it back.

## Limits and gotchas

- `:Z` relabels volumes for SELinux (Fedora/RHEL); drop it on Debian.
- Rootless containers can't bind ports below 1024 without a sysctl change (`net.ipv4.ip_unprivileged_port_start`).

## Related

- [Write your own systemd service](kb:linux/systemd-units)
- [SELinux basics on Fedora and RHEL](kb:linux/selinux)
