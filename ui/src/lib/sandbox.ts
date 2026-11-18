/** Sandbox clones (M12c): read a sandbox's marks from its Proxmox tags. */
import type { PveGuest } from "./proxmox";
import type { PveBridge } from "./api";

export const TAG = "nightgrid-sandbox";
export const OF = "ng-sbx-of-";
export const UNTIL = "ng-sbx-until-";

export interface SandboxInfo {
  guest: PveGuest;
  /** The guest it was copied from. */
  source: number | null;
  /** Unix seconds; null never expires. */
  until: number | null;
}

/** Our marks, or null for an ordinary guest. */
export function sandboxOf(g: PveGuest): SandboxInfo | null {
  if (!g.tags.includes(TAG)) return null;
  const num = (p: string) => {
    const t = g.tags.find((x) => x.startsWith(p));
    const n = t ? Number(t.slice(p.length)) : NaN;
    return Number.isFinite(n) && n > 0 ? n : null;
  };
  return { guest: g, source: num(OF), until: num(UNTIL) };
}

export const sandboxes = (gs: PveGuest[]): SandboxInfo[] =>
  gs.map(sandboxOf).filter((s): s is SandboxInfo => !!s).sort((a, b) => (a.until ?? Infinity) - (b.until ?? Infinity) || a.guest.vmid - b.guest.vmid);

/** Tags a person sees: ours are shown as their own badge instead. */
export const ownTags = (tags: string[]): string[] => tags.filter((t) => t !== TAG && !t.startsWith(OF) && !t.startsWith(UNTIL));

/** "in 3 h 20 min", "in 2 days", "overdue by 5 min", "never". */
export function expiryText(until: number | null, now = Date.now() / 1000): string {
  if (until == null) return "never expires";
  const d = Math.round(until - now);
  const span = (s: number) => {
    const m = Math.max(1, Math.round(s / 60));
    if (m < 60) return `${m} min`;
    const h = Math.floor(m / 60);
    if (h < 48) return m % 60 && h < 10 ? `${h} h ${m % 60} min` : `${h} h`;
    return `${Math.round(h / 24)} days`;
  };
  return d >= 0 ? `expires in ${span(d)}` : `overdue by ${span(-d)}`;
}

export function expiryTone(until: number | null, now = Date.now() / 1000): "ok" | "warn" | "err" {
  if (until == null) return "ok";
  return until < now ? "err" : until - now < 3600 ? "warn" : "ok";
}

export const LIFETIMES: { hours: number; label: string }[] = [
  { hours: 2, label: "2 hours" },
  { hours: 8, label: "8 hours" },
  { hours: 24, label: "1 day" },
  { hours: 72, label: "3 days" },
  { hours: 168, label: "1 week" },
  { hours: 0, label: "Never (destroy it yourself)" },
];

export const EXTEND: { hours: number; label: string }[] = [
  { hours: 2, label: "2 h from now" },
  { hours: 24, label: "1 day from now" },
  { hours: 168, label: "1 week from now" },
  { hours: 0, label: "Never expire" },
];

/** `media-sandbox`, cut to Proxmox's 63 characters. */
export function sandboxName(g: PveGuest): string {
  const base = (g.name || `guest-${g.vmid}`).replace(/[^A-Za-z0-9.-]/g, "-").replace(/^[-.]+|[-.]+$/g, "") || `guest-${g.vmid}`;
  return `${base.slice(0, 63 - 8).replace(/[-.]+$/, "")}-sandbox`;
}

/** Bridges with no physical ports go first: those are cut off already. */
export function bridgeChoices(bs: PveBridge[]): { name: string; isolated: boolean; label: string }[] {
  return bs
    .map((b) => {
      const isolated = !b.ports.trim();
      return { name: b.name, isolated, label: `${b.name}${isolated ? " (no ports: isolated)" : ` (ports: ${b.ports})`}${b.comment ? ` ${b.comment}` : ""}` };
    })
    .sort((a, b) => Number(b.isolated) - Number(a.isolated) || a.name.localeCompare(b.name));
}
