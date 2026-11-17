# SSH keys and agents

An SSH key pair replaces a password with something that can't be guessed or phished: a **private key** that stays on your computer and a **public key** you copy to each server. Nightgrid prefers the SSH agent, so it never needs to read the private key itself.

## Make a key

Ed25519 keys are short, fast and supported everywhere that matters.

```sh
ssh-keygen -t ed25519 -C "laptop-2026"
```

- Accept the default path (`~/.ssh/id_ed25519`) unless you keep several keys.
- Give it a **passphrase**. The agent remembers it after the first use, so you won't type it all day, and a stolen laptop doesn't hand over your servers.
- The comment (`-C`) is only a label. Name the machine, not yourself, so you can tell keys apart in `authorized_keys`.

Or do it in Nightgrid: Settings → **CONNECTIONS** → **SSH KEYS** → **NEW KEY…**. The passphrase goes to the OS keychain only if you tick remember.

## Put the public key on a server

```sh
ssh-copy-id -i ~/.ssh/id_ed25519.pub user@server
```

Without `ssh-copy-id` (Windows), append the `.pub` file's one line to `~/.ssh/authorized_keys` on the server. That folder must be `700` and the file `600`, owned by the user, or sshd ignores it.

In Nightgrid, the **WHERE THEY WORK** table under Settings → **CONNECTIONS** → **SSH KEYS** shows which key works on which host; **+** adds a key to a host and **●** removes it (with a confirmation).

## Use the agent

- **macOS**: the agent runs already. `ssh-add --apple-use-keychain ~/.ssh/id_ed25519` stores the passphrase in the Keychain.
- **Windows**: start the "OpenSSH Authentication Agent" service (Services → set to Automatic), then `ssh-add`.
- **Linux**: most desktops start one. Check with `ssh-add -l`; if it says it can't connect, `eval "$(ssh-agent)"` and `ssh-add`.

`ssh-add -l` lists the keys the agent holds. A host set to **Agent** in Nightgrid uses exactly those.

## Good habits

- One key per computer, not one key copied everywhere. A lost laptop then means removing one line per server.
- Remove keys you no longer use from `authorized_keys`. Fleet → **ACCESS** shows the keys on each host and when they last logged in.
- Never share a private key, paste it into chat, or commit it. The `.pub` file is the only part that leaves your machine.
- Agent forwarding (`ssh -A`) lets the server use your agent while you're connected. Only forward to servers you trust; a jump host (ProxyJump) is safer.

## When a key is refused

Run `ssh -v user@server` and look for `Offering public key` and the reply. Usual causes: the wrong user, permissions on `~/.ssh`, the key not loaded in the agent, or `PubkeyAuthentication no` in the server's sshd config.

## Related

- [Hardening sshd](kb:basics/sshd-hardening): turn off password logins once your key works.
- [Tailscale and WireGuard](kb:basics/tailscale-wireguard): reach SSH without exposing it.
