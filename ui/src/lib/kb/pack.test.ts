import { afterEach, describe, expect, it } from "vitest";
import { CATALOG, applyCatalog, categoryOf } from "./catalog";
import { matchErrors } from "./errorref";
import { FIGURE_NAMES, applyFigures, figure, svgSafe } from "./figures";
import { ARTICLES, applyPack, article, contentGeneration, search } from "./library";
import { kbVersionLabel } from "./version";

// Library content packs (D-174): newer docs/kb text swapped in place.
describe("content packs", () => {
  afterEach(() => {
    applyPack(null);
    applyCatalog(null);
    applyFigures(null);
  });

  const files = {
    "apps/brand-new.md": "# Brand new app\n\nA zorblatt server for testing.\n\n## Part 1 · Install\n\n- [ ] Run it",
    "errors/pack.md": "# Errors: pack\n\n### zorblatt refused the frobnicator handshake\nMeans: test.\nFix: test.",
    "figures/new-fig.svg": '<svg viewBox="0 0 1 1"></svg>',
    "figures/bad-fig.svg": '<svg viewBox="0 0 1 1"><script>x()</script></svg>',
  };

  it("replaces the docs/kb shelves but keeps Using Nightgrid", () => {
    const own = ARTICLES.filter((a) => a.shelf === "nightgrid").length;
    const gen = contentGeneration();
    applyPack(files);
    expect(contentGeneration()).toBeGreaterThan(gen);
    expect(ARTICLES.filter((a) => a.shelf === "nightgrid").length).toBe(own);
    expect(ARTICLES.filter((a) => a.shelf !== "nightgrid").map((a) => a.id).sort()).toEqual(["apps/brand-new", "errors/pack"]);
    expect(article("apps/brand-new")?.title).toBe("Brand new app");
    expect(article("basics/backups")).toBeUndefined();
    // Search and the error reference rebuild from the new text.
    expect(search("zorblatt server").map((h) => h.article.id)).toContain("apps/brand-new");
    expect(matchErrors("error: zorblatt refused the frobnicator handshake")[0]?.articleId).toBe("errors/pack");
    applyPack(null);
    expect(article("basics/backups")).toBeDefined();
    expect(search("zorblatt")).toEqual([]);
  });

  it("swaps catalog lines and keeps the app's own", () => {
    const own = CATALOG.filter((c) => c.shelf === "nightgrid").length;
    const before = CATALOG.length;
    applyCatalog([
      { id: "apps/brand-new", shelf: "apps", title: "Brand new app", cat: "media" },
      { id: "nightgrid/fake", shelf: "nightgrid", title: "Not allowed", cat: "start" },
      { id: "apps/odd", shelf: "apps", title: "Unknown topic", cat: "nope" },
    ]);
    expect(CATALOG.filter((c) => c.shelf === "nightgrid").length).toBe(own);
    expect(CATALOG.some((c) => c.id === "nightgrid/fake")).toBe(false);
    expect(categoryOf("apps/brand-new")).toBe("media");
    expect(categoryOf("apps/odd")).toBe("start");
    applyCatalog(null);
    expect(CATALOG.length).toBe(before);
  });

  it("adds safe figures only", () => {
    applyFigures(files);
    expect(figure("new-fig")).toMatch(/^data:image\/svg\+xml/);
    expect(figure("bad-fig")).toBeUndefined();
    applyFigures(null);
    expect(figure("new-fig")).toBeUndefined();
    // The bundled figures pass the same check a pack's must.
    const raw = import.meta.glob("../../../../docs/kb/figures/*.svg", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
    for (const [p, svg] of Object.entries(raw)) expect(svgSafe(svg), p).toBe(true);
    expect(FIGURE_NAMES.length).toBe(Object.keys(raw).length);
  });

  it("labels versions", () => {
    expect(kbVersionLabel(0)).toBe("—");
    expect(kbVersionLabel(1_700_000_000)).toMatch(/^\d{4}-\d\d-\d\d \d\d:\d\d$/);
  });
});
