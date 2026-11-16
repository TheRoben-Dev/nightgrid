# Give a server a static IP

Servers need addresses that don't move. Set one with netplan (Ubuntu), NetworkManager (Fedora, Debian desktop, many others) or `/etc/network/interfaces` (Debian server), or reserve it in the router.

## What you end up with

- A fixed LAN address the rest of your network (and Nightgrid) can rely on
- DNS servers set explicitly

## Before you start

- Easiest option: a DHCP reservation in the router for the server's MAC. Nothing changes on the server. Use the steps below only if you want the server to set it.
- Pick an address outside the router's DHCP range.
- Have console access, or use a rollback (below): a typo drops SSH.

## Part 1 · Ubuntu (netplan)

- [ ] `/etc/netplan/50-static.yaml` (mode 600):

```yaml
network:
  version: 2
  ethernets:
    eth0:
      dhcp4: false
      addresses: [192.168.1.10/24]
      routes:
        - to: default
          via: 192.168.1.1
      nameservers:
        addresses: [192.168.1.2, 9.9.9.9]
```

- [ ] Replace `eth0` with the name from `ip -br link`.
- [ ] `sudo netplan try` applies it and rolls back after 120 seconds unless you press Enter. Reconnect from Nightgrid on the new address before confirming if the address changed.

## Part 2 · NetworkManager

```sh
nmcli con show
sudo nmcli con mod "<connection>" ipv4.method manual ipv4.addresses 192.168.1.10/24 ipv4.gateway 192.168.1.1 ipv4.dns "192.168.1.2 9.9.9.9"
sudo nmcli con up "<connection>"
```

## Part 3 · Debian ifupdown

- [ ] `/etc/network/interfaces`:

```
auto eth0
iface eth0 inet static
    address 192.168.1.10/24
    gateway 192.168.1.1
    dns-nameservers 192.168.1.2 9.9.9.9
```

- [ ] `sudo systemctl restart networking` (from a console, or schedule a revert first with `sudo systemd-run --on-active=3m cp /root/interfaces.bak /etc/network/interfaces`).

## Part 4 · In Nightgrid

- [ ] Update the host's address in the host editor if it changed.
- [ ] The [Network](view:network) map shows the new address after the next scan.

#### Check

- [ ] `ip -br a` shows the address, and `ping -c2 9.9.9.9` and `ping -c2 example.com` work.

## Limits and gotchas

- Docker's default bridge uses `172.17.0.0/16`; avoid that range for your LAN.
- Cloud-init on some images rewrites netplan on boot; disable its network config with `/etc/cloud/cloud.cfg.d/99-disable-network-config.cfg` containing `network: {config: disabled}`.

## Related

- [When a host won't connect](kb:howto/troubleshoot-connection)
- [Map your network and find exposed ports](kb:howto/network-map)
