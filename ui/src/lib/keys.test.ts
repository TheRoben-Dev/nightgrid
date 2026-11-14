import { describe, expect, it } from "vitest";
import { actionFor, capturedInTerminal, DEFAULT_KEYMAP, formatChord, matches, parseChord } from "./keys";

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

describe("terminal focus rule", () => {
  const ev = (key: string, mods: Partial<Record<"ctrlKey" | "altKey" | "shiftKey" | "metaKey", boolean>> = {}) => ({
    key,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    ...mods,
  });

  it("leaves plain Ctrl chords to the shell on Windows/Linux", () => {
    expect(capturedInTerminal("Mod+K", false)).toBe(false);
    expect(capturedInTerminal("Mod+B", false)).toBe(false);
    expect(capturedInTerminal("Mod+Shift+K", false)).toBe(true);
    expect(capturedInTerminal("Ctrl+Tab", false)).toBe(true);
    expect(actionFor(ev("k", { ctrlKey: true }), true, false)).toBeNull();
    expect(actionFor(ev("k", { ctrlKey: true }), false, false)).toBe("palette.open");
    expect(actionFor(ev("K", { ctrlKey: true, shiftKey: true }), true, false)).toBe("palette.openAlt");
  });

  it("keeps Cmd chords on macOS", () => {
    expect(capturedInTerminal("Mod+K", true)).toBe(true);
    expect(actionFor(ev("k", { metaKey: true }), true, true)).toBe("palette.open");
    expect(actionFor(ev("k", { ctrlKey: true }), true, true)).toBeNull();
  });

  it("has no duplicate shortcuts", () => {
    const specs = Object.values(DEFAULT_KEYMAP).map((s) => s.toLowerCase());
    expect(new Set(specs).size).toBe(specs.length);
  });
});
