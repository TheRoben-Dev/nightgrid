/** Expiry watch (M10l): one list of everything that runs out on a date. */
import type { ProxyReport, Service, ServiceStatus, WatchItem, WatchKind } from "./api";

const DAY = 86_400;

export const KIND_WORD: Record<WatchKind, string> = {
  certFile: "CERT FILE",
  domain: "DOMAIN",
  token: "TOKEN",
  license: "LICENCE",
  other: "OTHER",
};

export type Tone = "ok" | "warn" | "bad" | "unknown";

export interface ExpiryRow {
  key: string;
  /** Where the row comes from: a watch item, a service check or a proxy route. */
  source: "watch" | "service" | "proxy";
  kind: string;
  name: string;
  /** A host, path or URL, for context. */
  where: string;
  expiresAt: number | null;
  days: number | null;
  warnDays: number;
  error: string | null;
  tone: Tone;
  item?: WatchItem;
}

/** Whole days left (negative once expired). */
export function daysLeft(at: number | null | undefined, now: number): number | null {
  return at == null ? null : Math.floor((at - now) / DAY);
}

export function toneOf(days: number | null, warnDays: number, error: string | null = null): Tone {
  if (days === null) return error ? "bad" : "unknown";
  if (days <= 3) return "bad";
  return days < warnDays ? "warn" : "ok";
}

/** "expired 3 days ago", "today", "tomorrow", "in 20 days". */
export function leftText(days: number | null): string {
  if (days === null) return "unknown";
  if (days < 0) return `expired ${-days} day${days === -1 ? "" : "s"} ago`;
  if (days === 0) return "today";
  if (days === 1) return "tomorrow";
  return `in ${days} days`;
}

/** `YYYY-MM-DD` (local) for a date input, and back (local midnight). */
export function toDateInput(at: number | null): string {
  if (at == null) return "";
  const d = new Date(at * 1000);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}
export function fromDateInput(s: string): number | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s.trim());
  if (!m) return null;
  return Math.floor(new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3])).getTime() / 1000);
}

/**
 * Watch items, service TLS certificates and (when read) proxy route
 * certificates, soonest first; unknown dates last.
 */
export function expiryRows(
  o: {
    items: WatchItem[];
    services: Service[];
    status: Record<number, ServiceStatus>;
    proxy?: { report: ProxyReport | null; hostName: string } | null;
    hostName: (id: number | null | undefined) => string;
    certDays: number;
  },
  now: number,
): ExpiryRow[] {
  const rows: ExpiryRow[] = [];
  for (const i of o.items) {
    const days = daysLeft(i.expiresAt, now);
    const where = i.kind === "certFile" ? `${o.hostName(i.hostId)}:${i.target}` : i.target;
    rows.push({
      key: `w${i.id}`,
      source: "watch",
      kind: KIND_WORD[i.kind],
      name: i.name,
      where,
      expiresAt: i.expiresAt ?? null,
      days,
      warnDays: i.warnDays,
      error: i.checkError ?? null,
      tone: toneOf(days, i.warnDays, i.checkError ?? null),
      item: i,
    });
  }
  for (const s of o.services) {
    const tls = o.status[s.id]?.result.tls;
    if (!tls?.expires) continue;
    const days = daysLeft(tls.expires, now);
    rows.push({
      key: `s${s.id}`,
      source: "service",
      kind: "SERVICE TLS",
      name: s.name,
      where: s.url,
      expiresAt: tls.expires,
      days,
      warnDays: o.certDays,
      error: tls.trusted ? null : (tls.problem ?? "not trusted"),
      tone: toneOf(days, o.certDays),
    });
  }
  const seen = new Set(rows.filter((r) => r.source === "service").map((r) => hostOf(r.where)));
  for (const r of o.proxy?.report?.routes ?? []) {
    const domain = r.domains[0];
    if (!domain || !r.cert || seen.has(domain)) continue;
    seen.add(domain);
    const at = r.cert.notAfter ?? null;
    const days = daysLeft(at, now);
    rows.push({
      key: `p${domain}`,
      source: "proxy",
      kind: "PROXY TLS",
      name: domain,
      where: `${o.proxy?.hostName ?? ""} → ${r.container ?? r.upstream}`,
      expiresAt: at,
      days,
      warnDays: o.certDays,
      error: r.cert.error ?? null,
      tone: toneOf(days, o.certDays, r.cert.error ?? null),
    });
  }
  return rows.sort((a, b) => (a.days ?? Infinity) - (b.days ?? Infinity) || a.name.localeCompare(b.name));
}

function hostOf(url: string): string {
  const m = /^[a-z]+:\/\/([^/:]+)/i.exec(url);
  return m ? m[1].toLowerCase() : url;
}

/** Counts for the panel header. */
export function expiryCounts(rows: ExpiryRow[]): { bad: number; warn: number } {
  return { bad: rows.filter((r) => r.tone === "bad").length, warn: rows.filter((r) => r.tone === "warn").length };
}
