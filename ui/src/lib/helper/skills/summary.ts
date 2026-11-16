/**
 * Daily summary (M10p): what Nightgrid itself knows about the last 24 hours,
 * as plain facts for DRIFT to summarize. Nothing is fetched for it; widgets
 * and views that ran checks leave their results here.
 */

export interface SummaryInput {
  /** Unix seconds. */
  now: number;
  active: { title: string; detail: string; since: number; host: string | null; severity: string }[];
  /** Alerts resolved or raised in the window. */
  resolved: { title: string; since: number; resolved: number; host: string | null }[];
  hostsTotal: number;
  /** Hosts monitoring can't reach right now. */
  offline: string[];
  /** Hosts that restarted in the window (uptime shorter than it). */
  restarted: { host: string; agoSecs: number }[];
  services: { total: number; down: string[]; warn: string[] };
  /** Null when no update check ran yet. */
  updates: { packages: number; hosts: number; security: number; reboot: string[] } | null;
  /** Hosts with disk findings, "name: detail". Null when not read yet. */
  disks: string[] | null;
  /** Failed or stale backup jobs. Null when not read yet. */
  backups: string[] | null;
}

const ago = (secs: number) => (secs < 3600 ? `${Math.max(1, Math.round(secs / 60))} min ago` : `${Math.round(secs / 3600)} h ago`);

/** The facts as lines; each starts with what it is about. */
export function summaryFacts(i: SummaryInput): string[] {
  const out: string[] = [];
  const DAY = 86400;
  out.push(i.hostsTotal ? `Hosts: ${i.hostsTotal} saved${i.offline.length ? `, unreachable now: ${i.offline.join(", ")}` : ", all reachable"}.` : "Hosts: none saved yet.");
  if (i.restarted.length) out.push(`Restarted in the last 24 h: ${i.restarted.map((r) => `${r.host} (${ago(r.agoSecs)})`).join(", ")}.`);
  if (i.active.length) out.push(`Active alerts (${i.active.length}): ${i.active.map((a) => `${a.severity === "critical" ? "CRITICAL " : ""}${a.title}${a.host ? ` on ${a.host}` : ""} since ${ago(i.now - a.since)}`).join("; ")}.`);
  else out.push("Active alerts: none.");
  const recent = i.resolved.filter((a) => i.now - a.resolved < DAY);
  if (recent.length) out.push(`Resolved in the last 24 h (${recent.length}): ${recent.slice(0, 8).map((a) => `${a.title}${a.host ? ` on ${a.host}` : ""} (lasted ${ago(a.resolved - a.since).replace(" ago", "")})`).join("; ")}.`);
  if (i.services.total) out.push(`Services: ${i.services.total - i.services.down.length - i.services.warn.length}/${i.services.total} up${i.services.down.length ? `; down: ${i.services.down.join(", ")}` : ""}${i.services.warn.length ? `; warning: ${i.services.warn.join(", ")}` : ""}.`);
  if (i.updates) out.push(i.updates.packages ? `Updates: ${i.updates.packages} pending on ${i.updates.hosts} host(s)${i.updates.security ? `, ${i.updates.security} security` : ""}${i.updates.reboot.length ? `; reboot needed: ${i.updates.reboot.join(", ")}` : ""}.` : `Updates: none pending${i.updates.reboot.length ? `; reboot needed: ${i.updates.reboot.join(", ")}` : ""}.`);
  else out.push("Updates: not checked this session (Fleet → UPDATES → Check all).");
  if (i.disks) out.push(i.disks.length ? `Disks needing attention: ${i.disks.join("; ")}.` : "Disks: no problems found.");
  else out.push("Disks: not read this session.");
  if (i.backups) out.push(i.backups.length ? `Backups failing or stale: ${i.backups.join(", ")}.` : "Backups: all recent and passing.");
  return out;
}

/** Questions that ask for the daily summary. */
export const wantsSummary = (q: string): boolean => /\b(daily (summary|report|brief)|summary of (today|the day|my (lab|servers|hosts))|summari[sz]e (today|the day|my (lab|servers|hosts|homelab))|what happened (today|overnight|while i was away)|morning (report|brief|summary)|status report)\b/i.test(q);
