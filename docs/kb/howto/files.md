# Move files with the Files view

Copy, upload, edit and clean up files on any host over SFTP, with a transfer queue and no extra server software.

## What you end up with

- A two-pane browser: this computer and a host, or two hosts
- Bookmarks for the folders you visit often (stack folders, `/etc`, media)
- Safe edits: saving warns when the file changed on the server since you opened it

## Before you start

- SFTP is part of OpenSSH and on by default. If a host has `Subsystem sftp` commented out in `/etc/ssh/sshd_config`, put it back.

## Part 1 · Browse and bookmark

- [ ] Open [Files](view:files) (`Mod+3`) and pick a host for each pane.
- [ ] Bookmark the folders you use: your compose folder, `/etc`, the media root.
- [ ] Hidden files: the `.*` button in a pane shows dotfiles like `.env`.

## Part 2 · Transfers

- [ ] Upload or download between the panes; the queue shows progress and lets you work meanwhile.
- [ ] Name clashes ask whether to overwrite or skip.
- [ ] Large media: for many GB at once, `rsync -aP` in a [Terminal](view:terminal) between the two hosts is faster and resumable, because it doesn't pass through this computer.

## Part 3 · Edit

- [ ] Open a text file to edit it in the built-in editor. **✦ Ask DRIFT** in the editor explains a config file or an error in it.
- [ ] If someone (or a container) changed the file since you opened it, saving asks first.
- [ ] Files your login can't write: edit from a terminal with `sudoedit`, which opens in Nightgrid's editor when **Edit server files in Nightgrid** is on.

## Part 4 · Delete safely

- [ ] A delete always lists exactly what will go. Read the list.
- [ ] Before deleting a stack's folder, bring the stack down in [Docker](view:docker) first.

## Limits and gotchas

- SFTP acts as your login. Root-owned paths need a terminal and sudo.
- Permissions are kept as the server sets them for new files (your umask). Fix ownership afterwards with `chown` if a container runs as another uid.
- Transfers stop when Nightgrid quits. Use `rsync` or `rclone` on the host for anything that must finish unattended.

## Related

- [Work faster in the terminal](kb:howto/terminal)
- [Deploy a new compose stack](kb:howto/deploy-stack)
- [Samba file shares](kb:apps/samba)
- [Offsite copies with rclone](kb:apps/rclone)
