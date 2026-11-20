import { describe, expect, it } from "vitest";
import { typedMatches } from "./confirm";

describe("typedMatches", () => {
  it("ignores case and surrounding spaces", () => {
    expect(typedMatches("BRAINCAGE", "braincage")).toBe(true);
    expect(typedMatches("  braincage ", "braincage")).toBe(true);
    expect(typedMatches("Lumen", "LUMEN")).toBe(true);
  });
  it("still needs the whole name", () => {
    expect(typedMatches("brain", "braincage")).toBe(false);
    expect(typedMatches("", "braincage")).toBe(false);
    expect(typedMatches("braincage2", "braincage")).toBe(false);
  });
  it("never matches a missing or empty name", () => {
    expect(typedMatches("", "")).toBe(false);
    expect(typedMatches("x", null)).toBe(false);
    expect(typedMatches("x", undefined)).toBe(false);
  });
});
