/** Pi-hole / AdGuard Home helpers (M10v). */
import type { DnsList, DnsStatus } from "./api";

/** "48,213 queries · 12.7% blocked" or "BLOCKING OFF · resumes in 4 min". */
export function dnsSummary(s: DnsStatus): string {
  const q = `${s.queries.toLocaleString("en-US")} queries · ${(s.percentBlocked ?? 0).toFixed(1)}% blocked`;
  if (s.blocking) return q;
  return s.resumesIn ? `BLOCKING PAUSED · back in ${duration(s.resumesIn)}` : "BLOCKING OFF";
}

/** 45 → "45 s", 300 → "5 min", 5400 → "1 h 30 min". */
export function duration(secs: number): string {
  if (secs < 60) return `${secs} s`;
  const m = Math.round(secs / 60);
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  return m % 60 ? `${h} h ${m % 60} min` : `${h} h`;
}

/** Pause choices for the blocking menu. */
export const PAUSES: { label: string; seconds: number | null }[] = [
  { label: "5 minutes", seconds: 300 },
  { label: "30 minutes", seconds: 1800 },
  { label: "1 hour", seconds: 3600 },
  { label: "Until I turn it on", seconds: null },
];

/** Block lists first, then allow lists; each by name. */
export function sortLists(lists: DnsList[]): DnsList[] {
  return [...lists].sort((a, b) => Number(a.allow) - Number(b.allow) || (a.name || a.url).localeCompare(b.name || b.url));
}

/** A list's short name: its name, or the file part of its URL. */
export function listName(l: DnsList): string {
  if (l.name) return l.name;
  const path = l.url.split("?")[0].replace(/\/+$/, "");
  return path.slice(path.lastIndexOf("/") + 1) || l.url;
}

/** "nas.lan" / "*.apps.lan" (AdGuard allows wildcards; Pi-hole doesn't). */
export function validRecordDomain(d: string, wildcard: boolean): boolean {
  const name = wildcard ? d.replace(/^\*\./, "") : d;
  return /^(?=.{1,253}$)([a-z0-9_]([a-z0-9_-]{0,61})?)(\.[a-z0-9_]([a-z0-9_-]{0,61})?)*$/i.test(name);
}

export function isIp(s: string): boolean {
  return /^(\d{1,3}\.){3}\d{1,3}$/.test(s) || (/^[0-9a-f:]+$/i.test(s) && s.includes(":"));
}
