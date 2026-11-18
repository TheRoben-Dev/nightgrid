/** Local notifications (1.1, D-167): the inbox's wording and where a notice leads. */
import type { Notice, NoticeLevel, NoticeSource } from "./api";

export const SOURCES: { id: NoticeSource; label: string; hint: string }[] = [
  { id: "alert", label: "Alerts", hint: "Alert rules from the ALERTS tab" },
  { id: "drill", label: "Restore drills", hint: "A scheduled drill passed or failed" },
  { id: "rotation", label: "Rotations due", hint: "A secret or certificate is due for rotation" },
  { id: "posture", label: "Posture drops", hint: "A host's security score fell" },
  { id: "capacity", label: "Capacity", hint: "A host is running out of room" },
];

export const sourceLabel = (s: NoticeSource): string => SOURCES.find((x) => x.id === s)?.label ?? "Nightgrid";

export function levelTone(l: NoticeLevel): "bad" | "warn" | "ok" | "info" {
  return l === "critical" ? "bad" : l === "warn" ? "warn" : l === "ok" ? "ok" : "info";
}

export const LEVELS: { id: NoticeLevel; label: string }[] = [
  { id: "warn", label: "Warnings and up" },
  { id: "critical", label: "Critical only" },
];

/** `native:drill` style route key. */
export const routeKey = (channel: "native" | "ntfy", s: NoticeSource) => `${channel}:${s}`;

export function toggleRoute(muted: readonly string[], key: string, on: boolean): string[] {
  const rest = muted.filter((m) => m !== key);
  return on ? rest : [...rest, key];
}

/** Splits `fleet/drills` into a view and an optional tab. */
export function target(n: Pick<Notice, "view">): { view: string; tab: string | null } | null {
  if (!n.view) return null;
  const [view, tab] = n.view.split("/");
  return { view, tab: tab ?? null };
}

/** "just now", "5 min", "3 h", "2 d". */
export function ago(at: number, now = Date.now() / 1000): string {
  const s = Math.max(0, now - at);
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)} min`;
  if (s < 86400) return `${Math.floor(s / 3600)} h`;
  return `${Math.floor(s / 86400)} d`;
}

/** Checks an ntfy topic URL the same way the backend does. */
export function ntfyProblem(url: string): string | null {
  const u = url.trim();
  const m = /^https?:\/\/(?:[^@/]*@)?(\[[^\]]+\]|[^/:?#]+)(?::\d+)?(\/[^?#]*)?/i.exec(u);
  if (!m) return "Start with https:// or http:// and your ntfy server's address.";
  const host = m[1].toLowerCase();
  if (["ntfy.sh", "ntfy.net"].some((p) => host === p || host.endsWith(`.${p}`))) {
    return "ntfy.sh is a public service run by someone else. Use your own ntfy server.";
  }
  const topic = (m[2] ?? "").replace(/^\/+|\/+$/g, "");
  if (!topic) return "Add the topic to the URL, like https://ntfy.home.lan/nightgrid.";
  if (!/^[A-Za-z0-9_\-/]{1,64}$/.test(topic)) return "A topic is letters, digits, - and _.";
  return null;
}
