# Back up and move your Nightgrid setup

Export everything Nightgrid knows (hosts, groups, snippets, services, dashboards, notes and optionally saved passwords) into one encrypted file, and bring it to another computer or OS.

![One encrypted profile, restored on any OS.](fig:ng-profile)

## What you end up with

- An age-encrypted profile you can restore on macOS, Windows or Linux
- One-click export to a sync folder
- A weekly reminder so the backup never gets old

## Part 1 · Export

- [ ] **Settings → Data → Export** (or **Export profile…** in the palette). Choose a passphrase you'll still know in a year.
- [ ] Tick **Include saved passwords and tokens** to bring keychain entries along. The file stays encrypted either way.
- [ ] Store the passphrase in your password manager. Without it the file can't be opened.

## Part 2 · Sync folder

- [ ] Pick a Syncthing, cloud-drive or git folder as the **SYNC FOLDER**. **Export to sync folder** and **Import from sync folder** use `nightgrid-backup.age` there.
- [ ] Self-hosted sync: [Syncthing](kb:apps/syncthing) or [Nextcloud](kb:apps/nextcloud).

## Part 3 · Import

- [ ] **Import profile…** shows what's inside first.
- [ ] **Merge** keeps what you have and adds what's new. **Replace** swaps everything; type `replace` to confirm. Nightgrid restarts.

#### Check

- [ ] Import on a second computer: hosts, notes and dashboards appear. Connect to one host to accept its key.

## Part 4 · Reminder

- [ ] A card appears when your last export is older than a week. Change the interval or turn it off under **REMIND ME TO EXPORT**.

## Limits and gotchas

- Uptime and metric history stay on this computer; they're not in the profile.
- SSH private keys aren't in the profile. Back up `~/.ssh` separately (or keep keys in a password manager's agent).
- The DRIFT model isn't included; download it again on the new machine.

## Related

- [Sync Nightgrid between your devices](kb:howto/device-sync)
- [Monitor backups and test restores](kb:howto/backups)
- [Syncthing](kb:apps/syncthing)
- [Set up and rotate SSH keys](kb:howto/ssh-keys)
