# Find slow links with iperf3

Measure throughput and latency between two of your hosts, keep a history per pair, and catch a link that quietly dropped to 100 Mbit/s.

![An iperf3 test between two of your hosts.](fig:ng-iperf)

## What you end up with

- Speed both ways and ping latency between any two hosts
- A history line per pair, with slow runs called out
- A quick way to tell a bad cable from a slow disk

## Before you start

- `iperf3` on both hosts: `sudo apt install iperf3` (say no to running it as a daemon; Nightgrid starts the server only for the test).
- The target's firewall must allow the source on TCP port 5201.

## Part 1 · Run a test

- [ ] [Fleet](view:fleet) → **Speed**, pick two hosts.
- [ ] Enter the address the source should use for the target (usually its LAN IP, not the tailnet one, unless you want to test the tailnet).
- [ ] **Run test**.

## Part 2 · Read it

- [ ] About 940 Mbit/s on gigabit and 2.3 Gbit/s on 2.5 GbE is normal.
- [ ] About 100 Mbit/s is flagged: a cable, a port or a switch negotiated Fast Ethernet. Check with `ethtool <nic> | grep Speed`.
- [ ] Fast one way, slow the other: often a NIC driver or offload problem, or Wi-Fi on one end.

#### Check

- [ ] Each pair you care about has a baseline run you can compare against later.

## Limits and gotchas

- Network speed isn't file-copy speed. If iperf3 is fast but copies are slow, look at the disks or the protocol (SMB signing, NFS sync).
- Testing over the tailnet measures the VPN and the internet path, not your LAN.

## Related

- [Map your network and find exposed ports](kb:howto/network-map)
- [Samba file shares](kb:apps/samba)
- [NFS shares](kb:apps/nfs)
