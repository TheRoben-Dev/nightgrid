# Prove your backups restore

A backup you've never restored is a hope, not a backup. Set up restore drills once and Nightgrid restores something from each backup on a schedule, checks it and tells you when one fails.

## What you end up with

- A drill per backup job and per important Proxmox guest on [Fleet](view:fleet) → **DRILLS**
- A pass or fail for each run with every step written down, and a Markdown report to keep
- A notice when a drill fails

## Before you start

- Backup jobs on [Fleet](view:fleet) → **BACKUPS** ([Monitor backups and test restores](kb:howto/backups)). Restic, borg and folder-copy jobs can be tested.
- Drills run as root. Scheduled ones never ask for a password, so the host needs sudo without a password for your user, or **remember** ticked for its sudo password. A Proxmox host where you log in as root needs nothing more.
- For a Proxmox drill: a storage with room for one more copy of the guest's disks.

## Part 1 · Drill a backup job

- [ ] [Fleet](view:fleet) → **DRILLS** → **+ Drill** → **Backup job**, and pick the job.
- [ ] Choose how often: weekly or every 30 days is plenty for most homelabs.
- [ ] **Run now** once. It restores one random file under 20 MB from the latest snapshot into a temporary folder on the host, checks it and deletes the folder. The repository password never leaves the host.

## Part 2 · Drill a Proxmox guest

- [ ] **+ Drill** → **Proxmox guest**. Give the node, the guest's id, the storage its backups are on and a storage to restore to.
- [ ] **Run now**. Nightgrid restores the newest backup as a new guest, pulls its network cables (link down), removes passthrough devices and bind mounts so it can't touch real hardware or data, starts it and waits for the guest agent (or a command inside a container). Then it destroys the copy.
- [ ] Install `qemu-guest-agent` in VMs you drill: without it Nightgrid can only say the VM is running, not that its system booted.

#### Check

- [ ] The drill shows **PASSED** and its steps end with the copy destroyed.

## Part 3 · Keep the evidence

- [ ] **Report** saves every run of a drill as Markdown: when, how long, what was restored and each step.
- [ ] A failed drill sends a notice; route it to your phone with ntfy in **Settings → Alerts → Notifications**.

## Limits and gotchas

- A test restore proves the backup can be read, not that the app inside it works. Restore a whole stack now and then into a sandbox clone ([Day-to-day Proxmox from Nightgrid](kb:howto/proxmox)) and open it.
- The restored guest gets a 2 hour expiry timer on the node, so it's removed even if Nightgrid or the connection goes away mid-drill.
- Scheduled drills only run while Nightgrid is open and connected to the host; a missed one runs at the next chance.

## Related

- [Monitor backups and test restores](kb:howto/backups)
- [Day-to-day Proxmox from Nightgrid](kb:howto/proxmox)
