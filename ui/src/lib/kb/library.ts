/**
 * The knowledge base library (M11): articles bundled with the app, on six
 * shelves. "Using Nightgrid" is the user guide split at its `##` headings,
 * so it never drifts from the docs; the other shelves are the markdown files
 * under docs/kb (howto, basics, linux, apps, guides, errors). Everything is local: no links out, nothing
 * fetched (figures are bundled SVGs, kb/figures.ts), rendered as the safe markdown subset (helper/md.ts).
 *
 * This module holds the article text, so only the Library view and DRIFT's
 * retrieval load it (lazily); the palette uses the small catalog instead.
 *
 * A verified content pack (D-174, stores/kbcontent) can replace the docs/kb
 * shelves with newer text through `applyPack`; the arrays and maps here are
 * updated in place so every importer sees the same Library.
 */
import guideMd from "../../../../docs/USER_GUIDE.md?raw";
import { chunkMarkdown, Index, type Chunk } from "../helper/knowledge";
import { parseMd } from "../helper/md";
import type { Shelf } from "./catalog";
import type { QuestStep } from "./quest";

const FILES = import.meta.glob("../../../../docs/kb/{basics,guides,howto,apps,linux,errors}/*.md", { query: "?raw", import: "default", eager: true }) as Record<string, string>;

export interface Article {
  /** `shelf/slug`, stable: used for links, checklist progress and the catalog. */
  id: string;
  shelf: Shelf;
  title: string;
  /** The first paragraph, as plain text. */
  summary: string;
  /** Markdown without the title line (and without `{check: …}` marks). */
  body: string;
  /** Each `- [ ]` step's read-only check (`{check: id}` at its end), in order; null for none. */
  checks?: (string | null)[];
}

const STEP_LINE = /^\s*(?:[-*]|\d+[.)])\s+\[[ xX]\]\s/;
const CHECK_MARK = /\s*\{check:\s*([A-Za-z0-9:._@-]+)\s*\}\s*$/;

/** Takes the `{check: id}` marks off step lines, keeping one entry per step in order. */
export function takeChecks(body: string): { body: string; checks: (string | null)[] } {
  const checks: (string | null)[] = [];
  let fence = false;
  const lines = body.split("\n").map((l) => {
    if (/^\s*```/.test(l)) fence = !fence;
    if (fence || !STEP_LINE.test(l)) return l;
    const m = CHECK_MARK.exec(l);
    checks.push(m ? m[1] : null);
    return m ? l.slice(0, m.index) : l;
  });
  return { body: lines.join("\n"), checks };
}

/** Block-item key → check id, for the steps that carry one. */
export function stepChecks(a: Article): Record<string, string> {
  const out: Record<string, string> = {};
  if (!a.checks?.some(Boolean)) return out;
  const keys = parts(a).flatMap((p) => p.keys);
  keys.forEach((k, i) => {
    const c = a.checks?.[i];
    if (c) out[k] = c;
  });
  return out;
}

export function slug(s: string): string {
  return s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

/** Markdown → plain text for summaries and search snippets. */
export function plain(md: string): string {
  return md
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/!\[([^\]\n]*)\]\(fig:[\w-]+\)/g, "$1")
    .replace(/\[([^\]]+)\]\((?:view|kb):[\w/-]+\)/g, "$1")
    .replace(/[`*]/g, "")
    .replace(/^\s*[-*]\s+(\[[ xX]\]\s+)?/gm, "")
    .replace(/^#+\s+/gm, "")
    .replace(/\s+/g, " ")
    .trim();
}

function firstParagraph(body: string): string {
  const p = body
    .split(/\n{2,}/)
    .map((b) => b.trim())
    .find((b) => b && !b.startsWith("#") && !b.startsWith("```") && !b.startsWith("![") && !/^\s*[-*\d]/.test(b));
  const text = plain(p ?? "");
  return text.length > 220 ? `${text.slice(0, 217).replace(/\s+\S*$/, "")}…` : text;
}

/** One article per `##` section of the user guide (its `###` stay inside). */
export function fromGuide(md: string): Article[] {
  const out: Article[] = [];
  let title: string | null = null;
  let buf: string[] = [];
  const flush = () => {
    if (title) {
      const body = buf.join("\n").trim();
      out.push({ id: `nightgrid/${slug(title)}`, shelf: "nightgrid", title, summary: firstParagraph(body), body });
    }
    buf = [];
  };
  for (const line of md.split("\n")) {
    const h = /^##\s+(.*)$/.exec(line);
    if (h) {
      flush();
      title = h[1].trim();
      continue;
    }
    if (/^#\s/.test(line)) continue;
    buf.push(line);
  }
  flush();
  return out;
}

/** An article file: `# Title`, then the body. */
export function fromFile(path: string, raw: string): Article | null {
  const m = /\/kb\/(basics|guides|howto|apps|linux|errors)\/([\w-]+)\.md$/.exec(path);
  if (!m || m[2].toLowerCase() === "readme") return null;
  const lines = raw.replace(/\r\n/g, "\n").split("\n");
  const at = lines.findIndex((l) => /^#\s+/.test(l));
  const title = at >= 0 ? lines[at].replace(/^#\s+/, "").trim() : m[2];
  const body = lines
    .slice(at + 1)
    .join("\n")
    .trim();
  const t = takeChecks(body);
  const art: Article = { id: `${m[1]}/${m[2]}`, shelf: m[1] as Shelf, title, summary: firstParagraph(t.body), body: t.body };
  if (t.checks.some(Boolean)) art.checks = t.checks;
  return art;
}

let index: Index | null = null;

const SHELF_ORDER: Shelf[] = ["nightgrid", "howto", "basics", "linux", "apps", "guides", "errors"];

function build(files: Record<string, string>): Article[] {
  return [
    ...fromGuide(guideMd),
    ...Object.entries(files)
      .map(([p, raw]) => fromFile(p, raw))
      .filter((a): a is Article => a !== null)
      .sort((a, b) => SHELF_ORDER.indexOf(a.shelf) - SHELF_ORDER.indexOf(b.shelf) || a.title.localeCompare(b.title)),
  ];
}

export const ARTICLES: Article[] = build(FILES);

const BY_ID = new Map(ARTICLES.map((a) => [a.id, a]));
export const article = (id: string): Article | undefined => BY_ID.get(id);

let generation = 0;
/** Bumped whenever the articles change, so caches built from them (search, the error reference) rebuild. */
export const contentGeneration = (): number => generation;

/**
 * Swaps the docs/kb shelves for a content pack's markdown (keys like
 * `apps/x.md`), or back to the bundled files with null. The "Using
 * Nightgrid" shelf always comes from the app's own user guide.
 */
export function applyPack(files: Record<string, string> | null): void {
  const md = files ? Object.fromEntries(Object.entries(files).filter(([k]) => k.endsWith(".md")).map(([k, v]) => [`/docs/kb/${k}`, v])) : FILES;
  ARTICLES.splice(0, ARTICLES.length, ...build(md));
  BY_ID.clear();
  for (const a of ARTICLES) BY_ID.set(a.id, a);
  index = null;
  generation++;
}

/** A guide's progress: its `- [ ]` steps (Markdown's `block-item` keys), by `##` part. */
export interface Part {
  title: string;
  keys: string[];
}
export function parts(a: Article): Part[] {
  const out: Part[] = [];
  let cur: Part | null = null;
  parseMd(a.body, { headings: true, steps: true, figures: true }).forEach((b, bi) => {
    if (b.kind === "h" && b.level === 2) {
      cur = { title: b.text.map((t) => t.text).join(""), keys: [] };
      out.push(cur);
    } else if ((b.kind === "ul" || b.kind === "ol") && b.checks) {
      if (!cur) {
        cur = { title: a.title, keys: [] };
        out.push(cur);
      }
      const part = cur;
      b.checks.forEach((c, ii) => {
        if (c !== null) part.keys.push(`${bi}-${ii}`);
      });
    }
  });
  return out.filter((p) => p.keys.length > 0);
}

/** A guide's steps in order, with their text and part, for the quest log. */
export function stepList(a: Article): QuestStep[] {
  const out: QuestStep[] = [];
  let part = -1;
  let partTitle = a.title;
  let n = 0;
  parseMd(a.body, { headings: true, steps: true, figures: true }).forEach((b, bi) => {
    if (b.kind === "h" && b.level === 2) {
      partTitle = b.text.map((t) => t.text).join("");
      part = -1;
    } else if ((b.kind === "ul" || b.kind === "ol") && b.checks) {
      b.checks.forEach((c, ii) => {
        if (c === null) return;
        if (part < 0) part = new Set(out.map((s) => s.part)).size;
        const text = b.items[ii]
          .map((t) => t.text)
          .join("")
          .replace(/\s+/g, " ")
          .trim()
          .replace(/:$/, "");
        out.push({ key: `${bi}-${ii}`, text, part, partTitle, check: a.checks?.[n] ?? null });
        n++;
      });
    }
  });
  return out;
}

/** Guides that can be started as quests: the ones with `- [ ]` steps, outside the Nightgrid and basics shelves. */
export function questable(a: Article): boolean {
  return a.shelf !== "nightgrid" && a.shelf !== "basics" && parts(a).length > 0;
}

/** Article chunks for search and DRIFT, titled "Library › Article › Section". */
export function articleChunks(a: Article): (Chunk & { articleId: string })[] {
  return chunkMarkdown(`## ${a.title}\n${a.body}`).map((c) => ({
    title: c.title === a.title ? `${a.title}` : `${a.title} › ${c.title}`,
    text: c.text,
    articleId: a.id,
  }));
}

/** Full-text index over every article (built on first use, again after a content update). */
export function libraryIndex(): Index {
  return (index ??= new Index(ARTICLES.flatMap(articleChunks)));
}

/** Articles matching `q`, best first, each with the section that matched. */
export function search(q: string, k = 20): { article: Article; section: string; snippet: string }[] {
  const seen = new Set<string>();
  const out: { article: Article; section: string; snippet: string }[] = [];
  for (const c of libraryIndex().search(q, k * 3) as (Chunk & { articleId?: string })[]) {
    const a = c.articleId ? BY_ID.get(c.articleId) : undefined;
    if (!a || seen.has(a.id)) continue;
    seen.add(a.id);
    const text = plain(c.text);
    out.push({ article: a, section: c.title, snippet: text.length > 180 ? `${text.slice(0, 177)}…` : text });
    if (out.length >= k) break;
  }
  return out;
}
