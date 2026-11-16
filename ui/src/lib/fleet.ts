/** Fleet health (M10m): the update center's rows, counts and wording. */
import type { DisksReport, Health, Host, ImageUpdate, MountForecast, PackageManager, PoolKind, PoolsReport, Stack, UpdatesReport, UsagePoint } from "./api";

export type FleetTab = "updates" | "disks" | "backups" | "ups";

export interface HostUpdates {
  status: "checking" | "ok" | "error";
  report: UpdatesReport | null;
  error: string | null;
  /** Unix ms of the last finished check. */
  checkedAt: number | null;
}

export interface HostRun {
  lines: string[];
  running: boolean;
  code: number | null;
  error: string | null;
  /** Not started: why (no sudo password remembered, nothing to do, ...). */
  skipped: string | null;
}

export interface ImageRow {
  hostId: number;
  stack: Stack;
  update: ImageUpdate;
}

export const MANAGER_WORD: Record<PackageManager, string> = { apt: "APT", dnf: "DNF", pacman: "PACMAN", ostree: "RPM-OSTREE" };

export type RowTone = "bad" | "warn" | "ok" | "unknown" | "busy";

export interface UpdateRow {
  host: Host;
  state: HostUpdates | undefined;
  count: number;
  security: number;
  reboot: boolean;
  tone: RowTone;
  /** Can be upgraded from here (a supported manager and something to install). */
  upgradable: boolean;
}

/** Packages from a security pocket or repository (apt and zypper name them). */
export function securityCount(r: UpdatesReport | null | undefined): number {
  return r ? r.updates.filter((u) => /security/i.test(u.repo ?? "")).length : 0;
}

export function rowTone(s: HostUpdates | undefined): RowTone {
  if (!s) return "unknown";
  if (s.status === "checking") return "busy";
  if (s.status === "error" || !s.report) return "unknown";
  if (securityCount(s.report) || s.report.rebootRequired) return "bad";
  return s.report.updates.length ? "warn" : "ok";
}

const ORDER: Record<RowTone, number> = { bad: 0, warn: 1, busy: 2, unknown: 3, ok: 4 };

/** One row per host: security and reboots first, then pending, then the rest by name. */
export function updateRows(hosts: Host[], state: Record<number, HostUpdates>): UpdateRow[] {
  return hosts
    .map((host) => {
      const s = state[host.id];
      const r = s?.report ?? null;
      return {
        host,
        state: s,
        count: r?.updates.length ?? 0,
        security: securityCount(r),
        reboot: !!r?.rebootRequired,
        tone: rowTone(s),
        upgradable: !!r?.manager && r.updates.length > 0,
      };
    })
    .sort((a, b) => ORDER[a.tone] - ORDER[b.tone] || b.count - a.count || a.host.name.localeCompare(b.host.name));
}

export function totals(rows: UpdateRow[]): { hosts: number; packages: number; security: number; reboot: number; failed: number } {
  return {
    hosts: rows.filter((r) => r.count > 0).length,
    packages: rows.reduce((n, r) => n + r.count, 0),
    security: rows.reduce((n, r) => n + r.security, 0),
    reboot: rows.filter((r) => r.reboot).length,
    failed: rows.filter((r) => r.state?.status === "error").length,
  };
}

/**
 * What to type to confirm a batch upgrade (rule 5): the host's name for one
 * host, the group's name when the selection is exactly one group, and
 * otherwise "upgrade N hosts".
 */
export function confirmPhrase(selected: Host[], groups: { id: number; name: string }[], allHosts: Host[]): string {
  if (selected.length === 1) return selected[0].name;
  const gid = selected[0]?.groupId;
  if (gid != null && selected.every((h) => h.groupId === gid)) {
    const members = allHosts.filter((h) => h.groupId === gid);
    const g = groups.find((x) => x.id === gid);
    if (g && members.length === selected.length) return g.name;
  }
  return `upgrade ${selected.length} hosts`;
}

/** Images whose tag moved, newest check first, grouped by host then stack. */
export function pendingImages(rows: ImageRow[]): ImageRow[] {
  return rows
    .filter((r) => r.update.status === "available")
    .sort((a, b) => a.hostId - b.hostId || a.stack.name.localeCompare(b.stack.name) || a.update.image.localeCompare(b.update.image));
}

/** "3 min ago", "just now". */
export function agoText(at: number | null, now: number): string {
  if (at == null) return "never";
  const s = Math.max(0, Math.round((now - at) / 1000));
  if (s < 45) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  return h < 48 ? `${h} h ago` : `${Math.round(h / 24)} days ago`;
}

/** Plain text for DRIFT. */
export function updatesText(rows: UpdateRow[], images: ImageRow[]): string {
  const lines: string[] = [];
  for (const r of rows) {
    const s = r.state;
    if (!s || s.status === "checking") continue;
    if (s.status === "error") {
      lines.push(`${r.host.name}: couldn't check (${s.error ?? "error"})`);
      continue;
    }
    const rep = s.report;
    if (!rep) continue;
    const mgr = rep.manager ?? "no package manager";
    const head = `${r.host.name} (${mgr}): ${r.count ? `${r.count} pending${r.security ? `, ${r.security} security` : ""}` : "up to date"}${r.reboot ? `; reboot required (${rep.rebootReasons.join("; ")})` : ""}`;
    lines.push(head);
    for (const u of rep.updates.slice(0, 25)) lines.push(`  - ${u.name} ${u.current ?? "?"} → ${u.available}${u.repo ? ` [${u.repo}]` : ""}`);
    if (rep.updates.length > 25) lines.push(`  - …and ${rep.updates.length - 25} more`);
  }
  const pend = pendingImages(images);
  if (pend.length) {
    lines.push("Container images with a newer version:");
    for (const i of pend) lines.push(`  - stack ${i.stack.name}: ${i.update.image} ${i.update.current.version ?? ""} → ${i.update.latest?.version ?? "newer"}`);
  }
  return lines.join("\n");
}

// ---- disks -------------------------------------------------------------

export interface HostDisks {
  status: "checking" | "ok" | "error";
  /** Couldn't connect at all. */
  error: string | null;
  pools: PoolsReport | null;
  smart: DisksReport | null;
  /** Why SMART wasn't read (sudo needs a password, no smartctl, ...). */
  smartNote: string | null;
  forecasts: MountForecast[];
  checkedAt: number | null;
}

const GB = 1_000_000_000;

/** "1.2 TB", "640 GB" (decimal, like df -H). */
export function size(n: number | null | undefined): string {
  if (n == null) return "—";
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let v = Math.abs(n);
  let i = 0;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000;
    i++;
  }
  return `${n < 0 ? "−" : ""}${v >= 100 || i === 0 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
}

/** "+31 GB/day", "−1.2 GB/day", "flat". */
export function rateText(perDay: number | null): string {
  if (perDay == null) return "";
  if (Math.abs(perDay) < 0.05 * GB) return "flat";
  return `${perDay > 0 ? "+" : "−"}${size(Math.abs(perDay))}/day`;
}

/** "full in about 12 days", "not growing", "learning…". */
export function fillText(f: MountForecast): string {
  if (f.perDay == null) return (f.spanDays ?? 0) < 1 ? "learning (needs a day of history)" : "not enough history";
  if (f.daysLeft == null) return "not growing";
  const d = f.daysLeft;
  if (d < 1) return `full in about ${Math.max(1, Math.round(d * 24))} hours`;
  if (d >= 365) return "full in over a year";
  return `full in about ${Math.floor(d)} day${Math.floor(d) === 1 ? "" : "s"}`;
}

export function fillTone(f: MountForecast, within: number): RowTone {
  if (f.daysLeft == null) return f.perDay == null ? "unknown" : "ok";
  if (f.daysLeft < 3) return "bad";
  return f.daysLeft < within ? "warn" : "ok";
}

/** Percent used, 0–100. */
export const usedPct = (f: MountForecast): number => (f.size ? Math.min(100, (f.used / f.size) * 100) : 0);

/**
 * SVG path for a sparkline of daily usage, scaled to its own range but at
 * least `minRange` tall (so noise on a flat mount stays flat).
 */
export function sparkPath(points: UsagePoint[], w: number, h: number, minRange = 0): string {
  if (points.length < 2) return "";
  let lo = Math.min(...points.map((p) => p.used));
  let hi = Math.max(...points.map((p) => p.used));
  if (hi - lo < minRange) {
    const mid = (hi + lo) / 2;
    lo = mid - minRange / 2;
    hi = mid + minRange / 2;
  }
  const t0 = points[0].ts;
  const span = points[points.length - 1].ts - t0 || 1;
  const range = hi - lo || 1;
  return points
    .map((p, i) => {
      const x = ((p.ts - t0) / span) * w;
      const y = h - 1 - ((p.used - lo) / range) * (h - 2);
      return `${i ? "L" : "M"}${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
}

const HEALTH_TONE: Record<Health, RowTone> = { ok: "ok", unknown: "unknown", warn: "warn", bad: "bad" };

/** The worst of a host's pools, SMART and forecasts. */
export function diskTone(d: HostDisks | undefined, within: number): RowTone {
  if (!d) return "unknown";
  if (d.status === "checking" && !d.checkedAt) return "busy";
  if (d.status === "error") return "unknown";
  const tones: RowTone[] = [
    d.pools ? HEALTH_TONE[d.pools.health] : "ok",
    d.smart ? HEALTH_TONE[d.smart.health] : "ok",
    ...d.forecasts.map((f) => fillTone(f, within)),
  ];
  for (const t of ["bad", "warn"] as const) if (tones.includes(t)) return t;
  return "ok";
}

export function diskRows(hosts: Host[], state: Record<number, HostDisks>, within: number): { host: Host; state: HostDisks | undefined; tone: RowTone }[] {
  return hosts
    .map((host) => ({ host, state: state[host.id], tone: diskTone(state[host.id], within) }))
    .sort((a, b) => ORDER[a.tone] - ORDER[b.tone] || a.host.name.localeCompare(b.host.name));
}

export const POOL_WORD: Record<PoolKind, string> = { zfs: "ZFS", mdraid: "RAID", btrfs: "BTRFS" };

/** Plain text for DRIFT. */
export function disksText(rows: { host: Host; state: HostDisks | undefined }[]): string {
  const lines: string[] = [];
  for (const { host, state: d } of rows) {
    if (!d || d.status === "checking") continue;
    if (d.status === "error") {
      lines.push(`${host.name}: couldn't read (${d.error ?? "error"})`);
      continue;
    }
    lines.push(`${host.name}:`);
    for (const p of d.pools?.pools ?? []) {
      lines.push(
        `  - ${POOL_WORD[p.kind]} ${p.name}${p.detail ? ` (${p.detail})` : ""}: ${p.state}, ${p.health}${p.issues.length ? `; ${p.issues.join("; ")}` : ""}${p.activity ? `; ${p.activity}` : ""}`,
      );
    }
    if (d.smart) for (const k of d.smart.disks) lines.push(`  - disk ${k.name} ${k.model ?? ""}: SMART ${k.health}${k.issues.length ? `; ${k.issues.join("; ")}` : ""}`);
    else if (d.smartNote) lines.push(`  - SMART not read: ${d.smartNote}`);
    for (const f of d.forecasts) lines.push(`  - ${f.mount}: ${size(f.used)} of ${size(f.size)} (${usedPct(f).toFixed(0)}%), ${rateText(f.perDay) || "trend unknown"}, ${fillText(f)}`);
  }
  return lines.join("\n");
}
