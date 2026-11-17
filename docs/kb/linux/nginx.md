# nginx as a web server and reverse proxy

nginx serves static sites and forwards traffic to apps, and it's on almost every Linux server. This article covers the layout of its config, a static site, a reverse proxy that handles WebSockets, HTTPS with certbot, and how to test changes before they break anything.

![nginx as a reverse proxy in front of several apps.](fig:reverse-proxy)

## What you end up with

- nginx from the distro packages, with one file per site
- A static site and a reverse-proxied app on their own names
- Let's Encrypt certificates that renew themselves

## Part 1 · Install and layout

```bash
sudo apt install nginx
```

- [ ] Debian/Ubuntu: site files live in `/etc/nginx/sites-available/` and are enabled with a symlink in `sites-enabled/`. Fedora/RHEL: put them in `/etc/nginx/conf.d/<name>.conf`.
- [ ] Always test, then reload (never restart, so live connections survive):

```bash
sudo nginx -t && sudo systemctl reload nginx
```

## Part 2 · A static site

- [ ] `/etc/nginx/sites-available/docs.example.com`:

```nginx
server {
    listen 80;
    server_name docs.example.com;
    root /var/www/docs;
    index index.html;

    location / {
        try_files $uri $uri/ =404;
    }
}
```

- [ ] `sudo ln -s /etc/nginx/sites-available/docs.example.com /etc/nginx/sites-enabled/`, put files in `/var/www/docs`, test and reload.

## Part 3 · A reverse proxy

```nginx
server {
    listen 80;
    server_name app.example.com;

    client_max_body_size 100m;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

- [ ] The `Upgrade`/`Connection` lines let WebSockets through (chat apps, dashboards, code-server). `client_max_body_size` raises the 1 MB upload default.

## Part 4 · HTTPS

```bash
sudo apt install certbot python3-certbot-nginx
sudo certbot --nginx -d docs.example.com -d app.example.com
```

- [ ] certbot edits the server blocks to listen on 443 with the new certificates and redirect HTTP. Renewal runs from a systemd timer: `systemctl list-timers | grep certbot`.
- [ ] For names that aren't reachable from the internet, use a DNS challenge instead (see [Get TLS certificates with ACME](kb:linux/acme-certs)).

#### Check

- [ ] `curl -I https://app.example.com` returns the app's headers with a valid certificate, and `sudo certbot renew --dry-run` succeeds.

## In Nightgrid

- [Services](view:services) → **Expiry watch → Add…** watches each certificate's expiry, in case renewal silently fails.
- [System](view:system) → **services** shows `nginx`; a failed reload after a bad edit appears there and in the host's timeline.
- If you'd rather click than edit, Nightgrid's proxy route editor handles Caddy and Traefik ([Manage reverse proxy routes](kb:howto/proxy-routes)); for nginx, use [Files](view:files) and the steps above.

## Limits and gotchas

- `nginx -t` checks syntax, not whether the app behind `proxy_pass` is up; a 502 means nginx is fine and the app isn't answering.
- A `server_name` that matches nothing falls back to the first server block (the default site). Remove `sites-enabled/default` once your sites are in place.
- The same port can't be used by nginx and a Docker container; publish containers on other ports or `127.0.0.1`.

## Related

- [Caddy reverse proxy](kb:apps/caddy)
- [Nginx Proxy Manager](kb:apps/nginx-proxy-manager)
- [Get TLS certificates with ACME](kb:linux/acme-certs)
