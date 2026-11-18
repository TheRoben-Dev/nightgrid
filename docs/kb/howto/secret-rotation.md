# Rotate secrets and certificates

Keep every key, token and password on a schedule: see what's overdue in one list, change each one where it lives, and let Nightgrid remind you before the next one is due. Nightgrid never reads or shows a secret's value here, only names and dates.

## What you end up with

- Every secret Nightgrid knows about on [Security](view:security) → **SECRETS**, with its age and due date
- A rotation period per secret that fits it (a year for SSH keys, six months for API tokens)
- A notice 30 days before a secret is due, and again when it's overdue

## Before you start

- Turn on **Security → Secrets** in **Settings → Features** if it's off.
- For SSH keys, keep a second session open on each host until the new key is proven to work.

## Part 1 · See what you have

- [ ] Open [Security](view:security) → **SECRETS**. Overdue items come first, then those due within 30 days.
- [ ] Items that say **NO DATE** were never stamped. If you know one was changed recently, press **Mark rotated today** to start its clock.
- [ ] Add what lives outside Nightgrid with **+ Add one kept elsewhere**: the router admin password, a DNS provider's API token, a cloud account's recovery codes. Write where it's used, never the value.

## Part 2 · Rotate an SSH key

- [ ] Open the key and press **Find hosts that accept it**. It reads `authorized_keys` on every server host.
- [ ] Make a new key in **Settings → Connections → SSH keys** and push it to each of those hosts ([Set up and rotate SSH keys](kb:howto/ssh-keys)).
- [ ] Point each host at the new key in its connection settings and connect once to prove it.
- [ ] Remove the old key from every host, then delete or archive the old file. The new key's file date restarts its clock.

## Part 3 · Rotate tokens and passwords

- [ ] Integration tokens: make a new token in the service's own admin page, paste it in [Services](view:services) → the integration's **Edit**, and revoke the old one there. Saving stamps the date.
- [ ] `.env` vault values: change the password in the app first if it has one (a database user, for example), then the value in [Docker](view:docker) → **STACKS** → **.env vault**, and redeploy the stack.
- [ ] Nightgrid's own tokens: **New token** for the MCP server or the status page link, then update the clients or bookmarks that used the old one.
- [ ] Host passwords: change it on the host with `passwd`, then connect once and tick **remember** to replace the saved copy.
- [ ] Anything rotated outside Nightgrid: **Mark rotated today**.

## Part 4 · Certificates and expiry dates

- [ ] Certificates and domains come from [Services](view:services) → **EXPIRY WATCH**: add a certificate file or a domain there and its date shows in the list.
- [ ] Let's Encrypt certificates renew themselves; a certificate that's still due soon a week before it expires means renewal is failing. Check the proxy's logs.

#### Check

- [ ] Nothing on **SECRETS** shows **OVERDUE**.

## Part 5 · Keep it there

- [ ] Set a period per secret with **Rotate every**: shorter for tokens with wide access, **never** for something that can't be rotated.
- [ ] Leave **Notify when one is due** on. Each due date sends one notice, routed like every other notice in **Settings → Alerts → Notifications**.

## Limits and gotchas

- Nightgrid only knows what it set or was told. A token changed in the service's own page needs **Mark rotated today** here.
- An SSH key's date is its file's date. Copying the file to a new computer resets it; mark it with the real date if it matters.
- Rotating a secret is only done when the old one stops working: revoke tokens and remove old keys.

## Related

- [Set up and rotate SSH keys](kb:howto/ssh-keys)
- [Raise your fleet's security score](kb:guides/security-posture)
- [Audit who can log in](kb:howto/access-audit)
