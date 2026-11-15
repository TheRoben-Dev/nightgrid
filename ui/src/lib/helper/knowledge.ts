/**
 * The "Using Nightgrid" reference DRIFT answers from: the bundled user guide,
 * split into small sections and ranked with BM25 for each question. No
 * embeddings model is needed, so retrieval is instant and works offline.
 */
import guideMd from "../../../../docs/USER_GUIDE.md?raw";

export interface Chunk {
  /** "Section › Subsection" path, for the prompt and the sources list. */
  title: string;
  text: string;
  /** Set for a chunk of one of the user's notes. */
  noteId?: number;
}

const MAX_CHUNK = 900;

/** Splits markdown into chunks by `##` heading, then by paragraph/list blocks. */
export function chunkMarkdown(md: string): Chunk[] {
  const out: Chunk[] = [];
  let title = "Nightgrid";
  let buf: string[] = [];
  const flush = () => {
    const blocks = buf.join("\n").split(/\n{2,}/).map((b) => b.trim()).filter(Boolean);
    let cur = "";
    for (const b of blocks) {
      if (cur && cur.length + b.length + 2 > MAX_CHUNK) {
        out.push({ title, text: cur });
        cur = "";
      }
      cur = cur ? `${cur}\n\n${b}` : b;
    }
    if (cur) out.push({ title, text: cur });
    buf = [];
  };
  for (const line of md.split("\n")) {
    const h = /^(#{1,3})\s+(.*)$/.exec(line);
    if (h) {
      flush();
      title = h[1].length === 1 ? "Nightgrid" : h[2].trim();
      continue;
    }
    buf.push(line);
  }
  flush();
  return out;
}

const STOP = new Set(
  "a an and are as at be by can do does for from how i if in into is it its me my of on or so that the this to was what when where which who why will with you your".split(
    " ",
  ),
);

/** Lower-case word stems: crude suffix stripping is enough for short docs. */
export function terms(text: string): string[] {
  return (text.toLowerCase().match(/[a-z0-9+#.]+/g) ?? [])
    .map((w) => w.replace(/^[.#]+|[.]+$/g, ""))
    .filter((w) => w.length > 1 && !STOP.has(w))
    .map((w) => w.replace(/(ing|ed|es|s)$/, "") || w);
}

export class Index {
  private docs: { chunk: Chunk; tf: Map<string, number>; len: number }[];
  private df = new Map<string, number>();
  private avg: number;

  constructor(chunks: Chunk[]) {
    this.docs = chunks.map((chunk) => {
      const tf = new Map<string, number>();
      // The title counts twice: it says what the chunk is about.
      const words = [...terms(chunk.title), ...terms(chunk.title), ...terms(chunk.text)];
      for (const w of words) tf.set(w, (tf.get(w) ?? 0) + 1);
      return { chunk, tf, len: words.length };
    });
    for (const d of this.docs) for (const w of d.tf.keys()) this.df.set(w, (this.df.get(w) ?? 0) + 1);
    this.avg = this.docs.reduce((a, d) => a + d.len, 0) / Math.max(1, this.docs.length);
  }

  /** The `k` best chunks for `query` (BM25, k1 = 1.2, b = 0.75). */
  search(query: string, k = 4): Chunk[] {
    const q = [...new Set(terms(query))];
    if (!q.length) return [];
    const n = this.docs.length;
    const scored = this.docs.map((d) => {
      let s = 0;
      for (const w of q) {
        const f = d.tf.get(w);
        if (!f) continue;
        const df = this.df.get(w) ?? 0;
        const idf = Math.log(1 + (n - df + 0.5) / (df + 0.5));
        s += (idf * f * 2.2) / (f + 1.2 * (0.25 + (0.75 * d.len) / this.avg));
      }
      return { s, c: d.chunk };
    });
    return scored
      .filter((x) => x.s > 0)
      .sort((a, b) => b.s - a.s)
      .slice(0, k)
      .map((x) => x.c);
  }
}

let index: Index | null = null;
/** The index over the bundled user guide (built on first use). */
export function guideIndex(): Index {
  return (index ??= new Index(chunkMarkdown(guideMd)));
}

/** A note as DRIFT needs it (the store's `Note` has more). */
export interface NoteLike {
  id: number;
  title: string;
  body: string;
  tags: string[];
  hideFromDrift: boolean;
  updatedAt: number;
}

/** Chunks of the user's notes, minus the ones hidden from DRIFT. */
export function noteChunks(notes: NoteLike[]): Chunk[] {
  return notes
    .filter((n) => !n.hideFromDrift)
    .flatMap((n) => {
      const head = n.tags.length ? `${n.tags.map((t) => `#${t}`).join(" ")}\n\n` : "";
      const parts = chunkMarkdown(`## ${n.title}\n${head}${n.body}`);
      return parts.map((c) => ({ title: c.title === n.title ? `Note “${n.title}”` : `Note “${n.title}” › ${c.title}`, text: c.text, noteId: n.id }));
    });
}

let notesKey = "";
let notesIdx: Index | null = null;
/** The index over the user's notes, rebuilt when they change. */
export function notesIndex(notes: NoteLike[]): Index {
  const key = notes.map((n) => `${n.id}:${n.updatedAt}:${n.hideFromDrift ? 1 : 0}`).join("|");
  if (!notesIdx || key !== notesKey) {
    notesIdx = new Index(noteChunks(notes));
    notesKey = key;
  }
  return notesIdx;
}

/** The notes among the retrieved chunks, once each. */
export function noteRefs(docs: { noteId?: number; title: string }[]): { id: number; title: string }[] {
  const seen = new Map<number, string>();
  for (const d of docs) {
    if (d.noteId === undefined || seen.has(d.noteId)) continue;
    const m = /^Note “(.*?)”/.exec(d.title);
    seen.set(d.noteId, m ? m[1] : d.title);
  }
  return [...seen].map(([id, title]) => ({ id, title }));
}
