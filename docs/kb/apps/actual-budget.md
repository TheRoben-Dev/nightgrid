# Actual Budget

Envelope-style personal budgeting that syncs between devices through your own server, with optional end-to-end encryption.

## What you end up with

- Actual's sync server at `budget.example.com`
- Budgets on every device, encrypted with a key only you know

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `actual`:

```yaml
services:
  actual:
    image: actualbudget/actual-server:latest
    restart: unless-stopped
    volumes:
      - ./data:/data
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Route `budget.example.com` to `actual:5006`. HTTPS is required for the web app to work.
- [ ] Set the server password on first visit.

## Part 2 · Encryption and import

- [ ] Settings → End-to-end encryption: set a key. The server then stores only encrypted data; losing the key loses the budget.
- [ ] Import from YNAB or CSV, or start fresh.

#### Check

- [ ] A transaction entered on the phone shows on the computer.

## Limits and gotchas

- Back up `./data`, and export a budget file now and then from the app itself.
- Keep it on the tailnet or behind SSO; it's financial data.

## Related

- [Vaultwarden password manager](kb:apps/vaultwarden)
- [Authelia single sign-on](kb:apps/authelia)
