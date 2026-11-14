import { describe, expect, it } from "vitest";
import { barcode, hexId } from "./hex";
import { scrambleFrame } from "./scramble";
import { SECTIONS } from "./sections";
import { DEFAULT_KEYMAP } from "./keys";

describe("decoration helpers", () => {
  it("hexId is deterministic and formatted", () => {
    expect(hexId("lumen")).toBe(hexId("lumen"));
    expect(hexId("lumen")).toMatch(/^0x[0-9A-F]{4}$/);
    expect(hexId("lumen")).not.toBe(hexId("other"));
  });

  it("barcode is deterministic with widths 1–4", () => {
    const b = barcode("x", 32);
    expect(b).toEqual(barcode("x", 32));
    expect(b.every((w) => w >= 1 && w <= 4)).toBe(true);
  });

  it("scramble reveals progressively and keeps spaces/length", () => {
    const t = "SYS STATUS";
    expect(scrambleFrame(t, 1)).toBe(t);
    const f = scrambleFrame(t, 0.5, () => 0);
    expect(f).toHaveLength(t.length);
    expect(f.startsWith("SYS S")).toBe(true);
    expect(f[3]).toBe(" ");
  });
});

describe("sections", () => {
  it("every section has a keymap action", () => {
    for (const s of SECTIONS) expect(DEFAULT_KEYMAP[s.action]).toBeTruthy();
  });
});
