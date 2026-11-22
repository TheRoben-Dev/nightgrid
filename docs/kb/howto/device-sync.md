# Sync Nightgrid between your devices

Keep the same hosts, services, notes, dashboards and settings on your desktop, laptop and handheld. The apps talk to each other directly over your LAN or tailnet, encrypted end to end, with no server or account in between.

## What you end up with

- Two or more Nightgrid apps paired with each other
- Either push and pull by hand, or automatic sync within seconds
- Each device still has its own scale, handheld options and window setup

## Quick start · Import from your other computer on the LAN

- [ ] On the computer that has your setup: **Settings → Sync**, turn on **Let my other devices find me on this network**. It answers when another Nightgrid on the same network looks for it. No internet or tailnet needed.
- [ ] Optional, same place: **Pair with your profile passphrase → Set up…** and enter the passphrase you use for profile backups. Then a new device can pair by typing it instead of reading a code. Only a key made from it is kept, in the OS keychain; it never crosses the network.
- [ ] On the new install: the empty host list shows **Found on your network** within a few seconds (also listed under **Settings → Sync → On this network**). Press **Import and pair…**.
- [ ] Pick **Code on …** and press **Show a code on …**: a code pops up on the other computer. Type it. Or pick **Profile passphrase** and type that.
- [ ] Leave **Import …'s profile** and **Keep syncing both ways** on, then **Pair and import**, and restart when asked.

#### Check

- [ ] After the restart your hosts and services are there, and both devices list each other under **PAIRED DEVICES** in **TWO-WAY**.

## Part 1 · Pair two devices by address

- [ ] On the first device: **Settings → Sync**, turn on **Let paired devices reach this one**. Note an address under **Reach it at** (a LAN address, a tailnet name or a 100.x address).
- [ ] Press **Show a code**. It's valid for 5 minutes and three tries.
- [ ] On the second device: **Settings → Sync → Join a device…**. Enter the first device's address and the code, then **Pair**. With the built-in Tailscale on, online tailnet devices are offered as you type.
- [ ] **It can change this device's profile** decides whether the other device may push or send changes here. Turn it off for a device that should only read.

#### Check

- [ ] Both devices list each other under **PAIRED DEVICES** with a green dot after **Check**. The **KEY** under THIS DEVICE matches what you'd expect if you compare them side by side.

## Part 2 · Choose how they sync

- [ ] **MANUAL**: **Push to…** sends this device's profile there; **Pull from…** takes theirs. **Merge** adds what's new and keeps existing items and settings; **Replace** makes the receiving side a copy (type `replace`). The receiving side restarts to load it.
- [ ] **TWO-WAY**: changes on either side go to the other within seconds. Edits to different items both survive; if the same host was edited on both, the later edit wins. Deleting something deletes it on the other side too.
- [ ] **SEND ONLY** / **RECEIVE ONLY**: automatic in one direction, for a device that should follow another.
- [ ] Starting automatic sync between two devices that both have a setup? Push or pull once in MANUAL first, so they start from the same profile.

#### Check

- [ ] Rename a host on one device. Within about 15 seconds it has the new name on the other (or press **Sync now**).

## Part 3 · Secrets

- [ ] Saved secrets don't travel unless you tick their group under **Secrets and permissions**: **SSH logins** (passwords, key passphrases, sudo passwords), **Integration tokens** (and the alerts webhook) and **.env vault keys**.
- [ ] Tick the same groups on both devices: each one only sends and accepts what it has ticked.

## Limits and gotchas

- Both devices need Nightgrid running and able to reach each other. A device that was offline catches up within 5 minutes of coming back.
- Automatic sync needs the same Nightgrid version on both; update both, then sync.
- Never synced: the device's name, scale, handheld and controller options, tray, built-in Tailscale login, phone status page, MCP server, sync folder and the DRIFT model.
- SSH key files in `~/.ssh` stay where they are. Copy them yourself or use **Settings → SSH keys** on each device.
- Metric, uptime and alert history stay on each device.
- Finding devices uses a small UDP message on port 47921 within your LAN (the broadcast doesn't cross routers or VPNs). Across a tailnet or another subnet, pair by address instead (Part 1).
- Five wrong passphrases in a row lock passphrase pairing on that device for 10 minutes.
- On macOS and Windows the system may ask once whether Nightgrid may accept incoming connections; allow it for pairing and incoming changes to work.

## Related

- [Back up and move your Nightgrid setup](kb:howto/profiles)
- [Reach your servers away from home](kb:howto/remote-access)
