/**
 * Browser-only stand-ins for the backup board, proxy routes and log samples
 * (M10g), for `pnpm dev`, tests and screenshots.
 */
import type {
  BackupJob,
  BackupJobInput,
  BackupStatus_Serialize as BackupStatus,
  BackupSuggestion,
  ProxyReport_Serialize as ProxyReport,
} from "./bindings";

let nextId = 1;
let jobs: BackupJob[] = [];

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

export const mockOps = {
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
