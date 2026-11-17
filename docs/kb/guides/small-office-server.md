# Small office server

![One server for a small office's daily tools.](fig:guide-small-office-server)

One server for a small shop or office of 2–15 people: shared files, calendars, passwords, a task board for jobs, document signing, scanned paperwork and backups of the workstations. Everything self-hosted, reachable from the office and from phones via a tailnet, and watched from Nightgrid.

## What you end up with

- A quiet server (or a mini PC plus a NAS) with Docker stacks for the office's daily tools
- Staff logins for each app, and an admin who isn't one of the daily users
- Workstation backups, server backups off-site, and alerts that reach you

## Before you start

- Hardware: a used business desktop or mini PC with 16–32 GB RAM and two SSDs in a mirror is plenty. Add a UPS; offices lose power more than homes do.
- A domain for HTTPS names, even if the apps are only reachable internally (DNS challenge certificates work without opening ports).
- A short list of who needs what. Fewer apps used well beats many apps nobody opens.

## Part 1 · Base system

- [ ] Install Debian with a mirrored system disk ([Software RAID with mdadm](kb:linux/mdadm-raid) or ZFS), then follow [Prepare a Linux Docker host](kb:apps/docker-host).
- [ ] Add it to Nightgrid ([Add your first host](kb:howto/first-host)) and give it a static address.
- [ ] Put a reverse proxy in front of everything ([Caddy reverse proxy](kb:apps/caddy)) with names like `files.office.example.com`.
- [ ] Remote access for staff phones and laptops: a tailnet with the server as subnet router ([Tailscale subnet router](kb:apps/tailscale-subnet-router)), instead of opening ports.

## Part 2 · The daily tools

- [ ] **Passwords**: [Vaultwarden password manager](kb:apps/vaultwarden) with an organisation for shared logins (supplier portals, Wi-Fi, accounts).
- [ ] **Files**: [Samba file shares](kb:apps/samba) for the office PCs, or [Nextcloud](kb:apps/nextcloud) if people also need files on phones and sharing links.
- [ ] **Calendars and contacts**: [Radicale calendars and contacts](kb:apps/radicale) with a shared office calendar.
- [ ] **Jobs and tasks**: [Kanboard project boards](kb:apps/kanboard) with columns that match the work (Quote, Booked, In progress, Waiting, Done).
- [ ] **Paperwork**: [Paperless-ngx documents](kb:apps/paperless-ngx) with the office scanner's scan-to-folder pointed at its consume folder, and [DocuSeal document signing](kb:apps/docuseal) for forms customers sign.
- [ ] **Writing help**: [LanguageTool grammar checker](kb:apps/languagetool) so emails and quotes get checked without text leaving the office.

## Part 3 · Accounts

- [ ] Give each person their own login in each app; no shared "office" account except where an app has no sharing (a shared Radicale calendar user is fine).
- [ ] With more than a handful of people, one login for everything saves time: [LLDAP lightweight user directory](kb:apps/lldap) with [Authelia single sign-on](kb:apps/authelia) in front of apps that support it.
- [ ] Keep a separate admin account for yourself, used only for setup.

## Part 4 · Backups

- [ ] Workstations: [UrBackup for Windows and Linux clients](kb:apps/urbackup) for image and file backups of the office PCs.
- [ ] Server: [restic backups with systemd timers](kb:apps/restic) of the stack folders and database dumps, to a second disk and to an off-site target.
- [ ] Watch them in [Fleet](view:fleet) → **Backups** and do a **test restore** every month ([Monitor backups and test restores](kb:howto/backups)).

## Part 5 · Watch it from Nightgrid

- [ ] Add every app to [Services](view:services), with **Expiry watch → Add…** for the domain and certificates.
- [ ] Connect the UPS ([UPS monitoring with NUT](kb:howto/ups)) so a long outage shuts the server down cleanly.
- [ ] [Fleet](view:fleet) → **Updates** weekly, outside opening hours; **Disks** for SMART and fill-up forecasts.
- [ ] Set alert rules so only real problems notify ([Tune alerts so only real problems notify](kb:howto/alerts)), and write runbook notes for the things staff call you about ([Write runbooks for alerts](kb:howto/runbooks)).

#### Check

- [ ] A staff member can, from their phone off-site over the tailnet: open a shared file, see the office calendar, look up a shared password and move a job card. Then restore yesterday's version of a file from backup in under ten minutes.

## When Nightgrid is closed

- Everything runs on the server; backups and power schedules are systemd timers there. Nightgrid adds the dashboards, alerts and update runs when it's open.

## Limits and gotchas

- Customer data brings legal duties (GDPR and similar). Keep personal data in as few apps as possible, encrypt off-site backups, and know how you'd delete a customer's records.
- An office without you needs a written "what to do if the server is down" note and a second person who can at least power-cycle it.
- Email itself is better left to a hosted provider; self-hosted mail is a project of its own.

## Related

- [Mini PC homelab](kb:guides/mini-pc-homelab)
- [Remote family IT](kb:guides/remote-family-it)
- [Repair bench server](kb:guides/repair-bench-server)
