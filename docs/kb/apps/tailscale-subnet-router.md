# Tailscale subnet router

Make one always-on Linux machine share your home LAN with your tailnet, so everything at home is reachable from your devices (and from Nightgrid) without installing Tailscale on each server.

![A subnet router lets tailnet devices reach the whole LAN.](fig:tailnet-subnet)

## What you end up with

- LAN addresses like `192.168.1.20` reachable from anywhere on your tailnet
- No ports forwarded on the router
- Nightgrid hosts saved with LAN addresses that work at home and away

## Part 1 · Install and advertise

- [ ] Install Tailscale with its install script or from its apt repository, then:

```sh
echo 'net.ipv4.ip_forward = 1' | sudo tee /etc/sysctl.d/99-tailscale.conf
echo 'net.ipv6.conf.all.forwarding = 1' | sudo tee -a /etc/sysctl.d/99-tailscale.conf
sudo sysctl -p /etc/sysctl.d/99-tailscale.conf
sudo tailscale up --advertise-routes=192.168.1.0/24
```

- [ ] Replace the subnet with yours (`ip -br a`).
- [ ] In the Tailscale admin console, open the machine → Edit route settings → approve the route. Turn off key expiry for this machine.

## Part 2 · Use it

- [ ] Linux clients need `--accept-routes`; macOS, Windows, iOS and Android accept routes by default.
- [ ] In Nightgrid: **Settings → Connections → Built-in Tailscale** with **Use subnet routes** on (see [Reach your servers away from home](kb:howto/remote-access)).

#### Check

- [ ] On mobile data, open a terminal in Nightgrid on a host saved with its LAN IP.

## Part 3 · Optional extras

- [ ] Exit node: add `--advertise-exit-node` to route all traffic through home on public Wi-Fi.
- [ ] Use your home DNS (Pi-hole/AdGuard) on the tailnet: admin console → DNS → add a nameserver with your DNS server's LAN IP and "override local DNS".

## Limits and gotchas

- If the router machine goes down, the whole LAN is unreachable remotely. A second subnet router for the same route gives failover.
- On a Proxmox LXC, the container needs `/dev/net/tun`; a small VM is simpler.
- Overlapping subnets (home and the café both `192.168.1.0/24`) confuse routing; give home an unusual range like `192.168.47.0/24`.

## Related

- [Reach your servers away from home](kb:howto/remote-access)
- [Run Headscale](kb:apps/headscale)
- [Tailscale and WireGuard](kb:basics/tailscale-wireguard)
