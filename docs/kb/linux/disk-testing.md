# Test a drive before you trust it

New, used or returned, every drive gets the same treatment before it holds data: read its SMART history, run the long self-test, and for spinning disks, write and read every sector once.

![The drive test sequence.](fig:lx-disk-health)

## What you end up with

- A pass or fail you can write on the drive (or the RMA form)
- SMART numbers saved before and after, to compare later
- No surprises a month after the drive goes into a pool

## Before you start

- A Linux box with the drive attached (SATA, or a USB dock that passes SMART through; most UAS docks do with `-d sat`)
- `sudo apt install smartmontools` (or `dnf install smartmontools`)
- Be sure which `/dev/sdX` is the test drive: the write test erases it

## Part 1 · Identify the drive

- [ ] List drives with model and serial, so you don't test the wrong one:

```sh
lsblk -d -o NAME,SIZE,MODEL,SERIAL,ROTA,TRAN
ls -l /dev/disk/by-id/ | grep -v part
```

- [ ] Use the `/dev/disk/by-id/...` path from here on: it can't change between reboots like `/dev/sdb` can.

## Part 2 · Read SMART

```sh
sudo smartctl -x /dev/disk/by-id/ata-MODEL_SERIAL | tee smart-before.txt
```

- [ ] **Power_On_Hours**: tells you a "new" drive isn't. Many vendors also log it in `-x` output as the device statistics page.
- [ ] **Reallocated_Sector_Ct**, **Current_Pending_Sector**, **Offline_Uncorrectable**: anything above 0 on a drive you just got is a reason to return it.
- [ ] **UDMA_CRC_Error_Count**: cable or backplane trouble, not the disk. Reseat or swap the cable.
- [ ] NVMe: look at **Media and Data Integrity Errors**, **Percentage Used** and **Available Spare**.

## Part 3 · Long self-test

```sh
sudo smartctl -t long /dev/disk/by-id/ata-MODEL_SERIAL
sudo smartctl -c /dev/disk/by-id/ata-MODEL_SERIAL | grep -A1 "Extended self-test"
```

- [ ] The second line says how many minutes it takes (hours for big HDDs). The test runs inside the drive; the machine can do other work.
- [ ] Some USB docks spin the drive down mid-test. Keep it busy with `watch -n 60 smartctl -A /dev/...` or test on SATA.
- [ ] Read the result: `sudo smartctl -l selftest /dev/...`. "Completed without error" is the only pass.

## Part 4 · Full surface test (HDDs only, destructive)

A write pass on every sector makes weak sectors show up now instead of under your data.

- [ ] Run it in tmux, it takes about a day per 8 TB per pass:

```sh
sudo badblocks -b 4096 -wsv -o badblocks.txt /dev/disk/by-id/ata-MODEL_SERIAL
```

- [ ] `-w` writes four patterns and reads each back. Drives over 16 TB need `-b 8192` or larger (badblocks counts in 32-bit blocks).
- [ ] Skip this on SSDs: it burns write endurance and tells you little. A long SMART test plus a full read (`sudo dd if=/dev/... of=/dev/null bs=4M status=progress`) is enough.

## Part 5 · Compare

```sh
sudo smartctl -x /dev/disk/by-id/ata-MODEL_SERIAL > smart-after.txt
diff smart-before.txt smart-after.txt | grep -i -E "realloc|pending|uncorrect|crc"
```

#### Check

- [ ] `badblocks.txt` is empty, the self-test passed, and reallocated and pending counts didn't move.

## In Nightgrid

- Run all of this in a [Terminal](view:terminal) tab on the host; tmux keeps it running if the app closes (see [tmux for long jobs](kb:linux/tmux)).
- Save the before and after reports in a note in [Knowledge](view:notes) titled with the serial, for warranty claims later.
- Once the drive is in service, [Fleet](view:fleet) → **Disks** shows its SMART health with the rest.

## Limits and gotchas

- SMART "PASSED" only means no attribute crossed the vendor's failure threshold. Read the raw values.
- SMR drives slow to a crawl during long write tests and in ZFS resilvers. Check the model before buying for a pool.
- Some USB bridges hide SMART completely; try `-d sat`, `-d sat,12` or `-d usbjmicron`.

## Related

- [Keep an eye on disks and pools](kb:howto/disks)
- [Burn in a server: CPU, memory and stability](kb:linux/burn-in)
- [Wipe a drive properly](kb:linux/secure-wipe)
- [Scrutiny SMART dashboard](kb:apps/scrutiny)
