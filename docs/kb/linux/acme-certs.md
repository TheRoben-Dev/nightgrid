# Get TLS certificates with ACME

Free, auto-renewing certificates from Let's Encrypt (or ZeroSSL) for services that don't sit behind Caddy or Traefik: a mail server, a NAS, an MQTT broker, or a proxy that wants files on disk.

## What you end up with

- A certificate and key in a known path, renewed automatically
- Wildcard certificates for internal names, with no port opened, through DNS validation
- A reload hook so services pick up the new certificate

## Before you start

- A domain you control, with DNS at a provider that has an API (Cloudflare, deSEC, Hetzner, Porkbun and many more)
- An API token limited to editing that zone's DNS

## Part 1 · Pick the client

- [ ] **acme.sh**: a single shell script, dozens of DNS providers, no dependencies. Used below.
- [ ] **certbot**: the reference client, packaged everywhere; DNS plugins per provider (`python3-certbot-dns-cloudflare`).
- [ ] If the service is web and you already run Caddy or Traefik, let the proxy do it instead.

## Part 2 · Install acme.sh

```sh
curl -fsSL https://get.acme.sh | sh -s email=admin@example.com
source ~/.bashrc
acme.sh --set-default-ca --server letsencrypt
```

- [ ] It installs to `~/.acme.sh` and adds a daily cron job for renewals.

## Part 3 · Issue with DNS validation

- [ ] Cloudflare example (other providers use other variables, see `dnsapi` in the acme.sh docs):

```sh
export CF_Token="<token with Zone.DNS edit>"
acme.sh --issue --dns dns_cf -d example.com -d '*.example.com'
```

- [ ] The token is saved in `~/.acme.sh/account.conf` for renewals. Protect that file like a password.

## Part 4 · Install it where the service reads it

```sh
acme.sh --install-cert -d example.com \
  --key-file /etc/ssl/private/example.com.key \
  --fullchain-file /etc/ssl/certs/example.com.pem \
  --reloadcmd "systemctl reload mosquitto"
```

- [ ] Never point services at files inside `~/.acme.sh`; `--install-cert` copies them out and reruns the reload command after every renewal.

#### Check

- [ ] `acme.sh --list` shows the certificate with its renewal date, and `openssl s_client -connect host:port -servername example.com </dev/null | openssl x509 -noout -dates` shows the new expiry.

## In Nightgrid

- [Services](view:services) → **Expiry watch** → **Add…** → **certificate file on a host** with the path from Part 4. Nightgrid reads its end date over SSH twice a day and raises **EXPIRING** if a renewal quietly failed (see [Watch services and expiry dates](kb:howto/service-checks)). HTTPS services are watched automatically.
- Run the issue command in a [Terminal](view:terminal) tab; keep the token out of notes and history (start the line with a space if `HISTCONTROL=ignorespace`).

## Limits and gotchas

- Let's Encrypt has rate limits (50 certificates per domain per week). Test with `--staging` first.
- Certificates show up in public Certificate Transparency logs, names included. A wildcard keeps individual service names out.
- For names that are internal only and not in a public domain, run your own CA instead: [step-ca](kb:apps/step-ca).

## Related

- [Reverse proxies](kb:basics/reverse-proxies)
- [Watch services and expiry dates](kb:howto/service-checks)
- [Caddy reverse proxy](kb:apps/caddy)
