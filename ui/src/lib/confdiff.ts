/** Config drift (M10l): filters, counts and DRIFT text for System → CONFIG. */
import type { DriftReport, FileChange, PackageChange } from "./api";

export const CHANGE_WORD = { changed: "CHANGED", added: "ADDED", removed: "REMOVED" } as const;

export function filterFiles(files: FileChange[], q: string): FileChange[] {
  const s = q.trim().toLowerCase();
  return s ? files.filter((f) => f.path.toLowerCase().includes(s)) : files;
}

export function filterPackages(pkgs: PackageChange[], q: string): PackageChange[] {
  const s = q.trim().toLowerCase();
  return s ? pkgs.filter((p) => p.name.toLowerCase().includes(s)) : pkgs;
}

/** How a package changed, in words. */
export function pkgChange(p: PackageChange): "upgraded" | "downgraded" | "changed" | "added" | "removed" {
  if (p.left == null) return "added";
  if (p.right == null) return "removed";
  const c = compareVersions(p.left, p.right);
  return c < 0 ? "upgraded" : c > 0 ? "downgraded" : "changed";
}

/** Rough version order: numeric runs compared as numbers, the rest as text. */
export function compareVersions(a: string, b: string): number {
  const split = (v: string) => v.split(/(\d+)/).filter((x) => x !== "");
  const x = split(a);
  const y = split(b);
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    const p = x[i];
    const q = y[i];
    if (p === undefined) return -1;
    if (q === undefined) return 1;
    const np = /^\d+$/.test(p);
    const nq = /^\d+$/.test(q);
    if (np && nq) {
      const d = Number(p) - Number(q);
      if (d) return d < 0 ? -1 : 1;
    } else if (p !== q) {
      return p < q ? -1 : 1;
    }
  }
  return 0;
}

/** Top-level folder groups for the file list: `/etc/ssh`, `/srv/stacks/media`. */
export function groupFiles(files: FileChange[]): { dir: string; files: FileChange[] }[] {
  const out: { dir: string; files: FileChange[] }[] = [];
  for (const f of files) {
    const dir = f.path.slice(0, f.path.lastIndexOf("/")) || "/";
    const last = out[out.length - 1];
    if (last && last.dir === dir) last.files.push(f);
    else out.push({ dir, files: [f] });
  }
  return out;
}

export function counts(r: DriftReport): { changed: number; added: number; removed: number; packages: number } {
  return {
    changed: r.files.filter((f) => f.change === "changed").length,
    added: r.files.filter((f) => f.change === "added").length,
    removed: r.files.filter((f) => f.change === "removed").length,
    packages: r.packages.length,
  };
}

/** Plain text for DRIFT: what changed, capped. */
export function reportText(r: DriftReport, max = 120): string {
  const lines = [`Comparing ${r.left.label} (left) with ${r.right.label} (right).`];
  const files = r.files.slice(0, max);
  if (files.length) lines.push("Files:");
  for (const f of files) lines.push(`- ${f.change}: ${f.path}${f.secret ? " (secret file, contents not compared)" : ""}`);
  if (r.files.length > max) lines.push(`- …and ${r.files.length - max} more files`);
  const pkgs = r.packages.slice(0, max);
  if (pkgs.length) lines.push("Packages:");
  for (const p of pkgs) lines.push(`- ${p.name}: ${p.left ?? "not installed"} → ${p.right ?? "not installed"}`);
  if (r.packages.length > max) lines.push(`- …and ${r.packages.length - max} more packages`);
  if (r.hidden) lines.push(`(${r.hidden} host-specific files left out)`);
  return lines.join("\n");
}
