/**
 * Browser-only stand-in for the System view: a fake host with units, a
 * journal, pending updates and disks. sudo "needs" a password once per
 * action; any password works except "wrong". Used by `pnpm dev` and tests.
 */
import type {
  DisksReport_Serialize as DisksReport,
  JournalEntry_Serialize as JournalEntry,
  JournalEvent_Serialize as JournalEvent,
  JournalQuery_Deserialize as JournalQuery,
  Manager,
  RunEvent_Serialize as RunEvent,
  HistoryResult_Serialize as HistoryResult,
  Schedules_Serialize as Schedules,
  SecurityReport_Serialize as SecurityReport,
  SudoInput,
  SystemInfo,
  Unit_Serialize as Unit,
  UnitAction,
  UpdatesReport_Serialize as UpdatesReport,
  UpgradeOp,
} from "./bindings";

let remembered = false;

const sudoError = (retry: boolean) => ({ message: retry ? "sudo: wrong password" : "sudo needs your password", connect: null, system: { kind: "sudoRequired", retry } });

function checkSudo(sudo: SudoInput | null) {
  if (remembered && !sudo) return;
  if (!sudo) throw sudoError(false);
  if (sudo.password === "wrong") throw sudoError(true);
  if (sudo.remember) remembered = true;
}

const u = (name: string, active: string, sub: string, description: string, fileState: string | null = "enabled"): Unit => ({
  name,
  load: "loaded",
  active,
  sub,
  description,
  fileState,
});

let units: Unit[] = [
  u("cron.service", "active", "running", "Regular background program processing daemon"),
  u("docker.service", "active", "running", "Docker Application Container Engine"),
  u("fail2ban.service", "active", "running", "Fail2Ban Service"),
  u("nginx.service", "failed", "failed", "A high performance web server and a reverse proxy server"),
  u("rsync.service", "inactive", "dead", "", "disabled"),
  u("smartmontools.service", "active", "running", "Self Monitoring and Reporting Technology (SMART) Daemon"),
  u("ssh.service", "active", "running", "OpenBSD Secure Shell server"),
  u("systemd-journald.service", "active", "running", "Journal Service", "static"),
  u("ufw.service", "active", "exited", "Uncomplicated firewall"),
  u("unattended-upgrades.service", "active", "running", "Unattended Upgrades Shutdown"),
];

const IDENTS = ["sshd", "dockerd", "kernel", "systemd", "nginx", "cron", "fail2ban"];
const MESSAGES: [number, string][] = [
  [6, "Accepted publickey for admin from 10.0.0.5 port 51234 ssh2: ED25519 SHA256:abc"],
  [6, "Started Session 42 of User admin."],
  [5, "container 3f2a1c exited with code 0"],
  [4, "eth0: link is not ready"],
  [3, "bind() to 0.0.0.0:80 failed (98: Address already in use)"],
  [6, "(root) CMD (run-parts /etc/cron.hourly)"],
  [4, "[sshd] Found 203.0.113.9 - 2026-11-15 00:12:01"],
  [7, "debug: worker pool resized to 8"],
  [2, "Out of memory: Killed process 4242 (java)"],
];

function entry(ts: number, i: number, unit: string | null): JournalEntry {
  const [priority, message] = MESSAGES[i % MESSAGES.length];
  const ident = IDENTS[i % IDENTS.length];
  return { ts, priority, unit: unit ?? `${ident}.service`, ident, pid: 1000 + (i % 97), message };
}

const streams = new Map<number, ReturnType<typeof setInterval>>();
let nextRun = 1;

export const mockSystem = {
  async info(_hostId: number): Promise<SystemInfo> {
    return { sudo: "password", remembered };
  },
  async forget(_hostId: number): Promise<null> {
    remembered = false;
    return null;
  },
  async units(_hostId: number): Promise<Unit[]> {
    return units.map((x) => ({ ...x }));
  },
  async security(hostId: number): Promise<SecurityReport> {
    const tidy = hostId % 2 === 0;
    const L = (port: number, scope: "local" | "vpn" | "address" | "all", service: string | null, process: string | null, container: string | null = null, proto = "tcp") => ({
      proto,
      addr: scope === "all" ? "0.0.0.0" : scope === "local" ? "127.0.0.1" : scope === "vpn" ? "100.101.7.3" : "192.168.1.20",
      port,
      scope,
      process,
      container,
      service,
    });
    const checks: SecurityReport["checks"] = [
      { id: "ssh-root", title: "Root can't log in over SSH", status: "pass", detail: "PermitRootLogin prohibit-password: root needs a key.", fix: null, weight: 20 },
      tidy
        ? { id: "ssh-password", title: "SSH needs keys, not passwords", status: "pass", detail: "Password and keyboard-interactive logins are off.", fix: null, weight: 15 }
        : {
            id: "ssh-password",
            title: "SSH needs keys, not passwords",
            status: "fail",
            detail: "Password logins are allowed and SSH listens on every interface.",
            fix: "Make sure your key works first, then set `PasswordAuthentication no` and `KbdInteractiveAuthentication no` under `/etc/ssh/sshd_config.d/`, run `sudo sshd -t` and `sudo systemctl reload ssh`.",
            weight: 15,
          },
      { id: "firewall", title: "A firewall is on", status: "pass", detail: "ufw is active.", fix: null, weight: 15 },
      tidy
        ? { id: "exposed-services", title: "Databases aren't open to the network", status: "pass", detail: "No database, cache or Docker API port listens on every interface.", fix: null, weight: 20 }
        : {
            id: "exposed-services",
            title: "Databases aren't open to the network",
            status: "fail",
            detail: "Listening on every interface: redis on 6379 (container cache).",
            fix: 'Bind them to `127.0.0.1` (or a Docker network without a published port), e.g. `ports: ["127.0.0.1:6379:6379"]` in compose.',
            weight: 20,
          },
      {
        id: "docker-ufw",
        title: "Docker ports respect the firewall",
        status: "warn",
        detail: "ufw is on, but Docker publishes 3000, 6379 on every interface; Docker's iptables rules bypass ufw.",
        fix: "Publish only what must be reachable, bind the rest to `127.0.0.1:` in compose, or use the `DOCKER-USER` chain (e.g. ufw-docker).",
        weight: 10,
      },
      { id: "intrusion", title: "Brute-force protection", status: "info", detail: "Neither fail2ban nor CrowdSec is running.", fix: "With key-only SSH this matters less. Otherwise `sudo apt install fail2ban`.", weight: 5 },
      { id: "auto-updates", title: "Security updates install themselves", status: "pass", detail: "unattended-upgrades or dnf-automatic is set up.", fix: null, weight: 10 },
      { id: "reboot", title: "No reboot pending", status: tidy ? "pass" : "warn", detail: tidy ? "Nothing is waiting for a reboot." : "Updates installed a new kernel or library; it isn't in use until a reboot.", fix: tidy ? null : "Reboot when it suits you (Updates tab → Reboot).", weight: 5 },
      { id: "uid0", title: "Only root has uid 0", status: "pass", detail: "No other account has root's uid.", fix: null, weight: 20 },
    ];
    const lost = checks.reduce((a, c) => a + (c.status === "fail" ? c.weight : c.status === "warn" ? Math.floor(c.weight / 2) : 0), 0);
    const score = 100 - lost;
    return {
      score,
      grade: score >= 90 ? "A" : score >= 75 ? "B" : score >= 60 ? "C" : score >= 40 ? "D" : "F",
      checks,
      processesPartial: true,
      listeners: [
        L(22, "all", "ssh", null),
        L(53, "local", "dns", null, null, "udp"),
        L(80, "all", "http", null, "caddy"),
        L(443, "all", "https", null, "caddy"),
        L(3000, "all", "web app", "docker-proxy", "homepage"),
        L(5432, "local", "postgres", null),
        ...(tidy ? [] : [L(6379, "all", "redis", "docker-proxy", "cache")]),
        L(9100, "vpn", "node-exporter", "node_exporter"),
        L(32400, "address", "plex", null),
      ],
    };
  },
  async history(_hostId: number, query: string): Promise<HistoryResult> {
    const all = [
      "docker compose -f /opt/stacks/media/compose.yaml restart",
      "docker logs -f --tail 200 web",
      "journalctl -u nginx --since -1h",
      "export API_TOKEN=abc123456789def",
      "sudo systemctl restart nginx",
      "df -h /srv",
      "zpool status -v tank",
      "git -C /opt/stacks pull",
      "htop",
    ];
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    const hits = all.filter((c) => words.every((w) => c.toLowerCase().includes(w)));
    const secret = hits.filter((c) => /token=|password=/i.test(c));
    const now = Math.floor(Date.now() / 1000);
    return {
      entries: hits.filter((c) => !secret.includes(c)).map((command, i) => ({ command, shell: i % 3 ? "bash" : "zsh", when: i % 3 ? null : now - i * 3600 })),
      shells: ["bash", "zsh"],
      hidden: secret.length,
      truncated: false,
    };
  },
  async schedules(_hostId: number): Promise<Schedules> {
    const now = Math.floor(Date.now() / 1000);
    const h = 3600;
    return {
      systemd: true,
      now,
      utcOffsetMin: -new Date().getTimezoneOffset(),
      timers: [
        { name: "certbot.timer", description: "Run certbot twice daily", active: "active", enabled: "enabled", unit: "certbot.service", calendar: ["*-*-* 00,12:00:00"], monotonic: [], next: now + 2 * h + 600, last: now - 10 * h, result: "success", exitStatus: 0 },
        { name: "restic-backup.timer", description: "Nightly restic backup", active: "active", enabled: "enabled", unit: "restic-backup.service", calendar: ["*-*-* 03:30:00"], monotonic: [], next: now + 9 * h, last: now - 15 * h, result: "exit-code", exitStatus: 3 },
        { name: "apt-daily.timer", description: "Daily apt download activities", active: "active", enabled: "enabled", unit: "apt-daily.service", calendar: ["*-*-* 6,18:00"], monotonic: [], next: now + 5 * h, last: now - 7 * h, result: "success", exitStatus: 0 },
        { name: "fstrim.timer", description: "Discard unused filesystem blocks once a week", active: "active", enabled: "enabled", unit: "fstrim.service", calendar: ["weekly"], monotonic: [], next: now + 50 * h, last: now - 118 * h, result: "success", exitStatus: 0 },
        { name: "docker-prune.timer", description: "Prune unused Docker images", active: "inactive", enabled: "disabled", unit: "docker-prune.service", calendar: [], monotonic: ["OnUnitActiveSec=1w"], next: null, last: null, result: null, exitStatus: null },
      ],
      cron: [
        { source: "crontab", user: null, schedule: "*/15 * * * *", every: "every 15 minutes", command: "/home/me/bin/ddns-update.sh --token=abc123 >/dev/null 2>&1", next: now + 420 },
        { source: "/etc/crontab", user: "root", schedule: "17 * * * *", every: "hourly at :17", command: "cd / && run-parts --report /etc/cron.hourly", next: now + 1500 },
        { source: "/etc/cron.d/zfs", user: "root", schedule: "0 2 1 * *", every: "monthly on day 1 at 02:00", command: "zpool scrub tank", next: now + 12 * 86400 },
        { source: "crontab", user: null, schedule: "@reboot", every: "at boot", command: "sleep 30 && /home/me/bin/mount-shares.sh", next: null },
      ],
      periodic: [
        { period: "daily", scripts: ["apt-compat", "dpkg", "logrotate", "man-db"] },
        { period: "weekly", scripts: ["man-db"] },
      ],
    };
  },
  async unitStatus(_hostId: number, unit: string): Promise<string> {
    const x = units.find((y) => y.name === unit);
    if (!x) return `Unit ${unit} could not be found.`;
    return [
      `● ${x.name} - ${x.description || x.name}`,
      `     Loaded: ${x.load} (/lib/systemd/system/${x.name}; ${x.fileState ?? "static"}; preset: enabled)`,
      `     Active: ${x.active} (${x.sub}) since Sat 2026-11-14 22:01:13 UTC; 2h 11min ago`,
      x.active === "active" ? `   Main PID: 812 (${x.name.replace(".service", "")})` : `    Process: 812 ExecStart=/usr/sbin/${x.name.replace(".service", "")} (code=exited, status=1/FAILURE)`,
    ].join("\n");
  },
  async unitAction(_hostId: number, unit: string, action: UnitAction, sudo: SudoInput | null): Promise<null> {
    checkSudo(sudo);
    await new Promise((r) => setTimeout(r, 300));
    units = units.map((x) => {
      if (x.name !== unit) return x;
      switch (action) {
        case "start":
        case "restart":
        case "reload":
          return { ...x, active: "active", sub: "running" };
        case "stop":
          return { ...x, active: "inactive", sub: "dead" };
        case "enable":
          return { ...x, fileState: "enabled" };
        case "disable":
          return { ...x, fileState: "disabled" };
      }
      return x;
    });
    return null;
  },
  async journalStart(_hostId: number, q: JournalQuery, elevated: boolean, sudo: SudoInput | null, on: (e: JournalEvent) => void): Promise<number> {
    if (elevated) checkSudo(sudo);
    const id = nextRun++;
    const now = Date.now();
    const n = Math.min(q.lines ?? 200, 2000);
    const unit = q.unit || null;
    const maxPrio = q.priority ?? 7;
    const backlog = Array.from({ length: n }, (_, i) => entry(now - (n - i) * 7000, i, unit)).filter((e) => e.priority <= maxPrio);
    setTimeout(() => {
      if (!elevated) on({ kind: "limited" });
      on({ kind: "entries", entries: backlog });
      if (!q.follow) on({ kind: "end", error: null });
    }, 120);
    if (q.follow) {
      let i = n;
      streams.set(
        id,
        setInterval(() => {
          const e = entry(Date.now(), i++, unit);
          if (e.priority <= maxPrio) on({ kind: "entries", entries: [e] });
        }, 1500),
      );
    }
    return id;
  },
  async streamStop(run: number): Promise<null> {
    const t = streams.get(run);
    if (t) clearInterval(t);
    streams.delete(run);
    return null;
  },
  async updates(hostId: number): Promise<UpdatesReport> {
    await new Promise((r) => setTimeout(r, 400 + (hostId % 3) * 300));
    if (hostId % 4 === 2) return { manager: "dnf", updates: [], rebootRequired: false, rebootReasons: [], note: null };
    if (hostId % 4 === 3)
      return {
        manager: "dnf",
        updates: [
          { name: "kernel", current: null, available: "6.11.6-200.fc40", repo: "updates" },
          { name: "podman", current: null, available: "5:5.2.5-1.fc40", repo: "updates" },
        ],
        rebootRequired: false,
        rebootReasons: [],
        note: null,
      };
    return {
      manager: "apt",
      updates: [
        { name: "openssl", current: "3.0.2-0ubuntu1.17", available: "3.0.2-0ubuntu1.18", repo: "jammy-security" },
        { name: "linux-image-generic", current: "6.8.0.48.48", available: "6.8.0.49.49", repo: "jammy-updates" },
        { name: "docker-ce", current: "5:27.3.0-1", available: "5:27.3.1-1", repo: "jammy" },
        { name: "tzdata", current: "2024a-0ubuntu0.22.04", available: "2024b-0ubuntu0.22.04", repo: "jammy-updates" },
      ],
      rebootRequired: true,
      rebootReasons: ["updated: linux-image-6.8.0-48-generic"],
      note: null,
    };
  },
  async upgradeRun(_hostId: number, _m: Manager, op: UpgradeOp, sudo: SudoInput | null, on: (e: RunEvent) => void): Promise<number> {
    checkSudo(sudo);
    const id = nextRun++;
    const lines =
      op === "refresh"
        ? ["Hit:1 http://archive.example/ubuntu jammy InRelease", "Get:2 http://archive.example/ubuntu jammy-updates InRelease [128 kB]", "Fetched 128 kB in 1s (210 kB/s)", "Reading package lists... Done"]
        : [
            "Reading package lists...",
            "Building dependency tree...",
            "The following packages will be upgraded:",
            "  docker-ce linux-image-generic openssl tzdata",
            "4 upgraded, 0 newly installed, 0 to remove and 0 not upgraded.",
            "Unpacking openssl (3.0.2-0ubuntu1.18) over (3.0.2-0ubuntu1.17) ...",
            "Setting up openssl (3.0.2-0ubuntu1.18) ...",
            "Processing triggers for man-db (2.10.2-1) ...",
          ];
    let i = 0;
    const t = setInterval(() => {
      if (i < lines.length) on({ kind: "lines", lines: [lines[i++]] });
      else {
        clearInterval(t);
        on({ kind: "done", code: 0, error: null });
      }
    }, 250);
    return id;
  },
  async reboot(_hostId: number, sudo: SudoInput | null): Promise<null> {
    checkSudo(sudo);
    return null;
  },
  async disks(_hostId: number, sudo: SudoInput | null): Promise<DisksReport> {
    checkSudo(sudo);
    await new Promise((r) => setTimeout(r, 300));
    return {
      health: "warn",
      disks: [
        {
          name: "/dev/nvme0",
          protocol: "NVMe",
          model: "Example NVMe 1TB",
          serial: "EX0000000003",
          firmware: "3B2QGXA7",
          capacity: 1000204886016,
          rotationRpm: null,
          passed: true,
          tempC: 41,
          powerOnHours: 8123,
          health: "ok",
          issues: [],
          attributes: [],
          nvme: { criticalWarning: 0, availableSpare: 100, spareThreshold: 10, percentageUsed: 7, mediaErrors: 0, errorLogEntries: 12, unsafeShutdowns: 41, bytesWritten: 63209472000000 },
        },
        {
          name: "/dev/sda",
          protocol: "ATA",
          model: "EXAMPLE HDD 4TB",
          serial: "EX0000000002",
          firmware: "CC43",
          capacity: 4000787030016,
          rotationRpm: 5400,
          passed: true,
          tempC: 38,
          powerOnHours: 42411,
          health: "warn",
          issues: ["8 reallocated sectors", "2 sectors pending reallocation"],
          attributes: [
            { id: 1, name: "Raw_Read_Error_Rate", value: 117, worst: 99, thresh: 6, raw: "148212344", failed: null, health: "ok" },
            { id: 5, name: "Reallocated_Sector_Ct", value: 100, worst: 100, thresh: 10, raw: "8", failed: null, health: "warn" },
            { id: 9, name: "Power_On_Hours", value: 52, worst: 52, thresh: 0, raw: "42411", failed: null, health: "ok" },
            { id: 194, name: "Temperature_Celsius", value: 38, worst: 51, thresh: 0, raw: "38 (0 15 0 0 0)", failed: null, health: "ok" },
            { id: 197, name: "Current_Pending_Sector", value: 100, worst: 100, thresh: 0, raw: "2", failed: null, health: "warn" },
          ],
          nvme: null,
        },
      ],
    };
  },
};
