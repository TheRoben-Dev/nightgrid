import { describe, expect, it } from "vitest";
import { ICON_MAX_COLS, ICON_MAX_LINES, ICON_PRESETS, iconProblem, normalizeIcon } from "./icons";

describe("ascii host icons", () => {
  it("presets are valid and already normalized", () => {
    for (const p of ICON_PRESETS) {
      expect(iconProblem(p.art), p.id).toBeNull();
      expect(normalizeIcon(p.art), p.id).toBe(p.art);
    }
  });

  it("normalizes like the backend", () => {
    expect(normalizeIcon("\r\n   ┌──┐  \r\n   │▓▓│\r\n   └──┘\n\n")).toBe("┌──┐\n│▓▓│\n└──┘");
    expect(normalizeIcon("  \n \n")).toBeNull();
    expect(normalizeIcon("\ta\n  b")).toBe("  a\nb");
  });

  it("flags what the backend refuses", () => {
    expect(iconProblem("x\u001b[31m")).toMatch(/can't be used/);
    expect(iconProblem("🖥")).toMatch(/can't be used/);
    expect(iconProblem("x\n".repeat(ICON_MAX_LINES + 1))).toMatch(/lines/);
    expect(iconProblem("=".repeat(ICON_MAX_COLS + 1))).toMatch(/characters per line/);
    expect(iconProblem("█".repeat(ICON_MAX_COLS))).toBeNull();
  });
});
