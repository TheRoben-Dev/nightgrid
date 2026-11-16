# Firefly III finances

Track income, spending and budgets with double-entry bookkeeping, rules and reports. More detailed than Actual Budget; good for people who like to categorise everything.

## Part 1 · The stack

- [ ] Firefly III publishes a compose file and `.env` templates (app, database and a cron container). Download them for your version into `/opt/stacks/firefly`.
- [ ] Set at least `APP_KEY` (exactly 32 characters), `APP_URL`, `TZ` and the database passwords. Keep them in the stack's **Vault** with **Import from host .env**.
- [ ] Bring it up in [Docker](view:docker), route `money.example.com` to the app container's port 8080, register the first user.

## Part 2 · Data in

- [ ] The separate Data Importer container imports CSV files from your bank (and some bank APIs where supported).
- [ ] Rules auto-categorise recurring transactions.

#### Check

- [ ] An imported CSV shows transactions with categories applied by your rules.

## Limits and gotchas

- The cron container must run daily for recurring transactions and budgets.
- Financial data: keep it on the tailnet or behind SSO, and back up the database dump.

## Related

- [Actual Budget](kb:apps/actual-budget)
- [PostgreSQL and MariaDB in Docker](kb:apps/databases)
