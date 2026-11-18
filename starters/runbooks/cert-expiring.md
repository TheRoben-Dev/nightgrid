---
id: cert-expiring
title: "Runbook: certificate expiring"
summary: Check why a certificate didn't renew by itself and renew it.
alertKinds: [certExpiring, expirySoon]
tags: [runbook, starter, tls]
library: [linux/acme-certs, errors/tls, howto/secret-rotation]
---
A certificate (or a watched domain, token or licence) expires soon. Certificates from Let's Encrypt normally renew 30 days ahead, so an alert usually means renewal has been failing quietly.

- [ ] See which one and when in [Services](view:services), under expiry watch.
- [ ] Check what the server actually presents (replace `example.com`):

```sh
echo | openssl s_client -connect example.com:443 -servername example.com 2>/dev/null | openssl x509 -noout -dates -issuer
```

- [ ] Caddy or Traefik: read the proxy's log for "challenge" or "acme" errors. Common causes are port 80 blocked, DNS pointing elsewhere, or rate limits after many failed tries.
- [ ] certbot: run a dry renewal to see the error:

```sh
sudo certbot renew --dry-run
```

- [ ] Fix the cause, renew, then reload the web server so it serves the new certificate.
- [ ] A domain or token rather than a certificate: renew it where it was bought, then mark it rotated in Security → SECRETS.

More in [ACME certificates](kb:linux/acme-certs) and [TLS errors](kb:errors/tls).
