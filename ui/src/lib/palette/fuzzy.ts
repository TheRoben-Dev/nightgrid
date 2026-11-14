/**
 * Small subsequence fuzzy matcher for the command palette. Scores favour
 * consecutive runs, word starts and early matches. Returns null on no match.
 */
export interface FuzzyMatch {
  score: number;
  /** Indices into the haystack that matched, for highlighting. */
  positions: number[];
}

const isBoundary = (s: string, i: number) => i === 0 || /[\s\-_./:>]/.test(s[i - 1]);

export function fuzzyMatch(query: string, text: string): FuzzyMatch | null {
  const q = query.trim().toLowerCase();
  if (!q) return { score: 0, positions: [] };
  const t = text.toLowerCase();
  const positions: number[] = [];
  let score = 0;
  let ti = 0;
  let prev = -2;
  for (const ch of q) {
    if (ch === " ") continue;
    const found = t.indexOf(ch, ti);
    if (found === -1) return null;
    let s = 1;
    if (found === prev + 1) s += 4;
    if (isBoundary(t, found)) s += 3;
    s -= Math.min(found - ti, 5) * 0.2;
    score += s;
    positions.push(found);
    prev = found;
    ti = found + 1;
  }
  if (t.startsWith(q)) score += 5;
  // Shorter targets rank higher for equal matches.
  score -= t.length * 0.01;
  return { score, positions };
}

export function fuzzyFilter<T>(items: readonly T[], query: string, text: (item: T) => string): Array<T & { match: FuzzyMatch }> {
  const out: Array<T & { match: FuzzyMatch }> = [];
  for (const item of items) {
    const m = fuzzyMatch(query, text(item));
    if (m) out.push({ ...item, match: m });
  }
  if (query.trim()) out.sort((a, b) => b.match.score - a.match.score);
  return out;
}
