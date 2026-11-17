/**
 * The user's own hosts as DRIFT context: which saved hosts a question is
 * about (loose name match), and what Nightgrid knows about each right now.
 * Pure, so it can be tested and shown verbatim in "What I was given".
 */
import type { Alert, HistoryEntry, Host, HostPing } from "../api";
import type { HostMetrics } from "../stores/metrics.svelte";

/** Words too common to identify a host on their own. */
const COMMON = new Set([
  "the",
  "and",
  "for",
  "with",
  "what",
  "host",
  "hosts",
  "server",
  "servers",
  "machine",
  "box",
  "home",
  "local",
  "main",
  "new",
  "old",
  "test",
  "my",
  "is",
  "it",
  "on",
  "to",
  "of",
]);

const norm = (s: string) => s.toLowerCase().replace(/[^a-z0-9]+/g, "");
const words = (s: string) =>
  s
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter((w) => w.length >= 2);

/** Levenshtein distance ≤ 1 (one typo), for words of 5+ letters. */
function nearly(a: string, b: string): boolean {
  if (a === b) return true;
  if (Math.min(a.length, b.length) < 5 || Math.abs(a.length - b.length) > 1) return false;
  let i = 0;
  let j = 0;
  let edits = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      i++;
      j++;
      continue;
    }
    if (++edits > 1) return false;
    if (a.length > b.length) i++;
    else if (b.length > a.length) j++;
    else {
      i++;
      j++;
    }
  }
  return edits + (a.length - i) + (b.length - j) <= 1;
}

/** How strongly `q` points at `h`: 0 none, 1 a shared word, 3 the whole name. */
export function hostScore(h: Host, q: string): number {
  const nq = norm(q);
  const names = [h.name, h.sshAlias ?? "", h.hostname.split(".")[0] ?? ""].filter((n) => norm(n).length >= 2);
  if (names.some((n) => norm(n).length >= 3 && nq.includes(norm(n)))) return 3;
  const qw = words(q);
  const nameWords = new Set(names.flatMap(words).filter((w) => w.length >= 3 && !COMMON.has(w)));
  let hits = 0;
  for (const w of nameWords) if (qw.some((x) => !COMMON.has(x) && nearly(x, w))) hits++;
  return hits >= 2 ? 2 : hits;
}

/** Saved hosts the question is about, best first (at most `max`). */
export function mentionedHosts(hosts: Host[], q: string, max = 2): Host[] {
  const scored = hosts.map((h) => ({ h, s: hostScore(h, q) })).filter((x) => x.s > 0);
  if (!scored.length) return [];
  const best = Math.max(...scored.map((x) => x.s));
  // A whole-name match outranks hosts that only share a word.
  return scored
    .filter((x) => x.s === best)
    .slice(0, max)
    .map((x) => x.h);
}

export function since(ts: number, now: number): string {
  const s = Math.max(0, now - ts);
  if (s < 90) return `${s}s ago`;
  if (s < 5400) return `${Math.round(s / 60)} min ago`;
  if (s < 172800) return `${Math.round(s / 3600)} h ago`;
  return `${Math.round(s / 86400)} days ago`;
}

const when = (ts: number) => new Date(ts * 1000).toISOString().slice(0, 16).replace("T", " ");

function uptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  return d ? `${d}d ${h}h` : `${h}h ${Math.floor((secs % 3600) / 60)}m`;
}

const STATE: Record<string, string> = {
  ok: "online, metrics arriving",
  connecting: "connecting",
  offline: "offline (can't reach it; retrying)",
  needsLogin: "needs a password, passphrase or host-key decision before Nightgrid can connect",
  error: "connected, but the metrics script failed",
  off: "metrics turned off",
  suspended: "suspended (planned, not an outage)",
  rebooting: "rebooting (planned)",
  shuttingDown: "shut down (planned, not an outage)",
  asleep: "inside its expected sleep window (not an outage)",
};

export interface HostFactsInput {
  host: Host;
  ping?: HostPing;
  metrics?: HostMetrics;
  /** Alerts for this host, active and past (any order). */
  alerts: Alert[];
  /** Recent shell history (oldest first), when the host shares it. */
  history?: HistoryEntry[] | null;
  /** Older history lines that mention the question's words (M10p). */
  historyMatches?: HistoryEntry[] | null;
  historyError?: string | null;
  /** Recent journal entries for the host, as text (newest first). */
  changes?: string | null;
  /** Unix seconds. */
  now: number;
}

const TERM_SKIP = new Set([...COMMON, "did", "does", "why", "how", "when", "who", "where", "which", "was", "were", "has", "have", "had", "can", "could", "should", "would", "this", "that", "there", "last", "time", "check", "history", "shell", "run", "ran", "command", "commands", "happened", "about", "from", "into", "again", "today", "yesterday", "after", "before", "its", "his", "her", "their", "you", "your", "me", "please", "show", "tell", "find", "look", "anything", "something", "wrong", "broken", "work", "working"]);

/**
 * The words of a question worth searching shell history for: not host
 * names, not common words, at least three letters (M10p).
 */
export function historyTerms(q: string, hostNames: string[]): string[] {
  const skip = new Set(hostNames.flatMap((n) => words(n)));
  const out: string[] = [];
  for (const w of q.toLowerCase().split(/[^a-z0-9_.-]+/)) {
    const t = w.replace(/^[.-]+|[.-]+$/g, "");
    if (t.length >= 3 && !TERM_SKIP.has(t) && !skip.has(t) && !out.includes(t)) out.push(t);
  }
  return out.slice(0, 8);
}

/** A plain-text block of facts about one host. */
export function hostFacts(i: HostFactsInput): string {
  const { host: h, ping, metrics: m, now } = i;
  const lines: string[] = [];
  lines.push(`### ${h.name} (saved host; address ${h.hostname}${h.port !== 22 ? `:${h.port}` : ""}${h.tailnetAddress ? `, tailnet fallback ${h.tailnetAddress}` : ""})`);
  if (h.tags.length) lines.push(`Tags: ${h.tags.join(", ")}`);
  if (h.role === "client") lines.push("Role: client (a laptop, desktop or handheld, not a server)");
  if (h.sleepWindow) lines.push(`Expected sleep window: ${h.sleepWindow} (local time)`);
  if (h.macAddress) lines.push("Wake-on-LAN: set up (System → WAKE, or the ⏻ button on its tile)");
  if (ping) {
    lines.push(
      `Reachability check: ${ping.state}${ping.latencyMs != null ? ` (${ping.latencyMs} ms)` : ""}${ping.connected ? ", SSH session open" : ""}`,
    );
  }
  if (m) {
    const why = ["suspended", "rebooting", "shuttingDown", "asleep"].includes(m.state) ? "Why" : "Last error";
    lines.push(`Monitoring: ${STATE[m.state] ?? m.state}${m.message ? `. ${why}: ${m.message}` : ""}`);
    if (m.sample) {
      const s = m.sample;
      const ts = Math.floor(s.ts / 1000);
      lines.push(`Last seen: ${since(ts, now)} (${when(ts)} UTC). Uptime then: ${uptime(s.uptimeSecs)}. Kernel ${s.kernel}. Load ${s.load.map((x) => x.toFixed(2)).join(" ")}.`);
    } else if (m.state !== "ok") lines.push("Last seen: no sample since Nightgrid started.");
  }
  const alerts = i.alerts.slice().sort((a, b) => b.since - a.since);
  const active = alerts.filter((a) => a.resolved == null);
  const past = alerts.filter((a) => a.resolved != null).slice(0, 6);
  if (active.length) lines.push("Active alerts:\n" + active.map((a) => `- ${a.title} (since ${since(a.since, now)}): ${a.detail}`).join("\n"));
  if (past.length)
    lines.push(
      "Recent alerts (resolved):\n" +
        past.map((a) => `- ${a.title}: ${when(a.since)} → ${when(a.resolved ?? a.since)} UTC (${since(a.resolved ?? a.since, now)})${a.detail ? `. ${a.detail}` : ""}`).join("\n"),
    );
  if (!active.length && !past.length) lines.push("Alerts: none recorded.");
  if (i.history?.length) {
    lines.push(
      `Recent shell history (oldest first, the user allowed this; secret-looking lines removed):\n` +
        i.history.map((e) => `- ${e.when ? `[${when(e.when)}] ` : ""}${e.command}`).join("\n"),
    );
  }
  if (i.historyMatches?.length) {
    lines.push(`Older shell history lines that mention the question's words (best match first):\n` + i.historyMatches.map((e) => `- ${e.when ? `[${when(e.when)}] ` : ""}${e.command}`).join("\n"));
  }
  if (i.changes) lines.push(`Changes made from Nightgrid in the last 14 days (journal, newest first):\n${i.changes}`);
  else if (i.changes === "") lines.push("Changes made from Nightgrid in the last 14 days: none recorded.");
  if (!i.history?.length && !i.historyMatches?.length) {
    if (i.historyError) lines.push(`Shell history: couldn't read it (${i.historyError}).`);
    else if (i.history) lines.push("Shell history: empty.");
    else if (!h.driftHistory) lines.push("Shell history: not shared (the user can allow it in the host's settings).");
  }
  return lines.join("\n");
}
