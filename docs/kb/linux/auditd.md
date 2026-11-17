# Track changes with auditd

Answer "who changed this file?" and "what did root run last night?" with the Linux audit system. Rules watch chosen files and system calls in the kernel, and each event records the user who originally logged in, even after `sudo`.

![From an audit rule to a search result.](fig:lx-auditd)

## What you end up with

- auditd running with a small, focused rule set
- Watches on SSH, sudo and user database files, and on your compose folders
- A log of commands run as root by real users, searchable with `ausearch`

## Part 1 · Install

```bash
sudo apt install auditd audispd-plugins     # Debian/Ubuntu
sudo dnf install audit                      # Fedora/RHEL (usually installed)
sudo systemctl enable --now auditd
```

## Part 2 · Rules

- [ ] Create `/etc/audit/rules.d/50-homelab.rules`:

```text
-w /etc/ssh/sshd_config -p wa -k sshd
-w /etc/ssh/sshd_config.d/ -p wa -k sshd
-w /etc/sudoers -p wa -k sudoers
-w /etc/sudoers.d/ -p wa -k sudoers
-w /etc/passwd -p wa -k users
-w /etc/shadow -p wa -k users
-w /etc/group -p wa -k users
-w /opt/stacks/ -p wa -k stacks
-a always,exit -F arch=b64 -S execve -F euid=0 -F auid>=1000 -F auid!=-1 -k root-cmds
```

- [ ] `-w path -p wa` watches writes and attribute changes; `-k` is the key you search by.
- [ ] The last rule logs every program started as root by someone who logged in as a normal user (via sudo or su). Cron jobs and services don't match because they have no login user.
- [ ] Load the rules: `sudo augenrules --load`, then check them with `sudo auditctl -l`.

## Part 3 · Search

- [ ] Changes to sshd's config, with names instead of numbers:

```bash
sudo ausearch -k sshd -i
sudo ausearch -k root-cmds -i --start today
sudo aureport --summary
sudo aureport -x --summary
```

- [ ] In the output, `auid` is who logged in, `uid` is who they were at that moment (root after sudo), `exe` is the program that changed the file.

#### Check

- [ ] `sudo touch /etc/ssh/sshd_config`, then `sudo ausearch -k sshd -i --start recent` shows the event with your login name as `auid`.

## In Nightgrid

- Audit events go to `/var/log/audit/audit.log`, not the journal, so [Fleet](view:fleet) → **Logs** doesn't search them. Run `ausearch` in the host's [Terminal](view:terminal), or save it as a runbook note in [Knowledge](view:notes).
- [System](view:system) → **config** → **Compare** tells you *what* changed in `/etc`; auditd tells you *who* changed it and when.
- Nightgrid's own work over SSH shows up in audit logs under the account it logs in as, like any other session.

## Limits and gotchas

- The `execve` rule is noisy on busy hosts. Keep it limited to `auid>=1000` as above, and watch `/var/log/audit` size (`max_log_file` and `num_logs` in `/etc/audit/auditd.conf`).
- A root user can disable auditing. Add `-e 2` as the last rule to lock the config until reboot, once you're happy with it.
- Containers' writes to bind-mounted folders are recorded with the container's process, not a login user.

## Related

- [Audit who can log in](kb:howto/access-audit)
- [Track config drift and host history](kb:howto/config-drift)
- [Journald: keep logs, but not forever](kb:linux/journald)
