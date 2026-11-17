# WordPress site in Docker

Run a WordPress blog or website on your own server with MariaDB, behind your reverse proxy with HTTPS, and keep it patched and backed up. Good for a club, a family blog or a portfolio.

## What you end up with

- WordPress and MariaDB in one stack
- HTTPS through Caddy, Traefik or Nginx Proxy Manager, with WordPress aware of it
- Automatic minor updates, a backup routine and a few safety settings

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `wordpress`, with `DB_PASSWORD` and `DB_ROOT_PASSWORD` in `.env`:

```yaml
services:
  wordpress:
    image: wordpress:6-apache
    restart: unless-stopped
    depends_on: [db]
    environment:
      WORDPRESS_DB_HOST: db
      WORDPRESS_DB_USER: wordpress
      WORDPRESS_DB_PASSWORD: ${DB_PASSWORD}
      WORDPRESS_DB_NAME: wordpress
      WORDPRESS_CONFIG_EXTRA: |
        if (isset($$_SERVER['HTTP_X_FORWARDED_PROTO']) && $$_SERVER['HTTP_X_FORWARDED_PROTO'] === 'https') { $$_SERVER['HTTPS'] = 'on'; }
        define('DISALLOW_FILE_EDIT', true);
    volumes:
      - ./html:/var/www/html
    ports:
      - "127.0.0.1:8082:80"

  db:
    image: mariadb:11
    restart: unless-stopped
    environment:
      MARIADB_DATABASE: wordpress
      MARIADB_USER: wordpress
      MARIADB_PASSWORD: ${DB_PASSWORD}
      MARIADB_ROOT_PASSWORD: ${DB_ROOT_PASSWORD}
    volumes:
      - ./db:/var/lib/mysql
```

- [ ] `$$` is how compose writes a literal `$`. The `HTTP_X_FORWARDED_PROTO` line tells WordPress it's on HTTPS behind the proxy; without it you get redirect loops or mixed-content warnings.
- [ ] `DISALLOW_FILE_EDIT` removes the theme and plugin code editor from the admin, a common attack path.
- [ ] Point the proxy at `127.0.0.1:8082` and open the site to run the installer. Use a non-obvious admin user name and a long password.

## Part 2 · Harden and maintain

- [ ] **Settings → General**: site URL with `https://`.
- [ ] Keep plugins few and maintained. Delete, don't just deactivate, ones you don't use.
- [ ] Minor and security updates install automatically; plugin auto-updates can be turned on per plugin. Update the image tag for major PHP/Apache updates.
- [ ] Add rate limiting or a login-protection plugin, or put `/wp-login.php` behind your SSO proxy.

## Part 3 · Backups

- [ ] A database dump plus the `html` folder (uploads, themes, plugins) is a complete backup:

```bash
docker compose exec db sh -c 'mariadb-dump -u root -p"$MARIADB_ROOT_PASSWORD" wordpress' > wordpress.sql
```

- [ ] Schedule it with a timer and keep copies off the host.

#### Check

- [ ] The site loads over HTTPS without warnings, and a restore of the dump plus `html` into a test stack shows the same posts.

## In Nightgrid

- Add the site to [Services](view:services) with expiry watch on its certificate and domain (**Expiry watch → Add…**).
- [System](view:system) → **schedules** → **+ Timer** runs the dump nightly on the host; [Fleet](view:fleet) → **Backups** → **+ Add job** watches that timer.
- [Fleet](view:fleet) → **Logs**: a saved search for `wp-login.php` on the proxy container shows brute-force attempts.

## Limits and gotchas

- Most WordPress compromises come through outdated plugins and themes. Fewer is safer.
- The `html` folder is owned by `www-data` (uid 33) inside the container; fix ownership there if uploads fail.
- A busy site wants page caching (a caching plugin or the proxy) and more PHP memory (`WORDPRESS_CONFIG_EXTRA` with `WP_MEMORY_LIMIT`).

## Related

- [Caddy reverse proxy](kb:apps/caddy)
- [PostgreSQL and MariaDB in Docker](kb:apps/databases)
- [Schedule jobs with cron and timers](kb:howto/schedules)
