// Nightgrid Node pairing (M12f): words and tones for Fleet → NODE.
import type { NodeSetup, NodeState, NodeView, Release, SyncReport } from "./api";

export const STATE_TEXT: Record<NodeState, string> = {
  unknown: "not checked yet",
  off: "not used",
  notInstalled: "not installed",
  notRunning: "installed, not running",
  noAccess: "no access",
  incompatible: "needs an update",
  ready: "connected",
  error: "error",
};

export type Tone = "ok" | "warn" | "bad" | "muted";

export function stateTone(s: NodeState): Tone {
  switch (s) {
    case "ready":
      return "ok";
    case "notRunning":
    case "noAccess":
    case "incompatible":
      return "warn";
    case "error":
      return "bad";
    default:
      return "muted";
  }
}

/** `v0.23.1` is newer than `0.23.0`. Missing parts count as 0. */
export function newer(a: string, b: string): boolean {
  const parts = (v: string) =>
    v
      .replace(/^v/, "")
      .split(/[-+]/)[0]
      .split(".")
      .map((p) => Number.parseInt(p, 10) || 0);
  const x = parts(a);
  const y = parts(b);
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    const d = (x[i] ?? 0) - (y[i] ?? 0);
    if (d) return d > 0;
  }
  return false;
}

/** Node is built for these `uname -m` names only. */
const BUILT_FOR = ["x86_64", "amd64", "aarch64", "arm64"];

/** The host's CPU has no Node build ("armv7l" on many NAS boxes), or null when it has one or isn't known yet. */
export function unsupportedArch(v: Pick<NodeView, "arch">): string | null {
  const a = v.arch.trim();
  return a && !BUILT_FOR.includes(a) ? a : null;
}

/** The install button's label for this host, or null when there's nothing to do. */
export function installAction(v: NodeView, release: Release | null): "Install" | "Update" | "Fix access" | null {
  if (v.state === "notInstalled") return unsupportedArch(v) ? null : "Install";
  if (v.state === "noAccess") return "Fix access";
  if (v.state === "incompatible" || v.state === "notRunning") return "Update";
  if (v.state === "ready" && release && v.version && newer(release.tag, v.version)) return "Update";
  return null;
}

/** "1,440 points · 12 events · 24 hours". */
export function syncText(r: SyncReport | null | undefined): string {
  if (!r) return "not synced yet";
  const parts: string[] = [];
  const n = (x: number, one: string, many = `${one}s`) => `${x.toLocaleString("en-US")} ${x === 1 ? one : many}`;
  if (r.points) parts.push(n(r.points, "point"));
  if (r.events) parts.push(n(r.events, "event"));
  if (r.hours) parts.push(n(r.hours, "uptime hour"));
  if (r.diskRows) parts.push(n(r.diskRows, "disk reading"));
  const s = parts.length ? parts.join(" · ") : "nothing new";
  return r.reset ? `${s} (the node's history started over)` : s;
}

export const RES_TEXT: Record<number, string> = { 60: "1 min", 900: "15 min", 3600: "1 hour" };

/** "14 d" / "5 h" for a span in seconds. */
export function spanText(secs: number): string {
  if (secs >= 2 * 86400) return `${Math.round(secs / 86400)} d`;
  if (secs >= 3600) return `${Math.round(secs / 3600)} h`;
  return `${Math.max(1, Math.round(secs / 60))} min`;
}

export function bytesText(b: number | null | undefined): string {
  if (!b) return "–";
  if (b >= 1 << 30) return `${(b / (1 << 30)).toFixed(1)} GB`;
  if (b >= 1 << 20) return `${(b / (1 << 20)).toFixed(1)} MB`;
  return `${Math.round(b / 1024)} KB`;
}

/** What each optional grant on the node means. */
export const ACCESS_TEXT: Record<string, string> = {
  docker: "containers",
  journal: "logins and logs",
  netwatch: "DHCP and device names",
  helper: "unit and power actions",
  health: "SMART, pools, updates, UPS",
  power: "CPU energy counters",
  checks: "service checks from the LAN",
  jobs: "backup and job runs",
};

/** Capabilities shown as words, the ones the app uses first. */
export const USED_CAPS = ["metrics", "metrics.sync", "events", "uptime", "disk", "alerts"];

export function capsText(caps: readonly string[]): string {
  const used = USED_CAPS.filter((c) => caps.includes(c));
  const rest = caps.length - used.length;
  return used.join(", ") + (rest > 0 ? ` +${rest} more` : "");
}

// ---- hub and roles (D-175) ----

export type NodeRole = "hub" | "member" | "otherHub" | "direct" | "none";

export const ROLE_TEXT: Record<NodeRole, string> = {
  hub: "hub",
  member: "reports to hub",
  otherHub: "reports to another hub",
  direct: "direct",
  none: "–",
};

/** A host's part in the network: the hub, a member of it, or synced directly. */
export function roleOf(v: Pick<NodeView, "hostId" | "state">, setup: NodeSetup | undefined, hub: { hostId: number; hubId: string } | null): NodeRole {
  if (hub && v.hostId === hub.hostId) return "hub";
  const named = setup?.link?.hubId;
  if (named) return hub && named === hub.hubId ? "member" : "otherHub";
  return v.state === "ready" ? "direct" : "none";
}

/** What a member's state on the hub means. */
export const MEMBER_TEXT: Record<string, string> = {
  new: "not pulled yet",
  ok: "pulled",
  stale: "behind",
  unreachable: "unreachable",
  consent: "refused (no consent)",
  version: "needs Node 0.26+",
};

export function memberTone(state: string): Tone {
  if (state === "ok") return "ok";
  if (state === "new" || state === "stale") return "warn";
  return "bad";
}
