# Errors: users, sudo, PAM and cron

Messages about accounts, passwords, sudo, PAM, cron and time. See also [Users and sudo](kb:linux/users-sudo) and [Reset a root password](kb:linux/reset-root-password).

### <user> is not in the sudoers file. This incident will be reported.
Means: The user isn't allowed to use sudo.
Fix: As root: `usermod -aG sudo <user>` (Debian/Ubuntu) or `-aG wheel` (Fedora/RHEL), then log in again.

### <user> is not in the sudoers file.
Means: Newer sudo's shorter wording of the same problem.
Fix: Add the user to the sudo or wheel group as root.

### Sorry, user <user> is not allowed to execute '<cmd>' as root on <host>.
Means: sudo is configured for that user, but not for that command.
Fix: Check `sudo -l`; adjust the rule with `visudo`.

### sudo: a terminal is required to read the password; either use the -S option to read from standard input or configure an askpass helper
Means: sudo needs a password but there's no terminal (a script, cron, or `ssh host sudo cmd`).
Fix: Use `ssh -t`, `sudo -S` with care, or a NOPASSWD rule for that specific command.

### sudo: a password is required
Means: Non-interactive sudo (`sudo -n`) needs a password.
Fix: Run interactively, or add a narrow NOPASSWD rule.

### sudo: unable to resolve host <host>: Name or service not known
Means: The machine's own hostname isn't in /etc/hosts or DNS. sudo still works, but slowly.
Fix: Add `127.0.1.1 <host>` to /etc/hosts.

### sudo: /etc/sudoers is world writable
Means: sudoers has unsafe permissions, so sudo refuses to run.
Fix: From root (`su -`, or recovery mode): `chmod 440 /etc/sudoers`.

### sudo: parse error in /etc/sudoers near line <n>
Means: A syntax error in sudoers (edited without visudo). sudo is broken until fixed.
Fix: `pkexec visudo`, or boot to recovery/root shell and fix it. Always edit with `visudo`, which checks syntax.

### >>> /etc/sudoers.d/<file>: syntax error near line <n> <<<
Means: visudo found an error in a drop-in file.
Fix: Choose "e" to edit again; fix the line.

### sudo: /etc/sudoers.d/<file> is owned by uid <n>, should be 0
Means: A sudoers drop-in has the wrong owner, so sudo ignores or refuses it.
Fix: `chown root:root` and `chmod 440` the file.

### sudo: effective uid is not 0, is /usr/bin/sudo on a file system with the 'nosuid' option set or an NFS file system without root privileges?
Means: sudo lost its setuid bit, or runs from a nosuid mount or inside a container with no-new-privileges.
Fix: `ls -l /usr/bin/sudo` should show `-rwsr-xr-x root`; reinstall sudo. In containers, sudo usually can't work by design.

### su: Authentication failure
Means: The root (or target) password was wrong, or root has no password (Ubuntu default).
Fix: Use `sudo -i` instead; on Ubuntu root is locked by design.

### su: user <user> does not exist or the user entry does not contain all the required fields
Means: No such user, or a broken /etc/passwd line.
Fix: `getent passwd <user>`; fix the entry with `vipw`.

### useradd: user '<user>' already exists
Means: That user name is taken.
Fix: Pick another name or modify the existing user with usermod.

### useradd: group '<group>' does not exist
Means: The group given with -g/-G doesn't exist.
Fix: `groupadd <group>` first.

### usermod: user <user> is currently used by process <pid>
Means: You can't rename or change the uid of a logged-in user.
Fix: Log that user out (or kill their processes) and run it from another account.

### userdel: user <user> is currently used by process <pid>
Means: The user still has processes.
Fix: Stop them (`pkill -u <user>`) after checking, then delete.

### passwd: Authentication token manipulation error
Means: passwd couldn't update the password: a read-only root filesystem, wrong permissions on /etc/shadow, or a PAM policy failure.
Fix: Check `mount | grep ' / '` for ro (in recovery mode `mount -o remount,rw /`), and `ls -l /etc/shadow`.

### BAD PASSWORD: The password is shorter than <n> characters
Means: pwquality rejected the password.
Fix: Use a longer one (or a passphrase). As root you can override, but don't.

### You are required to change your password immediately (administrator enforced)
Means: The account's password is expired or was set to force a change.
Fix: Set a new password at the prompt; `chage -l <user>` shows the policy.

### Your account has expired; please contact your system administrator
Means: The account's expiry date passed.
Fix: As root: `chage -E -1 <user>`.

### pam_unix(sshd:auth): authentication failure; logname= uid=0 euid=0 tty=ssh ruser= rhost=<ip> user=<user>
Means: A failed password login over SSH (in auth.log or the journal).
Fix: If it's not you, it's brute force: disable password logins and add banning.

### pam_faillock(sshd:auth): Consecutive login failures for user <user> account temporarily locked
Means: Too many failed logins locked the account (Fedora/RHEL default policy).
Fix: `faillock --user <user> --reset` as root, or wait for the unlock time.

### pam_tally2: user <user> (<uid>) tally <n>, deny <n>
Means: Older pam_tally2 locked the account after failures.
Fix: `pam_tally2 --user <user> --reset`.

### pam_systemd(<service>:session): Failed to create session: Maximum number of sessions (<n>) reached, refusing further sessions.
Means: logind's session limit was reached (often stuck cron or SSH sessions).
Fix: `loginctl list-sessions`; terminate stale ones.

### Module is unknown
Means: A PAM config references a module that isn't installed (after removing a package like a 2FA module).
Fix: Remove or fix that line in /etc/pam.d/; keep a root shell open while editing PAM.

### Authentication token is no longer valid; new one required
Means: The password expired.
Fix: Change it with `passwd`, or as root `chage -d <date> <user>`.

### Last login: <details>
Means: Informational.
Fix: Nothing. Check it's a login you recognize.

### crontab: command not found
Means: The cron package isn't installed (minimal images, some Fedora installs use systemd timers only).
Fix: Install cron (`cronie` on Fedora), or use a systemd timer. Nightgrid's System → **Jobs** creates timers for you.

### no crontab for <user>
Means: That user has no crontab yet. Not an error.
Fix: `crontab -e` creates one.

### "/tmp/crontab.<id>":<n>: bad command
Means: A crontab line isn't valid (missing a field, or an environment line with spaces around `=`).
Fix: Each job needs five time fields and a command; `KEY=value` lines without spaces.

### errors in crontab file, can't install.
Means: crontab -e found a syntax error; your edit wasn't saved.
Fix: Choose to edit again; fix the line it names.

### (CRON) info (No MTA installed, discarding output)
Means: A cron job printed output, and cron would mail it but no mail server is installed. The output is lost.
Fix: Redirect output to a log file in the job (`>> /var/log/job.log 2>&1`), or use a systemd timer so it lands in the journal.

### (CRON) error (grandchild #<n> failed with exit status <n>)
Means: A cron job exited with an error.
Fix: Run the command by hand as that user with cron's minimal environment: `env -i /bin/sh -c '<command>'`. Usually a PATH or relative path problem.

### cron job works in the shell but not from cron
Means: Cron runs with a minimal PATH, a different working directory, and no profile.
Fix: Use absolute paths, set PATH at the top of the crontab, `cd` explicitly, and escape `%` as `\%` (cron treats % as a newline).

### Failed to set time: NTP unit is active
Means: You set the time by hand while NTP sync is on.
Fix: Let NTP manage it, or `timedatectl set-ntp false` first.

### System clock synchronized: no
Means: The clock isn't synced with NTP (no network to NTP servers, or the service is off).
Fix: `timedatectl set-ntp true`; check UDP 123 outbound. See [Time sync](kb:linux/time-sync).

### chronyd: System clock wrong by <n> seconds
Means: chrony found a large offset and will correct it (step or slew).
Fix: Let it correct; large offsets at boot are common on devices without an RTC battery.

### Clock skew detected. Your build may be incomplete.
Means: make found files with timestamps in the future (copied from another machine or a wrong clock).
Fix: Sync the clock; `touch` the files or run a clean build.

### ssh-keygen: generating new host keys: <details>
Means: Informational: host keys were created (first boot or after deleting them). Clients will see a changed host key.
Fix: Expected after reinstalls; tell clients to update known_hosts.

### Your password has expired. You must change your password now and login again!
Means: The password expired at login.
Fix: Set a new one; adjust the policy with `chage` if it's unwanted on a homelab box.

### newgrp: failed to crypt password with previous salt: Invalid argument
Means: `newgrp` with a group that has no password set.
Fix: Just log out and in again after `usermod -aG`; or use `sg <group> -c '<cmd>'`.

### id: '<user>': no such user
Means: That user doesn't exist on this host (or in LDAP/SSSD).
Fix: Check the name with `getent passwd`.

### Account locked due to <n> failed logins
Means: A lockout policy (faillock) is active for that account.
Fix: `faillock --user <user> --reset`.

### polkit: Not authorized to perform operation
Means: polkit denied an action (often over SSH where there's no agent).
Fix: Use sudo, or add a polkit rule for that action.
