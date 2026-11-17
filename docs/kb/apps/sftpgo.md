# SFTPGo file transfer server

A full-featured SFTP, FTP/S and WebDAV server with its own users, per-user folders and quotas, and a web client for people who'd rather use a browser. Good for giving a customer, a scanner or a backup tool a login that can only reach one folder, without creating Linux accounts.

## What you end up with

- SFTPGo in Docker with an admin account and a web client
- Virtual users, each locked to their own folder, with optional quotas
- SFTP on a separate port, so the host's own SSH stays untouched

## Part 1 · The stack

- [ ] [Docker](view:docker) → **+ New stack** named `sftpgo`:

```yaml
services:
  sftpgo:
    image: drakkan/sftpgo:latest
    restart: unless-stopped
    volumes:
      - ./data:/srv/sftpgo
      - ./home:/var/lib/sftpgo
    ports:
      - "127.0.0.1:8080:8080"
      - "2022:2022"
```

- [ ] The container runs as uid 1000; if it can't write, `sudo chown -R 1000:1000 data home` and redeploy.
- [ ] Open the web admin through **⧉** or your reverse proxy at `/web/admin` and create the first admin account on the setup page straight away.

## Part 2 · Users

- [ ] **Users → Add**: username, password and/or an SSH public key. The **home directory** defaults to a folder under `/srv/sftpgo/data/<user>`; each user is locked inside it.
- [ ] Set **quota** (size and number of files) and **permissions** (for an upload-only scanner: upload and list, no download or delete).
- [ ] **Virtual folders** map extra host paths (mounted into the container) into a user's view, such as a shared read-only `/media` folder.
- [ ] People log in to the web client at `/web/client` to upload and download, and can create share links if you allow it.

#### Check

- [ ] `sftp -P 2022 alice@<host>` logs in, lands in Alice's folder, and `cd /` doesn't leave it. An upload over the limit is refused when a quota is set.

## In Nightgrid

- [Files](view:files) can't use SFTPGo's virtual users (it logs in with your host account), but you can browse the same `data` folder there as the host user.
- Add the web UI to [Services](view:services); [System](view:system) → **FIREWALL** to allow port 2022 only from where it's needed.

## Limits and gotchas

- FTP and FTPS are off by default; turn them on only for old devices that need them (some scanners and cameras), and prefer FTPS.
- Exposing SFTP to the internet invites password guessing. Use keys, or turn on SFTPGo's built-in defender (it blocks repeat offenders) and limit by IP.
- Users are stored in SFTPGo's own database (SQLite in `home` by default); back that up with the data.

## Related

- [SFTP-only accounts locked to one folder](kb:linux/sftp-chroot)
- [File Browser web file manager](kb:apps/filebrowser)
- [Samba file shares](kb:apps/samba)
