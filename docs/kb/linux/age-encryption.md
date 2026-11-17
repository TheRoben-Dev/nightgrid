# Encrypt files with age

Encrypt a file before it goes to a USB stick, a cloud bucket or an email, with one small tool and no key servers. age can use its own keys, a passphrase, or the SSH keys you already have.

## What you end up with

- An age key pair for your own files
- Files encrypted to one or several people, including to their SSH public keys
- A way to encrypt in scripts, such as a database dump before upload

## Part 1 · Install and make a key

- [ ] `sudo apt install age` (Debian 12+, Ubuntu 22.04+), `sudo dnf install age`, or `brew install age` on a Mac.
- [ ] `age-keygen -o ~/.config/age/key.txt` prints the public key (`age1…`). The file holds the private key; `chmod 600` it and keep a copy in your password manager.

## Part 2 · Encrypt and decrypt

- [ ] To yourself: `age -r age1yourpublickey… -o notes.tar.age notes.tar`
- [ ] Decrypt: `age -d -i ~/.config/age/key.txt -o notes.tar notes.tar.age`
- [ ] With a passphrase instead (for someone without a key): `age -p -o file.age file`; decrypting asks for it.
- [ ] Several recipients: repeat `-r`, or list public keys one per line in a file and pass `-R recipients.txt`. Any one of them can decrypt.
- [ ] To someone's SSH key: `age -R ~/.ssh/id_ed25519.pub …` or `-R` a file of `ssh-ed25519` lines; they decrypt with `age -d -i ~/.ssh/id_ed25519`. RSA and Ed25519 keys work.

## Part 3 · In scripts

- [ ] age reads stdin and writes stdout, so it fits in pipes: `pg_dump app | gzip | age -r age1… > app-$(date +%F).sql.gz.age`
- [ ] Only the public key is needed to encrypt, so a backup server can encrypt without ever holding a key that decrypts.
- [ ] Restore: `age -d -i key.txt app-2026-11-17.sql.gz.age | gunzip | psql app`

#### Check

- [ ] Encrypt a test file, delete the original, decrypt it on a different machine with the key from your password manager, and compare checksums.

## In Nightgrid

- Edit and schedule such scripts with [Files](view:files) and [System](view:system) → **Schedules** → **+ Timer**; a failed last run shows on the timer.
- Keep the public key, not the private one, on servers. Nightgrid's stack **Vault** handles secrets for compose stacks separately.

## Limits and gotchas

- Lose the private key (or passphrase) and the data is gone; there's no recovery.
- age encrypts files; it doesn't sign them. If you need to prove who made a file, use minisign or GPG signatures in addition.
- Encrypted SSH-key files need the matching private key unlocked; a key with a passphrase asks for it on decrypt.

## Related

- [Backups](kb:basics/backups)
- [restic backups with systemd timers](kb:apps/restic)
- [Write admin scripts that don't bite](kb:linux/bash-scripts)
