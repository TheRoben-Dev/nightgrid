# Inventory a machine's hardware

Find out exactly what is in a box without opening it: board, BIOS version, CPU, memory slots, drives, NICs and their drivers. Useful for repairs, listings, RMAs and before upgrades.

## What you end up with

- One text report per machine with model, serials and firmware versions
- Which memory slots are free and what speed they run at
- Which kernel driver each device uses

## Before you start

- `sudo apt install lshw dmidecode pciutils usbutils inxi nvme-cli` (Fedora names are the same)

## Part 1 · The quick summary

```sh
inxi -Fxxz
```

- [ ] `-z` hides serials and MACs, handy before pasting in a forum. Drop it for your own records.

## Part 2 · Board, BIOS and memory

```sh
sudo dmidecode -t system -t baseboard -t bios | grep -E "Manufacturer|Product|Version|Serial|Release Date"
sudo dmidecode -t memory | grep -E "Locator|Size|Speed|Type:|Part Number" | grep -v Bank
```

- [ ] "No Module Installed" lines are free slots. **Configured Memory Speed** below **Speed** means XMP/EXPO is off or the CPU can't run faster.

## Part 3 · Devices and drivers

```sh
lspci -nnk | grep -A3 -E "Ethernet|Network|VGA|3D|SATA|NVMe|RAID"
lsusb -t
```

- [ ] `Kernel driver in use:` shows the driver; no such line means nothing claimed the device (missing firmware or driver).
- [ ] The `[8086:15f3]` style IDs are what to search for when a device has no driver.

## Part 4 · Drives and firmware

```sh
lsblk -d -o NAME,SIZE,MODEL,SERIAL,ROTA,TRAN
sudo nvme list
sudo smartctl -i /dev/sda | grep -E "Model|Serial|Firmware"
```

## Part 5 · One report to keep

```sh
sudo lshw -short > hw-$(hostname).txt
sudo lshw -sanitize -html > hw-$(hostname).html
```

#### Check

- [ ] You can answer from the report: which BIOS version, how many free RAM slots, which NIC chip, and every drive's serial.

## In Nightgrid

- [System](view:system) shows the OS, kernel and uptime for each host; this report adds the hardware underneath.
- Run Part 5 on every host at once with a snippet in [Automation](view:automation), then open the files in [Files](view:files).
- Paste the summary into the host's [Knowledge](view:notes) note so DRIFT can answer "which NIC does the NAS have?".

## Limits and gotchas

- VMs report the hypervisor's virtual hardware, not the real board.
- Some cheap boards fill DMI with "To Be Filled By O.E.M.": go by the board's silkscreen instead.
- `dmidecode` and serials need root.

## Related

- [Burn in a server: CPU, memory and stability](kb:linux/burn-in)
- [Test a drive before you trust it](kb:linux/disk-testing)
- [Read fans and sensors](kb:linux/sensors-fans)
