# Hardening sshd

SSH is the front door to every server. A few settings close the usual ways in without making daily use harder. Do these after your key login works.

![SSH hardening, layer by layer.](fig:bs-sshd-layers)

## Before you change anything

- Keep your current SSH session open while you edit and reload. If the new settings lock you out, that session is your way back; sshd has no automatic rollback.
- Check the config before reloading: `sudo sshd -t` prints nothing when it's valid.

## The settings that matter

Put your changes in a drop-in file so package updates don't overwrite them:

```sh
sudo tee /etc/ssh/sshd_config.d/10-hardening.conf <<'CONF'
PasswordAuthentication no
KbdInteractiveAuthentication no
PermitRootLogin prohibit-password
PubkeyAuthentication yes
MaxAuthTries 3
CONF
sudo sshd -t && sudo systemctl reload ssh
```

- **PasswordAuthentication no**: keys only. Bots try passwords all day; they can't try a key.
- **PermitRootLogin prohibit-password**: root may log in with a key only (Proxmox needs root over SSH). Use `no` on servers where you always log in as a normal user and use sudo.
- **MaxAuthTries 3**: fewer guesses per connection.

On some distributions the service is `sshd`, not `ssh`.

## Check it

From another terminal, `ssh -o PubkeyAuthentication=no user@server` should now fail with "Permission denied (publickey)". In Nightgrid, System → **SECURITY** checks password logins, root login, fail2ban and old keys, and explains each fix.

## Extras

- **fail2ban** or **CrowdSec** ban addresses that keep failing. With passwords off they matter less, but they keep logs quiet.
- **Firewall**: allow port 22 only from your LAN and your tailnet. System → **FIREWALL** in Nightgrid arms a rollback before applying, so a mistake undoes itself.
- **Changing the port** cuts log noise but isn't security on its own.
- Don't expose SSH to the internet at all if you can reach the server over Tailscale or WireGuard instead.

## If you lock yourself out

Use the server's console: Proxmox's console for a VM or container, a monitor and keyboard, or your hosting provider's web console. Remove the drop-in file and reload sshd.

## Related

- [SSH keys and agents](kb:basics/ssh-keys): set up the key login first.
- [Reading logs](kb:basics/reading-logs): `journalctl -u ssh` shows who tried to log in.
