# Encrypt a disk with LUKS

Encrypt a data disk or a whole server so a stolen drive, a returned RMA disk or a sold machine gives nothing away. Unlock it at boot with a passphrase, a key file, or the TPM.

## What you end up with

- An encrypted data disk mounted at boot
- A backup of the LUKS header, so a damaged header doesn't mean lost data
- Optional unlock without typing, from the TPM or a key on the root disk

## Before you start

- `sudo apt install cryptsetup` (installed on most distros)
- An empty disk or partition: formatting erases it
- A password manager entry for the passphrase. There is no recovery without it.

## Part 1 · Create the encrypted volume

```sh
sudo cryptsetup luksFormat --type luks2 /dev/disk/by-id/ata-MODEL_SERIAL
sudo cryptsetup open /dev/disk/by-id/ata-MODEL_SERIAL data
sudo mkfs.ext4 -L data /dev/mapper/data
sudo mkdir -p /srv/data && sudo mount /dev/mapper/data /srv/data
```

- [ ] Type `YES` in capitals when asked. The passphrase you set goes into key slot 0.

## Part 2 · Back up the header

```sh
sudo cryptsetup luksHeaderBackup /dev/disk/by-id/ata-MODEL_SERIAL --header-backup-file luks-header-data.img
```

- [ ] Store the file off the machine, like a key: with it and the passphrase, a corrupted header can be restored.

## Part 3 · Unlock at boot

- [ ] Data disk, unlocked by a key file on the (already encrypted) root disk:

```sh
sudo dd if=/dev/urandom of=/etc/cryptsetup-keys.d/data.key bs=512 count=1
sudo chmod 600 /etc/cryptsetup-keys.d/data.key
sudo cryptsetup luksAddKey /dev/disk/by-id/ata-MODEL_SERIAL /etc/cryptsetup-keys.d/data.key
```

- [ ] Find the volume's UUID with `sudo cryptsetup luksUUID /dev/disk/by-id/ata-MODEL_SERIAL`, then add a line to `/etc/crypttab`:

```
data UUID=<luks-uuid> /etc/cryptsetup-keys.d/data.key luks,nofail
```

- [ ] And to `/etc/fstab`: `/dev/mapper/data /srv/data ext4 defaults,nofail 0 2`
- [ ] TPM unlock instead (systemd 248 or newer, with a TPM 2.0 chip): `sudo systemd-cryptenroll --tpm2-device=auto --tpm2-pcrs=7 /dev/...` and `tpm2-device=auto` in place of the key file in crypttab.

#### Check

- [ ] `sudo reboot`, then `lsblk -f` shows `crypto_LUKS` with `data` open under it and `/srv/data` mounted.

## In Nightgrid

- Encrypted volumes look like any other mount in [Fleet](view:fleet) → **Disks** once open, with the same full-in-N-days forecast.
- A server that waits for a passphrase at boot stays offline in Nightgrid until someone types it at the console (or over IPMI). Use a key file or the TPM for headless servers, or `dropbear-initramfs` to type it over SSH.

## Limits and gotchas

- Full-disk encryption protects a powered-off machine. A running, unlocked server is as readable as an unencrypted one.
- TPM unlock is tied to the boot chain (PCR 7 = Secure Boot state); a BIOS update can require the passphrase once.
- Never keep the only key file on the disk it unlocks.

## Related

- [Add a disk: partition, format and mount](kb:linux/disks-fstab)
- [Wipe a drive properly](kb:linux/secure-wipe)
- [Fix a server that won't boot](kb:linux/boot-rescue)
