# A floating IP with keepalived

Give a service one address that moves between two servers: if the main one dies, the backup takes over the IP within seconds and clients never notice. The classic homelab use is two DNS servers (Pi-hole, AdGuard, Technitium) behind one DNS address handed out by DHCP.

## What you end up with

- A virtual IP (VIP) such as `192.168.1.53` owned by one of two hosts
- A health check that moves the VIP when the service stops answering, not only when the host dies
- Failover in about 3 seconds, and automatic failback

## Before you start

- Two hosts on the same LAN segment, each running the service (for example two Pi-holes with the same settings).
- A free address outside the DHCP range for the VIP.

## Part 1 · Install

```bash
sudo apt install keepalived dnsutils
```

## Part 2 · Main host

- [ ] `/etc/keepalived/keepalived.conf`:

```text
global_defs {
    enable_script_security
    script_user root
}

vrrp_script chk_dns {
    script "/usr/bin/dig +short +time=1 +tries=1 @127.0.0.1 example.com"
    interval 5
    fall 2
    rise 2
}

vrrp_instance DNS {
    state MASTER
    interface eth0
    virtual_router_id 53
    priority 150
    advert_int 1
    authentication {
        auth_type PASS
        auth_pass Dns53vip
    }
    virtual_ipaddress {
        192.168.1.53/24
    }
    track_script {
        chk_dns
    }
}
```

- [ ] Replace `eth0` with the interface from `ip -br addr`. `auth_pass` is at most 8 characters and identical on both hosts.

## Part 3 · Backup host

- [ ] Same file, with `state BACKUP` and `priority 100`. Everything else, including `virtual_router_id`, stays identical.
- [ ] Start both: `sudo systemctl enable --now keepalived`.

## Part 4 · Point clients at the VIP

- [ ] Set the router's DHCP DNS server to `192.168.1.53`. Optionally hand out a second DNS too (the backup's real IP) for belt and braces.
- [ ] If the service binds to specific addresses, make sure it also answers on the VIP (Pi-hole: listen on all interfaces).

#### Check

- [ ] `ip addr` shows `192.168.1.53` on the main host. Stop the DNS service there: within about 10 seconds the VIP moves to the backup and `dig @192.168.1.53 example.com` still answers. Start it again and the VIP moves back.

## In Nightgrid

- Add the VIP as a DNS lookup in [Network](view:network) → **Tools** → **DNS** (**Server** `192.168.1.53`) to test from other machines, and keep a [Services](view:services) check on each real host so you know when one is down even though clients are fine.
- Both hosts' `keepalived.conf` should match except `state` and `priority`: [System](view:system) → **config** → **Compare with** shows the difference.
- The VIP moving is logged in the journal (`Entering MASTER STATE`); a saved search for it in [Fleet](view:fleet) → **Logs** with ◉ on alerts you on every failover.

## Limits and gotchas

- VRRP uses multicast. Some Wi-Fi bridges, cloud networks and Proxmox firewall rules drop it: use `unicast_src_ip` and `unicast_peer` instead.
- Both hosts claiming the VIP ("split brain") means they can't hear each other's adverts; check the firewall allows protocol `vrrp` (112).
- keepalived moves an address, not data. The two services must hold the same configuration (Pi-hole: Teleporter export, or a sync tool).

## Related

- [Pi-hole](kb:apps/pihole)
- [Technitium DNS Server](kb:apps/technitium-dns)
- [Give a server a static IP](kb:linux/static-ip)
