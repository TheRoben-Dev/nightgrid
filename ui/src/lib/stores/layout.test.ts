import { describe, expect, it } from "vitest";
import { clampSize, layout, SIZES } from "./layout.svelte";

describe("layout store", () => {
  it("clamps, persists and resets sizes", () => {
    expect(layout.get("rail")).toBe(SIZES.rail.def);
    layout.set("rail", 10_000);
    expect(layout.get("rail")).toBe(SIZES.rail.max);
    layout.set("filesSplit", 5);
    expect(layout.get("filesSplit")).toBe(SIZES.filesSplit.min);
    expect(JSON.parse(localStorage.getItem("nightgrid.layout.v1")!)).toMatchObject({ rail: SIZES.rail.max });
    layout.reset("rail");
    expect(layout.get("rail")).toBe(SIZES.rail.def);
    layout.reset();
    expect(layout.customized).toBe(false);
  });

  it("rounds to one decimal", () => {
    expect(clampSize("dockerSide", 40.1234)).toBe(40.1);
  });
});
