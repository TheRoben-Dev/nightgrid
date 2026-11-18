# Nightgrid Node pairing: the app side

Status as of Nightgrid 0.10.88 (M12f, D-165). This is the app's answer to the node project's `docs/app-integration` specs: what the app now does, how it differs from the specs, and what would help on the node's side. The node project owns the node; nothing here changes it.

## What the app does

| Spec | State in the app | Notes |
|---|---|---|
| 0001 · Install and metric history | Done | Fleet → NODE: Install / Update / Remove per host, metric sync with the rollup back-fill. |
| 0002 · Events, uptime, disk, offline alerts | Done | "While you were away" is one notification per host plus the alerts panel; there is no separate Away section yet. |
| 0004 · Live streams | Partly | `events` and `metrics` streams are used. `containers` is not (Docker views still read Docker over SSH). |
| 0003, 0005–0019 | Not yet | The node does these on its own; the app doesn't show them yet. |

### Finding the node

On each new pooled SSH connection, one read-only script (no sudo) reports the login user, `uname -m`, whether `nightgrid-node` is on `PATH`, whether the socket exists and is writable, whether the user is listed in the `nightgrid-node` group and `systemctl is-active nightgrid-node.service`. From that the app picks:

- writable socket: a `direct-streamlocal` channel to `/run/nightgrid-node/node.sock`, or `exec nightgrid-node bridge` when sshd forbids stream-local forwarding;
- not writable but listed in the group (the login predates the install): `exec sg nightgrid-node -c 'exec nightgrid-node bridge'`;
- otherwise one of not installed, not running, no access.

`hello` offers protocol 1–2 with `client: "nightgrid/<version>"`. `unsupported_proto` shows "needs an update". A host with nothing usable is looked at again after 30 minutes or on the next connection.

### Sync

After `hello`, in order: `alerts.rules.set` with the app's whole `AlertSettings` (once per connection, and again whenever alert settings change), `metrics.sync` (minutes; 3600 and 900 rollups only for buckets that end before the node's oldest minute, merged with "the larger `n` wins"), `disk.sync` (an hour the app recorded wins), `uptime.sync` (max per column, clamped to 3600 s), `events.sync`. With protocol 2: `subscribe ["events"]`, then `events.sync` once more. Everything runs again every 15 minutes while connected. Cursors are kept per host in the app (`node_sync`) and dropped when the node id changes.

Events go to the host timeline as alert, recovered, boot (skipped when the app has a boot within 2 minutes), outage (`down` also becomes an outage row), container and action. `login`, `file`, `network`, `port` and `job` events are not shown yet. Alerts from `alert` / `recovered` events (latest per key, key mapped to `source:<hostId>:rest`) are imported: resolved ones into history, active ones as active unless the app already has the key.

### Live samples

While a host is visible the app subscribes to `metrics` and uses the pushed samples; when it's hidden it unsubscribes and calls `metrics.latest` at its background rate. In both cases the collect script over SSH is not run. Any error drops the node connection and the app falls back to SSH for that round.

### Install, update, remove

The binary is the latest release's `nightgrid-node-<tag>-<arch>-linux`, used only when its SHA-256 matches the release's `SHA256SUMS` (and it starts with an ELF header), cached in the app's data folder; or a file the person picks. It goes to `mktemp -d /tmp/nightgrid-node.XXXXXX`, mode 0700, and `<file> install --json --user <login user>` runs through the app's sudo flow. Remove runs `nightgrid-node uninstall --json [--purge]`, or the node's `dist/uninstall.sh` (bundled in the app) when the binary is gone. The app doesn't drop the SSH connection after an install; the `sg` route covers the new group.

## Hub and link setup (Nightgrid 1.3, D-175)

| Spec | State in the app | Notes |
|---|---|---|
| 0016 · Pairing nodes | Partly | Only pairing with a hub. The link is turned on (`allow peers`) when a host joins; host-to-host pairs aren't offered yet. Each host's HUB & LINK tab shows `peers.latest` readings. |
| 0020 · Hub (H1) | Setup done, sync not yet | Make a hub, add and remove members, the overview and hub events. The app still syncs every host directly (§4 not used yet) and has no hub notifications (H2 isn't built). |

**Making a hub** on a host with node 0.26+: download `nightgrid-hub-<tag>-<arch>-linux` from the same release (SHA256SUMS, ELF), upload to a private temp folder, `install --json --user <login>`, `nightgrid-node allow peers --json` if the host's link is off, `nightgrid-hub allow collect --json`. The hub host then joins as its own first member.

**Joining** (sudo on the member): `allow peers` if `peers.latest.access` isn't `ok` (then wait for the certificate). The app resolves the member's address on the hub host (`getent ahosts`, then `ip -o route get` for the hub's source address) and reads both configs:

- no `listening` on the member (no tailnet address): `[peers] listen` becomes `<LAN ip>:7419`, but only if that IP is one of the member's own addresses (`ip -o addr`);
- the hub's source address not in the member's `[peers] networks`: added as `/32` (`/128`);
- the member's link address not in the hub's `[collect] networks`: returned to the UI, which saves it on the hub with the hub's sudo (`hub_allow_networks`).

Config changes are saved through the same checked save as the CONFIG tab (below), which runs `allow peers` / `allow collect` again for the new networks. Then `peers.set` (the hub's id, collector fingerprint, and its tailnet address, or `<hub LAN ip>:7420` for a LAN member), `hub.member.set`, `serve add sync`, and `fleet.members.set` on the hub with the member's id, link address and fingerprint. **Leaving** undoes it in reverse (`fleet.members.set` without it, `hub.member.set null`, unpair, `serve remove sync`); networks stay as they are.

**Config, status and logs.** `cat` reads either config (both 0644). A save uploads the file, checks it with `nightgrid-node doctor --config <tmp> --json` (the hub: TOML and its four sections), then as root keeps `config.toml.bak`, installs it 0644, `systemctl try-restart`s the program's services, re-runs `allow <grant>` for sections whose `networks` changed and whose unit exists, and restores the old file if the main unit isn't active after 3 s. Status is `systemctl is-active` per unit plus `nightgrid-node doctor --json`; logs are `journalctl -u …` as root.

## What would help on the node's side

None of these block anything; the app works with the node as it is.

1. **Classify failures on the bridge.** When `bridge` can't reach the socket (missing, permission denied) it exits without a word on stdout, so the app needs a separate probe exec first. One JSON line before exiting, such as `{"error":{"code":"no_access","message":"…"}}` (codes `no_access`, `not_running`), would let the app try the bridge first and probe only when needed.
2. **A release manifest.** A `manifest.json` per release (version, protocol range, and per architecture the file name, size and SHA-256) would let the app read one file instead of GitHub's release API plus `SHA256SUMS`, and check protocol compatibility before downloading. Signatures (`.minisig`, as 0001 already mentions) would then cover the manifest.
3. **Say how `subscribe` combines.** The app assumes `subscribe` adds the listed streams to what's subscribed and `unsubscribe` removes only those (it subscribes `events` at connect and toggles `metrics` with visibility). PROTOCOL.md should state it.
4. **Say what `next` is on an empty page.** The app treats `next < after` as "no progress" and keeps its cursor; PROTOCOL.md could promise `next == after` when nothing was returned.
5. **Filter `events.sync` by kind (optional).** A `kinds` parameter would let a first sync skip the up to 20 000 login, file and network events the app doesn't show yet.
6. **Alert kinds the app doesn't have.** `loginBurst` has no counterpart in the app yet, so it is not imported (it still fires on the node). Any new kind should keep using the app's camelCase names so it maps without code.

7. **A 32-bit ARM build (`armv7`).** Many NAS boxes (a ReadyNAS RN102, older Synology and QNAP) are `armv7l`; the app shows "no build" there. An `armv7-unknown-linux-musleabihf` build would let them run the node.
8. **The collector listening on more than one address.** `[collect] listen` is one address, normally the tailnet one. A LAN-only member then pings the hub at an address nothing answers, so its Linked list shows the hub unreachable and its timeline gets "peer … unreachable" once (pulls aren't affected). A list, or `listen = ["tailnet", "192.168.1.10:7420"]`, would fix that.
9. **`[peers] listen = "lan"`.** Hosts without Tailscale need a hand-set `ip:port`. A `lan` value (the address of the default route) would survive DHCP changes.
10. **A config check for the hub.** `nightgrid-hub doctor` (or `check-config FILE`) would let the app refuse a bad hub config before restarting it, as `nightgrid-node doctor --config` does.

## Testing

The app's tests run the client, the sync algorithm and the merges against a scripted node over an in-memory pipe (`core/src/node/client.rs` `fake`, `core/src/node/sync.rs`): first sync with a gap longer than the node's minute retention, resume with nothing new, a node without capabilities, interleaved frames, `unsupported_proto`, the key mapping and the uptime clamp. A run against a real node over SSH is the next step on a host that has one.
