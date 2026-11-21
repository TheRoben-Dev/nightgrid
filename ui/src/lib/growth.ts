/** Helpers for Fleet → DISKS → What's growing (D-200). */
import type { GrowthEntry, GrowthReport, Host } from "./api";
import { size } from "./fleet";

const sep = (root: string) => (root.includes("\\") && !root.includes("/") ? "\\" : "/");

/** `/mnt/h` + `vms/win` → `/mnt/h/vms/win`; `H:\` + `vms/win` → `H:\vms\win`. */
export function childPath(root: string, rel: string): string {
  const s = sep(root);
  const base = root.endsWith(s) ? root : root + s;
  return base + (s === "\\" ? rel.replaceAll("/", "\\") : rel);
}

/** One folder up, never above `top`. */
export function parentPath(path: string, top: string): string {
  if (path === top) return top;
  const s = sep(path);
  const i = path.replace(/[\\/]+$/, "").lastIndexOf(s);
  const up = i <= 0 ? s : path.slice(0, i);
  return up.length < top.length ? top : up;
}

/** "+12 GB", "−300 MB", "new". */
export function deltaText(d: number | null | undefined): string {
  if (d == null) return "new";
  if (d === 0) return "no change";
  return `${d > 0 ? "+" : "−"}${size(Math.abs(d))}`;
}

/**
 * What grew since the last scan, most first: files and the deepest folders
 * that explain it (a parent whose growth is all one child is left out).
 */
export function growingEntries(r: GrowthReport, max = 8): GrowthEntry[] {
  if (!r.prevAt) return [];
  const up = r.entries.filter((e) => (e.delta ?? 0) > 0);
  const out = up.filter((e) => {
    // Drop a folder when one listed child accounts for most of its growth.
    const kids = up.filter((k) => k.path.startsWith(e.path + "/"));
    return !(e.dir && kids.some((k) => (k.delta ?? 0) >= (e.delta ?? 0) * 0.8));
  });
  return out.sort((a, b) => (b.delta ?? 0) - (a.delta ?? 0)).slice(0, max);
}

/** Saved hosts whose name appears in a VM image's file name. */
export function linkSuggestions(images: GrowthEntry[], hosts: Host[]): { host: Host; file: string }[] {
  const out: { host: Host; file: string }[] = [];
  for (const e of images) {
    const file = e.path.split("/").pop()?.toLowerCase() ?? "";
    for (const h of hosts) {
      const n = h.name.toLowerCase();
      if (n.length >= 3 && file.includes(n) && !out.some((o) => o.host.id === h.id)) out.push({ host: h, file: e.path });
    }
  }
  return out;
}
