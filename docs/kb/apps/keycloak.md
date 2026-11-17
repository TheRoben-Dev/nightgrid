# Keycloak identity server

Single sign-on with OpenID Connect and SAML for every app that supports it: one login, one place to add or remove users, optional two-factor for everyone. Keycloak is the enterprise-grade choice; Authentik and Authelia are lighter. Pick Keycloak if you want realms, fine-grained clients, LDAP federation or SAML.

## What you end up with

- Keycloak 26 with PostgreSQL behind your reverse proxy
- A realm for your homelab with users, groups and TOTP
- One app logging in through OIDC

## Before you start

- A reverse proxy with HTTPS (Caddy, Traefik or Nginx Proxy Manager) and a name such as `auth.example.com`. Keycloak's redirects and cookies assume a stable HTTPS address.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `keycloak`, with `DB_PASSWORD` and `ADMIN_PASSWORD` in `.env`:

```yaml
services:
  db:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_DB: keycloak
      POSTGRES_USER: keycloak
      POSTGRES_PASSWORD: ${DB_PASSWORD}
    volumes:
      - ./db:/var/lib/postgresql/data

  keycloak:
    image: quay.io/keycloak/keycloak:26.0
    restart: unless-stopped
    command: start
    depends_on: [db]
    environment:
      KC_DB: postgres
      KC_DB_URL: jdbc:postgresql://db:5432/keycloak
      KC_DB_USERNAME: keycloak
      KC_DB_PASSWORD: ${DB_PASSWORD}
      KC_HOSTNAME: https://auth.example.com
      KC_PROXY_HEADERS: xforwarded
      KC_HTTP_ENABLED: "true"
      KC_HEALTH_ENABLED: "true"
      KC_BOOTSTRAP_ADMIN_USERNAME: admin
      KC_BOOTSTRAP_ADMIN_PASSWORD: ${ADMIN_PASSWORD}
    ports:
      - "127.0.0.1:8080:8080"
```

- [ ] Point the proxy at `127.0.0.1:8080` and open `https://auth.example.com`. The bootstrap admin is temporary: in the **master** realm create a permanent admin user with the `admin` role, log in as it, and delete the bootstrap one.

## Part 2 · A realm and users

- [ ] Realm dropdown → **Create realm** named `home`. Keep `master` for Keycloak's own admins only.
- [ ] **Users → Add user**, then **Credentials → Set password**. Add groups such as `admins` and `family`.
- [ ] **Authentication → Required actions**: enable **Configure OTP** as a default action to make every user set up TOTP on first login.

## Part 3 · Connect an app

- [ ] **Clients → Create client**, type **OpenID Connect**, a client ID such as `grafana`, **Client authentication** on.
- [ ] **Valid redirect URIs**: the app's callback (for Grafana `https://grafana.example.com/login/generic_oauth`).
- [ ] Copy the secret from the **Credentials** tab. In the app use the issuer `https://auth.example.com/realms/home`; most apps discover everything else from `/.well-known/openid-configuration` under that address.
- [ ] To pass groups to apps: client → **Client scopes** → the dedicated scope → **Add mapper → Group Membership**, token claim name `groups`, full path off.

#### Check

- [ ] Logging in to the app redirects to Keycloak, asks for the TOTP code, and returns you logged in.

## In Nightgrid

- Add `https://auth.example.com/realms/home` to [Services](view:services). With health enabled, `/health/ready` is served on the management port 9000 inside the container; publish it on `127.0.0.1` only if you want a deeper check.
- Add the apps behind Keycloak as depending on it in **Settings → Alerts → Dependencies**, so an outage gives one alert, not ten.
- Back up the `db` folder with a database dump; see [PostgreSQL and MariaDB in Docker](kb:apps/databases).

## Limits and gotchas

- `KC_HOSTNAME` must match the public address exactly, or logins loop with "invalid redirect" or "cookie not found".
- Keycloak takes 20–60 s to start and needs about 1 GB RAM.
- Keep local admin logins on critical apps; if Keycloak is down nobody signs in with SSO.

## Related

- [Authentik single sign-on](kb:apps/authentik)
- [Authelia single sign-on](kb:apps/authelia)
- [LLDAP lightweight user directory](kb:apps/lldap)
