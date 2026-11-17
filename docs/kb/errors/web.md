# Errors: web servers and reverse proxies

Messages from nginx, Caddy, Traefik, Apache, and the HTTP status codes they return. See also [nginx](kb:linux/nginx), and Nightgrid's proxy route editor in [Docker](view:docker).

### 502 Bad Gateway
Means: The proxy works but couldn't get a valid answer from the app behind it: the app is down, listening on another port, or on a network the proxy can't reach.
Fix: Check the upstream container is running and the proxy targets the right host:port. In Docker, proxy and app must share a network and the proxy should use the container name, not localhost.

### 503 Service Unavailable
Means: The server or proxy has no healthy backend right now (all failed health checks, maintenance mode, or overloaded).
Fix: Check the backend's health and the proxy's upstream config; in Traefik, a router without a working service returns 503.

### 504 Gateway Timeout
Means: The proxy waited too long for the app (slow request, stuck app, or a firewall dropping traffic to it).
Fix: Check the app's logs and load; raise proxy timeouts (`proxy_read_timeout`) only for genuinely slow endpoints.

### 404 page not found
Means: Traefik's default answer when no router matches the request's host or path.
Fix: Check the router rule (Host(`…`)), the entrypoint, and that the container has `traefik.enable=true` and shares a network with Traefik.

### 400 Bad Request: The plain HTTP request was sent to HTTPS port
Means: nginx: a client spoke plain HTTP to a port configured for TLS.
Fix: Use https://, or redirect port 80 to 443 instead of pointing HTTP at 443.

### 413 Request Entity Too Large
Means: The upload is bigger than the proxy allows (nginx default 1 MB).
Fix: Raise `client_max_body_size` (nginx), or the equivalent in the app; Nextcloud and Immich need large values.

### 414 Request-URI Too Large
Means: The URL is longer than the server's buffer.
Fix: Raise `large_client_header_buffers` (nginx), or avoid huge query strings.

### 421 Misdirected Request
Means: The TLS name (SNI) and the Host header point at different sites, often with HTTP/2 connection reuse across names sharing a certificate.
Fix: Give each site its own certificate or make the proxy's SNI handling consistent; check `proxy_ssl_server_name on` for HTTPS upstreams.

### 426 Upgrade Required
Means: The server requires a protocol upgrade (often WebSocket or HTTP/2) that the request didn't make.
Fix: Pass Upgrade/Connection headers through the proxy for WebSocket apps.

### 429 Too Many Requests
Means: A rate limit (the app, the proxy or an API) was hit.
Fix: Slow down; check `limit_req` in nginx or the app's limits.

### 401 Unauthorized
Means: The request needs authentication and none (or a wrong one) was given.
Fix: Check credentials, API tokens, and that the proxy forwards the Authorization header.

### 403 Forbidden
Means: The server understood the request but refuses it: file permissions, an allow/deny rule, an index directory without autoindex, or a WAF rule.
Fix: nginx: check the error log (permission denied on the file, or "directory index is forbidden"). Make the web root readable by the server user.

### 500 Internal Server Error
Means: The app crashed or threw an error while handling the request.
Fix: The reason is in the app's own log (not the proxy's): `docker logs <app>`.

### 301 Moved Permanently loop (ERR_TOO_MANY_REDIRECTS)
Means: A redirect loop, usually an app that thinks it's on HTTP behind a TLS proxy and keeps redirecting to HTTPS.
Fix: Send `X-Forwarded-Proto` from the proxy and tell the app it's behind a trusted proxy. With Cloudflare, set SSL mode to Full, not Flexible.

### nginx: [emerg] bind() to 0.0.0.0:<port> failed (98: Address already in use)
Means: Another process already listens on that port (another nginx, Apache, or a container).
Fix: `sudo ss -ltnp 'sport = :<port>'`; stop the other one or change the port.

### nginx: [emerg] unknown directive "<directive>" in <file>:<n>
Means: A typo, a directive from a module that isn't loaded, or a missing semicolon on the line before.
Fix: Check the previous line ends with `;`; load the module if it's a module directive.

### nginx: [emerg] "<directive>" directive is not allowed here in <file>:<n>
Means: The directive is in the wrong block (for example `server` outside `http`, or `location` outside `server`).
Fix: Move it to the right context.

### nginx: [emerg] host not found in upstream "<host>" in <file>:<n>
Means: nginx resolves upstream names at start, and this one didn't resolve (a container not yet up, or a typo).
Fix: Make sure the name resolves when nginx starts, or use a variable with `resolver 127.0.0.11` (Docker DNS) so it's resolved at request time.

### nginx: [emerg] cannot load certificate "<file>": BIO_new_file() failed (SSL: error:<details>:No such file or directory)
Means: The certificate path doesn't exist (or isn't readable).
Fix: Check the path and that certbot actually issued it.

### nginx: [emerg] a duplicate default server for 0.0.0.0:<port> in <file>:<n>
Means: Two server blocks both claim `default_server` on the same port.
Fix: Keep `default_server` on only one.

### nginx: [warn] conflicting server name "<name>" on 0.0.0.0:<port>, ignored
Means: Two server blocks use the same server_name and port; one is ignored.
Fix: Remove the duplicate (often a leftover file in sites-enabled).

### nginx: configuration file /etc/nginx/nginx.conf test failed
Means: `nginx -t` found an error; the line before says which.
Fix: Fix that, run `nginx -t` again, then reload.

### connect() failed (111: Connection refused) while connecting to upstream
Means: nginx couldn't connect to the backend address (nothing listening).
Fix: Check the backend is up on that host:port; containers bound to 127.0.0.1 aren't reachable from another container.

### upstream timed out (110: Connection timed out) while reading response header from upstream
Means: The backend accepted the connection but didn't respond in time.
Fix: Look at the backend's load and logs; raise `proxy_read_timeout` for slow endpoints.

### upstream prematurely closed connection while reading response header from upstream
Means: The backend closed the connection without a response (it crashed or restarted mid-request, or a protocol mismatch like HTTP to an HTTPS backend).
Fix: Check the backend's logs; use `https://` in proxy_pass for TLS backends.

### no live upstreams while connecting to upstream
Means: nginx marked every server in the upstream as failed.
Fix: Fix the backends; they come back after `fail_timeout`.

### open() "<file>" failed (13: Permission denied)
Means: nginx's worker user can't read the file or one of its parent directories (or SELinux blocks it).
Fix: `namei -l <file>`; give the nginx user read and execute on the path. On SELinux: `restorecon -Rv` or `httpd_sys_content_t`.

### open() "<file>" failed (2: No such file or directory)
Means: A request asked for a file that isn't in the root (often a favicon, or a wrong `root`/`alias`).
Fix: Check `root` vs `alias` in the location; a missing favicon is harmless.

### directory index of "<dir>" is forbidden
Means: The request hit a directory without an index file and autoindex is off.
Fix: Add an index.html, set `index`, or route to the app.

### rewrite or internal redirection cycle while internally redirecting to "<uri>"
Means: A `try_files` or rewrite loops back to itself.
Fix: Make the last try_files item a real file or `=404`, or the app's front controller.

### client intended to send too large body: <n> bytes
Means: Logged with 413 errors: the upload exceeded client_max_body_size.
Fix: Raise `client_max_body_size` in that server block.

### SSL_do_handshake() failed (SSL: error:<details>) while SSL handshaking to upstream
Means: nginx couldn't do TLS to an HTTPS upstream (self-signed backend, SNI missing).
Fix: `proxy_ssl_server_name on`; for self-signed backends `proxy_ssl_verify off` only on trusted networks.

### Error: adapting config using caddyfile: <details>
Means: Caddy couldn't parse the Caddyfile; the details give the file and line.
Fix: `caddy validate --config /etc/caddy/Caddyfile` and `caddy fmt --overwrite` to tidy it. Nightgrid's proxy editor validates before saving.

### Error: loading initial config: loading new config: http app module: start: listening on :<port>: listen tcp :<port>: bind: address already in use
Means: Caddy's port is already taken.
Fix: Find the other listener (`ss -ltnp`); in Docker only one container can own 80/443.

### Error: unrecognized directive: <name>
Means: A Caddyfile directive is misspelled, from a plugin that isn't built in, or placed outside a site block.
Fix: Check spelling; plugin directives need a custom build (xcaddy).

### dial tcp <ip>:<port>: connect: connection refused
Means: Caddy (or Traefik) couldn't reach the upstream in reverse_proxy.
Fix: Check the upstream's address, port and that it shares a network with the proxy.

### dial tcp: lookup <host> on 127.0.0.11:53: no such host
Means: Inside Docker, the proxy can't resolve that container name: it's on another network or not running.
Fix: Put both on the same user-defined network; use the service or container name exactly.

### http: TLS handshake error from <ip>:<port>: <details>
Means: A client failed the TLS handshake with Caddy (scanners, old clients, or a name Caddy has no certificate for).
Fix: Usually noise from bots. If real clients fail, check the requested hostname matches a site block.

### level=error msg="Cannot create service: service not found" providerName=docker
Means: Traefik labels point a router at a service that isn't defined, or the container exposes no port.
Fix: Add `traefik.http.services.<name>.loadbalancer.server.port=<port>`.

### level=error msg="field not found, node: <key>"
Means: A Traefik label or file config key is misspelled or in the wrong place.
Fix: Check the key against Traefik's docs for your version (v2 vs v3 differ).

### level=error msg="Router uses a non-existent entryPoint" entryPointName=<name>
Means: The router names an entrypoint that isn't defined in the static config.
Fix: Match the name (often `web`/`websecure`) to your `entryPoints`.

### level=error msg="Skipping service: no backend IP" container=<name>
Means: Traefik found the container but no IP on a network it shares.
Fix: Attach the container to Traefik's network and set `traefik.docker.network=<net>`.

### AH00558: apache2: Could not reliably determine the server's fully qualified domain name
Means: A harmless Apache warning: no ServerName set globally.
Fix: Add `ServerName localhost` to the global config.

### AH00072: make_sock: could not bind to address [::]:<port>
Means: Apache's port is already taken.
Fix: Find the other listener; change Listen or stop the other service.

### AH01630: client denied by server configuration: <path>
Means: Apache's access rules (Require) deny that path.
Fix: Add `Require all granted` to the right Directory block.

### Invalid command '<cmd>', perhaps misspelled or defined by a module not included in the server configuration
Means: Apache doesn't know that directive (module not enabled).
Fix: `a2enmod <module>` (rewrite, proxy, ssl, headers…) and reload.

### WebSocket connection to '<url>' failed
Means: A browser couldn't open a WebSocket through the proxy (missing Upgrade headers or a proxy timeout).
Fix: nginx: `proxy_http_version 1.1; proxy_set_header Upgrade $http_upgrade; proxy_set_header Connection "upgrade";`. Caddy and Traefik handle it automatically.

### Blocked loading mixed active content "<url>"
Means: An HTTPS page tried to load a script over HTTP, usually because the app builds URLs with http:// behind a TLS proxy.
Fix: Set the app's public URL to https:// and forward `X-Forwarded-Proto`.

### Invalid Host header
Means: The app (often a dev server or Home Assistant) refuses requests for host names it doesn't know or from proxies it doesn't trust.
Fix: Add the proxy to the app's trusted proxies and the domain to its allowed hosts.

### 400: Bad Request (Home Assistant: A request from a reverse proxy was received from <ip>, but your HTTP integration is not set-up for reverse proxies)
Means: Home Assistant blocks proxied requests until you trust the proxy.
Fix: In configuration.yaml: `http: use_x_forwarded_for: true` and `trusted_proxies: [<proxy ip or subnet>]`.

### CSRF verification failed. Request aborted.
Means: A Django-based app (Paperless, Healthchecks, etc.) rejected a form because the origin doesn't match its configured URL.
Fix: Set the app's URL / CSRF_TRUSTED_ORIGINS to the https:// address you use through the proxy.
