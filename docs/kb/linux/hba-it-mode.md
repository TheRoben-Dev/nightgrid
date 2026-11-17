# HBA cards in IT mode for a NAS

A used LSI/Broadcom SAS HBA adds 8 or more SATA/SAS ports to a NAS or server cheaply. For ZFS, mdadm, SnapRAID or Unraid it must run in **IT mode** (plain pass-through), not RAID mode, so the OS sees every disk directly with SMART. This article helps you pick one, check what you got, and keep it cool.

![RAID firmware versus IT mode.](fig:lx-hba-it)

## What you end up with

- A card the OS sees as a plain HBA with every disk and its SMART data
- The right cables and enough airflow
- A firmware check so you know the card is in IT mode

## Part 1 · Pick a card

- [ ] Common, well-supported models (driver in every distro kernel):
  - **SAS2008** (9211-8i, IBM M1015, Dell H310 crossflashed): 6 Gb/s, PCIe 2.0, fine for HDDs, mpt3sas driver
  - **SAS2308** (9207-8i, 9217-8i): 6 Gb/s, PCIe 3.0, good value
  - **SAS3008** (9300-8i, Dell HBA330): 12 Gb/s, PCIe 3.0, runs hot
  - **SAS3408/3416** (9400-8i/16i): tri-mode, lower power, more expensive
- [ ] Buy one sold as "IT mode" from a seller who says so; crossflashing a RAID card yourself is possible but fiddly.
- [ ] `-8i` means 8 internal lanes on two **SFF-8087** (SAS2) or **SFF-8643** (SAS3) connectors. Each connector takes a breakout cable to 4 SATA drives, or one cable to a backplane.

## Part 2 · Check what you have

```bash
lspci | grep -i -E 'lsi|broadcom|sas'
sudo dmesg | grep -i mpt
lsblk -o NAME,SIZE,MODEL,SERIAL
sudo smartctl -a /dev/sdb | head -20
```

- [ ] `mpt2sas`/`mpt3sas` in `dmesg` with `Protocol=(Initiator)` and a firmware line ending in **IT** means IT mode. A `megaraid_sas` driver means RAID (IR/MegaRAID) firmware.
- [ ] Every disk should appear with its own model and serial, and `smartctl` should read it without extra flags.
- [ ] Note the firmware version from `dmesg`. For SAS2008/2308, **P20.00.07.00** is the version to have; older P20 builds had data-corruption bugs.

## Part 3 · Cooling and power

- [ ] These cards expect server airflow. In a desktop or NAS case the heatsink gets too hot to touch and the card can drop disks under load. Point a fan at it (a 40 mm fan zip-tied to the heatsink works well).
- [ ] They draw 8–15 W and stop the system from reaching deep idle states (C-states), so expect a few watts more at idle than onboard SATA.
- [ ] Turn off the card's boot ROM in its BIOS setup if you don't boot from it; it speeds up boot.

#### Check

- [ ] A long copy or a scrub runs without `mpt3sas` resets or timeouts in `dmesg`, and every disk keeps its SMART readings.

## In Nightgrid

- [Fleet](view:fleet) → **Disks** reads SMART through the HBA like onboard ports, and shows pool health during scrubs.
- A saved search for `mpt3sas|mpt2sas` with `reset|timeout` in [Fleet](view:fleet) → **Logs** with ◉ on alerts you to an overheating or failing card.

## Limits and gotchas

- RAID-mode cards (and "JBOD mode" on many RAID cards) hide SMART and can rewrite disk headers. Use true IT firmware for ZFS.
- Some consumer boards refuse to boot with these cards in certain slots; try another slot or disable the option ROM.
- Breakout cables come in forward and reverse versions. You want **forward** (HBA to drives).

## Related

- [Build a DIY NAS](kb:guides/diy-nas)
- [Test a drive before you trust it](kb:linux/disk-testing)
- [ZFS pools, datasets and snapshots](kb:apps/zfs)
