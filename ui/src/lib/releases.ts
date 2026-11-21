/**
 * Release notes in the app (D-204). CHANGELOG.md is bundled with the
 * interface, so "What's new" and the release log work offline. Headings look
 * like `## [1.23.0] "Cobalt Signal" - 2026-11-21` (scripts/codenames.mjs
 * writes them; older ones were backfilled).
 */

export interface Release {
  version: string;
  /** Two-word codename; null only for a heading written without one. */
  codename: string | null;
  /** YYYY-MM-DD, when given. */
  date: string | null;
  /** Anything after the date, like "(prerelease)". */
  tag: string | null;
  /** The section's markdown, without its heading. */
  body: string;
}

const HEADING = /^## \[(\d+\.\d+\.\d+)\](?: "([^"]+)")?(?: - (\d{4}-\d{2}-\d{2}))?(.*)$/;

/** Every released version in a changelog, newest first. Unreleased notes are left out. */
export function parseChangelog(md: string): Release[] {
  const out: Release[] = [];
  let cur: (Release & { lines: string[] }) | null = null;
  const close = () => {
    if (cur) out.push({ version: cur.version, codename: cur.codename, date: cur.date, tag: cur.tag, body: cur.lines.join("\n").trim() });
    cur = null;
  };
  for (const line of md.split("\n")) {
    if (line.startsWith("## ")) {
      close();
      const m = HEADING.exec(line);
      if (m) {
        const tag = m[4].trim().replace(/^\((.*)\)$/, "$1");
        cur = { version: m[1], codename: m[2] ?? null, date: m[3] ?? null, tag: tag || null, body: "", lines: [] };
      }
      continue;
    }
    cur?.lines.push(line);
  }
  close();
  return out;
}

const parts = (v: string) =>
  v
    .replace(/^v/, "")
    .split(/[-+]/)[0]
    .split(".")
    .map((n) => Number(n) || 0);

/** Semver-ish compare of `x.y.z` (pre-release suffixes ignored). */
export function cmpVersion(a: string, b: string): number {
  const [x, y] = [parts(a), parts(b)];
  for (let i = 0; i < 3; i++) if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) - (y[i] ?? 0);
  return 0;
}

/** Releases after `from` up to and including `to`, newest first. */
export function between(list: Release[], from: string, to: string): Release[] {
  return list.filter((r) => cmpVersion(r.version, from) > 0 && cmpVersion(r.version, to) <= 0);
}

/**
 * What to show after a start. `last` is the newest version whose notes were
 * shown here (null before this feature). A first install shows nothing; an
 * update shows every release since `last` (capped at `max`); the first start
 * with this feature on an existing install (`existing`) shows the current one.
 */
export function whatsNew(list: Release[], current: string, last: string | null, existing: boolean, max = 8): Release[] {
  if (!last) return existing ? list.filter((r) => r.version === current) : [];
  if (cmpVersion(current, last) <= 0) return [];
  return between(list, last, current).slice(0, max);
}

/** Splits the "Codename: X" line the updater manifest puts first in its notes. */
export function splitUpdateNotes(notes: string | null | undefined): { codename: string | null; notes: string } {
  const m = /^Codename: ([^\n]+)\n*/.exec(notes ?? "");
  return m ? { codename: m[1].trim(), notes: (notes ?? "").slice(m[0].length) } : { codename: null, notes: notes ?? "" };
}

/** "v1.23.0 · Cobalt Signal". */
export const releaseLabel = (r: Pick<Release, "version" | "codename">) => (r.codename ? `v${r.version} · ${r.codename}` : `v${r.version}`);

/** The bundled changelog, loaded on first use (its own chunk, so start-up doesn't pay for it). */
export async function loadBundled(): Promise<Release[]> {
  const mod = await import("../../../CHANGELOG.md?raw");
  return parseChangelog(mod.default);
}
