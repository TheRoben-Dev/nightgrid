# Mailpit test mail catcher

A fake mail server that accepts every email and shows it in a web inbox instead of delivering it. Point apps at it while you set them up, so password-reset and notification mails can be checked without spamming real addresses or configuring a relay first.

![Mailpit catching test mail.](fig:app-mailpit)

## What you end up with

- Mailpit in Docker with SMTP on port 1025 and a web inbox
- Apps on the same host (or LAN) sending their test mail to it
- A clear switch-over later to your real mail relay

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `mailpit`:

```yaml
services:
  mailpit:
    image: axllent/mailpit:latest
    restart: unless-stopped
    environment:
      MP_MAX_MESSAGES: "1000"
      MP_DATABASE: /data/mailpit.db
    volumes:
      - ./data:/data
    ports:
      - "127.0.0.1:8025:8025"
      - "127.0.0.1:1025:1025"
```

- [ ] To reach it from other stacks, put them on a shared Docker network and use `mailpit:1025` as the SMTP host, or bind 1025 to the host's LAN address for other machines.

## Part 2 · Point an app at it

- [ ] In the app's mail settings: host `mailpit` (or the host's address), port `1025`, no TLS, no authentication.
- [ ] Trigger a test mail (password reset, "send test email"); it appears in the inbox at once, with HTML and text views, headers and a spam/HTML compatibility check.
- [ ] When the app works, switch it to your real relay ([Send mail from your servers](kb:apps/mail-relay)) and test again.

#### Check

- [ ] `swaks --to test@example.com --server 127.0.0.1:1025` (or the app's test button) shows a message in the web inbox.

## In Nightgrid

- **⧉** in [Docker](view:docker) opens the inbox in a window, or use [Tunnels](view:tunnels) → **OPEN WEB UI VIA** from another host.

## Limits and gotchas

- Mailpit never delivers mail (unless you configure its relay feature); don't leave a production app pointed at it, or real users never get their mails.
- The web UI and SMTP have no login by default. Keep both on `127.0.0.1` or a private network, or set `MP_UI_AUTH_FILE` for a password.
- `MP_MAX_MESSAGES` caps storage; older messages are deleted first.

## Related

- [Send mail from your servers](kb:apps/mail-relay)
- [Dev and git deploy lab](kb:guides/dev-git-deploy-lab)
- [Docker networks: bridge, shared proxy network and macvlan](kb:linux/docker-networks)
