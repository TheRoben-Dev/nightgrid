# Nightgrid user guide

Nightgrid manages your servers over plain SSH. Nothing is installed on them: terminals, files, Docker, monitoring, services, tunnels, system tasks and automation all use the SSH access you already have. Everything is stored on this computer, and passwords live only in the OS keychain.

`Mod` below means **Cmd** on macOS and **Ctrl** on Windows and Linux.

## Getting around

- **Sidebar**: your hosts (with favorites and groups) at the top, the views below. `Mod+B` collapses it to icons.
- **Views**: Overview `Mod+1`, Terminal `Mod+2`, Files `Mod+3`, Docker `Mod+4`, Services `Mod+5`, Tunnels `Mod+6`, System `Mod+7`, Automation `Mod+8`, Settings `Mod+,`.
- **Command palette** `Mod+K` (also `Ctrl+Shift+K` inside a terminal): jump to any host, view, snippet, playbook or action by typing.
- **DRIFT helper** `Mod+J`: ask a question in the side panel (see below).
- **Settings → Keybindings** lists every shortcut.

## Hosts

- **+** in the sidebar adds a host: name, address, port, user, and how to log in.
  - **SSH agent** (recommended): uses the keys your agent holds, including 1Password or Secretive if they act as your agent.
  - **Key file**: a private key, with its passphrase asked when needed.
  - **Password**: asked when needed.
  - **Remember** saves the password or passphrase in the OS keychain. It's off unless you tick it.
- **IMP** imports hosts from `~/.ssh/config`, including jump hosts (`ProxyJump`). Your SSH config is only read, never written.
- **Host keys**: the first connection shows the server's fingerprint for you to accept. If a key changes later, the connection is blocked until you type the host name to accept the new key. A diff of the fingerprints is shown.
- Groups, tags, colors, icons and favorites keep long lists tidy. Filter the list with text or `#tag`.

## Terminal

- Click a host or press `Mod+2` to open a terminal. Use `Ctrl+Shift+T` for a local shell.
- Panes and tabs:
  - Split right with `Ctrl+Shift+D` and split down with `Ctrl+Shift+S`.
  - Close a pane with `Ctrl+Shift+W`.
  - Switch tabs with `Ctrl+Tab`.
  - Find with `Ctrl+Shift+F`.
- Dropped connections reconnect on request. Links in the output open in your browser.
- **Edit server files in Nightgrid** (Settings → Terminal): `nano FILE` and `$EDITOR` open the file in Nightgrid's editor instead.

## Files

- Two-pane SFTP browser. Bookmarks, upload and download with a transfer queue, rename, new folder and delete.
- A delete always lists exactly what will go.
- Open text files in the built-in editor. If the file changed on the server since you opened it, saving asks first.

## Docker

- Containers per host with live CPU and memory, logs (follow and search), inspect, and a shell inside a container.
- Start, stop, restart and remove are available. Stop, restart and remove confirm the containers by name.
- **Stacks**: Compose projects, plus folders you add, with up, down, pull and restart.
- Environment values are masked until you click them.

## Overview and monitoring

- The Overview shows every host with CPU, memory, disk, network and temperatures, read over SSH with one short command per sample.
- Click a host for history charts. Minute detail is kept for the days set in Settings → Monitoring, and 15-minute averages for at least 90 days.
- Hosts that need a password or a new host key wait until you open a terminal on them; monitoring never prompts.

## Services and integrations

- **Services**: a board of web apps with an up/down check, response time, certificate expiry and 30 days of history. Import them from a Homepage `services.yaml`, or add them by hand.
- **Add integration**:
  - **Plex**: what's playing.
  - **AMP**: game servers with start, stop and restart.
  - **Uptime Kuma**: monitors from a status page.
  - **Portainer**: "Open in Portainer" links.
- Tokens are kept in the keychain only.

## Tunnels

- **Local** (`-L`): reach a server-side port from this computer. **Remote** (`-R`): expose a local port on the server. **Dynamic** (`-D`): a SOCKS proxy.
- Saved tunnels can start with the app. Listening anywhere other than localhost is flagged as public.

## System

- Per host:
  - **services**: systemd units, with start, stop, restart, enable and disable;
  - **journal**: live logs with filters;
  - **updates**: apt, dnf or pacman, with a reboot-needed flag;
  - **disks**: SMART health.
- **sudo**: asked each time by default. Tick "remember for this host" to keep it in the keychain. The password is sent on sudo's input, never on a command line.

## Automation

- Type a command, pick hosts (click a group to select all of it) and **Run**. Each host shows its own output, exit code and time. **Select failed** retries just the hosts that failed.
- **Snippets** are saved commands with variables:
  - `{{name}}` asks for a value when it runs;
  - `{{name=default}}` pre-fills one;
  - `{{host}}`, `{{hostname}}` and `{{user}}` are filled in for each host.
  - Values are quoted safely, so don't wrap placeholders in quotes yourself.
- **Playbooks** run steps in order: commands, file uploads, wait for a port, and Docker start, stop, restart or pull.
- Anything destructive (`rm`, `prune`, `kill`, `compose down`, `reboot`, `upgrade`, `> file`, ...) is confirmed with the targets named. On several hosts you also type the group name or the number of hosts.

## DRIFT helper

- DRIFT is a small assistant in a side panel. Open it with `Mod+J`, the **DRIFT** button in the top bar, or **Ask DRIFT** in the command palette.
- **Turn it on** under **Settings → Helper**, then **Download** the model once (about 2.5 GB). The download can be paused and resumed and is checked against a fixed SHA-256.
- It runs entirely on this computer: the GPU on macOS, the CPU on Windows and Linux. Nothing you ask is sent anywhere.
- **Explain something**: select log lines, an error or a file in the terminal or a log view, then open DRIFT. The selection and the current view and host are attached as chips; click a chip to leave it out.
- **Explain buttons**: container logs, the System journal, compose output and Automation results have an **✦ Explain** button, and the file editor has **✦ Ask DRIFT**. It sends the newest lines (or the lines you selected there, or the open file) with a label saying what they are, and asks what's wrong. If the model isn't downloaded yet, the question waits in the box.
- **Show me**: when an answer says where something is, **Show me** switches to that view and outlines the control. It only points; it never clicks or runs anything.
- **What I was given** under each reply shows the exact prompt and the guide sections used. Values that look like passwords or tokens are masked before DRIFT sees them.
- **Stop** ends a reply early; **NEW** starts a fresh conversation. **Free memory** in Settings unloads the model; **Delete model** removes the file.
- Answers can be wrong. Commands DRIFT suggests are yours to run, and Nightgrid still asks before anything destructive.

## Alerts and the tray

- Alerts fire for these conditions:
  - a host is offline for 2 minutes;
  - a disk is over its limit;
  - a container that was running exits with an error;
  - a service is down;
  - a certificate is expiring;
  - SMART reports a warning.
- Alerts arrive as desktop notifications, and optionally through an ntfy, Discord or JSON webhook. Quiet hours hold notifications but still show alerts in the app.
- The tray (menu bar on macOS) icon offers quick connect, lets you start and stop tunnels, and shows the alert count.

## Backups and moving between machines

- **Settings → Backup → Export** writes your setup to one file encrypted with a passphrase (an [age](https://age-encryption.org) file). It holds hosts, groups, tags, snippets, playbooks, services, tunnels, integrations, bookmarks and settings. Tick **Include saved passwords and tokens** to bring the keychain entries along.
- **Import** shows what's inside first.
  - **Merge** keeps everything you have and adds what's new.
  - **Replace** swaps your setup for the backup. Type `replace` to confirm.
  - Nightgrid restarts afterwards.
- **Sync folder**: pick a Syncthing, cloud-drive or git folder, and export and import use `nightgrid-backup.age` there in one click. The passphrase is never stored, so keep it somewhere safe. Without it the file can't be opened.

## Handhelds (ROG Ally, Steam Deck) and controllers

- **Settings → Display → Handheld layout** turns on finger-sized buttons, 130 % scale and an icon-only sidebar. Turn it off to go back to the desktop sizes.
- **Controller navigation** works with any gamepad the system sees:

| Control | Action |
|---|---|
| D-pad / left stick | Move focus (hold to repeat) |
| A | Select the focused item |
| B | Back: close the dialog or menu |
| LB / RB | Previous / next view |
| Start | Command palette |
| Select / View | Collapse or expand the sidebar |

In text fields and terminals, left and right are left to the field; up and down move focus out of it. Typing still needs a keyboard (on the Ally, the on-screen one).

## Look and feel

- Four themes: NIGHTGRID, ICE, BLOODLINE and PLAIN DARK. PLAIN DARK has no effects at all.
- **Reduce effects** turns off the glitch text, scanlines and glow. Your OS's reduced-motion setting does the same.
- **UI scale** goes from 90 to 150 %.

## Updates

Nightgrid checks GitHub Releases after start-up and offers **Install & restart** when a newer version is out. Turn this off, or check by hand, under Settings → Updates.

## Privacy

There's no telemetry. Nightgrid only connects to:
- your hosts;
- the services and integrations you added;
- GitHub Releases, for the update check;
- your webhook, if you set one.

Secrets are kept only in the OS keychain (or inside an encrypted backup, if you ask for that) and never in the database, logs or config files.

## Troubleshooting

- **"needs a login"**: open a terminal on that host once so you can answer the password or host-key prompt. Background features never prompt.
- **Keychain unavailable** (Linux without GNOME Keyring or KWallet): everything still works, but "remember" options can't save.
- **Blank window on Linux with NVIDIA**: see the README section on WebKitGTK.
- **Logs**: the folder is shown in Settings → Diagnostics. Set `NIGHTGRID_LOG=debug` for more detail.
