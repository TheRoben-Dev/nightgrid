# Atomic Fedora hosts (rpm-ostree)

Fedora CoreOS, Silverblue, Bazzite and other atomic systems update as whole images and roll back in one step. Here's how to run services on them and how Nightgrid handles their updates.

![Deployments side by side, with rollback.](fig:lx-rpm-ostree)

## Part 1 · Updates

```sh
rpm-ostree status
rpm-ostree upgrade
systemctl reboot
```

- [ ] An upgrade stages a new deployment; it applies on the next boot. The previous one stays for rollback: `rpm-ostree rollback`.
- [ ] Nightgrid's [Fleet](view:fleet) → **Updates** and [System](view:system) → **updates** support rpm-ostree: **Upgrade** stages the image, and the reboot flag shows until you reboot.

## Part 2 · Extra packages

- [ ] Layer only what must be on the host (drivers, `tailscale`): `sudo rpm-ostree install tailscale`, then reboot.
- [ ] Everything else goes in containers (Podman ships by default; see [Podman containers as systemd services (Quadlets)](kb:linux/podman-quadlets)) or a toolbox/distrobox for CLI tools.

## Part 3 · Config

- [ ] `/etc` and `/var` are writable and kept across updates; `/usr` is read-only. Configs and app data live in those.
- [ ] Nightgrid's config drift (System → **config**) works as usual on `/etc`.

#### Check

- [ ] `rpm-ostree status` shows the booted deployment and the staged one after an upgrade.

## Limits and gotchas

- Layering many packages slows upgrades and can block them when a layered package conflicts.
- Gaming-focused variants (Bazzite) are client machines; mark them **Client, not a server** in the host editor.

## Related

- [Patch every host from one place](kb:howto/fleet-updates)
- [Podman containers as systemd services (Quadlets)](kb:linux/podman-quadlets)
