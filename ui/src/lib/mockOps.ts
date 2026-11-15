/**
 * Browser-only stand-ins for the backup board, proxy routes and log samples
 * (M10g), for `pnpm dev`, tests and screenshots.
 */
import type {
  HostSchedule_Serialize as HostSchedule,
  HostEvent,
  Timeline,
  DriftReport_Serialize as DriftReport,
  FileDiff_Serialize as FileDiff,
  SideInfo,
  WatchItem_Serialize as WatchItem,
  WatchInput_Deserialize as WatchInput,
  ScheduleSpec_Deserialize as ScheduleSpec,
  BackupJob,
  BackupJobInput,
  BackupStatus_Serialize as BackupStatus,
  BackupSuggestion,
  LastTest,
  RestoreTest_Serialize as RestoreTest,
  SudoInput,
  ProxyReport_Serialize as ProxyReport,
  StatusPageInfo_Serialize as StatusPageInfo,
  StatusPageSettings_Serialize as StatusPageSettings,
  Vm_Serialize as Vm,
  VmAction,
  VmKind,
  VmReport_Serialize as VmReport,
  Discovery_Serialize as Discovery,
  Device_Serialize as Device,
  DeviceLabel,
} from "./bindings";

let labels: DeviceLabel[] = [
  { key: "3c:22:fb:00:00:52", name: "Kid's Switch", kind: "console", tags: ["kids", "wifi"], note: "Parental controls on the router." },
];

let nextId = 1;
let jobs: BackupJob[] = [];
let tests: LastTest[] = [];

const now = () => Date.now() / 1000;
const delay = <T>(v: T, ms = 300) => new Promise<T>((r) => setTimeout(() => r(v), ms));

function check(i: BackupJobInput): BackupJobInput {
  const name = i.name.trim();
  if (!name || name.length > 80) throw new Error("invalid value for name: must be 1-80 characters");
  const target = i.target.trim();
  if (i.source === "path" && !target.startsWith("/")) throw new Error("invalid value for target: must be an absolute path");
  if (i.source === "unit" && !/^[A-Za-z0-9@._:\\-]+$/.test(target)) throw new Error("invalid value for target: isn't a systemd unit name");
  if (i.staleHours < 1 || i.staleHours > 2160) throw new Error("invalid value for staleHours: must be 1-2160");
  return { ...i, name, target };
}

let guests: Vm[] = [
  { kind: "qemu", id: "100", name: "homeassistant", state: "running", vcpus: 2, memoryMb: 4096, autostart: true },
  { kind: "qemu", id: "101", name: "win11-test", state: "stopped", vcpus: 4, memoryMb: 8192, autostart: false },
  { kind: "lxc", id: "200", name: "pihole", state: "running", vcpus: 1, memoryMb: 512, autostart: true },
  { kind: "libvirt", id: "debian-lab", name: "debian-lab", state: "paused", vcpus: 2, memoryMb: 2048, autostart: false },
];
const dev = (ip: string, d: Partial<Device> = {}): Device => ({ ip, mac: null, names: [], services: [], seen: ["neighbour"], os: null, online: null, vendor: null, also: [], ...d });

/** A made-up home LAN for the network map. Addresses are documentation-style examples. */
function discovery(sweep: boolean): Discovery {
  const devices: Device[] = [
    dev("192.168.1.1", { mac: "a4:2b:b0:11:22:33", names: ["router.lan"], seen: ["gateway", "neighbour"] }),
    dev("192.168.1.20", { mac: "00:11:32:aa:bb:cc", vendor: "Synology", names: ["nas.local"], services: ["_smb._tcp", "_http._tcp", "NFS share → /mnt/media"], seen: ["neighbour", "mdns", "mount"] }),
    dev("192.168.1.31", { mac: "b8:27:eb:12:34:56", names: ["octoprint.local"], services: ["_octoprint._tcp", "_http._tcp"], seen: ["mdns"] }),
    dev("192.168.1.44", { mac: "f0:18:98:de:ad:01", names: ["Living Room TV"], services: ["_airplay._tcp"], seen: ["neighbour", "mdns"] }),
    dev("192.168.1.52", { mac: "3c:22:fb:00:00:52", vendor: "Apple", seen: ["neighbour"] }),
    dev("100.101.7.12", { names: ["macbook"], seen: ["tailscale"], os: "macOS", online: true }),
    dev("100.88.1.9", { names: ["phone"], seen: ["tailscale"], os: "iOS", online: false }),
  ];
  if (sweep) devices.push(dev("192.168.1.77", { seen: ["ping"] }), dev("192.168.1.78", { seen: ["ping"] }));
  return {
    ifaces: [
      { name: "eth0", addr: "192.168.1.40", prefix: 24, kind: "private" },
      { name: "tailscale0", addr: "100.70.3.4", prefix: 32, kind: "tailnet" },
    ],
    gateway: "192.168.1.1",
    devices,
    exposures: [
      { kind: "upnp", port: 32400, public: "203.0.113.7:32400", target: "192.168.1.40:32400", detail: "Plex Media Server" },
      { kind: "upnp", port: 51820, public: "203.0.113.7:51820", target: "192.168.1.20:51820", detail: "UDP" },
      { kind: "funnel", port: 8080, public: "https://lab.example-tailnet.ts.net", target: "http://127.0.0.1:8080", detail: "Tailscale Funnel" },
      { kind: "tunnel", port: 3000, public: "status.example.com", target: "http://localhost:3000", detail: "cloudflared" },
    ],
    swept: sweep ? "192.168.1.0/24" : null,
    notes: [],
  };
}

let status: StatusPageSettings = { enabled: false, bind: "lan", port: 8787 };
let tok = "3f9c2a71d0b84e6f9a1c5d7e2b4f6a8c0e1d3b5a7c9e2f4a6b8d0c1e3f5a7b9";

const statusInfo = (): StatusPageInfo => ({
  settings: { ...status },
  running: status.enabled,
  urls: status.enabled ? [`http://${status.bind === "tailscale" ? "100.101.7.12" : "192.168.1.40"}:${status.port}/s/${tok}`] : [],
  error: null,
});

let schedules: HostSchedule[] = [];
const unitFor = (sp: ScheduleSpec) => (sp.action === "wake" ? `nightgrid-wake-${(sp.mac ?? "aabbcc001122").replace(/:/g, "")}` : `nightgrid-power-${sp.action}`);

/** A few days of plausible history for the timeline. */
function sampleEvents(now: number): HostEvent[] {
  const h = 3600;
  const e = (ago: number, kind: HostEvent["kind"], level: HostEvent["level"], title: string, detail = "", origin: HostEvent["origin"] = "host"): HostEvent => ({
    at: now - ago,
    kind,
    level,
    title,
    detail,
    origin,
  });
  return [
    e(0.2 * h, "recovered", "ok", "CONTAINER DOWN: web on host", "cleared after 4m", "nightgrid"),
    e(0.3 * h, "container", "warn", "web restarted", "after exit 1"),
    e(0.27 * h, "alert", "critical", "CONTAINER DOWN: web on host", "exited with code 1", "nightgrid"),
    e(5 * h, "stack", "info", "Updated stack media", "from Nightgrid; new images: jellyfin", "nightgrid"),
    e(26 * h, "packages", "info", "apt: upgraded 12, installed 1", "apt-get upgrade -y"),
    e(27 * h, "boot", "info", "Booted", "down for about 1m (last log before it at the previous boot's end)"),
    e(27.1 * h, "power", "info", "Rebooted from Nightgrid", "", "nightgrid"),
    e(50 * h, "oom", "critical", "Out of memory: killed java"),
    e(51 * h, "unit", "warn", "backup.service failed", "result: exit-code"),
    e(74 * h, "automation", "info", "Ran Update all", "finished", "nightgrid"),
    e(75 * h, "alert", "warn", "DISK FULL: host /srv", "91 % used", "nightgrid"),
  ].sort((a, b) => b.at - a.at);
}
const clearedTimelines = new Set<number>();

const DAY = 86400;
const nowS = () => Math.floor(Date.now() / 1000);
let watchId = 3;
let watch: WatchItem[] = [
  { id: 1, kind: "domain", name: "example.com", hostId: null, target: "example.com", expiresAt: nowS() + 21 * DAY, warnDays: 30, note: "Renews at the registrar", checkedAt: null, checkError: null },
  { id: 2, kind: "token", name: "DNS API token", hostId: null, target: "", expiresAt: nowS() + 90 * DAY, warnDays: 14, note: "", checkedAt: null, checkError: null },
  { id: 3, kind: "certFile", name: "Internal CA", hostId: 1, target: "/etc/ssl/certs/lab-ca.pem", expiresAt: nowS() + 2 * DAY, warnDays: 30, note: "", checkedAt: nowS() - 3600, checkError: null },
];

const baselines = new Map<number, SideInfo>();

export const mockOps = {
  confdiffBaseline: (hostId: number): Promise<SideInfo | null> => Promise.resolve(baselines.get(hostId) ?? null),
  confdiffSaveBaseline(hostId: number): Promise<SideInfo> {
    const b: SideInfo = { label: "baseline", taken: nowS(), manager: "apt", files: 412, packages: 1532 };
    baselines.set(hostId, b);
    return Promise.resolve(b);
  },
  confdiffCompare(hostId: number, other: number | null, hide: boolean): Promise<DriftReport> {
    const base = baselines.get(hostId);
    if (other === null && !base) return Promise.reject(new Error("No baseline on this host yet: save one first."));
    const left: SideInfo = other === null ? base! : { label: `host ${hostId}`, taken: nowS(), manager: "apt", files: 412, packages: 1532 };
    const right: SideInfo = { label: other === null ? `host ${hostId}` : `host ${other}`, taken: nowS(), manager: "apt", files: 415, packages: 1534 };
    return Promise.resolve({
      left,
      right,
      files: [
        { path: "/etc/ssh/sshd_config", change: "changed", secret: false },
        { path: "/etc/fail2ban/jail.local", change: "added", secret: false },
        { path: "/etc/cron.d/old-backup", change: "removed", secret: false },
        { path: "/srv/stacks/media/compose.yaml", change: "changed", secret: false },
        { path: "/srv/stacks/media/.env", change: "changed", secret: true },
      ],
      packages: [
        { name: "openssl", left: "3.0.13-1", right: "3.0.15-1" },
        { name: "fail2ban", left: null, right: "1.0.2-2" },
        { name: "telnet", left: "0.17-44", right: null },
      ],
      hidden: other !== null && hide ? 3 : 0,
      mixedManagers: false,
    });
  },
  confdiffFile(path: string): Promise<FileDiff> {
    return Promise.resolve({
      path,
      added: 2,
      removed: 1,
      binary: false,
      tooLarge: false,
      truncated: false,
      lines: [
        { tag: "skip", old: null, new: null, text: "12" },
        { tag: "same", old: 13, new: 13, text: "Port 22" },
        { tag: "del", old: 14, new: null, text: "PasswordAuthentication yes" },
        { tag: "add", old: null, new: 14, text: "PasswordAuthentication no" },
        { tag: "add", old: null, new: 15, text: "PermitRootLogin no" },
        { tag: "same", old: 15, new: 16, text: "UsePAM yes" },
        { tag: "skip", old: null, new: null, text: "40" },
      ],
    });
  },
  timeline(hostId: number, days: number): Promise<Timeline> {
    const now = Math.floor(Date.now() / 1000);
    const since = now - days * 86400;
    const all = sampleEvents(now).filter((x) => x.at >= since && !(clearedTimelines.has(hostId) && x.origin === "nightgrid"));
    return Promise.resolve({ events: all, notes: hostId % 2 ? [] : ["Docker events: Docker only remembers recent events."], since });
  },
  watchList: (): Promise<WatchItem[]> => Promise.resolve([...watch]),
  watchSave(input: WatchInput): Promise<WatchItem> {
    if (!input.name.trim()) return Promise.reject(new Error("name: 1 to 120 characters"));
    const old = input.id != null ? watch.find((w) => w.id === input.id) : undefined;
    const item: WatchItem = {
      id: old?.id ?? ++watchId,
      kind: input.kind,
      name: input.name.trim(),
      hostId: input.hostId ?? null,
      target: input.target.trim(),
      expiresAt: input.kind === "certFile" ? nowS() + 200 * DAY : (input.expiresAt ?? null),
      warnDays: input.warnDays,
      note: input.note,
      checkedAt: input.kind === "certFile" ? nowS() : null,
      checkError: null,
    };
    watch = old ? watch.map((w) => (w.id === old.id ? item : w)) : [...watch, item];
    return Promise.resolve(item);
  },
  watchDelete(id: number): Promise<null> {
    watch = watch.filter((w) => w.id !== id);
    return Promise.resolve(null);
  },
  watchLookup(id: number): Promise<WatchItem> {
    const w = watch.find((x) => x.id === id);
    if (!w) return Promise.reject(new Error("item not found"));
    const item = { ...w, expiresAt: nowS() + 300 * DAY, checkedAt: nowS(), checkError: null };
    watch = watch.map((x) => (x.id === id ? item : x));
    return Promise.resolve(item);
  },
  timelineClear(hostId: number): Promise<null> {
    clearedTimelines.add(hostId);
    return Promise.resolve(null);
  },
  powerSchedules: (hostId: number): Promise<HostSchedule[]> => Promise.resolve(schedules.filter((s) => s.onHostId === hostId || s.spec.action === "wake")),
  powerScheduleSet(hostId: number, spec: ScheduleSpec): Promise<HostSchedule> {
    const unit = unitFor(spec);
    const days = spec.days ?? [];
    const s: HostSchedule = {
      onHostId: hostId,
      onHostName: spec.action === "wake" ? "relay" : `host ${hostId}`,
      unit,
      spec: { action: spec.action, time: spec.time, days, mac: spec.mac ?? null },
      next: `Mon 2026-11-16 ${spec.time}:00 UTC`,
    };
    schedules = [...schedules.filter((x) => x.unit !== unit), s];
    return new Promise((r) => setTimeout(() => r(s), 300));
  },
  powerScheduleRemove(unit: string): Promise<null> {
    schedules = schedules.filter((x) => x.unit !== unit);
    return Promise.resolve(null);
  },
  labelsList: async (): Promise<DeviceLabel[]> => labels.map((l) => ({ ...l, tags: [...l.tags] })),
  labelSave: async (l: DeviceLabel): Promise<DeviceLabel | null> => {
    const key = l.key.trim().toLowerCase();
    const tags = [...new Set(l.tags.map((t) => t.trim().replace(/^#/, "").toLowerCase()).filter(Boolean))];
    if (tags.some((t) => /\s/.test(t))) throw new Error("tags are single words of at most 32 characters");
    const v = { key, name: l.name.trim(), kind: l.kind, tags, note: l.note.trim() };
    labels = labels.filter((x) => x.key !== key);
    if (!v.name && !v.tags.length && !v.note && v.kind === "other") return null;
    labels = [...labels, v];
    return v;
  },
  labelDelete: async (key: string): Promise<null> => {
    labels = labels.filter((x) => x.key !== key);
    return null;
  },
  discover: (_hostId: number, sweep: boolean): Promise<Discovery> => delay(discovery(sweep), sweep ? 1200 : 500),
  vms: (_hostId: number, elevate: boolean): Promise<VmReport> =>
    delay(
      elevate
        ? { vms: [...guests], libvirt: true, proxmox: true, needsRoot: false, notes: [] }
        : { vms: guests.filter((g) => g.kind !== "libvirt"), libvirt: true, proxmox: true, needsRoot: true, notes: ["virsh: authentication failed: access denied"] },
      400,
    ),
  vmAction: (kind: VmKind, id: string, action: VmAction): Promise<null> => {
    const to = action === "start" || action === "reboot" ? "running" : "stopped";
    guests = guests.map((g) => (g.kind === kind && g.id === id ? { ...g, state: to } : g));
    return delay(null, 500);
  },
  statusGet: async (): Promise<StatusPageInfo> => statusInfo(),
  statusSet: (s: StatusPageSettings): Promise<StatusPageInfo> => {
    if (s.port < 1024) return Promise.reject(new Error("invalid value for statusPage.port: must be between 1024 and 65535"));
    status = { ...s };
    return delay(statusInfo(), 200);
  },
  statusNewToken: async (): Promise<StatusPageInfo> => {
    tok = [...crypto.getRandomValues(new Uint8Array(32))].map((b) => b.toString(16).padStart(2, "0")).join("");
    return statusInfo();
  },
  jobsList: async (): Promise<BackupJob[]> => [...jobs],
  jobSave: async (id: number | null, i: BackupJobInput): Promise<BackupJob> => {
    const v = check(i);
    if (id === null) {
      const j = { id: nextId++, ...v };
      jobs = [...jobs, j];
      return j;
    }
    const j = { id, ...v };
    jobs = jobs.map((x) => (x.id === id ? j : x));
    return j;
  },
  jobDelete: async (id: number): Promise<null> => {
    jobs = jobs.filter((j) => j.id !== id);
    return null;
  },
  jobsCheck: (hostId: number): Promise<BackupStatus[]> => {
    const t = now();
    return delay(
      jobs
        .filter((j) => j.hostId === hostId)
        .map((j, i): BackupStatus => {
          const failed = j.name.includes("offsite");
          const age = failed ? 3 * 86400 : (i + 1) * 5 * 3600;
          const last = t - age;
          return {
            jobId: j.id,
            result: failed ? "failed" : "ok",
            lastRun: last,
            lastOk: failed ? t - 4 * 86400 : last,
            exitCode: failed ? 1 : 0,
            size: j.source === "path" ? 182_000_000_000 : 41_500_000_000,
            added: j.source === "path" ? null : 312_000_000,
            summary: failed ? "Fatal: unable to create lock in backend: repository is already locked" : j.tool === "restic" ? "snapshot 4f1a9c2e saved" : null,
            stale: failed || age > j.staleHours * 3600,
            error: null,
          };
        }),
      400,
    );
  },
  testsList: async (): Promise<LastTest[]> => [...tests],
  testRestore: async (jobId: number, sudo: SudoInput | null): Promise<RestoreTest> => {
    if (!sudo) throw { message: "sudo needs your password", connect: null, system: { kind: "sudoRequired", retry: false } };
    const j = jobs.find((x) => x.id === jobId);
    if (!j) throw { message: "backup job not found", connect: null, system: null };
    const at = now();
    const fail = j.name.includes("offsite");
    const t: RestoreTest = fail
      ? { jobId, ok: false, file: null, size: null, sha256: null, snapshot: null, snapshotTime: null, steps: ["using the environment of borg-offsite.service", "reading the latest archive"], error: "couldn't read archives: Repository is already locked", at }
      : {
          jobId,
          ok: true,
          file: "/srv/appdata/homepage/config/services.yaml",
          size: 4182,
          sha256: "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
          snapshot: "4f1a9c2e",
          snapshotTime: "2026-11-15T03:00:12Z",
          steps: ["using the environment of restic-backup.service", "reading the latest snapshot", "listing snapshot 4f1a9c2e", "restoring it into a temporary folder"],
          error: null,
          at,
        };
    tests = [...tests.filter((x) => x.jobId !== jobId), { jobId, at, ok: t.ok, note: t.ok ? `restored ${t.file} from ${t.snapshot}` : (t.error ?? "failed") }];
    return delay(t, 1500);
  },
  jobsDetect: (_hostId: number): Promise<BackupSuggestion[]> =>
    delay(
      [
        { name: "restic-backup", tool: "restic", source: "unit", target: "restic-backup.timer", detail: "timer restic-backup.timer: /usr/bin/restic backup /srv /etc" },
        { name: "rsync (crontab)", tool: "rsync", source: "path", target: "/mnt/backup/srv", detail: "crontab: 30 2 * * * rsync -a --delete /srv/ /mnt/backup/srv/" },
        { name: "borg (offsite)", tool: "borg", source: "path", target: "", detail: "/etc/cron.d/offsite: (command hidden: it looks like it holds a secret)" },
      ],
      500,
    ),
  proxyRoutes: (_hostId: number): Promise<ProxyReport> => {
    const t = now();
    const cert = (days: number, issuer = "Let's Encrypt R11") => ({ notAfter: t + days * 86400, issuer, error: null });
    return delay(
      {
        now: t,
        proxies: [
          { kind: "traefik", name: "traefik", routes: 3, error: null },
          { kind: "caddy", name: "host", routes: 1, error: null },
        ],
        routes: [
          { proxy: "traefik", proxyName: "traefik", domains: ["cloud.example.com"], upstream: "nextcloud:80", container: "nextcloud", tls: true, cert: cert(61), origin: "router nextcloud" },
          { proxy: "traefik", proxyName: "traefik", domains: ["grafana.example.com"], upstream: "grafana:3000", container: "grafana", tls: true, cert: cert(9), origin: "router grafana" },
          { proxy: "traefik", proxyName: "traefik", domains: ["old.example.com"], upstream: "legacy:8080", container: null, tls: true, cert: { notAfter: null, issuer: null, error: "No certificate answered on port 443 of this host for this name." }, origin: "router legacy" },
          { proxy: "caddy", proxyName: "host", domains: ["media.lan"], upstream: "localhost:8096", container: "jellyfin", tls: false, cert: null, origin: "Caddyfile" },
        ],
      },
      600,
    );
  },
  logSamples: async (key: string): Promise<string[]> =>
    key.startsWith("logs:")
      ? [
          "2026-11-15T10:41:02Z ERROR db: connection refused (postgres:5432)",
          "2026-11-15T10:41:07Z ERROR db: connection refused (postgres:5432)",
          "2026-11-15T10:41:12Z FATAL giving up after 5 retries",
        ]
      : [],
};
