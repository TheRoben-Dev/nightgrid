# LLDAP lightweight user directory

One list of users and groups that your apps log in against over LDAP: Jellyfin, Nextcloud, Gitea/Forgejo, Authelia, Proxmox and more. Much simpler than OpenLDAP, with a web UI for adding people and resetting passwords.

![LLDAP as the user directory for apps and SSO.](fig:app-ldap)

## What you end up with

- Users and groups in one place, managed from a web page
- Apps that check passwords against it instead of each keeping their own
- A base for SSO with Authelia or Authentik

## Part 1 · The stack

- [ ] Generate two secrets: `openssl rand -base64 32` twice, into the stack's `.env` as `JWT_SECRET` and `KEY_SEED`, plus an `ADMIN_PASSWORD`.
- [ ] [Docker](view:docker) → **+ New stack** named `lldap`:

```yaml
services:
  lldap:
    image: lldap/lldap:stable
    restart: unless-stopped
    ports:
      - "3890:3890"
      - "17170:17170"
    environment:
      TZ: Etc/UTC
      LLDAP_JWT_SECRET: ${JWT_SECRET}
      LLDAP_KEY_SEED: ${KEY_SEED}
      LLDAP_LDAP_BASE_DN: dc=example,dc=lan
      LLDAP_LDAP_USER_PASS: ${ADMIN_PASSWORD}
    volumes:
      - ./data:/data
```

- [ ] 3890 is LDAP, 17170 the web UI. Log in as `admin` with `ADMIN_PASSWORD`.

## Part 2 · Users and a bind account

- [ ] Create groups per app or role (`media`, `git`, `admins`) and add users.
- [ ] Create a user `bind` in the `lldap_strict_readonly` group: apps use it to look users up, so they never hold the admin password.

## Part 3 · Connect an app

- [ ] The settings most apps ask for:

```
Server:   ldap://lldap-host:3890
Bind DN:  uid=bind,ou=people,dc=example,dc=lan
Base DN:  dc=example,dc=lan
Users:    ou=people,dc=example,dc=lan   filter (memberOf=cn=media,ou=groups,dc=example,dc=lan)
Username attribute: uid
```

#### Check

- [ ] Log in to the connected app with an LLDAP user; remove them from the group and the login fails.

## In Nightgrid

- [Fleet](view:fleet) → **Access** audits SSH logins on your hosts; LLDAP covers app logins. Use both when reviewing who can get in.
- Back up `./data` (SQLite) with the stack; losing it locks everyone out of every connected app.

## Limits and gotchas

- Plain LDAP sends passwords unencrypted; keep 3890 on a Docker network or the LAN, or turn on LDAPS with a certificate.
- `LLDAP_LDAP_USER_PASS` only sets the admin password on first start; change it in the UI later.

## Related

- [Authelia single sign-on](kb:apps/authelia)
- [Authentik single sign-on](kb:apps/authentik)
- [Audit who can log in](kb:howto/access-audit)
