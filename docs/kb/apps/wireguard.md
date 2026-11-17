# WireGuard VPN server

Run a plain WireGuard server on a Linux host so phones and laptops can reach your LAN from anywhere, with one UDP port forwarded and nothing else exposed.

## What you end up with

- `wg0` on the server with a private VPN subnet
- One config per device, phones set up with a QR code
- Access to your LAN through the server

## Before you start

- A Linux host on your LAN, a forwarded UDP port (51820) on the router to it, and a stable public address or dynamic DNS name.
- Prefer zero port forwards? Tailscale or Headscale do the same with NAT traversal (see [Tailscale and WireGuard](kb:basics/tailscale-wireguard)).

## Part 1 · Server

- [ ] `sudo apt install -y wireguard qrencode`.
- [ ] Keys:

```sh
wg genkey | sudo tee /etc/wireguard/server.key | wg pubkey | sudo tee /etc/wireguard/server.pub
sudo chmod 600 /etc/wireguard/server.key
```

- [ ] `/etc/wireguard/wg0.conf` (replace `eth0` with your LAN interface from `ip -br a`):

```ini
[Interface]
Address = 10.8.0.1/24
ListenPort = 51820
PrivateKey = <server private key>
PostUp = nft add table ip wgnat; nft add chain ip wgnat post { type nat hook postrouting priority 100 \; }; nft add rule ip wgnat post oifname "eth0" ip saddr 10.8.0.0/24 masquerade
PostDown = nft delete table ip wgnat

[Peer]
# phone
PublicKey = <phone public key>
AllowedIPs = 10.8.0.2/32
```

- [ ] Enable forwarding: `echo net.ipv4.ip_forward=1 | sudo tee /etc/sysctl.d/99-wg.conf && sudo sysctl --system`.
- [ ] `sudo systemctl enable --now wg-quick@wg0`.
- [ ] Firewall: [System](view:system) → **FIREWALL** → **Add…** allow 51820/udp from anywhere.

## Part 2 · A device

- [ ] Make keys for the device (on the server is fine for a phone), and a config:

```ini
[Interface]
PrivateKey = <phone private key>
Address = 10.8.0.2/32
DNS = 192.168.1.2

[Peer]
PublicKey = <server public key>
Endpoint = vpn.example.com:51820
AllowedIPs = 10.8.0.0/24, 192.168.1.0/24
PersistentKeepalive = 25
```

- [ ] Show it as a QR code for the WireGuard app: `qrencode -t ansiutf8 < phone.conf`. Delete the file afterwards.
- [ ] Add the device's `[Peer]` block to the server and `sudo systemctl reload wg-quick@wg0` (or `wg syncconf`).

#### Check

- [ ] On mobile data, the phone reaches a LAN service. `sudo wg show` lists a recent handshake.

## Part 3 · In Nightgrid

- [ ] The [Network](view:network) map shows the forwarded UDP port as a way in from the internet. That's expected for this one.
- [ ] A command widget for connected peers: `wg show wg0 latest-handshakes | awk '$2>systime()-180' | wc -l` (needs a login allowed to run `wg`, otherwise skip).

## Limits and gotchas

- `AllowedIPs` on the device decides what goes through the tunnel. `0.0.0.0/0` sends everything, which is slower but safer on public Wi-Fi.
- Two sites with the same LAN subnet can't route to each other; renumber one.
- If you prefer a web UI for peers, wg-easy wraps the same thing in a container.

## Related

- [Tailscale and WireGuard](kb:basics/tailscale-wireguard)
- [Tailscale subnet router](kb:apps/tailscale-subnet-router)
- [Reach your servers away from home](kb:howto/remote-access)
