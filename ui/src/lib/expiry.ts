/** Expiry watch (M10l): one list of everything that runs out on a date. */
import type { ExpiryPref, PrefTarget, ProxyReport, Service, ServiceStatus, WatchItem, WatchKind } from "./api";

const DAY = 86_400;

export const KIND_WORD: Record<WatchKind, string> = {
  certFile: "CERT FILE",
  domain: "DOMAIN",
  token: "TOKEN",
  license: "LICENCE",
  other: "OTHER",
};

export type Tone = "ok" | "warn" | "bad" | "unknown" | "muted";

/** How a service certificate stands with the OS and with the user. */
export type Trust = "trusted" | "pinned" | "changed" | "untrusted";

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
  /** Shown, but never alerts. */
  muted: boolean;
  /** Service and proxy rows: what the user's edits are keyed by. */
  target?: PrefTarget;
  pref?: ExpiryPref;
  /** The date the certificate itself gives, when the user typed another. */
  certExpiresAt?: number | null;
  /** Service rows: trust of the certificate, and its SHA-256. */
  trust?: Trust;
  fingerprint?: string | null;
  /** Service rows: the certificate's own problem, even when trusted by the user. */
  problem?: string | null;
}

const RANK: Tone[] = ["muted", "unknown", "ok", "warn", "bad"];
const worse = (a: Tone, b: Tone): Tone => (RANK.indexOf(a) >= RANK.indexOf(b) ? a : b);

/** Same edits target? */
export function sameTarget(a: PrefTarget, b: PrefTarget): boolean {
  return a.kind === "service" && b.kind === "service" ? a.serviceId === b.serviceId : a.kind === "proxy" && b.kind === "proxy" && a.domain === b.domain;
}

/** `ab12…` → `AB:12:…`, the way browsers and openssl show it. */
export function fingerprintText(fp: string | null | undefined): string {
  return fp ? (fp.match(/../g) ?? []).join(":").toUpperCase() : "";
}

/** Trust of a service certificate, given the fingerprint the user accepted. */
export function trustOf(tls: { trusted: boolean; fingerprint?: string | null }, pinned: string | null | undefined): Trust {
  if (tls.trusted) return "trusted";
  if (!pinned) return "untrusted";
  return tls.fingerprint && tls.fingerprint === pinned ? "pinned" : "changed";
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
    prefs?: ExpiryPref[];
  },
  now: number,
): ExpiryRow[] {
  const rows: ExpiryRow[] = [];
  const prefFor = (t: PrefTarget) => o.prefs?.find((p) => sameTarget(p.target, t));
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
      tone: i.muted ? "muted" : toneOf(days, i.warnDays, i.checkError ?? null),
      item: i,
      muted: i.muted,
    });
  }
  for (const s of o.services) {
    const tls = o.status[s.id]?.result.tls;
    if (!tls?.expires) continue;
    const target: PrefTarget = { kind: "service", serviceId: s.id };
    const pref = prefFor(target);
    const at = pref?.expiresAt ?? tls.expires;
    const days = daysLeft(at, now);
    const warnDays = pref?.warnDays ?? o.certDays;
    const trust = trustOf(tls, pref?.trustedFingerprint);
    const problem = tls.trusted ? null : (tls.problem ?? "not trusted");
    const error = trust === "changed" ? `certificate changed since you trusted it (${problem})` : trust === "untrusted" ? problem : null;
    const muted = !!pref?.muted;
    rows.push({
      key: `s${s.id}`,
      source: "service",
      kind: "SERVICE TLS",
      name: s.name,
      where: s.url,
      expiresAt: at,
      days,
      warnDays,
      error,
      // A certificate other than the one the user trusted is never quiet.
      tone: trust === "changed" ? worse(toneOf(days, warnDays), "warn") : muted ? "muted" : toneOf(days, warnDays),
      muted,
      target,
      pref,
      certExpiresAt: tls.expires,
      trust,
      fingerprint: tls.fingerprint ?? null,
      problem,
    });
  }
  const seen = new Set(rows.filter((r) => r.source === "service").map((r) => hostOf(r.where)));
  for (const r of o.proxy?.report?.routes ?? []) {
    const domain = r.domains[0];
    if (!domain || !r.cert || seen.has(domain)) continue;
    seen.add(domain);
    const target: PrefTarget = { kind: "proxy", domain };
    const pref = prefFor(target);
    const at = pref?.expiresAt ?? r.cert.notAfter ?? null;
    const days = daysLeft(at, now);
    const warnDays = pref?.warnDays ?? o.certDays;
    const muted = !!pref?.muted;
    rows.push({
      key: `p${domain}`,
      source: "proxy",
      kind: "PROXY TLS",
      name: domain,
      where: `${o.proxy?.hostName ?? ""} → ${r.container ?? r.upstream}`,
      expiresAt: at,
      days,
      warnDays,
      error: r.cert.error ?? null,
      tone: muted ? "muted" : toneOf(days, warnDays, r.cert.error ?? null),
      muted,
      target,
      pref,
      certExpiresAt: r.cert.notAfter ?? null,
    });
  }
  // Muted rows go last; the rest soonest first.
  return rows.sort((a, b) => Number(a.muted) - Number(b.muted) || (a.days ?? Infinity) - (b.days ?? Infinity) || a.name.localeCompare(b.name));
}

function hostOf(url: string): string {
  const m = /^[a-z]+:\/\/([^/:]+)/i.exec(url);
  return m ? m[1].toLowerCase() : url;
}

/** Counts for the panel header. */
export function expiryCounts(rows: ExpiryRow[]): { bad: number; warn: number } {
  return { bad: rows.filter((r) => r.tone === "bad").length, warn: rows.filter((r) => r.tone === "warn").length };
}
