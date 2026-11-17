# Kasm Workspaces browser desktops

Stream disposable Linux desktops and single apps (a browser, LibreOffice, a terminal) into any web browser. Each session is a fresh container that's thrown away when you close it, which makes it a good place to open dodgy links, test a download or give someone a temporary desktop.

## What you end up with

- Kasm Workspaces installed on a dedicated Linux host or VM
- A few workspace images (Firefox, Ubuntu desktop) ready to launch
- Your own admin and user accounts, the defaults changed

## Before you start

- A clean Ubuntu or Debian VM with 4 CPU cores, 8 GB RAM and 75 GB disk or more. Kasm installs and manages its own Docker containers and expects the machine to itself.
- Kasm's free Community Edition is meant for personal and non-commercial use and limits concurrent sessions; check the current terms on its site.

## Part 1 · Install

- [ ] In a [Terminal](view:terminal) tab on the VM, download the current release tarball linked from Kasm's install docs into `/tmp`, then `tar -xf kasm_release_*.tar.gz` and `sudo bash kasm_release/install.sh`.
- [ ] It listens on port 443 by default. To keep 443 free for a reverse proxy, add `-L 8443` to the install command.
- [ ] At the end the installer prints passwords for `admin@kasm.local` and `user@kasm.local`. Copy them into your password manager before closing the tab.
- [ ] Too little swap makes the installer stop and ask; let it create a swap file, or add one yourself (see [Swap and zram](kb:linux/swap-zram)).

## Part 2 · Set it up

- [ ] Log in at `https://<vm>:8443` as the admin, change both default passwords under **Access Management → Users**, or create your own users and delete `user@kasm.local`.
- [ ] **Workspaces → Registry** lists ready images. Install a handful (Firefox, Chrome, Ubuntu Jammy desktop). Each is a multi-GB pull.
- [ ] Per workspace, **Edit** sets CPU, memory and whether sessions keep a persistent profile. Leave profiles off for throwaway browsing.

#### Check

- [ ] Launch the Firefox workspace from a phone or laptop browser, browse, then end the session. Launching again gives a fresh browser with no history.

## In Nightgrid

- Add the VM as a host; its Kasm containers appear in [Docker](view:docker), but manage them from Kasm's admin, not from Nightgrid, or Kasm loses track of sessions.
- Add `https://<vm>:8443` to [Services](view:services) with **Allow untrusted certificate**, since Kasm starts with a self-signed one.
- Use [Fleet](view:fleet) → **Disks** to watch space; workspace images fill the disk faster than you expect.

## Limits and gotchas

- Behind a reverse proxy, Kasm needs WebSockets and the proxy's address set under **Admin → Infrastructure → Zones** (upstream auth address and proxy port), or sessions connect and immediately drop.
- GPU acceleration for workspaces needs extra setup (NVIDIA container toolkit); without it, video-heavy sites stutter.
- Upgrades run through Kasm's own upgrade script, not by pulling newer images.

## Related

- [Webtop Linux desktop in a browser](kb:apps/webtop)
- [Apache Guacamole remote desktop gateway](kb:apps/guacamole)
- [Swap and zram](kb:linux/swap-zram)
