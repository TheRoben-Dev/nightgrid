/**
 * The Library's error reference (the "errors" shelf) as exact checks for
 * DRIFT: each `### message` heading is matched against the question and the
 * attached text, with `<placeholders>` standing for any short run of text.
 * BM25 retrieval finds the same entries by words; this finds them by the
 * message itself, so a pasted log line lands on its entry even when the
 * question says nothing else. Loaded lazily with the library.
 */
import { ARTICLES, contentGeneration } from "./library";
import type { Check } from "../helper/skills";

export interface RefEntry {
  /** `errors/<area>`. */
  articleId: string;
  area: string;
  /** The message as written in the heading. */
  message: string;
  means: string;
  fix: string;
}

interface Compiled extends RefEntry {
  re: RegExp;
  /** The longest literal piece, checked with includes() before the regex runs. */
  key: string;
  /** Literal characters: longer is more specific. */
  weight: number;
}

/** Folds case, quote styles and runs of spaces, so copies from terminals and logs still match. */
export function norm(s: string): string {
  return s
    .toLowerCase()
    .replace(/[`'"‘’“”«»]/g, "'")
    .replace(/[ \t]+/g, " ");
}

const MIN_LITERAL = 12;

export function compile(message: string): { re: RegExp; key: string; weight: number } | null {
  const m = norm(message).trim().replace(/[.:]+$/, "");
  const pieces = m
    .split(/<[^<>\n]{1,40}>/)
    .filter((p, i, a) => p.length > 0 || (i > 0 && i < a.length - 1));
  const literal = pieces.join("");
  if (literal.replace(/\s/g, "").length < MIN_LITERAL) return null;
  const key = pieces.reduce((a, b) => (b.trim().length > a.trim().length ? b : a), "").trim();
  const src = pieces.map((p) => p.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("[^\\n]{1,160}?");
  return { re: new RegExp(src), key, weight: literal.length };
}

/** Entries parsed from the errors shelf: `### message`, then `Means:` and `Fix:` lines. */
export function parseEntries(articleId: string, area: string, body: string): RefEntry[] {
  const out: RefEntry[] = [];
  let cur: RefEntry | null = null;
  for (const line of body.split("\n")) {
    const h = /^###\s+(.+)$/.exec(line);
    if (h) {
      if (cur) out.push(cur);
      cur = { articleId, area, message: h[1].trim(), means: "", fix: "" };
      continue;
    }
    if (!cur) continue;
    if (/^##?\s/.test(line)) {
      out.push(cur);
      cur = null;
    } else if (line.startsWith("Means:")) cur.means = line.slice(6).trim();
    else if (line.startsWith("Fix:")) cur.fix = line.slice(4).trim();
  }
  if (cur) out.push(cur);
  return out.filter((e) => e.means && e.fix);
}

let all: Compiled[] | null = null;
let builtFor = -1;
function compiled(): Compiled[] {
  if (all && builtFor === contentGeneration()) return all;
  builtFor = contentGeneration();
  all = [];
  for (const a of ARTICLES) {
    if (a.shelf !== "errors") continue;
    const area = a.title.replace(/^Errors:\s*/, "");
    for (const e of parseEntries(a.id, area, a.body)) {
      const c = compile(e.message);
      if (c) all.push({ ...e, ...c });
    }
  }
  return all;
}

/** Every entry of the reference (for tests and counts). */
export function entries(): RefEntry[] {
  return ARTICLES.filter((a) => a.shelf === "errors").flatMap((a) => parseEntries(a.id, a.title.replace(/^Errors:\s*/, ""), a.body));
}

/** Entries whose message appears in `text`, most specific first. */
export function matchErrors(text: string, max = 3): RefEntry[] {
  const t = norm(text);
  if (t.trim().length < MIN_LITERAL) return [];
  const hits = compiled().filter((c) => t.includes(c.key) && c.re.test(t));
  hits.sort((a, b) => b.weight - a.weight);
  // A shorter message inside a longer one that matched adds nothing ("Permission denied" vs "Permission denied (publickey)").
  const out: Compiled[] = [];
  for (const h of hits) {
    if (out.some((o) => norm(o.message).includes(h.key))) continue;
    out.push(h);
    if (out.length >= max) break;
  }
  return out.map(({ articleId, area, message, means, fix }) => ({ articleId, area, message, means, fix }));
}

const MAX_ITEM = 480;

/** Adds reference hits to the "Error reference" check, keeping it short for the prompt. */
export function addErrorRefs(checks: Check[], hits: RefEntry[]): void {
  if (!hits.length) return;
  const items = hits.map((h) => {
    const t = `${h.message} (${h.area}): ${h.means} Fix: ${h.fix}`;
    return { text: t.length > MAX_ITEM ? `${t.slice(0, MAX_ITEM - 1)}…` : t };
  });
  const have = checks.find((c) => c.kind === "errors");
  if (have) have.items = [...have.items.slice(0, 2), ...items];
  else checks.push({ kind: "errors", title: "Error reference", items });
}
