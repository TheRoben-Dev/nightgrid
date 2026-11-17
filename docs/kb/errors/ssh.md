# Errors: SSH, keys and remote access

Messages from the ssh client, sshd, scp, sftp, rsync over SSH and ssh-agent. See also [SSH keys](kb:howto/ssh-keys), [SSH config](kb:linux/ssh-config) and [SSH tunnels](kb:linux/ssh-tunnels).

### Permission denied (publickey).
Means: The server accepts only keys and none you offered matched an entry in that user's authorized_keys.
Fix: Check the user name, then `ssh -v user@host` to see which keys are offered. On the server: the key in `~/.ssh/authorized_keys`, `~/.ssh` mode 700, the file 600, home not group-writable. Nightgrid's SSH keys page shows which key works where.

### Permission denied (publickey,password).
Means: Neither a key nor the password worked.
Fix: Check user, password (keyboard layout), and `journalctl -u ssh -n 30` on the server for the reason (wrong user, bad permissions, AllowUsers).

### Permission denied, please try again.
Means: The password was wrong (or the account can't log in with a password, for example root with `PermitRootLogin prohibit-password`).
Fix: Check the account and sshd's `PermitRootLogin`/`PasswordAuthentication`.

### Authentication refused: bad ownership or modes for directory <dir>
Means: sshd (StrictModes) ignored authorized_keys because the home directory or ~/.ssh is writable by others.
Fix: `chmod go-w ~; chmod 700 ~/.ssh; chmod 600 ~/.ssh/authorized_keys` and make the user own them.

### Authentication refused: bad ownership or modes for file <file>
Means: authorized_keys has unsafe permissions or the wrong owner.
Fix: `chown <user>: <file>; chmod 600 <file>`.

### userauth_pubkey: key type ssh-rsa not in PubkeyAcceptedAlgorithms
Means: A new OpenSSH server refuses old SHA-1 RSA signatures (ssh-rsa) from an old client or key.
Fix: Use an ed25519 key. If you must, allow it for that client only with `PubkeyAcceptedAlgorithms +ssh-rsa`.

### no matching host key type found. Their offer: ssh-rsa,ssh-dss
Means: The server only offers old host key types that your client disabled (old NAS, switch or router firmware).
Fix: For that host only, in ~/.ssh/config: `HostKeyAlgorithms +ssh-rsa` and `PubkeyAcceptedAlgorithms +ssh-rsa`. Update the device if you can.

### no matching key exchange method found. Their offer: <list>
Means: Client and server share no key exchange algorithm (an old device with diffie-hellman-group1-sha1).
Fix: For that host only: `KexAlgorithms +diffie-hellman-group14-sha1` (or what the offer lists).

### no matching cipher found. Their offer: <list>
Means: No common cipher (old device using aes-cbc or 3des).
Fix: For that host only: `Ciphers +aes128-cbc`. Prefer updating the device.

### Unable to negotiate with <ip> port <port>: no matching MAC found
Means: No common message authentication algorithm.
Fix: For that host only: `MACs +hmac-sha1`.

### Host key verification failed.
Means: The host's key isn't in known_hosts and you're in batch mode, or it changed and you refused it.
Fix: Connect once interactively to accept it after checking the fingerprint, or see the "REMOTE HOST IDENTIFICATION HAS CHANGED" entry.

### WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!
Means: The host's key differs from the saved one: the server was reinstalled, the IP now belongs to another machine, or someone is intercepting.
Fix: Verify the new fingerprint on the server's console (`ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub`). Then `ssh-keygen -R <host>` and connect again.

### The authenticity of host '<host> (<ip>)' can't be established.
Means: First connection to that host: its key isn't in known_hosts yet.
Fix: Compare the fingerprint with the server's own (`ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub`) before typing yes.

### Offending ECDSA key in <file>:<n>
Means: The line in known_hosts with the old key for that host.
Fix: `ssh-keygen -R <host>` removes it (after verifying the new key is legitimate).

### Warning: Permanently added '<host>' (<type>) to the list of known hosts.
Means: Informational: the host key was saved.
Fix: Nothing.

### ssh: connect to host <host> port <port>: Connection refused
Means: The host is up but nothing listens on that port: sshd stopped, a different port, or a firewall that rejects.
Fix: On the console: `systemctl status ssh` (or sshd), `ss -ltnp | grep ssh`. Check the Port setting.

### ssh: connect to host <host> port <port>: Connection timed out
Means: No answer at all: the host is down, a firewall drops packets, or the address is wrong or unreachable from here (off LAN without VPN).
Fix: `ping <host>`, check you're on the right network or tailnet, and the firewall (ufw, the router, cloud security groups).

### ssh: connect to host <host> port <port>: No route to host
Means: Your computer has no path to that address, or a host firewall rejects with an ICMP error.
Fix: `ip route get <ip>`, check the VPN/tailnet, and the server's firewall.

### ssh: Could not resolve hostname <host>: Name or service not known
Means: DNS or /etc/hosts doesn't know that name.
Fix: Use the IP, add a Host entry with HostName in ~/.ssh/config, or fix DNS. MagicDNS names need Tailscale running.

### ssh: Could not resolve hostname <host>: nodename nor servname provided, or not known
Means: macOS's wording of a failed name lookup.
Fix: Same as above: check the name, DNS, or use the IP.

### kex_exchange_identification: read: Connection reset by peer
Means: The server closed the connection before the SSH handshake: MaxStartups exceeded, fail2ban or a firewall blocking you, TCP wrappers, or something other than sshd on that port.
Fix: Check from the console: `journalctl -u ssh`, `fail2ban-client status sshd`. Try from another IP.

### kex_exchange_identification: Connection closed by remote host
Means: Same family: the server hung up during the first exchange (rate limiting, a ban, or a broken sshd).
Fix: Check sshd's log and any ban lists; `sshd -t` checks its config.

### Connection closed by <ip> port <port>
Means: The server closed the connection, often during authentication (too many failures, AllowUsers/DenyUsers, or a ban).
Fix: `ssh -v` shows how far it got; the server's journal says why.

### Received disconnect from <ip> port <port>:2: Too many authentication failures
Means: Your agent offered more keys than MaxAuthTries allows before trying the right one.
Fix: Add `IdentitiesOnly yes` and `IdentityFile` for that host in ~/.ssh/config.

### Connection reset by <ip> port <port>
Means: The TCP connection was reset: a firewall, an idle timeout on a NAT/router, or the server process died.
Fix: Add `ServerAliveInterval 30` to keep idle sessions alive; check the server's log.

### client_loop: send disconnect: Broken pipe
Means: The connection died while idle or the network changed (Wi-Fi switch, sleep, NAT timeout).
Fix: `ServerAliveInterval 30` and `ServerAliveCountMax 4` in ~/.ssh/config; use tmux on the server so work survives.

### Timeout, server <host> not responding.
Means: Keep-alives went unanswered (ServerAliveInterval), so the client gave up.
Fix: Network trouble between you and the host; tmux keeps sessions running.

### packet_write_wait: Connection to <ip> port <port>: Broken pipe
Means: Older wording: the connection dropped while writing.
Fix: Keep-alives and tmux as above.

### Load key "<file>": bad permissions
Means: Your private key file is readable by others, so ssh refuses it.
Fix: `chmod 600 <file>`.

### WARNING: UNPROTECTED PRIVATE KEY FILE!
Means: Same: the private key's permissions are too open.
Fix: `chmod 600 <file>` (on Windows, remove other users from the file's ACL).

### Load key "<file>": invalid format
Means: The file isn't a private key ssh understands: a public key, a PuTTY .ppk, or a damaged copy (Windows line endings, a missing final newline).
Fix: Convert .ppk with PuTTYgen ("Export OpenSSH key"); check the file starts with `-----BEGIN OPENSSH PRIVATE KEY-----` and ends with a newline.

### Load key "<file>": error in libcrypto
Means: The key file is damaged or has a missing trailing newline (common when pasted into an env var or secret store).
Fix: Make sure it ends with a newline and has no CRLF line endings.

### sign_and_send_pubkey: signing failed for <type> "<file>" from agent: agent refused operation
Means: The agent has the key but refused to sign: a hardware key not touched, the agent locked, or wrong key permissions.
Fix: Touch the security key; `chmod 600` the key; restart the agent (`ssh-add -D`, then add again).

### Could not open a connection to your authentication agent.
Means: ssh-add can't find an agent (SSH_AUTH_SOCK not set in this shell).
Fix: `eval "$(ssh-agent -s)"`, then `ssh-add`.

### Error connecting to agent: No such file or directory
Means: SSH_AUTH_SOCK points at a socket that no longer exists (old tmux session, expired forwarding).
Fix: Start a new agent, or in tmux refresh the variable (`tmux setenv`), or reconnect with `-A`.

### Agent admitted failure to sign using the key.
Means: The agent couldn't use the key (often a GNOME keyring quirk with newer key types).
Fix: `ssh-add <key>` explicitly, or use OpenSSH's own agent.

### Enter passphrase for key '<file>':
Means: The key is encrypted. Not an error; if it repeats every time, no agent is caching it.
Fix: `ssh-add <file>` once per session, or `AddKeysToAgent yes` in ~/.ssh/config (and `UseKeychain yes` on macOS).

### Bad owner or permissions on <file>
Means: ~/.ssh/config (or a file it includes) is writable by others.
Fix: `chmod 600 ~/.ssh/config` and own it.

### <file> line <n>: Bad configuration option: <option>
Means: An unknown or misspelled option in ssh_config, or one too new for this ssh version.
Fix: Check spelling (`man ssh_config`); `ssh -V` shows the version.

### /etc/ssh/sshd_config line <n>: Bad configuration option: <option>
Means: sshd doesn't know that option; sshd won't start with it.
Fix: `sudo sshd -t` checks the config; fix the line before restarting sshd, or you may lock yourself out.

### sshd: no hostkeys available -- exiting.
Means: sshd has no host keys (deleted, or a cloned image without them).
Fix: `sudo ssh-keygen -A` creates the default host keys, then start sshd.

### Missing privilege separation directory: /run/sshd
Means: sshd started without its runtime directory (often in containers or after a /run cleanup).
Fix: `sudo mkdir -p /run/sshd && sudo chmod 755 /run/sshd`; the packaged unit creates it normally.

### error: Bind to port <port> on 0.0.0.0 failed: Address already in use.
Means: Something else (or a second sshd, or ssh.socket) already listens on that port.
Fix: `sudo ss -ltnp | grep :<port>`. On Ubuntu 22.10+, ssh.socket owns port 22: change ports via a socket override, not sshd_config alone.

### error: kex_exchange_identification: banner line contains invalid characters
Means: Something other than an SSH server answered on that port (an HTTP server, a VPN), or your client spoke to a non-SSH service.
Fix: Check what listens there: `nc <host> <port>` shows the banner.

### User <user> from <ip> not allowed because not listed in AllowUsers
Means: sshd's AllowUsers (or AllowGroups) doesn't include that user.
Fix: Add the user to AllowUsers or the allowed group, `sshd -t`, then reload sshd.

### Invalid user <user> from <ip> port <port>
Means: Someone tried to log in with a user that doesn't exist on this host. On internet-facing servers this is constant background scanning.
Fix: Nothing for scans; use keys only and fail2ban/CrowdSec. If it's you, check the user name.

### Failed password for <user> from <ip> port <port> ssh2
Means: A wrong password for an existing user.
Fix: If it's not you and it's frequent, disable password logins and add banning.

### error: maximum authentication attempts exceeded for <user> from <ip> port <port> ssh2
Means: The client tried more authentications than MaxAuthTries.
Fix: Client side: `IdentitiesOnly yes`. Server side: it's usually a brute-force attempt.

### Disconnected from authenticating user <user> <ip> port <port> [preauth]
Means: The connection ended before login completed: a scanner, or a client that gave up.
Fix: Usually noise. Watch for many in a row from one address.

### error: PAM: Authentication failure for <user> from <ip>
Means: The password check via PAM failed.
Fix: Wrong password, a locked account (`passwd -S <user>`), or a PAM module (2FA) failing.

### fatal: Access denied for user <user> by PAM account configuration [preauth]
Means: PAM's account phase refused the login: expired account or password, pam_access, or a locked user.
Fix: `chage -l <user>`, `passwd -S <user>`; check /etc/security/access.conf.

### This account is currently not available.
Means: The user's shell is /usr/sbin/nologin or /bin/false.
Fix: Intended for service accounts. Use `sudo -u <user> bash` for admin work, or change the shell with `usermod -s /bin/bash <user>` if it should log in.

### Could not chdir to home directory <dir>: No such file or directory
Means: The user's home directory doesn't exist, so the session starts in /.
Fix: `sudo mkhomedir_helper <user>` or create it and chown it.

### PTY allocation request failed on channel 0
Means: The server couldn't give you a terminal: /dev/pts not mounted (containers), no free ptys, or `no-pty` in authorized_keys.
Fix: Check `mount | grep devpts` on the server; remove `no-pty` if set unintentionally. `ssh -T` works without one.

### Pseudo-terminal will not be allocated because stdin is not a terminal.
Means: ssh was run with input from a pipe or script, so no terminal is set up.
Fix: Harmless for commands. Force one with `ssh -t` if a program needs it.

### channel <n>: open failed: administratively prohibited: open failed
Means: The server refused a port forward or socket forward (AllowTcpForwarding no, AllowStreamLocalForwarding no, or PermitOpen limits).
Fix: Enable forwarding in sshd_config for that user, or use a jump host that allows it.

### channel <n>: open failed: connect failed: Connection refused
Means: The forward reached the server, but nothing listened at the target address from the server's point of view.
Fix: Check the target host:port is right as seen from the SSH server (localhost there is the server itself).

### bind [127.0.0.1]:<port>: Address already in use
Means: The local port for your -L forward is taken (often by a previous ssh still running).
Fix: Pick another local port, or kill the old forward (`lsof -i :<port>`).

### Warning: remote port forwarding failed for listen port <port>
Means: The server couldn't open the -R port: in use, a privileged port without root, or forwarding disabled.
Fix: Use a port above 1024 that's free; check `AllowTcpForwarding` and `GatewayPorts`.

### X11 forwarding request failed on channel 0
Means: The server doesn't allow X11 forwarding or lacks xauth.
Fix: `X11Forwarding yes` in sshd_config and install xauth on the server.

### subsystem request failed on channel 0
Means: The server has no SFTP subsystem configured, so sftp and newer scp fail.
Fix: Add `Subsystem sftp internal-sftp` (or the sftp-server path) to sshd_config and reload. Old servers: `scp -O` uses the legacy protocol.

### scp: Connection closed
Means: scp (which uses SFTP since OpenSSH 9) couldn't start, often because the server lacks the sftp subsystem or a login script prints text.
Fix: Try `scp -O` (legacy mode); remove output from the remote shell's rc files for non-interactive shells.

### protocol version mismatch -- is your shell clean?
Means: rsync or scp got unexpected text from the remote shell's startup files (an echo in .bashrc).
Fix: Make rc files silent for non-interactive shells: wrap output in `[[ $- == *i* ]]`.

### rsync: connection unexpectedly closed (<n> bytes received so far) [sender]
Means: The SSH side of rsync failed: authentication, rsync missing on the remote host, or a dropped connection.
Fix: Run `ssh <host> rsync --version`; check the error above it; install rsync on both ends.

### rsync error: some files/attrs were not transferred (see previous errors) (code 23)
Means: rsync finished, but some files failed (permissions, vanished files, or unsupported attributes on the target).
Fix: Scroll up for the specific files. Run as root for system files; drop `-A`/`-X` for targets that don't support ACLs or xattrs.

### rsync warning: some files vanished before they could be transferred (code 24)
Means: Files were deleted during the copy (logs, temp files). Usually harmless.
Fix: Exclude temp paths, or snapshot first for consistent backups.

### rsync: [Receiver] mkstemp "<file>" failed: Permission denied (13)
Means: The receiving side can't write into that directory.
Fix: Check ownership on the target; run the receiver as the right user (`--rsync-path="sudo rsync"`).

### Host key for <host> has changed and you have requested strict checking.
Means: The host key changed and StrictHostKeyChecking stops the connection.
Fix: Verify the new key on the server, then `ssh-keygen -R <host>`.

### Bad port '<port>'
Means: The port given with -p or in ssh_config isn't a valid number.
Fix: Check `-p` and the Port line; `host:port` syntax isn't accepted by ssh (use `-p`).

### ssh_dispatch_run_fatal: Connection to <ip> port <port>: message authentication code incorrect
Means: Data was corrupted in transit: a faulty NIC, bad RAM, MTU or offload problems, or a broken middlebox.
Fix: Try another network path, disable NIC offloads, check MTU along a VPN; test RAM if it happens everywhere.

### Connection to <host> closed by remote host.
Means: The server ended the session: a reboot, sshd restart, idle timeout (ClientAliveInterval), or the session's process was killed.
Fix: Check the server's uptime and journal; tmux keeps work alive across drops.

### Shared connection to <host> closed.
Means: Informational: a command run over a multiplexed (ControlMaster) or `-t` connection finished.
Fix: Nothing.

### ControlSocket <path> already exists, disabling multiplexing
Means: A stale ControlMaster socket is left from a dead connection.
Fix: Delete the socket file, or use `ssh -O exit <host>`.

### mux_client_request_session: session request failed: Session open refused by peer
Means: The multiplexed master connection hit MaxSessions on the server.
Fix: Raise `MaxSessions` in sshd_config, or open fewer parallel sessions.

### Bad stdio forwarding specification '<spec>'
Means: ProxyJump/ProxyCommand with `-W` got an invalid host:port.
Fix: Check the jump host syntax: `ProxyJump user@jump:22`.

### channel 0: open failed: connect failed: Name or service not known
Means: A jump host couldn't resolve the final target's name.
Fix: Use a name the jump host can resolve, or its IP.

### Tailscale SSH: this action requires additional check; please visit <url>
Means: The tailnet's SSH policy uses check mode: approve the login in a browser.
Fix: Open the link and approve; Nightgrid shows this as an Approve-in-browser card. See [Tailscale SSH](kb:howto/tailscale-ssh).

### tailscale: tailnet policy does not permit you to SSH to this node
Means: The ACL's ssh rules don't allow your user to that host or as that login user.
Fix: Add a rule in the tailnet's access controls (`ssh` section) for that source, destination and user.
