# Private certificate authority with step-ca

Issue trusted HTTPS certificates for LAN-only names (`nas.home.lan`) with your own ACME server, so Caddy and Traefik renew internal certs automatically like they do with Let's Encrypt.

![Your own CA, an ACME client and devices that trust it.](fig:app-step-ca)

## What you end up with

- A root CA you install once on your devices
- An ACME endpoint your reverse proxies use for internal names
- No browser warnings on internal services

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `step-ca`:

```yaml
services:
  step-ca:
    image: smallstep/step-ca:latest
    restart: unless-stopped
    environment:
      DOCKER_STEPCA_INIT_NAME: Home CA
      DOCKER_STEPCA_INIT_DNS_NAMES: ca.home.lan,localhost
      DOCKER_STEPCA_INIT_ACME: "true"
    ports:
      - "9000:9000"
    volumes:
      - ./step:/home/step
```

- [ ] First start initialises the CA and prints the admin password in the logs. Save it in your password manager.
- [ ] Add a DNS record for `ca.home.lan` (Pi-hole or AdGuard local DNS).

## Part 2 · Trust the root

- [ ] The root certificate is `./step/certs/root_ca.crt`. Download it with [Files](view:files) and install it on your computers and phones (keychain / certificate store / profile).
- [ ] On Linux servers: copy it to `/usr/local/share/ca-certificates/home-ca.crt` and run `sudo update-ca-certificates`.

## Part 3 · Use it from Caddy

```
nas.home.lan {
    tls {
        ca https://ca.home.lan:9000/acme/acme/directory
        ca_root /etc/caddy/root_ca.crt
    }
    reverse_proxy 192.168.1.20:5000
}
```

- [ ] Mount the root cert into Caddy at that path. Edit the Caddyfile through Docker → Routes → **Edit as text**.

#### Check

- [ ] `nas.home.lan` opens without a warning on a device that trusts the root.
- [ ] Nightgrid's expiry watch shows the short-lived cert renewing.
- [ ] On a computer that doesn't trust your root yet, its **SERVICE TLS** rows warn "unknown issuer or self-signed". Click **Edit** on the row and tick **Trust this certificate** to hide that for this exact certificate; the service's up/down check still needs **Allow untrusted certificate** on the service itself. With short-lived certificates every renewal is a new certificate, so trusting the root on this computer is the better fix.

## Limits and gotchas

- Default certificate lifetime is 24 hours; renewals are frequent by design.
- Protect `./step/secrets`: whoever has the CA key can impersonate any internal site.

## Related

- [Caddy reverse proxy](kb:apps/caddy)
- [Watch services and expiry dates](kb:howto/service-checks)
