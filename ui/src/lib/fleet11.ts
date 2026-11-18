/** Fleet 1.1 tabs: catalog, drills, capacity and replay wording and maths. */
import type { CatalogHost, Coverage, Drill, DrillView, HostCapacity, MetricPoint as Point, Replay, ReplayEvent } from "./api";

// ---- catalog

export const coverageLabel: Record<Coverage, string> = { covered: "BACKED UP", partial: "PARTLY", none: "NO BACKUP", stateless: "NO DATA" };
export const coverageTone: Record<Coverage, "ok" | "warn" | "bad" | "muted"> = { covered: "ok", partial: "warn", none: "bad", stateless: "muted" };

export function catalogSummary(hosts: CatalogHost[]) {
  const all = hosts.flatMap((h) => h.entries);
  return {
    containers: all.length,
    running: all.filter((e) => e.state === "running").length,
    unprotected: all.filter((e) => e.coverage === "none" || e.coverage === "partial").length,
    urls: all.reduce((n, e) => n + e.urls.length, 0),
  };
}

/** Case-insensitive match on name, image, project, URL or a mount path. */
export function catalogMatch(e: CatalogHost["entries"][number], q: string): boolean {
  const s = q.trim().toLowerCase();
  if (!s) return true;
  return [e.name, e.image, e.project ?? "", ...e.urls, ...e.mounts.map((m) => m.source)].some((x) => x.toLowerCase().includes(s));
}

// ---- drills

export const EVERY = [0, 7, 14, 30, 90];
export const everyLabel = (d: number) => (d === 0 ? "by hand" : d === 7 ? "weekly" : `every ${d} days`);

export function blankDrill(hostId: number): Drill {
  return { id: null, name: "", kind: "job", hostId, jobId: null, node: null, guestKind: null, vmid: null, storage: null, targetStorage: null, everyDays: 30, enabled: true };
}

export type DrillTone = "ok" | "bad" | "muted" | "warn";
export function drillTone(v: DrillView, now: number): DrillTone {
  if (!v.last) return "muted";
  if (!v.last.ok) return "bad";
  // A scheduled drill that missed its date by more than two days.
  if (v.nextAt != null && now - v.nextAt > 2 * 86400) return "warn";
  return "ok";
}

// ---- capacity

export interface Workload {
  id: string;
  label: string;
  /** Cores kept busy on average at its busiest. */
  cores: number;
  memGb: number;
  diskGb: number;
}

/** Typical demands; rough on purpose and editable in the UI. */
export const WORKLOADS: Workload[] = [
  { id: "container", label: "Small web app container", cores: 0.2, memGb: 0.5, diskGb: 2 },
  { id: "db", label: "Database (Postgres, MariaDB)", cores: 0.5, memGb: 2, diskGb: 20 },
  { id: "transcode-1080", label: "Media server, one 1080p CPU transcode", cores: 2, memGb: 2, diskGb: 10 },
  { id: "transcode-4k", label: "Media server, one 4K CPU transcode", cores: 6, memGb: 3, diskGb: 10 },
  { id: "game", label: "Game server (Minecraft, ARK)", cores: 2, memGb: 8, diskGb: 30 },
  { id: "ha", label: "Home Assistant VM", cores: 0.3, memGb: 2, diskGb: 32 },
  { id: "linux-vm", label: "Linux VM (2 vCPU, 4 GB)", cores: 1, memGb: 4, diskGb: 32 },
  { id: "windows-vm", label: "Windows VM (4 vCPU, 8 GB)", cores: 2, memGb: 8, diskGb: 80 },
];

export type FitVerdict = "fits" | "tight" | "no" | "unknown";

export interface Fit {
  verdict: FitVerdict;
  cpuAfter: number | null;
  memAfter: number | null;
  /** The mount with the most room after, or null when none has room. */
  disk: string | null;
  why: string;
}

const GB = 1e9;

/** Whether `w` fits on a host at its p95, leaving headroom. */
export function fit(c: HostCapacity, w: Workload): Fit {
  const spec = c.spec;
  if (!spec || c.cpuP95 == null || c.memP95 == null) return { verdict: "unknown", cpuAfter: null, memAfter: null, disk: null, why: "no size or history yet: keep it monitored for a day" };
  const cpuAfter = c.cpuP95 + (w.cores / spec.cores) * 100;
  const memAfter = c.memP95 + ((w.memGb * GB) / spec.memTotal) * 100;
  const roomy = c.disks.filter((d) => d.size - d.used >= w.diskGb * GB * 1.1 && (d.used + w.diskGb * GB) / d.size <= 0.9).sort((a, b) => b.size - b.used - (a.size - a.used));
  const disk = roomy[0]?.mount ?? null;
  const r = (x: number) => Math.round(x);
  let verdict: FitVerdict = "fits";
  const why: string[] = [];
  if (cpuAfter > 95 || memAfter > 95 || !disk) verdict = "no";
  else if (cpuAfter > 80 || memAfter > 85) verdict = "tight";
  if (cpuAfter > 80) why.push(`CPU ${r(cpuAfter)}% at its busiest`);
  if (memAfter > 85) why.push(`memory ${r(memAfter)}%`);
  if (!disk) why.push(`no disk with ${w.diskGb} GB to spare`);
  return { verdict, cpuAfter: r(cpuAfter), memAfter: r(memAfter), disk, why: why.join(", ") || `CPU ${r(cpuAfter)}%, memory ${r(memAfter)}% at its busiest` };
}

/** A server for these workloads plus 30% headroom (for a new Proxmox host). */
export function sizing(ws: { w: Workload; n: number }[]) {
  const sum = (f: (w: Workload) => number) => ws.reduce((a, x) => a + f(x.w) * x.n, 0);
  const cores = sum((w) => w.cores);
  const mem = sum((w) => w.memGb);
  const disk = sum((w) => w.diskGb);
  const up = (x: number, step: number) => Math.max(step, Math.ceil(x / step) * step);
  return { cores: up(cores * 1.3 + 1, 2), memGb: up(mem * 1.3 + 4, 8), diskGb: up(disk * 1.3 + 64, 128) };
}

// ---- replay

/** Events up to and including `t`. */
export const eventsUntil = (r: Replay, t: number): ReplayEvent[] => r.events.filter((e) => e.at <= t);

/** The metric point at or just before `t`. */
export function pointAt(points: Point[], t: number): Point | null {
  let best: Point | null = null;
  for (const p of points) {
    if (p.ts <= t) best = p;
    else break;
  }
  return best;
}

/** 0..1 position of `t` in the window. */
export const pos = (r: Replay, t: number) => (r.to === r.from ? 0 : Math.min(1, Math.max(0, (t - r.from) / (r.to - r.from))));

export function clock(t: number): string {
  const d = new Date(t * 1000);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

export function duration(sec: number): string {
  if (sec < 90) return `${Math.max(0, Math.round(sec))} s`;
  if (sec < 5400) return `${Math.round(sec / 60)} min`;
  return `${(sec / 3600).toFixed(1)} h`;
}

/** The replay as a Markdown timeline. */
export function replayMarkdown(r: Replay): string {
  const lines = r.events.map((e) => `- ${new Date(e.at * 1000).toLocaleString()} · ${e.source} · ${e.title}${e.detail ? `: ${e.detail}` : ""}`);
  return `# ${r.incident.title}\n\n${new Date(r.incident.start * 1000).toLocaleString()}, lasted ${duration(r.incident.end - r.incident.start)}. ${r.incident.detail}\n\n## Timeline\n\n${lines.join("\n") || "Nothing else was recorded."}\n`;
}
