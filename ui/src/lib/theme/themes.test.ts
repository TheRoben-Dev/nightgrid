// @vitest-environment node
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { contrastRatio } from "./contrast";
import { COLOURS, resolveCustom, SURFACES, TEXTS, THEME_IDS, THEMES } from "./themes";

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

  it("four schemes, the fourth is custom", () => {
    expect(THEME_IDS).toEqual(["nightgrid", "ice", "bloodline", "custom"]);
  });
});

describe("custom theme swatches", () => {
  it("every surface + text combination meets WCAG AA", () => {
    for (const s of SURFACES)
      for (const t of TEXTS)
        for (const bg of [s.values["bg-0"], s.values["bg-1"], s.values["bg-2"]])
          for (const fg of [t.values["text-0"], t.values["text-1"]])
            expect(contrastRatio(fg, bg), `${t.label} on ${s.label} ${bg}`).toBeGreaterThanOrEqual(4.5);
  });

  it("every colour swatch is legible on every panel surface", () => {
    for (const [k, slot] of Object.entries(COLOURS))
      for (const c of slot.values)
        for (const s of SURFACES) expect(contrastRatio(c, s.values["bg-1"]), `${k} ${c} on ${s.label}`).toBeGreaterThanOrEqual(3);
  });

  it("dark text is readable on every accent swatch", () => {
    for (const c of COLOURS.accent.values) expect(contrastRatio("#07080B", c), c).toBeGreaterThanOrEqual(4.5);
  });

  it("each built-in scheme is reachable as a starting point", () => {
    for (const id of THEME_IDS) {
      const t = THEMES[id].tokens;
      expect(resolveCustom({ ...t })).toEqual(t);
    }
  });

  it("ignores values outside the curated choices", () => {
    const base = THEMES.custom.tokens;
    expect(resolveCustom({ accent: "#123456", "bg-0": "#07080B" })).toEqual(base);
    expect(resolveCustom({ accent: "#ff7ac6" }).accent).toBe("#FF7AC6");
    expect(resolveCustom({ ...SURFACES[4].values })["bg-2"]).toBe(SURFACES[4].values["bg-2"]);
  });
});

describe("contrastRatio", () => {
  it("matches known values", () => {
    expect(contrastRatio("#000000", "#FFFFFF")).toBeCloseTo(21, 1);
    expect(contrastRatio("#777777", "#FFFFFF")).toBeCloseTo(4.48, 1);
  });
});
