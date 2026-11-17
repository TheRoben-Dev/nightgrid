# Game servers with LinuxGSM

Run dedicated servers for 100+ games (Valheim, Counter-Strike 2, Rust, Terraria, Project Zomboid, ARK and more) with LinuxGSM's scripts: install, update, monitor and back up with one command each.

## What you end up with

- A game server under its own Linux user with tmux-managed console
- Updates, monitoring and backups on cron
- Commands saved as Nightgrid snippets

## Before you start

- A Debian or Ubuntu host (or a VM/LXC per game); check the game's page on LinuxGSM for RAM and dependencies.
- The game's ports (LinuxGSM lists them with `./<game>server details`).

## Part 1 · Install (Valheim as the example)

- [ ] In a [Terminal](view:terminal):

```sh
sudo adduser --disabled-password --gecos "" vhserver
sudo -iu vhserver
curl -Lo linuxgsm.sh https://linuxgsm.sh && chmod +x linuxgsm.sh && bash linuxgsm.sh vhserver
./vhserver install
```

- [ ] The installer lists missing packages and the `sudo apt install` line to add them (including 32-bit libraries for SteamCMD).
- [ ] Settings go in `lgsm/config-lgsm/vhserver/vhserver.cfg` (server name, password, world). Edit it with Nightgrid's editor.

## Part 2 · Run and maintain

- [ ] `./vhserver start`, `./vhserver console` (detach with `Ctrl+b` then `d`), `./vhserver details`.
- [ ] Cron for the game user (from [System](view:system) → Schedules → **Crontab…** → **OTHER USER**):

```
*/5 * * * * /home/vhserver/vhserver monitor > /dev/null 2>&1
*/30 * * * * /home/vhserver/vhserver update > /dev/null 2>&1
0 5 * * 0 /home/vhserver/vhserver update-lgsm > /dev/null 2>&1
0 4 * * * /home/vhserver/vhserver backup > /dev/null 2>&1
@reboot /home/vhserver/vhserver start > /dev/null 2>&1
```

## Part 3 · In Nightgrid

- [ ] Snippets in [Automation](view:automation): `sudo -iu {{game=vhserver}} ./{{game=vhserver}} {{action=details}}`.
- [ ] Forward only the game's ports and check [System](view:system) → **security**.
- [ ] Add `lgsm/backup` as a path job in [Fleet](view:fleet) → **Backups**.

#### Check

- [ ] `./vhserver details` shows ONLINE, and players connect.

## Limits and gotchas

- `update` stops and restarts the server when a game update is out; warn players or schedule it off-peak.
- Backups include the whole server folder by default; exclude the game files in the config to keep them small.

## Related

- [Game server host](kb:guides/game-server-host)
- [Minecraft server](kb:apps/minecraft)
- [Schedule jobs with cron and timers](kb:howto/schedules)
