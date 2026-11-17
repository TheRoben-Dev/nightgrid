# Two-factor codes for SSH logins

Add a TOTP code (from any authenticator app) to SSH logins on a server, without breaking key logins, scripts or Nightgrid. The usual homelab setup: keys work as before, and anyone logging in with a password also needs a code.

![SSH with a key and a one-time code.](fig:lx-ssh-2fa)

## What you end up with

- `pam_google_authenticator` enrolled per user, with emergency codes
- Password logins that ask for a one-time code too
- Key logins (Nightgrid, scripts, backups) untouched, or a code required only from outside your networks

## Before you start

- Key login works for your user. Read [Hardening sshd](kb:basics/sshd-hardening) first.
- The server's clock is right (TOTP codes depend on it): see [Keep server clocks in sync](kb:linux/time-sync).
- Keep one SSH session open the whole time. A mistake in PAM can lock out every new login.

## Part 1 · Enrol a user

- [ ] Install the module:

```bash
sudo apt install libpam-google-authenticator    # Debian/Ubuntu
sudo dnf install google-authenticator qrencode  # Fedora/RHEL (EPEL)
```

- [ ] As the user who will log in (not root), run `google-authenticator` and answer: time-based **y**, update the file **y**, disallow reuse **y**, increase the window **n**, rate limiting **y**.
- [ ] Scan the QR code into your authenticator app and store the emergency scratch codes in your password manager. The secret lives in `~/.google_authenticator`, readable only by that user.

## Part 2 · PAM

- [ ] Edit `/etc/pam.d/sshd` and add this line right after `@include common-auth` (Debian) or `auth substack password-auth` (Fedora):

```text
auth required pam_google_authenticator.so nullok
```

- [ ] `nullok` lets users who haven't enrolled yet log in without a code. Remove it once everyone has.

## Part 3 · sshd

- [ ] Create `/etc/ssh/sshd_config.d/20-2fa.conf`:

```text
UsePAM yes
KbdInteractiveAuthentication yes
AuthenticationMethods publickey keyboard-interactive
```

- [ ] A space between methods means "either": a key alone, or password plus code (both asked through keyboard-interactive). Keys are unaffected.
- [ ] `sudo sshd -t && sudo systemctl reload ssh` (the unit is `sshd` on Fedora).

## Part 4 · Key and code from outside instead (optional)

- [ ] This replaces Part 3: inside your networks a key is enough, from anywhere else a key **and** a code are required, and passwords are never accepted. `/etc/ssh/sshd_config.d/20-2fa.conf` becomes:

```text
UsePAM yes
KbdInteractiveAuthentication yes
PasswordAuthentication no
AuthenticationMethods publickey

Match Address *,!192.168.0.0/16,!10.0.0.0/8,!100.64.0.0/10
    AuthenticationMethods publickey,keyboard-interactive
```

- [ ] A comma means "then": key first, then the keyboard-interactive step. Comment out `@include common-auth` (Debian) or the `password-auth` line (Fedora) in `/etc/pam.d/sshd` so that step asks only for the code. Do this only with this setup: under Part 3 it would let a code alone log in.
- [ ] Remove `nullok` here, or users without a code file get in from outside with a key alone.

#### Check

- [ ] From a second terminal: a password login asks for `Verification code:` and fails with a wrong one; your key login works without a code; Nightgrid still connects.

## In Nightgrid

- Nightgrid answers password prompts but can't type a one-time code, and its background checks never prompt. Give it a key (**Settings → Connections → SSH keys**) and keep the code for password logins, or exempt the networks Nightgrid connects from with the `Match` block.
- [Fleet](view:fleet) → **Access** lists recent logins with the key used, failed attempts and the sshd settings, so you can see who still logs in with a password.

## Limits and gotchas

- `sudo` doesn't ask for the code unless you add the same PAM line to `/etc/pam.d/sudo`.
- Locked out? Use a scratch code, the console (Proxmox, IPMI) or the session you kept open, and remove the PAM line.
- Root logins with a password should stay off entirely (`PermitRootLogin prohibit-password`).

## Related

- [Hardening sshd](kb:basics/sshd-hardening)
- [Audit who can log in](kb:howto/access-audit)
- [Organise SSH with ~/.ssh/config](kb:linux/ssh-config)
