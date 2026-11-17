# Authelia single sign-on

Put a login page with two-factor authentication in front of apps that have weak or no login of their own, using Authelia with your reverse proxy's forward-auth.

![Forward auth: the proxy checks every request with Authelia first.](fig:app-forward-auth)

## What you end up with

- One login (with TOTP or a passkey) for all your admin apps
- Per-domain rules: bypass for the LAN, two-factor from outside
- Users in a simple YAML file, no database server needed

## Before you start

- A reverse proxy with HTTPS (see [Caddy reverse proxy](kb:apps/caddy)) and a parent domain, such as `auth.example.com` for Authelia and `*.example.com` for apps.
- An SMTP server for password reset mails, or the filesystem notifier for testing (see [Send mail from your servers](kb:apps/mail-relay)).

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `authelia`:

```yaml
services:
  authelia:
    image: authelia/authelia:4
    restart: unless-stopped
    volumes:
      - ./config:/config
    environment:
      AUTHELIA_IDENTITY_VALIDATION_RESET_PASSWORD_JWT_SECRET_FILE: /config/secrets/jwt
      AUTHELIA_SESSION_SECRET_FILE: /config/secrets/session
      AUTHELIA_STORAGE_ENCRYPTION_KEY_FILE: /config/secrets/storage
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Make three random secrets (64+ characters each) in `./config/secrets/`, mode 600:

```sh
mkdir -p config/secrets && for s in jwt session storage; do openssl rand -hex 64 > config/secrets/$s; done && chmod 600 config/secrets/*
```

## Part 2 · Configure

- [ ] `./config/configuration.yml` (the essentials):

```yaml
server:
  address: tcp://0.0.0.0:9091
authentication_backend:
  file:
    path: /config/users.yml
access_control:
  default_policy: deny
  rules:
    - domain: "*.example.com"
      networks: [192.168.0.0/16]
      policy: one_factor
    - domain: "*.example.com"
      policy: two_factor
session:
  cookies:
    - domain: example.com
      authelia_url: https://auth.example.com
storage:
  local:
    path: /config/db.sqlite3
notifier:
  filesystem:
    filename: /config/notification.txt
```

- [ ] Hash a password for `users.yml`:

```sh
docker run --rm authelia/authelia:4 authelia crypto hash generate argon2 --password 'choose-a-password'
```

- [ ] `./config/users.yml`:

```yaml
users:
  alice:
    displayname: Alice
    password: "<argon2 hash>"
    email: alice@example.com
    groups: [admins]
```

## Part 3 · Wire up Caddy

- [ ] A site for Authelia and a snippet for protected apps:

```
auth.example.com {
    reverse_proxy authelia:9091
}

admin.example.com {
    forward_auth authelia:9091 {
        uri /api/authz/forward-auth
        copy_headers Remote-User Remote-Groups Remote-Email Remote-Name
    }
    reverse_proxy app:8080
}
```

- [ ] Save in Nightgrid's route editor (**Edit as text**) and reload.

#### Check

- [ ] Opening `admin.example.com` from outside the LAN asks for login and a second factor.

## Limits and gotchas

- Protect the app, not only the domain: make sure the app's own port isn't published, or the login can be bypassed.
- Mobile and desktop clients that talk to an API (Jellyfin apps, Nextcloud sync) can't do forward-auth logins; bypass their API paths or use OIDC where the app supports it.
- Keep the `secrets` folder in backups; losing the storage key makes the database unreadable.

## Related

- [Authentik single sign-on](kb:apps/authentik)
- [Caddy reverse proxy](kb:apps/caddy)
- [Reverse proxies](kb:basics/reverse-proxies)
