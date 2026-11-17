# Wipe a drive properly

Before a drive is sold, returned, sent for RMA or reused for someone else, erase it so nothing comes back. The right method depends on whether it's an SSD, an NVMe drive or a spinning disk.

![The right wipe for each kind of drive.](fig:lx-secure-wipe)

## What you end up with

- A drive with no recoverable data, using the drive's own erase where it has one
- A quick check that it really reads back empty

## Before you start

- Boot the machine from a live USB if the drive is the system disk
- `sudo apt install nvme-cli hdparm util-linux`
- Triple-check the device path: `lsblk -d -o NAME,SIZE,MODEL,SERIAL`

## Part 1 · NVMe SSDs

- [ ] See what the drive supports:

```sh
sudo nvme id-ctrl /dev/nvme0 -H | grep -E "Format|Crypto|Sanitize"
```

- [ ] Preferred, if listed: sanitize with block erase (`--sanact=2`) or crypto erase (`--sanact=4`):

```sh
sudo nvme sanitize /dev/nvme0n1 --sanact=2
sudo nvme sanitize-log /dev/nvme0n1
```

- [ ] Otherwise a format with secure erase (`--ses=1` user data, `--ses=2` crypto):

```sh
sudo nvme format /dev/nvme0n1 --ses=1
```

## Part 2 · SATA SSDs

- [ ] Check the drive isn't "frozen" (most BIOSes freeze it at boot; suspend and resume, or hot-plug the drive, to unfreeze):

```sh
sudo hdparm -I /dev/sdX | grep -A8 "^Security"
```

- [ ] Set a temporary password and issue the ATA secure erase:

```sh
sudo hdparm --user-master u --security-set-pass wipe /dev/sdX
sudo hdparm --user-master u --security-erase wipe /dev/sdX
```

- [ ] Quick alternative when you only need the data unreadable to normal tools: `sudo blkdiscard -f /dev/sdX` (TRIM every block).

## Part 3 · Hard drives

- [ ] One pass of zeros is enough for modern drives:

```sh
sudo dd if=/dev/zero of=/dev/sdX bs=4M status=progress oflag=direct
```

- [ ] Or random data, if a buyer's tool would flag an all-zero disk as suspicious: `sudo shred -v -n 1 /dev/sdX`.
- [ ] Hours per terabyte; run it in tmux.

#### Check

- [ ] Spot check that the start, middle and end read back blank:

```sh
sudo dd if=/dev/sdX bs=1M count=8 status=none | hexdump -C | head
sudo wipefs /dev/sdX
```

- [ ] `wipefs` lists no signatures.

## In Nightgrid

- Run the wipe in a [Terminal](view:terminal) tab inside tmux, so closing the laptop doesn't stop it.
- If the drive was in a monitored pool, remove it from the pool first; [Fleet](view:fleet) → **Disks** will show the pool as degraded until you do.

## Limits and gotchas

- Overwriting an SSD with `dd` does not reach spare and remapped blocks. Use the drive's own erase.
- Self-encrypting drives: crypto erase is near-instant and the strongest option.
- A drive that fails mid-erase and can't be finished should be physically destroyed, not sent out.

## Related

- [Test a drive before you trust it](kb:linux/disk-testing)
- [Encrypt a disk with LUKS](kb:linux/luks)
- [tmux: keep sessions running](kb:linux/tmux)
