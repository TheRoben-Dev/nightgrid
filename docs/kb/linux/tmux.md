# tmux: keep sessions running

Keep long jobs (upgrades, copies, builds) running when your SSH connection drops, and split one terminal into panes on the server side.

![A tmux session survives your SSH connection.](fig:lx-tmux)

## Part 1 · The essentials

```sh
sudo apt install -y tmux
tmux new -s work        # start a named session
tmux ls                 # list sessions
tmux attach -t work     # reattach after a disconnect
```

- [ ] Detach on purpose: `Ctrl+b` then `d`. The job keeps running.
- [ ] Panes: `Ctrl+b` `%` (side by side), `Ctrl+b` `"` (stacked), `Ctrl+b` arrows to move.
- [ ] Nightgrid's key hints strip shows tmux keys while it's in front.

## Part 2 · A small config

- [ ] `~/.tmux.conf`:

```
set -g mouse on
set -g history-limit 50000
set -g base-index 1
```

## Part 3 · When to use what

- [ ] Nightgrid's own splits and tabs are for working on several hosts at once.
- [ ] tmux is for anything that must outlive the connection: `do-release-upgrade`, `rsync` of many GB, `zpool` replaces.
- [ ] Jobs that should run without anyone attached belong in a systemd unit or timer ([Schedule jobs with cron and timers](kb:howto/schedules)).

#### Check

- [ ] Start `sleep 600` in tmux, close the Nightgrid tab, reopen and `tmux attach`: it's still running.

## Limits and gotchas

- `Ctrl+b` clashes with nothing in Nightgrid, but nested tmux (tmux on a host inside tmux locally) needs `Ctrl+b` twice.

## Related

- [Work faster in the terminal](kb:howto/terminal)
- [Upgrade Debian or Ubuntu to a new release](kb:linux/release-upgrade)
