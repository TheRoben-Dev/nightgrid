# fail2ban and CrowdSec

Ban addresses that keep failing to log in. fail2ban is simple and local; CrowdSec adds shared blocklists and parsers for web apps. Pick one per host.

![From failed logins to a firewall ban.](fig:app-fail2ban)

## What you end up with

- Brute-force SSH attempts banned after a few failures
- Optional protection for web apps behind your reverse proxy
- Bans visible in Nightgrid's access audit

## Before you start

- Key-only SSH first ([Hardening sshd](kb:basics/sshd-hardening)). Banning is a second layer, not a replacement.
- Your own LAN and tailnet ranges at hand, so you can whitelist them.

## Part 1 · fail2ban

- [ ] Install: `sudo apt install -y fail2ban` (Debian 12+ reads the journal; the package sets that up).
- [ ] Create `/etc/fail2ban/jail.local`:

```ini
[DEFAULT]
bantime = 1h
bantime.increment = true
findtime = 10m
maxretry = 5
ignoreip = 127.0.0.1/8 ::1 192.168.0.0/16 100.64.0.0/10
backend = systemd

[sshd]
enabled = true
```

- [ ] `sudo systemctl restart fail2ban`, then `sudo fail2ban-client status sshd`.

## Part 2 · Or CrowdSec

- [ ] Add CrowdSec's package repository as described in its install docs, then:

```sh
sudo apt install -y crowdsec crowdsec-firewall-bouncer-nftables
```

- [ ] It detects SSH and common services at install. Add collections for what you run:

```sh
sudo cscli collections install crowdsecurity/caddy crowdsecurity/nginx
sudo systemctl reload crowdsec
```

- [ ] Whitelist your ranges in `/etc/crowdsec/parsers/s02-enrich/mywhitelists.yaml` (copy the format from `cscli parsers inspect crowdsecurity/whitelists`).
- [ ] See what it's doing: `sudo cscli metrics` and `sudo cscli decisions list`.

## Part 3 · In Nightgrid

- [ ] [Fleet](view:fleet) → **Access** shows fail2ban jails and CrowdSec bans per host.
- [ ] [System](view:system) → **security** clears the "nothing bans brute force" finding.
- [ ] A command widget for current bans: `sudo` isn't allowed in tiles, so use `cscli` as a user in the `crowdsec` group or skip it.

#### Check

- [ ] Fail an SSH login five times from a test machine outside `ignoreip`; it gets banned.

## When Nightgrid is closed

- Both run on the host by themselves.

## Limits and gotchas

- Banning your own IP from outside your whitelist locks you out for `bantime`. Use the console or another host to unban: `sudo fail2ban-client unban <ip>` or `sudo cscli decisions delete --ip <ip>`.
- Containers publishing ports bypass host firewall chains unless the bouncer hooks `DOCKER-USER`; check CrowdSec's docs for your bouncer.
- Don't run fail2ban and CrowdSec on the same host for the same service.

## Related

- [Hardening sshd](kb:basics/sshd-hardening)
- [Audit who can log in](kb:howto/access-audit)
- [Raise a host's security score](kb:howto/security-score)
