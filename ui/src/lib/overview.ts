/**
 * Overview widgets that say more (0.10.52): the per-item rows the SERVICES,
 * UPDATES, DISKS, BACKUPS and ALERTS widgets list, and when the widgets
 * fetch on their own. Pure functions; the widget wires them to the stores.
 */
import type { Alert, BackupJob, BackupStatus, Host, HostPing, Service, ServiceStatus } from "./api";
import { fillText, type HostDisks, type RowTone, type UpdateRow } from "./fleet";
import type { Health } from "./stores/services.svelte";

const MIN = 60_000;
const HOUR = 60 * MIN;

/** How old a widget's data may get before it fetches again on its own. */
export const AUTO_AGE = { updates: 3 * HOUR, disks: HOUR, backups: 30 * MIN } as const;

/** Whether data checked at `at` (unix ms, null = never) is due a refresh. */
export const due = (at: number | null, maxAge: number, now: number): boolean => at === null || now - at > maxAge;

/**
 * Hosts a widget may read on its own: servers that aren't known to be
 * offline. Clients (a laptop, a handheld) are only read when you ask, so
 * an Overview never wakes one up or asks it for a Tailscale approval.
 */
export function autoHosts(hosts: Host[], pings: Record<number, HostPing | undefined>): number[] {
  return hosts.filter((h) => h.role !== "client" && pings[h.id]?.state !== "offline").map((h) => h.id);
}

// ---- services ------------------------------------------------------------

export interface SvcRow {
  svc: Service;
  health: Health;
  /** "42 ms", "HTTP 502", "timed out", "off". */
  note: string;
  /** The full reason, for a tooltip. */
  detail: string;
}

const SVC_ORDER: Record<Health, number> = { down: 0, warn: 1, pending: 2, up: 3, off: 4 };

/** First clause of a check error, short enough for a row. */
export function shortError(e: string): string {
  const first = e.split(" · ")[0].trim();
  return first.length > 28 ? `${first.slice(0, 27)}…` : first;
}

/** Every service, problems first, then by name. */
export function serviceRows(list: Service[], status: Record<number, ServiceStatus | undefined>, healthOf: (s: Service, st: ServiceStatus | undefined) => Health): SvcRow[] {
  return list
    .map((svc) => {
      const st = status[svc.id];
      const health = healthOf(svc, st);
      const r = st?.result;
      let note = health === "off" ? "off" : !r ? "…" : !r.up ? (r.status != null ? `HTTP ${r.status}` : shortError(r.error ?? "down")) : r.latencyMs != null ? `${r.latencyMs} ms` : "up";
      if (health === "warn" && r?.tls && !r.tls.trusted) note = `${note} · cert`;
      return { svc, health, note, detail: r?.error ?? "" };
    })
    .sort((a, b) => SVC_ORDER[a.health] - SVC_ORDER[b.health] || a.svc.name.localeCompare(b.svc.name));
}

// ---- updates -------------------------------------------------------------

export interface UpdLine {
  host: Host;
  tone: RowTone;
  /** "12 pending · 3 security · reboot", "up to date", "couldn't check". */
  text: string;
  detail: string;
}

/** One line per host that has been checked (in updateRows' order: worst first). */
export function updateLines(rows: UpdateRow[]): UpdLine[] {
  const out: UpdLine[] = [];
  for (const r of rows) {
    const s = r.state;
    if (!s || (s.status === "checking" && !s.report)) continue;
    if (s.status === "error") {
      out.push({ host: r.host, tone: "unknown", text: "couldn't check", detail: s.error ?? "" });
      continue;
    }
    const bits = [r.held ? "held" : r.count ? `${r.count} pending` : "up to date"];
    if (r.heldCount && !r.held) bits.push(`${r.heldCount} held`);
    if (r.security) bits.push(`${r.security} security`);
    if (r.reboot) bits.push("reboot");
    out.push({ host: r.host, tone: r.tone, text: bits.join(" · "), detail: s.report?.rebootReasons.join("; ") ?? "" });
  }
  return out;
}

// ---- disks ---------------------------------------------------------------

export interface DiskLine {
  host: Host;
  tone: RowTone;
  /** "ZFS tank degraded", "SMART sda failing", "/srv full in about 9 days", "2 pools · SMART ok". */
  text: string;
  /** The fullest mount: percent used, or null. */
  pct: number | null;
  detail: string;
}

/** One line per host that has been read: its worst finding, or a short all-clear. */
export function diskLines(rows: { host: Host; state: HostDisks | undefined; tone: RowTone }[]): DiskLine[] {
  const out: DiskLine[] = [];
  for (const { host, state: d, tone } of rows) {
    if (!d || (d.status === "checking" && !d.checkedAt)) continue;
    if (d.status === "error") {
      out.push({ host, tone: "unknown", text: "couldn't read", pct: null, detail: d.error ?? "" });
      continue;
    }
    const pct = d.forecasts.length ? Math.round(Math.max(...d.forecasts.map((f) => (f.size ? (f.used / f.size) * 100 : 0)))) : null;
    const badPool = d.pools?.pools.find((p) => p.health === "bad") ?? d.pools?.pools.find((p) => p.health === "warn");
    const badDisk = d.smart?.disks.find((k) => k.health === "bad") ?? d.smart?.disks.find((k) => k.health === "warn");
    const soon = d.forecasts.filter((f) => f.daysLeft != null && f.counted !== false).sort((a, b) => (a.daysLeft ?? 0) - (b.daysLeft ?? 0))[0];
    let text: string;
    let detail = "";
    if (badPool) {
      text = `${badPool.kind.toUpperCase()} ${badPool.name} ${badPool.state.toLowerCase()}`;
      detail = badPool.issues.join("; ");
    } else if (badDisk) {
      text = `SMART ${badDisk.name} ${badDisk.health === "bad" ? "failing" : "warning"}`;
      detail = badDisk.issues.join("; ");
    } else if (soon && tone !== "ok") {
      text = `${soon.mount} ${fillText(soon)}`;
    } else {
      const pools = d.pools?.pools.length ?? 0;
      const bits: string[] = [];
      if (pools) bits.push(`${pools} pool${pools === 1 ? "" : "s"} ok`);
      bits.push(d.smart ? `SMART ok (${d.smart.disks.length})` : "SMART not read");
      text = bits.join(" · ");
      detail = d.smart ? "" : (d.smartNote ?? "");
      if (soon) detail = `${detail ? `${detail}; ` : ""}${soon.mount} ${fillText(soon)}`;
    }
    out.push({ host, tone, text, pct, detail });
  }
  return out;
}

// ---- backups -------------------------------------------------------------

export type JobWord = "ok" | "failed" | "stale" | "running" | "unknown" | "error";

export interface BackupLine {
  job: BackupJob;
  state: JobWord;
  /** "2h ago", "never", "—". */
  age: string;
  detail: string;
}

const JOB_ORDER: Record<JobWord, number> = { error: 0, failed: 0, stale: 1, unknown: 2, running: 3, ok: 4 };

/** Every job, problems first, with how long ago its last good run was. */
export function backupLines(jobs: BackupJob[], status: Record<number, BackupStatus | undefined>, stateOf: (s: BackupStatus | undefined) => JobWord, nowSecs: number): BackupLine[] {
  return jobs
    .map((job) => {
      const s = status[job.id];
      const state = stateOf(s);
      const age = !s ? "—" : s.lastOk != null ? `${agoShort(s.lastOk, nowSecs)} ago` : "never";
      return { job, state, age, detail: s?.error ?? s?.summary ?? "" };
    })
    .sort((a, b) => JOB_ORDER[a.state] - JOB_ORDER[b.state] || a.job.name.localeCompare(b.job.name));
}

// ---- alerts --------------------------------------------------------------

/** "4m", "2h", "3d" between two unix-second times. */
export function agoShort(ts: number, now: number): string {
  const s = Math.max(0, now - ts);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 48) return `${h}h`;
  return `${Math.floor(h / 24)}d`;
}

/** The last few resolved alerts, newest first, within `withinSecs`. */
export function recentIncidents(history: Alert[], nowSecs: number, withinSecs = 7 * 86_400, limit = 8): Alert[] {
  return history
    .filter((a) => a.resolved != null && nowSecs - a.resolved <= withinSecs)
    .sort((a, b) => (b.resolved ?? 0) - (a.resolved ?? 0))
    .slice(0, limit);
}
