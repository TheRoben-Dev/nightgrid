# Work faster in the terminal

Splits, tabs, history search, editing server files in Nightgrid's editor and key hints for programs like vim and tmux. All of it works over plain SSH with nothing installed on the server.

![How the terminal knows which program is running.](fig:ng-terminal-hints)

## What you end up with

- Several hosts side by side in one tab
- Old commands found by typing a few words, across bash, zsh and fish history
- `nano file`, `crontab -e` and `git commit` opening in a desktop-style editor
- A strip of keys for whatever program is in front

## Part 1 · Panes and tabs

- [ ] Click a host for its terminal (Alt-click or Cmd-click always opens a new one). `Ctrl+Shift+T` opens a local shell.
- [ ] **+ NEW** in the tab bar opens any saved host as a new tab or a split next to the focused pane.
- [ ] Split right `Ctrl+Shift+D`, split down `Ctrl+Shift+S`, close a pane `Ctrl+Shift+W`, next tab `Ctrl+Tab`.
- [ ] Find in the output with `Ctrl+Shift+F`.
- [ ] Command palette from inside a terminal: `Ctrl+Shift+K`.

## Part 2 · History search

- [ ] Turn on **Settings → Terminal → History search**.
- [ ] **HIST** on an SSH terminal searches that host's shell history. Enter types the command at the prompt (it doesn't run it).
- [ ] **+ Note** on a result starts a runbook note with that command (see [Write runbooks for alerts](kb:howto/runbooks)).
- [ ] Lines that look like passwords or tokens are left out, and nothing is saved.

## Part 3 · Edit server files in Nightgrid

- [ ] Turn on **Settings → Terminal → Edit server files in Nightgrid**.
- [ ] `nano /etc/fstab` (or anything that uses `$EDITOR`) now opens the file in Nightgrid's editor with normal desktop shortcuts. Save, close the tab, and the terminal carries on.
- [ ] It's a session-only shell hook: nothing is installed on the server and it's gone when the shell exits.
- [ ] Root-owned files: run `sudoedit /etc/fstab` instead of `sudo nano`, so the editor works on a copy and sudo writes it back.

## Part 4 · Key hints

- [ ] The strip under the terminal shows the keys of nano, vim, less, `man`, htop, top, btop, tmux, screen, watch and anything following output (`journalctl -f`, `docker logs -f`).
- [ ] **×** hides the hints for that program; **Show hidden** brings them back. Turn the strip off under **Settings → Terminal → Key hints**.
- [ ] DRIFT knows the same keys: ask "how do I quit vim?".

## Part 5 · Send commands from notes and DRIFT

- [ ] Code blocks in a note have **Send to terminal**: typed at the prompt of the note's host, you press Enter.
- [ ] One-line commands in a DRIFT reply have **TERMINAL**. Commands marked DANGER need a second click.

## Limits and gotchas

- Long-running jobs die with the SSH session. Use `tmux` or `screen` on the server for anything that must survive a disconnect, or a systemd unit (see [Schedule jobs with cron and timers](kb:howto/schedules)).
- Bash writes history only when a shell exits, so very recent commands from an open shell may be missing in **HIST**.
- Terminal text sits on the app backdrop; **Settings → Terminal → Solid background** if you prefer a solid panel.

## Related

- [Move files with the Files view](kb:howto/files)
- [Write runbooks for alerts](kb:howto/runbooks)
- [Get the most out of DRIFT](kb:howto/drift)
- [Reading logs](kb:basics/reading-logs)
