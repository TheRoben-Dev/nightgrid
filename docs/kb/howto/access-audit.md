# Audit who can log in

See who logged in to each host over the last week, who tried and failed, which keys are authorized and who has sudo, without storing anything.

## What you end up with

- Per-host reports: logins with the key used, failed attempts grouped by address, bans, users, sudo rules, authorized keys and sshd settings
- **Findings** that flag what deserves a look
- Nothing saved: the report is read fresh each time

## Part 1 · Run the audit

- [ ] [Fleet](view:fleet) → **Access** → **Audit all**.
- [ ] Click a host for its report. Keys are named when they're yours from Settings → Connections → SSH keys.
- [ ] Much of this needs root. It's read as root when sudo needs no password or Nightgrid remembers it; otherwise **Read as root…** asks.

## Part 2 · Read the findings

- [ ] Password logins enabled with failed attempts and nothing banning them: fix with key-only SSH and fail2ban or CrowdSec.
- [ ] A second uid-0 account: almost always a sign of compromise or an old hack. Investigate before deleting.
- [ ] Unknown authorized keys: remove them from **Settings → Connections → SSH keys → OTHER**.
- [ ] Service accounts with a login shell: give them `/usr/sbin/nologin`.

#### Check

- [ ] Every authorized key is one you can name, and every sudo user is a person you know.

## Part 3 · Follow up

- [ ] Failed attempts from the internet on port 22: don't forward SSH at all; use a tailnet or WireGuard.
- [ ] Repeat the audit monthly, or after anyone leaves.

## Limits and gotchas

- Login history comes from the journal and `wtmp`. If logs rotate quickly, a week may not be available.
- Tailscale SSH logins are authorized by the tailnet, not by `authorized_keys`; check the tailnet policy too.

## Related

- [Raise a host's security score](kb:howto/security-score)
- [Set up and rotate SSH keys](kb:howto/ssh-keys)
- [fail2ban and CrowdSec](kb:apps/fail2ban-crowdsec)
