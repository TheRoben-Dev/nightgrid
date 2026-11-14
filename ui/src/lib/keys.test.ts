import { describe, expect, it } from "vitest";
import { DEFAULT_KEYMAP, formatChord, matches, parseChord } from "./keys";

const ev = (key: string, mods: Partial<Record<"ctrlKey" | "altKey" | "shiftKey" | "metaKey", boolean>> = {}) => ({
  key,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

describe("keys", () => {
  it("Mod maps to Cmd on mac and Ctrl elsewhere", () => {
    expect(matches(ev("k", { metaKey: true }), "Mod+K", true)).toBe(true);
    expect(matches(ev("k", { ctrlKey: true }), "Mod+K", true)).toBe(false);
    expect(matches(ev("k", { ctrlKey: true }), "Mod+K", false)).toBe(true);
    expect(matches(ev("k", { metaKey: true }), "Mod+K", false)).toBe(false);
  });

  it("requires exact modifiers", () => {
    expect(matches(ev("K", { ctrlKey: true, shiftKey: true }), "Mod+K", false)).toBe(false);
    expect(matches(ev("E", { ctrlKey: true, shiftKey: true }), "Mod+Shift+E", false)).toBe(true);
    expect(matches(ev("Escape"), "Escape", false)).toBe(true);
  });

  it("formats per platform", () => {
    expect(formatChord("Mod+K", true)).toBe("⌘K");
    expect(formatChord("Mod+Shift+E", false)).toBe("Ctrl+Shift+E");
    expect(formatChord("Escape", false)).toBe("Escape");
  });

  it("rejects unknown modifiers", () => {
    expect(() => parseChord("Hyper+K")).toThrow();
  });

  it("default keymap parses and has no duplicate bindings", () => {
    const specs = Object.values(DEFAULT_KEYMAP);
    specs.forEach((s) => parseChord(s));
    expect(new Set(specs).size).toBe(specs.length);
  });
});
