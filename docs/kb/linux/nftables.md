# Write nftables rules by hand

nftables is the firewall underneath ufw and firewalld on current distros. Writing rules directly gives you one readable file, sets of addresses, and per-port counters. This article builds a small default-deny input policy that coexists with Docker, with an automatic rollback while you test.

## What you end up with

- `/etc/nftables.conf` with a default-drop input chain and explicit allows
- LAN-only access for admin ports, using a named set
- A rollback that restores the old rules in 2 minutes unless you cancel it

## Before you start

- Don't mix: if the host uses ufw or firewalld, manage it there (Nightgrid's firewall tab can, with rollback). Use plain nftables only on hosts without them.
- Keep a console (Proxmox, IPMI, PiKVM) handy for the first time.

## Part 1 · The rules file

```text
#!/usr/sbin/nft -f

table inet host {
    set lan {
        type ipv4_addr
        flags interval
        elements = { 192.168.1.0/24, 100.64.0.0/10 }
    }

    chain input {
        type filter hook input priority filter; policy drop;

        ct state established,related accept
        ct state invalid drop
        iif lo accept
        meta l4proto { icmp, ipv6-icmp } accept

        tcp dport 22 ip saddr @lan accept
        tcp dport { 80, 443 } accept
        udp dport 53 ip saddr @lan accept

        counter comment "dropped"
    }
}
```

- [ ] Save it as `/etc/nftables.conf.new`. It uses its own table and no `flush ruleset`, so Docker's own tables keep working.
- [ ] `sudo nft -c -f /etc/nftables.conf.new` checks the syntax without applying it.

## Part 2 · Apply with a rollback armed

```bash
sudo nft list ruleset > /root/nft-before.conf
sudo systemd-run --on-active=120 --unit nft-rollback \
  sh -c 'nft delete table inet host; nft -f /root/nft-before.conf'
sudo nft -f /etc/nftables.conf.new
```

- [ ] Open a **new** SSH session. If it works, cancel the rollback: `sudo systemctl stop nft-rollback.timer`.
- [ ] If you're locked out, wait two minutes; the old rules come back.
- [ ] Make it permanent: `sudo cp /etc/nftables.conf.new /etc/nftables.conf` and `sudo systemctl enable nftables`. Check that the stock `/etc/nftables.conf` didn't `flush ruleset` before you replaced it.

## Part 3 · Read and change

- [ ] `sudo nft list table inet host` shows the rules with counters. `sudo nft list set inet host lan` shows the set.
- [ ] Add an address live: `sudo nft add element inet host lan { 10.0.0.5 }` (then add it to the file too).

#### Check

- [ ] From outside the LAN set, port 22 times out; from inside it connects. The `dropped` counter grows with blocked traffic.

## In Nightgrid

- [System](view:system) → **firewall** shows hosts on plain nftables read-only, with the full ruleset under **nft list ruleset**. Edit the file in [Files](view:files) or the terminal, and use the rollback above.
- [Network](view:network) → **Tools** → **Ports** run from another host confirms what's open from that side.
- [System](view:system) → **config** → **Compare** shows changes to `/etc/nftables.conf` against your baseline.

## Limits and gotchas

- Docker publishes ports through its own chains, which run before yours for forwarded traffic: a published container port is reachable even if your input chain doesn't allow it. Publish on `127.0.0.1` or filter in the `DOCKER-USER` chain.
- `nft flush ruleset` in a config file deletes Docker's rules too; restart Docker after it if you use it.
- IPv6: the `@lan` set is IPv4 only. Add an `ipv6_addr` set and matching rules if the host has IPv6.

## Related

- [Change a firewall safely](kb:howto/firewall)
- [fail2ban and CrowdSec](kb:apps/fail2ban-crowdsec)
- [Find what's using a port, a file or the CPU](kb:linux/ports-processes)
