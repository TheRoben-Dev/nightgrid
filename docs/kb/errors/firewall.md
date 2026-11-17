# Errors: firewalls, iptables and nftables

Messages from ufw, firewalld, iptables, nftables and fail2ban. See also [nftables](kb:linux/nftables) and Nightgrid's System → **Firewall**, which applies ufw and firewalld changes behind an automatic rollback.

### ERROR: Couldn't determine iptables version
Means: ufw can't run iptables (missing package, or an iptables backend mismatch in containers).
Fix: Install iptables; in LXC containers, manage the firewall on the host instead.

### ERROR: problem running ufw-init
Means: ufw failed loading its rules, often because of a syntax error in /etc/ufw/*.rules or a missing kernel module.
Fix: `sudo ufw disable && sudo ufw enable` shows the failing line; restore the rules file from a backup.

### Skipping adding existing rule
Means: Not an error: that ufw rule is already there.
Fix: Nothing; `sudo ufw status numbered` lists rules.

### ERROR: Could not find a profile matching '<name>'
Means: `ufw allow <app>` uses an application profile that isn't installed.
Fix: `sudo ufw app list`; use a port instead (`ufw allow 8080/tcp`).

### WARN: Rule changed after normalization
Means: ufw rewrote your rule into its canonical form (for example a host address in a network rule).
Fix: Check `ufw status` shows what you meant.

### Command may disrupt existing ssh connections. Proceed with operation (y|n)?
Means: Enabling ufw with no rule for SSH can lock you out.
Fix: `sudo ufw allow OpenSSH` (or your SSH port) first, then enable.

### ERROR: initcaps
Means: ufw couldn't use ip6tables (IPv6 disabled in the kernel or missing modules).
Fix: Set `IPV6=no` in /etc/default/ufw if the host has no IPv6.

### FirewallD is not running
Means: firewall-cmd can't reach the firewalld daemon.
Fix: `sudo systemctl enable --now firewalld`, or use the tool this host actually runs (ufw, nftables).

### Error: INVALID_ZONE: <zone>
Means: No firewalld zone by that name.
Fix: `firewall-cmd --get-zones` lists them.

### Error: INVALID_SERVICE: <name>
Means: No firewalld service definition by that name.
Fix: `firewall-cmd --get-services`; or open the port with `--add-port=8080/tcp`.

### Warning: ALREADY_ENABLED: <rule>
Means: The rule is already active.
Fix: Nothing.

### Warning: NOT_ENABLED: <rule>
Means: You removed a rule that wasn't there.
Fix: Check `firewall-cmd --list-all --zone=<zone>`.

### Error: COMMAND_FAILED: '<cmd>' failed: <details>
Means: firewalld's backend (nftables or iptables) rejected a rule, often a conflict with Docker or a missing module.
Fix: `journalctl -u firewalld` for the full error; restart firewalld after Docker updates.

### success
Means: firewall-cmd changed the runtime config. If you didn't add `--permanent`, it's gone after a reload or reboot.
Fix: Repeat with `--permanent` and `firewall-cmd --reload`, or `firewall-cmd --runtime-to-permanent`.

### iptables: No chain/target/match by that name.
Means: A rule references a chain that doesn't exist (DOCKER, f2b-sshd) or a match module that isn't loaded.
Fix: `iptables -S` lists chains. Restart the service that creates the chain (Docker, fail2ban) after a firewall flush.

### iptables: Bad rule (does a matching rule exist in that chain?).
Means: `iptables -D` couldn't find an identical rule to delete.
Fix: List with `iptables -S <chain>` and delete by exact spec or by number (`-D <chain> <n>`).

### iptables v<ver> (nf_tables): Couldn't load match `<match>':No such file or directory
Means: The xtables match module isn't available for the nft backend (old rules on a new system).
Fix: Install the extensions package, or switch to `iptables-legacy` if rules depend on it.

### iptables: Resource temporarily unavailable.
Means: Another process holds the xtables lock (Docker, kube-proxy and fail2ban all modify rules).
Fix: Use `iptables -w` to wait for the lock.

### Another app is currently holding the xtables lock. Perhaps you want to use the -w option?
Means: Same: concurrent iptables changes.
Fix: Add `-w`.

### # Warning: iptables-legacy tables present, use iptables-legacy to see them
Means: Rules exist in both the legacy and nf_tables backends, so `iptables` shows only half.
Fix: Pick one backend (`update-alternatives --config iptables`); usually move everything to nf_tables.

### Error: Could not process rule: No such file or directory
Means: nft can't find the table, chain or set your command refers to.
Fix: `nft list ruleset` to see what exists; create the table/chain first.

### Error: syntax error, unexpected <token>
Means: nftables couldn't parse the rule or file.
Fix: Check the line it points at; `nft -c -f <file>` checks a file without applying it.

### Error: Could not process rule: Operation not supported
Means: The kernel lacks a feature that rule needs (a missing module, or nft features too new for the kernel).
Fix: Load the module (`modprobe nft_<name>`); update the kernel.

### Error: Could not process rule: Device or resource busy
Means: Deleting a table or chain that's still referenced.
Fix: Delete the rules that jump to it first, or flush the table.

### Error: conflicting intervals specified
Means: An nftables set got overlapping ranges.
Fix: Merge the ranges, or add `flags interval; auto-merge` to the set.

### fail2ban.jail [<pid>]: ERROR Failed during configuration: Have not found any log file for <jail> jail
Means: The jail points at a log path that doesn't exist (the service logs to the journal instead).
Fix: Set `backend = systemd` for that jail, or correct `logpath`.

### ERROR NOK: ('<jail>',)
Means: fail2ban-client failed for that jail (not running, or misconfigured).
Fix: `sudo fail2ban-client -d` dumps the parsed config; `journalctl -u fail2ban`.

### fail2ban.actions [<pid>]: NOTICE [<jail>] Ban <ip>
Means: Informational: fail2ban banned that address.
Fix: `fail2ban-client set <jail> unbanip <ip>` to undo; add your own ranges to `ignoreip`.

### ERROR Failed to access socket path: /var/run/fail2ban/fail2ban.sock. Is fail2ban running?
Means: fail2ban-client can't reach the server.
Fix: `sudo systemctl start fail2ban` and check its journal for config errors.

### Bans aren't working with Docker: traffic still gets through
Means: Docker's published ports go through the FORWARD chain (DOCKER-USER), not INPUT, so INPUT bans don't apply.
Fix: Set fail2ban's `chain = DOCKER-USER` for jails protecting containers, or ban at the reverse proxy.

### docker: Error response from daemon: driver failed programming external connectivity on endpoint <name>: (iptables failed: <details>)
Means: Docker couldn't add its NAT rules, usually because a firewall reload wiped its chains.
Fix: `sudo systemctl restart docker` after firewall changes. With firewalld, put Docker's interfaces in the docker zone.

### ufw rules don't block Docker published ports
Means: Not an error message but a common surprise: Docker inserts its own iptables rules ahead of ufw's.
Fix: Publish ports on 127.0.0.1 or a specific IP, filter in the DOCKER-USER chain, or put services behind a reverse proxy.

### nf_conntrack: nf_conntrack: table full, dropping packet
Means: The connection tracking table filled up.
Fix: Raise `net.netfilter.nf_conntrack_max`; find the flood with `conntrack -L | awk '{print $5}' | sort | uniq -c | sort -rn | head`.

### CrowdSec: cscli: unable to connect to the local API
Means: The CrowdSec agent's local API isn't running or the credentials file is wrong.
Fix: `sudo systemctl status crowdsec`; `sudo cscli lapi status`.

### crowdsec-firewall-bouncer: unable to get decisions: <details>
Means: The bouncer can't reach the CrowdSec API or its key was revoked.
Fix: Check `api_url` and `api_key` in the bouncer's config; `cscli bouncers list`.
