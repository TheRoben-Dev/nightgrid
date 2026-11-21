// Layout consistency guard (D-187). Tab bars, segmented pickers and small
// buttons share one definition in styles/components.css; these checks stop
// components from growing their own copies again.
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// Tests run from the repository root.
const SRC = join(process.cwd(), "ui/src/");

function svelteFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) return svelteFiles(p);
    return name.endsWith(".svelte") ? [p] : [];
  });
}

const FILES = svelteFiles(SRC).map((p) => ({ path: p.slice(SRC.length), text: readFileSync(p, "utf8") }));
const styleOf = (text: string) => text.match(/<style>([\s\S]*?)<\/style>/)?.[1] ?? "";

// Document-style tab rows (closable editor/terminal/desktop tabs, dashboards,
// starter shelves) are a different control and keep their own look.
const OWN_TABS = new Set(["lib/views/Terminal.svelte", "lib/views/Desktops.svelte", "lib/views/Files.svelte", "lib/views/Overview.svelte", "lib/components/fleet/StarterGallery.svelte", "lib/components/update/WhatsNew.svelte"]);

describe("shared layout pieces", () => {
  it("components.css defines the shared pieces", () => {
    const css = readFileSync(join(SRC, "styles/components.css"), "utf8");
    for (const sel of [".btn.tiny", ".tabstrip", ".tabstrip.sm", ".tabstrip.line", ".tabstrip.choice", ".viewbar"]) expect(css).toContain(sel);
  });

  it("no component redefines the small button", () => {
    const offenders = FILES.filter((f) => /(^|\n)\s*(\.btn)?\.tiny\s*[,{]/.test(styleOf(f.text))).map((f) => f.path);
    expect(offenders).toEqual([]);
  });

  it("tab lists use the shared tab bar", () => {
    const offenders = FILES.filter((f) => !OWN_TABS.has(f.path))
      .filter((f) => /<div class="(?![^"]*\btabstrip\b)[^"]*" role="tablist"/.test(f.text))
      .map((f) => f.path);
    expect(offenders).toEqual([]);
  });

  it("no component restyles tab buttons from scratch", () => {
    // A local `.tabs button { … font-family: var(--font-display) … }` is the old copy-pasted tab bar.
    const offenders = FILES.filter((f) => !OWN_TABS.has(f.path))
      .filter((f) => /\.(tabs|seg) button\s*\{[^}]*font-family:\s*var\(--font-display\)/.test(styleOf(f.text)))
      .map((f) => f.path);
    expect(offenders).toEqual([]);
  });
});
