# Upgrade the homelab to 10 GbE

![Fast links between the machines that need them.](fig:guide-10gbe-upgrade)

Move the busiest links (NAS to server, server to workstation) from 1 GbE to 10 GbE or 2.5 GbE without replacing everything: pick the right hardware, cable it, tune it, and prove the gain with measurements before and after.

## What you end up with

- Measured throughput before and after, between the hosts that matter
- 10 GbE (or 2.5 GbE) on the links that carry big transfers
- NICs and switch ports configured, with jumbo frames only where they help

## Before you start

- Know what you want faster. Copying to a NAS whose disks write 200 MB/s won't get faster than 200 MB/s on any network. One HDD tops out around 1.5–2 Gbit/s.
- Iperf3 installed on the hosts involved (`sudo apt install iperf3`)

## Part 1 · Measure today

- [ ] [Fleet](view:fleet) → **Speed**: run tests between each pair you care about (NAS ↔ server, server ↔ workstation). Note both directions.
- [ ] Note real file copy speeds too: a big file copied over SMB or NFS, and the disks' own speed (`fio` or a simple `dd` to the pool).
- [ ] If iperf3 already shows ~940 Mbit/s and the disks are faster than that, the network is your limit and the upgrade will help.

## Part 2 · Pick the hardware

- [ ] **SFP+ with DAC cables**: cheapest and coolest for short runs in a rack (used Intel X520/X710 or Mellanox ConnectX-3/4 cards, a small SFP+ switch, passive DAC cables up to 3–5 m).
- [ ] **10GBASE-T (RJ45)**: works over existing Cat6/Cat6a in walls, but cards and SFP+ RJ45 modules run hot and use more power.
- [ ] **2.5 GbE**: many new boards have it already; cheap switches and USB adapters. A good middle step for workstations and mini PCs.
- [ ] Check the card has a PCIe slot with enough lanes (x4 or x8 for most 10 GbE cards) and that Linux has a driver (`ixgbe`, `i40e`, `mlx4_en`/`mlx5_core` are all in-kernel).

## Part 3 · Install and check the link

- [ ] After installing a card: `lspci -nnk | grep -A3 Ethernet` shows the driver; `ip link` shows the new interface.
- [ ] Check the negotiated speed: `ethtool enp3s0 | grep -E "Speed|Duplex|Link detected"`.
- [ ] Give it an address on the same subnet (or a point-to-point subnet if two machines are cabled directly): see [Give a server a static IP](kb:linux/static-ip).
- [ ] SFP+ modules from another brand may be refused by Intel cards; `ixgbe` has an `allow_unsupported_sfp=1` module option if that happens.

## Part 4 · Tune

- [ ] Run [Fleet](view:fleet) → **Speed** again. Expect 9.4 Gbit/s on a clean 10 GbE link with iperf3's default single stream on modern CPUs; older CPUs may need parallel streams.
- [ ] Jumbo frames (MTU 9000) help older CPUs and NFS, but every device on that network segment must agree. Try it only on a dedicated storage VLAN or a direct link:

```sh
sudo ip link set enp3s0 mtu 9000
ping -M do -s 8972 10.10.10.2
```

- [ ] Make the MTU permanent in netplan (`mtu: 9000`) or NetworkManager once it works.

## Part 5 · Real-world transfers

- [ ] Copy the same big file as in Part 1. SMB: enable multichannel on Samba (`server multi channel support = yes`) for Windows clients. NFS: `nconnect=4` on the mount helps with many streams.
- [ ] If copies stay slow while iperf3 is fast, the disks or the protocol are the limit now: look at the pool layout or add an SSD cache.

#### Check

- [ ] Fleet → Speed shows the new numbers for each pair, and a large copy runs at the disk's speed rather than at ~110 MB/s.

## When Nightgrid is closed

- Nothing here runs in the background; the links and MTU settings are host configuration.

## Limits and gotchas

- 10GBASE-T SFP+ modules get very hot in small switches; leave a gap between them or use DAC where you can.
- A 10 GbE link through a 1 GbE switch port negotiates down to 1 GbE. Check every hop with `ethtool`.
- Mixing MTUs on one segment causes strange stalls (small pings work, big transfers hang).

## Related

- [Find slow links with iperf3](kb:howto/speed-test)
- [VLANs and bonded NICs on a server](kb:linux/vlans-bonds)
- [Build a DIY NAS](kb:guides/diy-nas)
- [LibreSpeed LAN speed test](kb:apps/librespeed)
