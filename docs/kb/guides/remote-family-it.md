# Remote family IT

Look after your family's machines (a parent's Linux mini PC, a sibling's NAS, a Mac or two) from your own desk, without installing a remote-support agent on any of them. Everything goes through SSH over your tailnet. Nightgrid gives you their updates, disk health, backups and logins on one screen, and you only connect when there's something to fix.

## What you end up with

- Every family device reachable from anywhere over Tailscale, with one subnet router per house for the devices that can't run Tailscale themselves.
- One host group per household, so "upgrade grandma's house" is one batch with one confirmation.
- Pending updates, SMART health, disk fill-up forecasts and backup freshness for all of them, checked when you ask.
- A tested restore, not just a backup that "probably ran".
- A login audit and a key rotation you can run when someone's laptop goes missing.
- A read-only phone status page you can glance at.

## Before you start

- Permission from the people whose machines these are. You're getting root on their stuff, so tell them what you can see.
- SSH on each target: Linux (`openssh-server`), macOS (System Settings → General → Sharing → Remote Login), NAS boxes (their SSH service).
- Windows PCs need OpenSSH Server (an optional Windows feature). Terminal and Files work against them; most other views expect a Linux shell and won't show anything useful.
- A Tailscale account. Nightgrid's built-in client (Settings → **Tailnet**) means your own computer doesn't need the Tailscale app.
- Settings → **FEATURES**: **Standard** covers this guide except UPS. Turn on UPS if a household has one.

## Part 1 · Reach every house

- [ ] Install Tailscale on each device that can run it (Linux, macOS, most NAS boxes have a package).
- [ ] In each house, pick one always-on Linux device as a subnet router so printers, cameras and the router itself are reachable too. Advertise the house LAN on it:

```sh
sudo tailscale up --advertise-routes=192.168.1.0/24
```

- [ ] Approve the route in the tailnet admin console. Two houses on the same `192.168.1.0/24` will clash: change one LAN's subnet, or use 4via6 routes.
- [ ] In Nightgrid: Settings → **Tailnet** → **Built-in Tailscale** → **LOG IN**. Leave **Use subnet routes** on.
- [ ] Add each device as a host with its LAN address, and put its 100.x address or MagicDNS name in the host editor's **Tailnet address**. Nightgrid tries the LAN first and the tailnet when that fails, so the same entry works at their house and from yours.
- [ ] Group hosts per household (for example `house-parents`, `house-sister`) and tag the device types (`#nas`, `#mac`, `#win`).
- [ ] Prefer **Tailscale SSH** for relatives' Macs and Linux boxes if you don't want to manage `authorized_keys`: the tailnet's policy decides who gets in, and Nightgrid shows an **Approve in browser** card when the policy asks for a check.

#### Check

- [ ] Turn off your own Wi-Fi and use a phone hotspot. Every host should still connect, through its tailnet address.

## Part 2 · Keys you can account for

- [ ] Settings → **CONNECTIONS** → **SSH KEYS** → **NEW KEY…**: make one ed25519 key just for family support, with a passphrase.
- [ ] **CHECK HOSTS** fills **WHERE THEY WORK**. Use **+** to add your key to each host.
- [ ] Look at **OTHER**: keys on their machines that aren't yours. Old laptops, a cousin who "fixed the Wi-Fi" once. Remove what nobody can explain.
- [ ] Turn off password logins once your key works. Test with `sshd -t` and keep your current session open while you reload, because sshd changes have no automatic rollback:

```sh
sudo sshd -t && sudo systemctl reload ssh
```

#### Check

- [ ] When your laptop is lost or replaced: **ROTATE A KEY** swaps the old key for the new one on every host, logs in with only the new key before removing the old one, and stops on any host where a step fails.

## Part 3 · Updates without surprises

- [ ] [Fleet](view:fleet) → **Updates** → **Check all**. Hosts with security packages or a pending reboot come first.
- [ ] Tick a household's hosts and **Upgrade selected…**; type the group's name to confirm. Nothing reboots by itself.
- [ ] Schedule the reboot for when nobody's using the machine: System → Schedules → **POWER SCHEDULE** → Reboot, at 04:00 on one day. It's a systemd timer on the host, so it runs with Nightgrid closed. Remove it afterwards.
- [ ] Hosts whose sudo needs a password Nightgrid doesn't remember are skipped. Tick **remember** once from that host's System → **UPDATES**, or give your family-support user passwordless sudo for package upgrades only.
- [ ] Macs and NAS firmware aren't covered by Fleet → Updates (it reads apt, dnf, pacman and rpm-ostree). For Macs, list pending updates from [Automation](view:automation) across `#mac` hosts:

```sh
softwareupdate -l
```

- [ ] For unattended security patches on Debian/Ubuntu hosts in between your visits:

```sh
sudo apt install -y unattended-upgrades
```

#### Check

- [ ] System → **security** on each Linux host: the hygiene score should note that automatic security updates are on.

## Part 4 · Disks and backups

- [ ] [Fleet](view:fleet) → **Disks**: ZFS, mdraid and btrfs health, plus **full in about N days** per mount once a host has been monitored for about a day. Read SMART once per host from System → **DISKS** with **remember** ticked, so Fleet can include it.
- [ ] Fleet → **Backups** → pick a host → **Find jobs**. It suggests restic, borg, rsync and rclone jobs from timers and cron tables. Nightgrid only watches; it never runs a backup.
- [ ] No backup on a family box yet? Restic to the NAS or a cloud bucket on a systemd timer is a good default. Create the timer from System → Schedules → **+ Timer**, then add it in Fleet → Backups.
- [ ] **Test restore** pulls one random small file from the newest snapshot into `/tmp` on the host, checks it and deletes it. Do it every month or so; each job shows when it was last tested.
- [ ] Windows PCs: Fleet can't see File History or Windows Backup. Back them up to the NAS instead, and watch the NAS-side folder as a path job.

#### Check

- [ ] Every household has at least one backup job with a recent **Test restore**, and no job is flagged stale.

## Part 5 · Who's been logging in

- [ ] Fleet → **Access** → **Audit all**. Per host: SSH logins of the last 7 days with the key used, failed attempts by address, sudo rules, users with a login shell and every authorized key.
- [ ] **Findings** flags password logins with failed attempts and nothing banning them. Fix it by turning off password auth (Part 2), or install fail2ban:

```sh
sudo apt install -y fail2ban
```

- [ ] Fleet → **Logs**: search for an error across a household at once when someone says "the computer is acting weird since Tuesday".

## Part 6 · Glanceable status and waking machines

- [ ] Settings → **Status page · Snapshot** → **Serve the phone status page**, listening on **Tailscale only**. Add the link to your phone's home screen.
- [ ] For machines that sleep: put the MAC address in the host editor and set **Wake from** to that house's always-on device. **WAKE** then works from anywhere. Wake-on-LAN must be on in the sleeping machine's firmware.
- [ ] Give hosts that sleep at night a **Sleep window** (`23:00-07:00`), so they show **ASLEEP** instead of raising offline alerts.
- [ ] Settings → **Dependencies**: "everything in house-parents depends on the parents' router" (add the router as a host or a service check), so a power cut there gives one alert.

## Part 7 · Don't lose your setup

- [ ] Settings → **DATA** → **BACKUP · PROFILE** → **Export** (or **Sync folder** with Syncthing or a cloud drive). Tick **Include saved passwords and tokens** only if the passphrase is somewhere safe; without it the file can't be opened.
- [ ] Write a note per household in [Knowledge](view:notes) tied to their main host: router login location (not the password), ISP, who to call, and any quirks. **Send to terminal** works on the commands in it.

## When Nightgrid is closed

- Runs without the app: Tailscale, unattended-upgrades, backup timers, power schedules and anything else in systemd timers or cron on the family machines.
- Needs Nightgrid running (the tray is enough): alerts and notifications, the phone status page, uptime history, monitoring history and disk forecasts, UPS shutdown.
- If you want alerts with your computer off, add a notify-only push from the family machines themselves (for example a backup unit's `OnFailure=` that posts to ntfy). Nightgrid's webhook only fires while it runs.

## Limits and gotchas

- Windows hosts are terminal and files only. Monitoring, Fleet checks, System views and audits expect a Linux shell; macOS gets terminal, files and some monitoring.
- Fleet → Updates doesn't cover macOS, Windows or NAS firmware yet.
- Overlapping home subnets (everyone's router defaults to `192.168.1.0/24`) are the most common reason a subnet route "doesn't work".
- Nightgrid never edits `~/.ssh/config`. Keys it adds go into the target's `authorized_keys` only.
- Be explicit with the family about what you can see. The Access audit and log search read more than "is the disk OK".

## Related

Proxmox media server · Game server host · Family cloud and photos · Repair bench server · Local AI GPU server · Smart home hub · Dev and git deploy lab
