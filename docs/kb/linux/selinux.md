# SELinux basics on Fedora and RHEL

Fix "permission denied" that isn't about file permissions on Fedora, RHEL, Alma and Rocky, without turning SELinux off.

## Part 1 · Is it SELinux?

```sh
getenforce
sudo ausearch -m AVC -ts recent
```

- [ ] An AVC denial naming your process and path means SELinux blocked it. Nightgrid's [System](view:system) → **journal** shows these too (search `avc:  denied`).

## Part 2 · The common fixes

- [ ] Container volumes: add `:Z` (private) or `:z` (shared) to bind mounts so they get the container label.
- [ ] A web server serving a non-standard folder:

```sh
sudo semanage fcontext -a -t httpd_sys_content_t "/srv/www(/.*)?"
sudo restorecon -Rv /srv/www
```

- [ ] A service on a non-standard port: `sudo semanage port -a -t http_port_t -p tcp 8081`.
- [ ] A reverse proxy that must connect to backends: `sudo setsebool -P httpd_can_network_connect 1`.
- [ ] Files moved (not copied) keep their old label: `restorecon -Rv <path>` fixes it.

## Part 3 · Last resort, scoped

```sh
sudo ausearch -m AVC -ts recent | audit2allow -M mylocal
sudo semodule -i mylocal.pp
```

- [ ] Read the generated `.te` file first; it allows exactly what was denied.

#### Check

- [ ] The app works with `getenforce` still saying `Enforcing`.

## Limits and gotchas

- `setenforce 0` is fine for a quick test, never as the fix.
- `semanage` is in `policycoreutils-python-utils`.

## Related

- [File permissions, ownership and ACLs](kb:linux/permissions)
- [Podman containers as systemd services (Quadlets)](kb:linux/podman-quadlets)
