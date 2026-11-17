# Send mail from your servers

Let cron, smartd, unattended-upgrades, ZFS and apps send email through a real mail provider, using msmtp as a tiny sendmail replacement, or Postfix as a relay for apps that want an SMTP server.

## What you end up with

- `mail` and `sendmail` working on every host, relayed through your provider
- A local SMTP relay for containers that need one
- Root's mail going to your real address

## Before you start

- An SMTP account: your email provider's submission server, or a transactional service. Use an app password, not your main one.

## Part 1 · msmtp (simplest)

- [ ] `sudo apt install -y msmtp msmtp-mta bsd-mailx`.
- [ ] `/etc/msmtprc` (mode 600, owned by root):

```
defaults
auth on
tls on
tls_trust_file /etc/ssl/certs/ca-certificates.crt
logfile /var/log/msmtp.log
aliases /etc/aliases

account provider
host smtp.example.com
port 587
from server-alerts@example.com
user server-alerts@example.com
password <app-password>

account default : provider
```

- [ ] `/etc/aliases`: `root: you@example.com` and `default: you@example.com`.
- [ ] Test: `echo "test from $(hostname)" | mail -s "msmtp test" root`.

## Part 2 · Postfix relay (for containers and apps)

- [ ] `sudo apt install -y postfix libsasl2-modules` (choose "Satellite system").
- [ ] In `/etc/postfix/main.cf`:

```
relayhost = [smtp.example.com]:587
smtp_sasl_auth_enable = yes
smtp_sasl_password_maps = hash:/etc/postfix/sasl_passwd
smtp_sasl_security_options = noanonymous
smtp_tls_security_level = encrypt
inet_interfaces = 127.0.0.1, 172.17.0.1
mynetworks = 127.0.0.0/8 172.16.0.0/12
```

- [ ] `/etc/postfix/sasl_passwd`: `[smtp.example.com]:587 user:app-password`, then `sudo postmap /etc/postfix/sasl_passwd && sudo chmod 600 /etc/postfix/sasl_passwd*` and `sudo systemctl restart postfix`.
- [ ] Containers send to `172.17.0.1:25` (the Docker bridge) without auth.

## Part 3 · Use it

- [ ] smartd: `-m root` in `smartd.conf` (see [Keep an eye on disks and pools](kb:howto/disks)).
- [ ] unattended-upgrades: `Unattended-Upgrade::Mail "root";`.
- [ ] ZFS: `ZED_EMAIL_ADDR="root"` in `/etc/zfs/zed.d/zed.rc`.

#### Check

- [ ] The test mail arrives, and `/var/log/msmtp.log` (or `journalctl -u postfix`) shows it sent.

## Limits and gotchas

- Many residential ISPs block port 25 outbound; use 587 or 465 to your provider.
- Credentials live in files readable by root only. Nightgrid's config drift masks them in diffs.
- Sending as a domain you own needs SPF/DKIM set up at the provider, or mail lands in spam.

## Related

- [Automatic security updates](kb:apps/unattended-upgrades)
- [ntfy push notifications](kb:apps/ntfy)
- [Keep an eye on disks and pools](kb:howto/disks)
