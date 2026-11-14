import { describe, expect, it } from "vitest";
import { fuzzyFilter, fuzzyMatch } from "./fuzzy";

describe("fuzzy", () => {
  it("matches subsequences and rejects non-matches", () => {
    expect(fuzzyMatch("dkr", "Go to Docker")).not.toBeNull();
    expect(fuzzyMatch("xyz", "Go to Docker")).toBeNull();
  });

  it("empty query matches everything in original order", () => {
    const items = [{ t: "b" }, { t: "a" }];
    expect(fuzzyFilter(items, "  ", (i) => i.t).map((i) => i.t)).toEqual(["b", "a"]);
  });

  it("ranks prefix and word-start matches first", () => {
    const items = ["Theme: ICE", "Go to Terminal", "Go to Settings"].map((t) => ({ t }));
    expect(fuzzyFilter(items, "term", (i) => i.t)[0].t).toBe("Go to Terminal");
    expect(fuzzyFilter(items, "set", (i) => i.t)[0].t).toBe("Go to Settings");
  });

  it("reports match positions", () => {
    expect(fuzzyMatch("dk", "docker")?.positions).toEqual([0, 3]);
  });
});
