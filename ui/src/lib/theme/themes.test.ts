// @vitest-environment node
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { contrastRatio } from "./contrast";
import { THEME_IDS, THEMES } from "./themes";

const tokensCss = readFileSync(new URL("../../styles/tokens.css", import.meta.url), "utf8");

describe("themes", () => {
  it.each(THEME_IDS)("%s: body text meets WCAG AA on every background", (id) => {
    const t = THEMES[id].tokens;
    for (const bg of [t["bg-0"], t["bg-1"], t["bg-2"]]) {
      expect(contrastRatio(t["text-0"], bg)).toBeGreaterThanOrEqual(4.5);
      expect(contrastRatio(t["text-1"], bg)).toBeGreaterThanOrEqual(4.5);
    }
  });

  it.each(THEME_IDS)("%s: status colours are legible on panels (≥ 3:1, used with icon + text)", (id) => {
    const t = THEMES[id].tokens;
    for (const c of [t.accent, t.cyan, t.magenta, t.danger, t.ok, t.warn, t.mint]) {
      expect(contrastRatio(c, t["bg-1"])).toBeGreaterThanOrEqual(3);
    }
  });

  it.each(THEME_IDS)("%s: dark text is readable on the accent fill", (id) => {
    expect(contrastRatio("#07080B", THEMES[id].tokens.accent)).toBeGreaterThanOrEqual(4.5);
  });

  it("tokens.css first-paint defaults match the NIGHTGRID theme", () => {
    for (const [k, v] of Object.entries(THEMES.nightgrid.tokens)) {
      expect(tokensCss, `--${k}`).toMatch(new RegExp(`--${k}:\\s*${v};`, "i"));
    }
  });

  it("only the plain theme disables effects", () => {
    expect(THEME_IDS.filter((id) => !THEMES[id].effects)).toEqual(["plain"]);
  });
});

describe("contrastRatio", () => {
  it("matches known values", () => {
    expect(contrastRatio("#000000", "#FFFFFF")).toBeCloseTo(21, 1);
    expect(contrastRatio("#777777", "#FFFFFF")).toBeCloseTo(4.48, 1);
  });
});
