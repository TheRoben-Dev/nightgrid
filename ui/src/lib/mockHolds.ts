/**
 * Browser mock for update holds (D-195): Nightgrid's own holds plus the
 * packages each mock host holds natively (apt-mark hold, versionlock, …).
 */
import type { HoldInput, PackageManager, UpdateHold, UpdatesReport } from "./api";

let holds: UpdateHold[] = [];
let next = 1;
const native = new Map<number, string[]>();

const live = () => {
  const now = Date.now() / 1000;
  holds = holds.filter((h) => h.until == null || h.until > now);
  return holds;
};

/** Marks held rows the way the backend does. */
export function applyMockHolds(hostId: number, r: UpdatesReport): UpdatesReport {
  const mine = live()
    .filter((h) => h.hostId === hostId && h.kind === "package")
    .map((h) => h.target);
  const host = r.manager === "windowsUpdate" || r.manager === "apt" ? (native.get(hostId) ?? []) : [];
  const held = [...new Set([...host, ...mine])];
  return { ...r, held, updates: r.updates.map((u) => ({ ...u, held: held.includes(u.name) })) };
}

export function mockImageHeld(hostId: number, stack: string, image: string): boolean {
  return live().some((h) => h.hostId === hostId && ((h.kind === "stack" && h.target === stack) || (h.kind === "image" && h.stack === stack && h.target === image)));
}

export const mockHolds = {
  list: async (): Promise<UpdateHold[]> => live().map((h) => ({ ...h })),
  save: async (i: HoldInput): Promise<UpdateHold> => {
    const target = i.target ?? "";
    const stack = i.stack ?? "";
    if (i.until != null && i.until <= Date.now() / 1000) throw new Error("invalid value for until: an end in the future");
    const old = live().find((h) => h.hostId === i.hostId && h.kind === i.kind && h.target === target && h.stack === stack);
    if (old) {
      Object.assign(old, { until: i.until ?? null, note: i.note ?? "", version: i.version ?? old.version });
      return { ...old };
    }
    const h: UpdateHold = { id: next++, hostId: i.hostId, kind: i.kind, target, stack, version: i.version ?? null, until: i.until ?? null, note: i.note ?? "", createdAt: Math.floor(Date.now() / 1000) };
    holds.push(h);
    return { ...h };
  },
  remove: async (id: number): Promise<null> => {
    holds = holds.filter((h) => h.id !== id);
    return null;
  },
  packageHold: async (hostId: number, _m: PackageManager, names: string[], hold: boolean): Promise<null> => {
    await new Promise((r) => setTimeout(r, 300));
    const cur = native.get(hostId) ?? [];
    native.set(hostId, hold ? [...new Set([...cur, ...names])] : cur.filter((n) => !names.includes(n)));
    return null;
  },
};
