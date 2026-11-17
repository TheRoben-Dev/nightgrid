# File Browser web file manager

A simple web file manager for a folder on your server: upload, download, share links and edit text files from any browser.

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `filebrowser`:

```yaml
services:
  filebrowser:
    image: filebrowser/filebrowser:latest
    restart: unless-stopped
    user: 1000:1000
    volumes:
      - /srv/share:/srv
      - ./database:/database
      - ./config:/config
    networks: [proxy]
networks:
  proxy:
    external: true
```

- [ ] Route `files.example.com` to `filebrowser:80`. The first start prints a random admin password in the container logs; change it.

#### Check

- [ ] Upload a file from a phone and see it in `/srv/share` with [Files](view:files) in Nightgrid.

## Limits and gotchas

- Share links are public URLs; set expiry on each one.
- For yourself, Nightgrid's [Files](view:files) view already does SFTP with nothing to install.

## Related

- [Move files with the Files view](kb:howto/files)
- [Samba file shares](kb:apps/samba)
