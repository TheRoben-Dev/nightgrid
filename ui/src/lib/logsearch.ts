/** Cross-host log search helpers (M10u): defaults, look-back choices and highlighting. */
import type { LogHit, LogSearchQuery } from "./api";

export const SINCE = [
  { mins: 15, label: "15 min" },
  { mins: 60, label: "1 hour" },
  { mins: 360, label: "6 hours" },
  { mins: 1440, label: "24 hours" },
  { mins: 10080, label: "7 days" },
] as const;

export function blankQuery(): LogSearchQuery {
  return { text: "", regex: false, caseSensitive: false, hosts: [], journal: true, containers: true, unit: null, container: null, priority: null, sinceMins: 60 };
}

/** Splits a line into plain and matching parts for display. */
export function highlight(text: string, q: Pick<LogSearchQuery, "text" | "regex" | "caseSensitive">): { t: string; hit: boolean }[] {
  const needle = q.text.trim();
  if (!needle) return [{ t: text, hit: false }];
  let re: RegExp;
  try {
    re = q.regex ? new RegExp(needle, q.caseSensitive ? "g" : "gi") : new RegExp(needle.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), q.caseSensitive ? "g" : "gi");
  } catch {
    return [{ t: text, hit: false }];
  }
  const out: { t: string; hit: boolean }[] = [];
  let last = 0;
  for (const m of text.matchAll(re)) {
    if (!m[0]) continue;
    const i = m.index ?? 0;
    if (i > last) out.push({ t: text.slice(last, i), hit: false });
    out.push({ t: m[0], hit: true });
    last = i + m[0].length;
    if (out.length > 60) break;
  }
  if (last < text.length) out.push({ t: text.slice(last), hit: false });
  return out.length ? out : [{ t: text, hit: false }];
}

/** "journal/sshd" → { kind: "journal", name: "sshd" }. */
export function splitSource(s: string): { kind: "journal" | "container"; name: string } {
  const i = s.indexOf("/");
  const kind = s.slice(0, i) === "container" ? "container" : "journal";
  return { kind, name: i >= 0 ? s.slice(i + 1) : s };
}

/** Hit counts per source, most first, for the filter chips. */
export function sourceCounts(hits: LogHit[]): { source: string; n: number }[] {
  const by: Record<string, number> = {};
  for (const h of hits) by[h.source] = (by[h.source] ?? 0) + 1;
  return Object.entries(by)
    .map(([source, n]) => ({ source, n }))
    .sort((a, b) => b.n - a.n || a.source.localeCompare(b.source));
}

/** Plain text of the results, for copying or asking DRIFT. */
export function hitsText(hits: LogHit[], hostName: (id: number) => string): string {
  return hits.map((h) => `${new Date((h.ts ?? 0) * 1000).toISOString()} ${hostName(h.hostId)} ${h.source}: ${h.text}`).join("\n");
}
