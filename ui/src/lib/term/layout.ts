/**
 * Split-pane layout trees for terminal tabs. Pure functions so they are easy
 * to test; the store swaps whole trees in.
 */

export type Dir = "row" | "col";

export type LayoutNode =
  | { kind: "pane"; pane: number }
  | { kind: "split"; dir: Dir; ratio: number; a: LayoutNode; b: LayoutNode };

export const leaf = (pane: number): LayoutNode => ({ kind: "pane", pane });

/** Pane ids in visual order (left/top first). */
export function leaves(n: LayoutNode): number[] {
  return n.kind === "pane" ? [n.pane] : [...leaves(n.a), ...leaves(n.b)];
}

/** Replaces the leaf `target` with a split of it and `added` (added goes right/below). */
export function splitLeaf(n: LayoutNode, target: number, added: number, dir: Dir): LayoutNode {
  if (n.kind === "pane") {
    return n.pane === target ? { kind: "split", dir, ratio: 0.5, a: n, b: leaf(added) } : n;
  }
  return { ...n, a: splitLeaf(n.a, target, added, dir), b: splitLeaf(n.b, target, added, dir) };
}

/** Removes a leaf; its sibling takes the parent's place. `null` if the tree is now empty. */
export function removeLeaf(n: LayoutNode, target: number): LayoutNode | null {
  if (n.kind === "pane") return n.pane === target ? null : n;
  const a = removeLeaf(n.a, target);
  const b = removeLeaf(n.b, target);
  if (!a) return b;
  if (!b) return a;
  return { ...n, a, b };
}

/** Sets the ratio of the split at `path` ("" = root, then "a"/"b" per level). */
export function setRatio(n: LayoutNode, path: string, ratio: number): LayoutNode {
  if (n.kind === "pane") return n;
  const r = Math.min(0.85, Math.max(0.15, ratio));
  if (path === "") return { ...n, ratio: r };
  const [head, rest] = [path[0], path.slice(1)];
  return head === "a" ? { ...n, a: setRatio(n.a, rest, r) } : { ...n, b: setRatio(n.b, rest, r) };
}

/** The neighbouring pane in visual order, used to move focus after a close. */
export function neighbour(n: LayoutNode, pane: number): number | null {
  const all = leaves(n);
  const i = all.indexOf(pane);
  if (i < 0) return all[0] ?? null;
  return all[i + 1] ?? all[i - 1] ?? null;
}
