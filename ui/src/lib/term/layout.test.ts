import { describe, expect, it } from "vitest";
import { leaf, leaves, neighbour, removeLeaf, setRatio, splitLeaf } from "./layout";

describe("layout", () => {
  it("splits a leaf and keeps visual order", () => {
    let t = leaf(1);
    t = splitLeaf(t, 1, 2, "row");
    t = splitLeaf(t, 1, 3, "col");
    expect(leaves(t)).toEqual([1, 3, 2]);
    expect(t.kind === "split" && t.dir).toBe("row");
  });

  it("removes a leaf and collapses its parent", () => {
    let t = splitLeaf(splitLeaf(leaf(1), 1, 2, "row"), 2, 3, "col");
    t = removeLeaf(t, 2)!;
    expect(leaves(t)).toEqual([1, 3]);
    expect(t.kind === "split" && t.b).toEqual(leaf(3));
    expect(removeLeaf(leaf(1), 1)).toBeNull();
  });

  it("clamps ratios and addresses nested splits by path", () => {
    let t = splitLeaf(splitLeaf(leaf(1), 1, 2, "row"), 2, 3, "col");
    t = setRatio(t, "", 0.01);
    t = setRatio(t, "b", 0.7);
    expect(t.kind === "split" && t.ratio).toBe(0.15);
    expect(t.kind === "split" && t.b.kind === "split" && t.b.ratio).toBe(0.7);
  });

  it("picks the next pane, else the previous", () => {
    const t = splitLeaf(splitLeaf(leaf(1), 1, 2, "row"), 2, 3, "row");
    expect(neighbour(t, 2)).toBe(3);
    expect(neighbour(t, 3)).toBe(2);
    expect(neighbour(leaf(9), 9)).toBeNull();
  });
});
