import { describe, expect, it } from "vitest";
import { SECTIONS } from "../sections";
import { CATALOG, CATEGORIES, VIEW_HELP } from "./catalog";
import { inline, parseMd } from "../helper/md";
import { FIGURE_NAMES, figure } from "./figures";
import { ARTICLES, article, fromFile, fromGuide, parts, plain, search } from "./library";

describe("library", () => {
  it("has every shelf and a catalog in step with them", () => {
    for (const s of ["nightgrid", "howto", "basics", "linux", "apps", "guides"]) expect(ARTICLES.some((a) => a.shelf === s), s).toBe(true);
    const ids = (xs: { id: string; title: string }[]) => xs.map((x) => `${x.id}|${x.title}`).sort();
    expect(ids(CATALOG)).toEqual(ids(ARTICLES));
    expect(new Set(ARTICLES.map((a) => a.id)).size).toBe(ARTICLES.length);
    expect(CATALOG.every((c) => c.id.startsWith(`${c.shelf}/`))).toBe(true);
  });

  it("puts every article in a topic, and every topic has articles", () => {
    const cats = new Set(CATEGORIES.map((c) => c.id as string));
    for (const c of CATALOG) expect(cats.has(c.cat), c.id).toBe(true);
    for (const c of CATEGORIES) expect(CATALOG.some((x) => x.cat === c.id), c.id).toBe(true);
  });

  it("keeps the library lean", () => {
    // Plain markdown only; the whole library stays far under its 25 MB ceiling.
    const bytes = ARTICLES.reduce((n, a) => n + a.body.length, 0);
    expect(bytes).toBeLessThan(25 * 1024 * 1024);
  });

  it("gives every view a help article", () => {
    for (const s of SECTIONS) expect(article(VIEW_HELP[s.id]), s.id).toBeDefined();
  });

  it("only links to views that exist, and never out of the app", () => {
    const views = new Set(SECTIONS.map((s) => s.id as string));
    for (const a of ARTICLES) {
      for (const m of a.body.matchAll(/\]\(view:([a-z]+)\)/g)) expect(views.has(m[1]), `${a.id} → view:${m[1]}`).toBe(true);
      for (const m of a.body.matchAll(/\]\(kb:([\w/-]+)\)/g)) expect(article(m[1]), `${a.id} → kb:${m[1]}`).toBeDefined();
      // Bundled and offline: images only as bundled figures, no raw HTML, no markdown links out.
      const rest = a.body.replace(/^!\[[^\]\n]*\]\(fig:[a-z0-9-]+\)$/gm, "");
      expect(rest, a.id).not.toMatch(/!\[|<img|<script|<iframe|\]\(https?:/i);
      for (const m of a.body.matchAll(/^!\[([^\]\n]*)\]\(fig:([a-z0-9-]+)\)$/gm)) {
        expect(figure(m[2]), `${a.id} → fig:${m[2]}`).toBeDefined();
        expect(m[1].trim().length, `${a.id} → fig:${m[2]} needs a caption`).toBeGreaterThan(0);
      }
    }
  });

  it("keeps figures local, safe and small", () => {
    const files = import.meta.glob("../../../../docs/kb/figures/*.svg", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
    let bytes = 0;
    for (const [p, svg] of Object.entries(files)) {
      bytes += svg.length;
      expect(svg, p).toMatch(/^<svg[\s>]/);
      expect(svg, p).toMatch(/viewBox=/);
      // An image can't run code, but keep the files clean anyway: nothing active, nothing fetched.
      expect(svg, p).not.toMatch(/<script|<foreignObject|\son[a-z]+\s*=|javascript:|(?:href|src)\s*=\s*["'](?!#)|url\((?!#)|@import/i);
      expect(svg.length, p).toBeLessThan(200 * 1024);
    }
    expect(Object.keys(files).length).toBe(FIGURE_NAMES.length);
    // Text and figures together stay far under the 25 MB ceiling.
    const text = ARTICLES.reduce((n, a) => n + a.body.length, 0);
    expect(text + bytes).toBeLessThan(25 * 1024 * 1024);
    // Every figure is used somewhere.
    const used = new Set(ARTICLES.flatMap((a) => [...a.body.matchAll(/\(fig:([a-z0-9-]+)\)/g)].map((m) => m[1])));
    for (const n of FIGURE_NAMES) expect(used.has(n), n).toBe(true);
  });

  it("parses figures only where asked", () => {
    const src = "![A diagram](fig:x)\n\ntext ![no](fig:y) inline";
    expect(parseMd(src, { figures: true })[0]).toEqual({ kind: "fig", alt: "A diagram", name: "x" });
    expect(parseMd(src, { figures: true })[1].kind).toBe("p");
    // DRIFT and notes never get images.
    expect(parseMd(src).some((b) => b.kind === "fig")).toBe(false);
    expect(plain("![Caption here](fig:x)")).toBe("Caption here");
  });

  it("splits the user guide by section and reads article files", () => {
    const g = fromGuide("# Guide\n\nintro\n\n## Hosts\n\nAdd **hosts** here.\n\n### More\n\ntext\n\n## Files\n\nBrowse.");
    expect(g.map((a) => a.id)).toEqual(["nightgrid/hosts", "nightgrid/files"]);
    expect(g[0].summary).toBe("Add hosts here.");
    expect(g[0].body).toContain("### More");
    expect(fromFile("/x/docs/kb/basics/ssh-keys.md", "# SSH keys\n\nKeys.\n")).toMatchObject({ id: "basics/ssh-keys", shelf: "basics", title: "SSH keys", summary: "Keys." });
    expect(fromFile("/x/docs/kb/guides/README.md", "# x")).toBeNull();
    expect(plain("Run `ls` in [Files](view:files) **now**")).toBe("Run ls in Files now");
    expect(inline("see [Backups](kb:basics/backups) and [Fleet](view:fleet)", true).filter((t) => t.kind === "view")).toEqual([
      { kind: "view", text: "Backups", view: "kb:basics/backups" },
      { kind: "view", text: "Fleet", view: "fleet" },
    ]);
  });

  it("turns a guide's parts into objectives", () => {
    const a = { id: "guides/t", shelf: "guides" as const, title: "T", summary: "", body: "Intro\n\n## Part 1 · Set up\n\n- [ ] one\n- [ ] two\n\n```sh\n- [ ] not a step\n```\n\n## Notes\n\nNo steps.\n\n## Part 2 · Run\n\n- [x] three" };
    const p = parts(a);
    expect(p.map((x) => [x.title, x.keys.length])).toEqual([
      ["Part 1 · Set up", 2],
      ["Part 2 · Run", 1],
    ]);
    // Keys match Markdown.svelte's block-item keys.
    expect(p[0].keys).toEqual(["2-0", "2-1"]);
    for (const g of ARTICLES.filter((x) => ["guides", "howto", "apps", "linux"].includes(x.shelf))) expect(parts(g).length, g.id).toBeGreaterThan(0);
  });

  it("finds articles by their words", () => {
    expect(search("restic password").some((h) => h.article.id === "basics/backups")).toBe(true);
    expect(search("zigbee2mqtt coordinator").slice(0, 3).some((h) => h.article.id === "apps/zigbee2mqtt")).toBe(true);
    expect(search("PasswordAuthentication sshd").some((h) => h.article.id === "basics/sshd-hardening")).toBe(true);
    expect(search("qwxyzzy")).toEqual([]);
  });
});
