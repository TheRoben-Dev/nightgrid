// @vitest-environment node
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { describe, expect, it } from "vitest";
import { findShadowingAssignments, rewriteShadowingAssignments } from "./freezeSafe";

const require = createRequire(import.meta.url);
const pkgFile = (pkg: string, file: string) => join(dirname(require.resolve(`${pkg}/package.json`)), file);

describe("freezeSafe", () => {
  it("rewrites assignments that would throw with a frozen Object.prototype", () => {
    const src = `"use strict";var ns;(e=>{function t(){return"x"}e.toString=t;function n(){}e.fromString=n})(ns||={});`;
    const { code, count } = rewriteShadowingAssignments(src);
    expect(count).toBe(1);
    expect(findShadowingAssignments(code)).toEqual([]);
    // Runs under a frozen prototype, which the original does not.
    const run = (c: string) => new Function(`Object.freeze(Object.prototype);${c};return ns.toString();`);
    expect(() => run(code)()).not.toThrow();
  });

  it("leaves comparisons and member chains alone", () => {
    const src = "a.toString==b;x.y.toString=z;";
    expect(rewriteShadowingAssignments(src).count).toBe(0);
  });

  // Guards xterm upgrades: every such assignment in the shipped files must be rewritable.
  it.each([
    ["@xterm/xterm", "lib/xterm.mjs"],
    ["@xterm/addon-fit", "lib/addon-fit.mjs"],
    ["@xterm/addon-search", "lib/addon-search.mjs"],
    ["@xterm/addon-web-links", "lib/addon-web-links.mjs"],
    ["@xterm/addon-unicode11", "lib/addon-unicode11.mjs"],
  ])("%s has no unhandled shadowing assignments", (pkg, file) => {
    const { code } = rewriteShadowingAssignments(readFileSync(pkgFile(pkg, file), "utf8"));
    expect(findShadowingAssignments(code)).toEqual([]);
  });
});
