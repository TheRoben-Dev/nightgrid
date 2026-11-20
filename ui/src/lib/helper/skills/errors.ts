/**
 * Offline error reference (M10p): what common exit codes, systemd results
 * and Docker, SSH and package manager errors mean, and what to try next.
 * Matched against the question and attached text; the knowledge base (M11)
 * extends it.
 */

export interface ErrorEntry {
  id: string;
  title: string;
  meaning: string;
  next: string;
}

interface Rule extends ErrorEntry {
  re: RegExp;
}

const SIGNALS: Record<number, [string, string]> = {
  1: ["SIGHUP", "the terminal or SSH session it belonged to closed"],
  2: ["SIGINT", "it was interrupted (Ctrl+C)"],
  3: ["SIGQUIT", "it was asked to quit and dump core"],
  4: ["SIGILL", "it ran an illegal CPU instruction (wrong architecture build or a broken binary)"],
  6: ["SIGABRT", "it aborted itself, usually after a failed internal check"],
  7: ["SIGBUS", "a bad memory access, often a file that shrank while mapped or failing hardware"],
  8: ["SIGFPE", "an arithmetic error such as division by zero"],
  9: ["SIGKILL", "it was killed outright, most often by the kernel's out-of-memory killer, a memory limit, or `kill -9`"],
  11: ["SIGSEGV", "it crashed on an invalid memory access (a bug, bad library versions or failing RAM)"],
  13: ["SIGPIPE", "it wrote to a pipe or socket nobody was reading any more"],
  14: ["SIGALRM", "a timer it set went off"],
  15: ["SIGTERM", "it was asked to stop (docker stop, systemctl stop, a reboot, or kill)"],
};

/** Plain-language meaning of a process exit code. */
export function exitCode(n: number): ErrorEntry | null {
  if (!Number.isInteger(n) || n < 0 || n > 255) return null;
  const id = `exit-${n}`;
  if (n === 0) return { id, title: "Exit code 0", meaning: "Success.", next: "Nothing failed; look elsewhere for the problem." };
  if (n === 1) return { id, title: "Exit code 1", meaning: "A general error: the program failed and chose 1 to say so.", next: "The reason is in its output or log just before it exited (`journalctl -u <unit>` or `docker logs`)." };
  if (n === 2) return { id, title: "Exit code 2", meaning: "Usually a usage error: a wrong option or argument (for shells: a syntax error).", next: "Check the command line or config for a typo; run it with --help." };
  if (n === 126) return { id, title: "Exit code 126", meaning: "The file was found but couldn't be run: not executable, or a script with Windows line endings or a bad #! line.", next: "`chmod +x` the file, check its first line, and run `file` on it." };
  if (n === 127) return { id, title: "Exit code 127", meaning: "Command not found: the program isn't installed or isn't on PATH (in containers: missing from the image).", next: "Check the spelling, install the package, or use the full path. Cron and systemd have a much shorter PATH than your shell." };
  if (n === 128) return { id, title: "Exit code 128", meaning: "Invalid exit argument, or (for git) a fatal error.", next: "Read the last lines of the output." };
  if (n === 255) return { id, title: "Exit code 255", meaning: "Out-of-range exit status; for ssh it means the SSH connection itself failed (not the remote command).", next: "For ssh: run `ssh -v` to see where it fails (DNS, refused, key, host key)." };
  if (n > 128 && n <= 128 + 64) {
    const sig = n - 128;
    const s = SIGNALS[sig];
    return {
      id,
      title: `Exit code ${n} (${s ? s[0] : `signal ${sig}`})`,
      meaning: `128 + ${sig}: the process was ended by signal ${sig}${s ? ` (${s[0]}): ${s[1]}` : ""}.`,
      next:
        sig === 9
          ? "Check for memory pressure: `dmesg -T | grep -i oom`, `docker inspect -f '{{.State.OOMKilled}}' <container>`, or the unit's MemoryMax. Raise the limit or find the leak."
          : sig === 11
            ? "Look for a core dump (`coredumpctl list`), try the previous version of the image or package, and run a memory test if it keeps happening."
            : sig === 15
              ? "Usually expected (someone or something stopped it). If not, look for what sent it: a deploy, a healthcheck restart, or a timer."
              : "Check the program's log for what happened just before.",
    };
  }
  return { id, title: `Exit code ${n}`, meaning: "A program-specific error code.", next: "Look up the program's own list of exit codes (`man <program>`, EXIT STATUS)." };
}

const SYSTEMD_EXIT: Record<number, [string, string]> = {
  200: ["CHDIR", "WorkingDirectory= doesn't exist or isn't accessible"],
  201: ["NICE", "couldn't set the Nice= level"],
  202: ["FDS", "couldn't set up file descriptors"],
  203: ["EXEC", "ExecStart= couldn't be run: wrong path, not executable, missing interpreter, or a script without a #! line"],
  204: ["MEMORY", "out of memory while starting"],
  205: ["LIMITS", "a Limit*= setting couldn't be applied"],
  206: ["OOM_ADJUST", "couldn't set the OOM score"],
  207: ["SIGNAL_MASK", "couldn't set the signal mask"],
  208: ["STDIN", "couldn't set up standard input"],
  209: ["STDOUT", "couldn't set up standard output (StandardOutput= points at something missing)"],
  210: ["CHROOT", "RootDirectory= failed"],
  211: ["IOPRIO", "couldn't set the I/O priority"],
  212: ["TIMERSLACK", "couldn't set the timer slack"],
  213: ["SECUREBITS", "couldn't set secure bits"],
  214: ["SETSCHEDULER", "couldn't set the CPU scheduler"],
  215: ["CPUAFFINITY", "couldn't set CPU affinity"],
  216: ["GROUP", "Group= doesn't exist"],
  217: ["USER", "User= doesn't exist (or DynamicUser failed)"],
  218: ["CAPABILITIES", "couldn't drop or set capabilities"],
  219: ["CGROUP", "couldn't set up the control group"],
  220: ["SETSID", "couldn't create a session"],
  221: ["CONFIRM", "start was cancelled at the confirmation prompt"],
  222: ["STDERR", "couldn't set up standard error"],
  224: ["PAM", "PAM session setup failed"],
  225: ["NETWORK", "PrivateNetwork= failed"],
  226: ["NAMESPACE", "a sandboxing setting failed: a path in ReadWritePaths=, ProtectSystem= or similar is missing"],
  227: ["NO_NEW_PRIVILEGES", "NoNewPrivileges= failed"],
  228: ["SECCOMP", "SystemCallFilter= failed"],
  229: ["SELINUX_CONTEXT", "the SELinux context couldn't be set"],
  230: ["PERSONALITY", "Personality= failed"],
  231: ["APPARMOR_PROFILE", "the AppArmor profile couldn't be applied"],
  232: ["ADDRESS_FAMILIES", "RestrictAddressFamilies= failed"],
  233: ["RUNTIME_DIRECTORY", "RuntimeDirectory= couldn't be created"],
  235: ["CHOWN", "couldn't change the owner of a socket or directory"],
  236: ["SMACK_PROCESS_LABEL", "the SMACK label couldn't be set"],
  237: ["KEYRING", "the kernel keyring couldn't be set up"],
  238: ["STATE_DIRECTORY", "StateDirectory= couldn't be created"],
  239: ["CACHE_DIRECTORY", "CacheDirectory= couldn't be created"],
  240: ["LOGS_DIRECTORY", "LogsDirectory= couldn't be created"],
  241: ["CONFIGURATION_DIRECTORY", "ConfigurationDirectory= couldn't be created"],
  242: ["NUMA_POLICY", "NUMAPolicy= failed"],
  243: ["CREDENTIALS", "LoadCredential= or SetCredential= failed"],
  245: ["BPF", "a BPF program couldn't be loaded"],
};

/** systemd's own codes in "status=203/EXEC". */
export function systemdExit(n: number): ErrorEntry | null {
  const e = SYSTEMD_EXIT[n];
  if (!e) return null;
  return { id: `systemd-${n}`, title: `status=${n}/${e[0]} (systemd)`, meaning: `systemd couldn't start the process: ${e[1]}. The program itself never ran.`, next: "Fix the unit file (`systemctl cat <unit>`), then `systemctl daemon-reload` and start it again. `journalctl -u <unit> -b` shows the line systemd logged." };
}

const RULES: Rule[] = [
  { id: "start-limit", re: /start request repeated too quickly|start-limit-hit/i, title: "systemd: start request repeated too quickly", meaning: "The unit crashed and restarted so often that systemd stopped trying (StartLimitBurst within StartLimitIntervalSec).", next: "Find the real error earlier in `journalctl -u <unit> -b`, fix it, then `systemctl reset-failed <unit>` and start it." },
  { id: "auto-restart", re: /activating \(auto-restart\)/i, title: "systemd: activating (auto-restart)", meaning: "The service exited and systemd is waiting RestartSec before starting it again: it's crash-looping.", next: "`journalctl -u <unit> -n 50` shows why each start fails." },
  { id: "result-timeout", re: /Result: timeout|start operation timed out|timed out waiting for/i, title: "systemd: timeout", meaning: "The service didn't report ready (or didn't stop) within TimeoutStartSec/TimeoutStopSec.", next: "Check what it waits for (network, a mount, a database). For Type=notify services, the program may never send READY. Raise the timeout only once you know why." },
  { id: "result-oom", re: /Result: oom-kill|oom-kill|Out of memory: Killed process|OOMKilled[":\s]+true/i, title: "Killed by the out-of-memory killer", meaning: "The kernel (or a cgroup/container memory limit) ran out of memory and killed the process.", next: "`dmesg -T | grep -i -A5 oom` shows who used the memory. Raise MemoryMax / the container's mem_limit, add swap, or find the leak." },
  { id: "unit-not-found", re: /Unit [\w@.-]+ not found|Failed to (start|restart|enable) [\w@.-]+: Unit .* not found/i, title: "systemd: unit not found", meaning: "No unit with that name is installed, or systemd hasn't read a new unit file yet.", next: "Check the name with `systemctl list-unit-files | grep <name>`; after adding a file run `systemctl daemon-reload`." },
  { id: "unit-masked", re: /Unit [\w@.-]+ is masked/i, title: "systemd: unit is masked", meaning: "The unit was deliberately disabled so nothing can start it.", next: "`systemctl unmask <unit>` if you really want it running." },
  { id: "dependency", re: /Dependency failed for/i, title: "systemd: dependency failed", meaning: "A unit this one needs (a mount, the network, another service) failed, so this one wasn't started.", next: "Look at the failed dependency first: `systemctl --failed`." },
  { id: "port-allocated", re: /port is already allocated|Bind for [\d.:[\]]+:\d+ failed/i, title: "Docker: port is already allocated", meaning: "Another container (or the same one from an old stack) already publishes that host port.", next: "`docker ps --format '{{.Names}} {{.Ports}}' | grep <port>` finds it. Stop it or pick another host port." },
  { id: "addr-in-use", re: /address already in use|EADDRINUSE/i, title: "Address already in use", meaning: "Some process on the host is already listening on that port.", next: "`sudo ss -ltnp | grep :<port>` shows which process. Stop it or move one of them." },
  { id: "no-space", re: /no space left on device|ENOSPC/i, title: "No space left on device", meaning: "The filesystem is full, or out of inodes (many small files) even with space left.", next: "`df -h` and `df -i`. Common culprits: Docker images and logs (`docker system df`), journald (`journalctl --vacuum-size=500M`), old kernels, /var/log." },
  { id: "docker-sock", re: /permission denied while trying to connect to the Docker daemon socket/i, title: "Docker: permission denied on the socket", meaning: "Your user isn't allowed to talk to Docker.", next: "Use sudo, or add the user to the docker group (`sudo usermod -aG docker $USER`, then log in again; on Synology DSM `sudo synogroup --add docker $USER` plus `sudo chown root:docker /var/run/docker.sock`). Nightgrid's Docker view can do it with Fix it for me. Note: the docker group is root-equivalent." },
  { id: "docker-daemon", re: /Cannot connect to the Docker daemon|Is the docker daemon running/i, title: "Docker: daemon not reachable", meaning: "The Docker service isn't running, or DOCKER_HOST points somewhere else.", next: "`systemctl status docker` and `journalctl -u docker -n 50`; check `echo $DOCKER_HOST` and `docker context ls`." },
  { id: "manifest-unknown", re: /manifest unknown|manifest for .* not found|not found: manifest/i, title: "Docker: manifest unknown", meaning: "That image tag doesn't exist in the registry (typo, removed tag, or private repo).", next: "Check the tag on the registry page; `docker manifest inspect <image>` tests it." },
  { id: "pull-denied", re: /pull access denied|requested access to the resource is denied|unauthorized: authentication required/i, title: "Docker: pull access denied", meaning: "The image is private, doesn't exist under that name, or you aren't logged in to its registry.", next: "Check the name, then `docker login <registry>`." },
  { id: "rate-limit", re: /toomanyrequests|You have reached your (unauthenticated )?pull rate limit/i, title: "Docker Hub rate limit", meaning: "Docker Hub limits anonymous pulls per IP over 6 hours.", next: "Wait, log in (`docker login`) for a higher limit, or use a mirror." },
  { id: "exec-format", re: /exec format error/i, title: "exec format error", meaning: "The binary or image was built for another CPU architecture (arm64 vs amd64), or a script lacks its #! line.", next: "`docker image inspect -f '{{.Architecture}}' <image>` vs `uname -m`; pull a multi-arch tag or set `platform:`." },
  { id: "oci-runtime", re: /OCI runtime (create|exec) failed/i, title: "Docker: OCI runtime failed", meaning: "The container couldn't start; the reason follows in the same message (missing entrypoint, bad mount path, permission, cgroup).", next: "Read the text after \"OCI runtime create failed\": it usually names the file or mount at fault." },
  { id: "name-conflict", re: /Conflict\. The container name .* is already in use/i, title: "Docker: container name in use", meaning: "A container (often stopped) already has that name.", next: "`docker rm <name>` the old one (after checking it isn't needed) or rename the new one." },
  { id: "network-missing", re: /network [\w.-]+ (not found|declared as external, but could not be found)/i, title: "Docker: network not found", meaning: "The stack expects a network that doesn't exist.", next: "Create it (`docker network create <name>`) or remove `external: true`." },
  { id: "unhealthy", re: /\(unhealthy\)|is unhealthy|dependency failed to start: container .* is unhealthy/i, title: "Docker: container is unhealthy", meaning: "Its healthcheck keeps failing; services that depend on it won't start.", next: "`docker inspect --format '{{json .State.Health}}' <container>` shows the last checks' output." },
  { id: "ssh-publickey", re: /Permission denied \(publickey/i, title: "SSH: Permission denied (publickey)", meaning: "The server didn't accept any key you offered (or only allows keys and you have none it knows).", next: "Check the user name, that your public key is in ~/.ssh/authorized_keys on the host (mode 600, ~/.ssh 700), and which keys you offer with `ssh -v`. Nightgrid's SSH keys page shows which key works where." },
  { id: "ssh-hostkey", re: /Host key verification failed|REMOTE HOST IDENTIFICATION HAS CHANGED/i, title: "SSH: host key changed or unknown", meaning: "The host's identity doesn't match what was saved: a reinstall, a new IP owner, or (rarely) someone in the middle.", next: "Confirm the new fingerprint on the host's console (`ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub`) before accepting it." },
  { id: "conn-refused", re: /Connection refused|ECONNREFUSED/i, title: "Connection refused", meaning: "The host answered but nothing listens on that port (service down, wrong port, or listening on 127.0.0.1 only).", next: "On the host: `ss -ltnp | grep <port>`. Check the service is running and which address it binds." },
  { id: "conn-timeout", re: /Connection timed out|ETIMEDOUT|Operation timed out/i, title: "Connection timed out", meaning: "Nothing answered: the host is off, a firewall drops the packets, or the address/route is wrong.", next: "`ping` the host, check firewalls (ufw/nftables, the router), and whether you're on the right network or VPN." },
  { id: "no-route", re: /No route to host|EHOSTUNREACH|Network is unreachable/i, title: "No route to host", meaning: "Your machine can't find a path to that address: wrong subnet, VPN down, or a firewall rejecting.", next: "`ip route get <ip>` shows the path; check the VPN/tailnet and the host's IP." },
  { id: "dns", re: /Name or service not known|Temporary failure in name resolution|Could not resolve host|nodename nor servname provided|NXDOMAIN/i, title: "Name resolution failed", meaning: "The host name couldn't be turned into an address.", next: "Try the IP directly; check `/etc/resolv.conf` or `resolvectl status`, and the name's spelling." },
  { id: "too-many-auth", re: /Too many authentication failures/i, title: "SSH: too many authentication failures", meaning: "Your agent offered more keys than the server's MaxAuthTries allows before the right one.", next: "Set `IdentityFile` and `IdentitiesOnly yes` for that host." },
  { id: "cmd-not-found", re: /command not found|: not found$|No such file or directory.*(bin|exec)/im, title: "Command not found", meaning: "The program isn't installed or isn't on PATH.", next: "Install it, or use the full path. Cron jobs and systemd units use a minimal PATH." },
  { id: "perm-denied", re: /\bPermission denied\b(?! \(publickey)|EACCES|Operation not permitted|EPERM/i, title: "Permission denied", meaning: "The user lacks rights to that file, folder or action (or SELinux/AppArmor blocks it).", next: "`ls -l` the path and each parent folder; `id` shows your groups. On Fedora/RHEL check `ausearch -m avc -ts recent`." },
  { id: "readonly-fs", re: /Read-only file system|EROFS/i, title: "Read-only file system", meaning: "The filesystem is mounted read-only, often because the kernel remounted it after disk errors.", next: "`dmesg -T | grep -iE 'error|remount'` and check SMART; on immutable systems (Bazzite, Silverblue) /usr is read-only by design." },
  { id: "busy", re: /Device or resource busy|target is busy|EBUSY/i, title: "Device or resource busy", meaning: "Something is still using the mount or device.", next: "`sudo fuser -vm <mountpoint>` or `lsof +f -- <mountpoint>` shows what." },
  { id: "too-many-files", re: /Too many open files|EMFILE/i, title: "Too many open files", meaning: "The process hit its file descriptor limit.", next: "Raise LimitNOFILE= in the unit (or ulimits in compose); look for a leak if it keeps climbing." },
  { id: "dpkg-lock", re: /Could not get lock \/var\/lib\/dpkg\/lock|dpkg frontend lock|Unable to acquire the dpkg/i, title: "apt: could not get the dpkg lock", meaning: "Another apt or unattended-upgrades run is in progress.", next: "Wait for it (`ps aux | grep -E 'apt|dpkg'`). Don't delete lock files while it runs." },
  { id: "dpkg-interrupted", re: /dpkg was interrupted/i, title: "apt: dpkg was interrupted", meaning: "A previous package install stopped halfway.", next: "`sudo dpkg --configure -a`, then `sudo apt -f install`." },
  { id: "no-pubkey", re: /NO_PUBKEY [0-9A-F]+|EXPKEYSIG|The following signatures couldn't be verified/i, title: "apt: missing or expired repository key", meaning: "A repository's signing key is missing or expired, so apt won't trust it.", next: "Re-add the repository's key from its official instructions (into /etc/apt/keyrings), or remove the repo." },
  { id: "hash-mismatch", re: /Hash Sum mismatch/i, title: "apt: Hash Sum mismatch", meaning: "A downloaded index didn't match its checksum (mirror syncing or a caching proxy).", next: "`sudo apt clean && sudo apt update`; switch mirrors if it persists." },
  { id: "cert-expired", re: /certificate has expired|certificate verify failed|x509: certificate|SSL certificate problem/i, title: "TLS certificate problem", meaning: "The certificate is expired, self-signed, for another name, or the clock is wrong.", next: "Check with `openssl s_client -connect host:443 -servername host`; check the system clock (`timedatectl`). Nightgrid's expiry watch tracks dates." },
  { id: "bad-gateway", re: /502 Bad Gateway|upstream connect error|connect\(\) failed .* while connecting to upstream/i, title: "502 Bad Gateway", meaning: "The reverse proxy is fine but the app behind it isn't answering (down, wrong port, or on another network).", next: "Check the upstream container is running and the proxy points at the right host:port; in Docker, both must share a network." },
  { id: "quota", re: /Disk quota exceeded|EDQUOT/i, title: "Disk quota exceeded", meaning: "The user or dataset hit its quota, though the disk may have space.", next: "`quota -s` or `zfs get quota,used <dataset>`." },
  { id: "segfault", re: /segfault at|Segmentation fault/i, title: "Segmentation fault", meaning: "The program crashed on an invalid memory access.", next: "Try the previous version; `coredumpctl info` shows the crash; repeated segfaults in many programs point at RAM (run memtest)." },
];

const EXIT_RE = /\b(?:exit(?:ed)?(?:\s+with)?(?:\s+(?:code|status))?|exit_code|ExitCode|code=exited,\s*status)\s*[:=]?\s*(\d{1,3})\b/gi;
const SYSTEMD_RE = /\bstatus=(2\d\d)\/[A-Z_]+/g;
const KILLED_RE = /code=killed,\s*status=(\d+)\/([A-Z]+)/g;

/** Reference entries that match the text, most specific first (at most `max`). */
export function lookupErrors(text: string, max = 4): ErrorEntry[] {
  if (!text.trim()) return [];
  const out: ErrorEntry[] = [];
  const have = new Set<string>();
  const push = (e: ErrorEntry | null) => {
    if (e && !have.has(e.id)) {
      have.add(e.id);
      out.push(e);
    }
  };
  for (const m of text.matchAll(SYSTEMD_RE)) push(systemdExit(Number(m[1])));
  for (const m of text.matchAll(KILLED_RE)) push(exitCode(128 + Number(m[1])));
  for (const r of RULES) if (r.re.test(text)) push({ id: r.id, title: r.title, meaning: r.meaning, next: r.next });
  for (const m of text.matchAll(EXIT_RE)) {
    const n = Number(m[1]);
    if (n >= 200 && n < 250 && SYSTEMD_EXIT[n] && /status=/.test(m[0])) continue;
    if (n !== 0) push(exitCode(n));
  }
  return out.slice(0, max);
}
