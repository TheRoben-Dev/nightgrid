# Errors: TLS, certificates and ACME

Messages from openssl, certbot, acme.sh, Caddy and Traefik certificate handling, and TLS errors in apps. See also [ACME certificates](kb:linux/acme-certs).

### SSL certificate problem: self-signed certificate
Means: The server's certificate is signed by itself, not by a CA the client trusts.
Fix: Trust it explicitly (add it, or your private CA, to the client's store), or use a real certificate (Let's Encrypt via DNS challenge works for LAN names on a public domain).

### SSL certificate problem: self-signed certificate in certificate chain
Means: The chain ends in a root CA the client doesn't trust: a private CA, or a TLS-intercepting proxy or antivirus.
Fix: Install that CA on the client (`/usr/local/share/ca-certificates/` then `sudo update-ca-certificates`).

### SSL certificate problem: unable to get local issuer certificate
Means: The client can't find the issuer: the server doesn't send its intermediate, or the client lacks the root.
Fix: Serve the full chain (fullchain.pem, not cert.pem). Check with `openssl s_client -connect <host>:443 -servername <host> -showcerts`.

### x509: certificate signed by unknown authority
Means: A Go program (Docker, Traefik, many tools) doesn't trust the issuer.
Fix: Add the CA to the system store or the container image. For a private registry, put the CA in /etc/docker/certs.d/<registry>/ca.crt.

### x509: certificate has expired or is not yet valid: current time <time> is after <time>
Means: The certificate expired, or the machine's clock is wrong.
Fix: Check `date` and NTP first. Then renew the certificate.

### x509: certificate is valid for <names>, not <host>
Means: The name you connected to isn't on the certificate.
Fix: Use a listed name, or reissue with the right SANs. Behind a proxy, check the SNI it sends.

### x509: cannot validate certificate for <ip> because it doesn't contain any IP SANs
Means: You connected by IP but the certificate only lists names.
Fix: Connect by name, or issue a certificate with an IP SAN.

### certificate verify failed: unable to get local issuer certificate (_ssl.c:<n>)
Means: Python's ssl module can't verify the chain (often a Python using its own certifi bundle on macOS, or a missing intermediate).
Fix: Install the system CA bundle for Python (`pip install certifi`, or run "Install Certificates.command" on macOS), or point `SSL_CERT_FILE` at your CA bundle.

### unable to verify the first certificate
Means: Node.js couldn't verify the server's certificate, usually a missing intermediate.
Fix: Fix the server's chain; for a private CA set `NODE_EXTRA_CA_CERTS=/path/ca.pem`.

### PKIX path building failed: unable to find valid certification path to requested target
Means: Java doesn't trust the server's certificate chain.
Fix: Import the CA into the JVM truststore with keytool, or use the OS store.

### SSL routines::wrong version number
Means: The client spoke TLS to something that isn't TLS (plain HTTP on that port), or a proxy answered in clear.
Fix: Use the right port or `http://`; check what's listening with `curl -v http://<host>:<port>`.

### SSL routines::unsupported protocol
Means: Client and server have no TLS version in common (an old device with TLS 1.0 against a modern client).
Fix: Update the device. Temporarily, lower the client's minimum version for that host only.

### SSL routines::tlsv1 alert internal error
Means: The server aborted the handshake: often no certificate for the requested SNI name.
Fix: Check the server has a site/cert for that hostname; `openssl s_client -servername <host>`.

### SSL routines::sslv3 alert handshake failure
Means: The handshake failed: no common cipher, a required client certificate, or no cert for that SNI.
Fix: `openssl s_client -connect <host>:443 -servername <host>` for details; check mTLS requirements.

### SSL routines::tlsv13 alert certificate required
Means: The server wants a client certificate (mutual TLS) and you didn't send one.
Fix: Provide the client cert and key, or remove the mTLS requirement.

### SSL_ERROR_RX_RECORD_TOO_LONG
Means: Firefox's version of "spoke TLS to plain HTTP": the HTTPS port isn't serving TLS.
Fix: Check the server listens with TLS on that port (nginx `listen 443 ssl`).

### ERR_CERT_AUTHORITY_INVALID
Means: Chrome doesn't trust the certificate's issuer.
Fix: Use a trusted certificate or install your private CA on the device.

### ERR_CERT_COMMON_NAME_INVALID
Means: The certificate doesn't cover the name in the address bar.
Fix: Reissue with that name, or use a name it covers.

### ERR_SSL_PROTOCOL_ERROR
Means: Chrome couldn't complete the TLS handshake (wrong port, broken server config).
Fix: Test with `openssl s_client`; check the proxy's TLS config.

### NET::ERR_CERT_DATE_INVALID
Means: The certificate is expired or not yet valid, or the device clock is wrong.
Fix: Fix the clock or renew the certificate.

### unable to load certificate
Means: openssl couldn't read the file as a certificate: wrong format (DER vs PEM), or it's a key or a chain in another order.
Fix: `openssl x509 -in <file> -text -noout`; for DER add `-inform der`.

### unable to load Private Key
Means: The file isn't a private key openssl can read, or it's encrypted and no passphrase was given.
Fix: Check the PEM header; decrypt with `openssl pkey -in <file> -out <file>.dec`.

### key values mismatch
Means: nginx/openssl: the private key doesn't belong to that certificate.
Fix: Compare `openssl x509 -noout -pubkey -in cert.pem | sha256sum` with `openssl pkey -pubout -in key.pem | sha256sum`.

### SSL_CTX_use_PrivateKey_file("<file>") failed (SSL: error:<details>:key values mismatch)
Means: nginx: the configured key doesn't match the certificate.
Fix: Point ssl_certificate_key at the matching key; reissue if it's lost.

### Challenge failed for domain <domain>
Means: Let's Encrypt couldn't validate your control of the domain.
Fix: Read the detail below it: DNS not pointing at you, port 80 blocked or redirected, or a wrong DNS-01 record.

### Detail: <ip>: Invalid response from http://<domain>/.well-known/acme-challenge/<token>: 404
Means: The HTTP-01 challenge file wasn't served: another server or proxy answered port 80, or the webroot is wrong.
Fix: Make port 80 reach the ACME client (or the proxy route /.well-known/acme-challenge/ to it).

### Detail: Fetching http://<domain>/.well-known/acme-challenge/<token>: Timeout during connect (likely firewall problem)
Means: Let's Encrypt couldn't reach port 80 on your public IP.
Fix: Forward TCP 80 on the router, open the firewall, or use the DNS-01 challenge (works without open ports; needed for CGNAT).

### Detail: DNS problem: NXDOMAIN looking up A for <domain>
Means: The domain has no A record (yet), or it's a name only your LAN knows.
Fix: Add the record at your DNS provider; for LAN-only names use DNS-01 with a real public domain.

### Detail: No TXT record found at _acme-challenge.<domain>
Means: The DNS-01 TXT record wasn't visible when Let's Encrypt checked (propagation delay, wrong zone, or API credentials lacking permission).
Fix: Increase the propagation wait; check the API token's zone permissions; `dig TXT _acme-challenge.<domain> @1.1.1.1`.

### Error creating new order :: too many certificates already issued for exact set of domains
Means: Let's Encrypt's duplicate certificate rate limit (5 per week for the same names).
Fix: Stop re-issuing: keep the ACME data volume persistent (Caddy /data, Traefik acme.json). Use the staging server for testing.

### too many failed authorizations recently
Means: Rate limit after repeated failed validations.
Fix: Fix the cause, wait an hour, and test against the staging endpoint first.

### urn:ietf:params:acme:error:rateLimited
Means: An ACME rate limit was hit.
Fix: The message names the limit and when it resets. Persist certificates between restarts.

### urn:ietf:params:acme:error:caa
Means: The domain's CAA DNS record doesn't allow this CA to issue.
Fix: Add a CAA record for letsencrypt.org (or your CA), or remove the restrictive one.

### Certbot failed to authenticate some domains (authenticator: <name>).
Means: certbot's summary when validation failed.
Fix: The "Detail:" lines above say why.

### Could not bind TCP port 80 because it is already in use by another process on this system (such as a web server).
Means: certbot --standalone needs port 80 free.
Fix: Use `--nginx`/`--apache` or `--webroot`, or stop the web server during renewal with pre/post hooks.

### Cert not yet due for renewal
Means: Informational: the certificate has more than 30 days left.
Fix: Nothing. `--force-renewal` only for testing (mind rate limits).

### acme: error: 403 :: urn:ietf:params:acme:error:unauthorized
Means: Validation failed: the challenge response was wrong or came from the wrong server.
Fix: Check DNS points to this server and nothing else answers the challenge.

### obtaining certificate: [<domain>] solving challenges: <details>
Means: Caddy (CertMagic) couldn't get a certificate; the rest says why.
Fix: Read the detail: ports, DNS or rate limits. Caddy retries with backoff automatically.

### unable to get local issuer certificate from Caddy's internal CA
Means: Clients don't trust Caddy's local CA (used for `tls internal` and LAN names).
Fix: Install Caddy's root (`/data/caddy/pki/authorities/local/root.crt`) on your devices, or run `caddy trust` on the same machine.

### Unable to obtain ACME certificate for domains "<domain>": unable to generate a certificate for the domains [<domain>]
Means: Traefik's ACME resolver failed for that router.
Fix: Check the resolver name on the router, the challenge type, and the provider credentials in the environment.

### the router <name> uses a non-existent resolver: <resolver>
Means: Traefik router labels name a certresolver that isn't defined in the static config.
Fix: Match `tls.certresolver` to a resolver under `certificatesResolvers`.

### acme.json: permissions 0644 are too open, please use 600
Means: Traefik refuses an ACME store readable by others.
Fix: `chmod 600 acme.json`.

### error:0308010C:digital envelope routines::unsupported
Means: OpenSSL 3 refused a legacy algorithm (commonly Node 17+ builds using MD4 hashing, or old PKCS#12 files).
Fix: Update the tool; for builds `NODE_OPTIONS=--openssl-legacy-provider` as a stopgap; for PKCS#12 use `-legacy` with openssl.

### Verification error: unable to get local issuer certificate
Means: openssl s_client couldn't verify the chain with the system store.
Fix: Same as for curl: missing intermediate or untrusted root.

### Verify return code: 10 (certificate has expired)
Means: openssl s_client found an expired certificate in the chain.
Fix: Renew it; also check for an old intermediate the server still sends.

### Verify return code: 21 (unable to verify the first certificate)
Means: The server sent only its own certificate without intermediates.
Fix: Configure the full chain.
