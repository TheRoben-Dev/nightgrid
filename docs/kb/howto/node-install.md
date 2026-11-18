# Install Nightgrid Node

Put the optional [Nightgrid Node](kb:basics/nightgrid-node) on a Linux server: get the binary, verify it, install it as a sandboxed service, check that it runs, and know how to upgrade or remove it later.

![From download to a running node.](fig:ng-node-install)

The quick way: **Fleet → NODE → Install…** on the host does Parts 1 and 2 for you (download, checksum, upload, install as root, your login user added to the group) and shows the installer's report. The steps below are the same by hand.

## What you end up with

- `nightgrid-node.service` running as the unprivileged `nightgrid-node` account
- Your login user in the `nightgrid-node` group, so `nightgrid-node status` works without sudo
- History recording from the first minute, with nothing listening on the network

## Before you start

- A Linux server with systemd, x86_64 or aarch64 (`uname -m`). It's tested on Debian 12, Ubuntu 22.04/24.04 and Fedora 41; Proxmox VE, Raspberry Pi OS (64-bit) and Fedora Atomic / Bazzite use the same paths.
- sudo on that server. Open a [Terminal](view:terminal) to it in Nightgrid.
- 32-bit systems (armv7, i686) aren't supported.

## Part 1 · Get the binary

- [ ] Download `nightgrid-node-vX.Y.Z-<arch>-linux` and `SHA256SUMS` from the Releases page of the TheRoben-Dev/nightgrid-node repository on GitHub, for your architecture (`x86_64` or `aarch64`).
- [ ] Verify it: `sha256sum -c --ignore-missing SHA256SUMS` must print `OK`. Optionally check the build provenance with `gh attestation verify <file> --repo TheRoben-Dev/nightgrid-node`.
- [ ] No release published yet, or you want to build it yourself? Clone the repository and run `cargo build --release` on the server (needs a Rust toolchain), or build a static binary on another machine with `cargo zigbuild --release --target x86_64-unknown-linux-musl` (or `aarch64-unknown-linux-musl`) and copy it over with [Files](view:files) or `scp`.
- [ ] `chmod +x nightgrid-node-*-linux`

## Part 2 · Install

- [ ] `sudo ./nightgrid-node-*-linux install`. It copies itself to `/usr/local/bin/nightgrid-node`, creates the `nightgrid-node` account and group, writes `/etc/nightgrid-node/config.toml`, installs the sandboxed unit and starts it. The user who ran sudo joins the group.
- [ ] Add more users to the group with `--user NAME` (repeatable), for example the account another computer's Nightgrid logs in as. `--no-start` installs without starting; `--json` prints a machine-readable report.
- [ ] Log out and in again (or start a new SSH session) so the new group membership applies.
- [ ] `nightgrid-node status`: prints the node's info, its history coverage, memory and database size, and `access` for each optional grant (`ok`, `denied`, `absent` or `off`).

## Part 3 · Check it's healthy

- [ ] `systemctl status nightgrid-node` shows `active (running)`.
- [ ] `nightgrid-node query metrics.latest` returns a sample after about 10 seconds (the very first one has `cpu: null`).
- [ ] `ss -ltnp | grep nightgrid` prints nothing: the main service never listens on the network.
- [ ] `journalctl -u nightgrid-node -n 50` is where errors go. "Permission denied" on the socket means your session isn't in the `nightgrid-node` group yet (log in again).

## Part 4 · Grant what you want (optional)

- [ ] Read [Choose what Nightgrid Node may do](kb:howto/node-grants) first. Each grant is one command, for example `sudo nightgrid-node allow docker`, and undone with `deny`.
- [ ] `nightgrid-node status` again: the grants you made show `ok`.

## Part 5 · Configure (optional)

- [ ] Settings live in `/etc/nightgrid-node/config.toml`, owned by root; the service only reads it. Every setting is written out with comments by `install`.
- [ ] Common changes: `[metrics] interval_secs` (2–60) and `minute_retention_days` (1–365), `[files] watch`, `[network] ignore_ifaces`, `[health] ups`, `[actions] allow`.
- [ ] Apply with `sudo systemctl restart nightgrid-node`. For `[checks] networks`, `[peers]`, `[statuspage]` and `[rollback] paths`, also run the matching `allow` command again so its unit picks up the change.

## Part 6 · Upgrade

- [ ] Download and verify the new binary as in Part 1.
- [ ] `sudo nightgrid-node upgrade ./nightgrid-node-vX.Y.Z-<arch>-linux --sha256 <hash from SHA256SUMS>`. It installs only if the hash matches, starts the new version, checks it comes up and stays up, and puts the old binary back by itself if not. History, config and node id are kept.
- [ ] Older versions are refused unless you pass `--allow-downgrade`.
- [ ] The timeline gets "upgraded to X.Y.Z", or "upgrade to X.Y.Z failed" with "back on" the old version.

## Part 7 · Remove it

- [ ] `sudo nightgrid-node uninstall` removes the service but keeps history and config, so a reinstall picks up where it left off.
- [ ] `sudo nightgrid-node uninstall --purge` removes everything: history, config, the service account and group, any firewall table and keys. A later install gets a new node id.
- [ ] Binary already gone? The repository's `dist/uninstall.sh` does the same with only `systemctl`, `rm`, `userdel` and `groupdel`.

## If something goes wrong

- **`status` says permission denied**: not in the group yet. `id` shows your groups; log in again, or `sudo nightgrid-node install --user <you>` to add yourself.
- **`cannot execute binary file: Exec format error`**: wrong architecture. Compare `uname -m` with the file name.
- **Service fails at start on an unusual distro**: `journalctl -u nightgrid-node -b` names the sandbox setting at fault (look for a status=2xx code; the Error reference shelf explains each one).
- **Immutable systems** (Bazzite, Silverblue): install works the same, because the node lives under `/usr/local` and `/var`, which stay writable.
