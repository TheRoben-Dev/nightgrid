# Cockpit web console

Red Hat's web admin console for a single Linux server: services, logs, storage, networking, updates and a terminal, in a browser. Handy for people who won't use Nightgrid.

![Cockpit's web console on a host.](fig:app-cockpit)

## Part 1 · Install

```sh
sudo apt install -y cockpit        # or: sudo dnf install cockpit
sudo systemctl enable --now cockpit.socket
```

- [ ] It listens on port 9090 with a self-signed certificate. Reach it through a tunnel instead of opening it: [Tunnels](view:tunnels) → **OPEN WEB UI VIA** the host, `localhost:9090`.
- [ ] Optional modules: `cockpit-machines` (VMs), `cockpit-podman`, and community modules for files and shares.

#### Check

- [ ] You log in with a normal Linux account and see the host's overview.

## Limits and gotchas

- Cockpit manages one host at a time (with a host switcher). Nightgrid covers fleets, Docker stacks, alerts and history; the two don't conflict.
- Don't expose 9090 to the internet.

## Related

- [Reach admin web UIs through SSH tunnels](kb:howto/tunnels)
- [Virtual machines with KVM and libvirt](kb:apps/libvirt-kvm)
