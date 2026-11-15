import { describe, expect, it } from "vitest";

describe("terminal key maps in DRIFT", async () => {
  const { guideIndex } = await import("./knowledge");
  const { contextBlock } = await import("./prompt");
  it("questions about a program's keys find its map", () => {
    expect(guideIndex().search("how do I quit vim", 4).map((c) => c.title)).toContain("Keys in vim");
    expect(guideIndex().search("htop sort kill", 4).map((c) => c.title)).toContain("Keys in htop");
  });
  it("the running program and its keys go into the context", () => {
    const block = contextBlock({ view: "terminal", program: { name: "htop", keys: "q: quit" } }, []);
    expect(block).toContain("Running in the terminal: htop. Its keys: q: quit.");
  });
});
