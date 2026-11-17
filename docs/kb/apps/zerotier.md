# ZeroTier virtual network

A virtual LAN across sites and devices: every member gets an address on the same network, peer to peer through NAT. An alternative to Tailscale when you want layer 2 (broadcasts, some LAN games) or a self-hosted controller.

![Members, the controller and a route into the LAN.](fig:app-overlay-vpn)

## What you end up with

- Servers, laptops and phones on one private network with fixed addresses
- Members you approve one by one
- Optional routes into a home LAN through one member

## Part 1 · Network and first member

- [ ] Create a network in ZeroTier Central (the hosted controller) and note the 16-character network ID. Self-hosting the controller is possible (for example ZTNET) but more work.
- [ ] On each Linux server:

```sh
curl -s https://install.zerotier.com | sudo bash
sudo zerotier-cli join <network-id>
sudo zerotier-cli listnetworks
```

- [ ] Approve the member in Central (tick **Auth**) and give it a name. It gets an address from the network's range.

## Part 2 · Reach the home LAN

- [ ] On a home server that's a member: enable forwarding (`net.ipv4.ip_forward=1` in `/etc/sysctl.d/99-zt.conf`, then `sudo sysctl --system`) and NAT from the ZeroTier interface to the LAN.
- [ ] In Central → Managed Routes: add `192.168.1.0/24` via that server's ZeroTier address.

#### Check

- [ ] From a laptop on mobile data, `ping` a server's ZeroTier address, then a LAN address behind the router member.

## In Nightgrid

- Add hosts by their ZeroTier addresses; Nightgrid connects over them like any other route, using this computer's ZeroTier client.
- Nightgrid's built-in Tailscale doesn't cover ZeroTier. If you already run Tailscale, you rarely need both; pick one per host.

## Limits and gotchas

- Free tier limits apply on the hosted controller (members and networks); check the current plan.
- Layer 2 means broadcast traffic crosses the network; keep big networks routed instead.
- ZeroTier's interface names (`ztxxxxxxxx`) change per network; firewall rules should match `zt+`.

## Related

- [Tailscale and WireGuard](kb:basics/tailscale-wireguard)
- [Reach your servers away from home](kb:howto/remote-access)
- [WireGuard VPN server](kb:apps/wireguard)
