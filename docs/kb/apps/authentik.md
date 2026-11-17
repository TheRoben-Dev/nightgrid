# Authentik single sign-on

A full identity provider: OIDC and SAML logins for apps that support them, a forward-auth proxy for those that don't, MFA, and a user portal. Heavier than Authelia, more capable.

![One authentik account signs you into OIDC apps and proxied ones.](fig:app-oidc)

## What you end up with

- One account per person across Nextcloud, Grafana, Forgejo, Immich and more via OIDC
- Proxy-protected apps for anything without OIDC
- Enrollment and recovery flows you can customise

## Before you start

- 2 CPU cores and 2 GB RAM free.
- A reverse proxy with HTTPS and a name such as `auth.example.com`.

## Part 1 · Install

- [ ] Download the compose file for the current release from Authentik's documentation into a stack folder (it pins matching server, worker and PostgreSQL versions; some versions also need Redis).
- [ ] Generate secrets into `.env` (or the stack's **Vault**):

```sh
echo "PG_PASS=$(openssl rand -base64 36 | tr -d '\n')" >> .env
echo "AUTHENTIK_SECRET_KEY=$(openssl rand -base64 60 | tr -d '\n')" >> .env
```

- [ ] Bring it up from [Docker](view:docker) (the folder shows under Stacks once added).
- [ ] Route `auth.example.com` to the `server` container's port 9000 with your proxy.
- [ ] Open `/if/flow/initial-setup/` on that name to create the admin (`akadmin`) password.

## Part 2 · Connect an app with OIDC

- [ ] Applications → Create with provider → OAuth2/OpenID. Note the client id, secret and the issuer URL.
- [ ] In the app (Grafana, Forgejo, Nextcloud's OIDC app, Immich), enter those and the redirect URL the app expects.

## Part 3 · Protect an app without OIDC

- [ ] Create a Proxy provider in "forward auth (single application)" mode and attach it to the embedded outpost.
- [ ] Caddy:

```
admin.example.com {
    forward_auth authentik-server-1:9000 {
        uri /outpost.goauthentik.io/auth/caddy
        copy_headers X-Authentik-Username X-Authentik-Groups X-Authentik-Email
        trusted_proxies private_ranges
    }
    reverse_proxy app:8080
}
```

## Part 4 · In Nightgrid

- [ ] Add `auth.example.com` to [Services](view:services). If Authentik is down nobody logs in anywhere, so add it as a dependency of the apps behind it in **Settings → Alerts → Dependencies**.
- [ ] Back up the database with a dump (see [PostgreSQL and MariaDB in Docker](kb:apps/databases)) plus the `media` and `certs` folders.

#### Check

- [ ] Logging in to an OIDC app redirects to Authentik and back.

## Limits and gotchas

- Upgrades must go one minor version at a time in order; read the release notes before each.
- Keep a local admin login on critical apps (and on Nightgrid's hosts, which use SSH, not SSO) in case Authentik is down.

## Related

- [Authelia single sign-on](kb:apps/authelia)
- [Caddy reverse proxy](kb:apps/caddy)
- [Update containers without fear](kb:howto/container-updates)
