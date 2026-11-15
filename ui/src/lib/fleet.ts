/** Fleet health (M10m): the update center's rows, counts and wording. */
import type { Host, ImageUpdate, PackageManager, Stack, UpdatesReport } from "./api";

export type FleetTab = "updates" | "disks" | "backups";

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
