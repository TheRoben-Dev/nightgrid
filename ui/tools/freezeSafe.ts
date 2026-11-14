/**
 * Tauri's `freezePrototype` (security hardening) freezes `Object.prototype`.
 * In strict-mode code, assigning a property that exists on a frozen
 * prototype throws ("override mistake"), even on an unrelated plain object.
 * xterm.js (via bundled VS Code helpers) does `ns.toString = fn` at module
 * load, which kills the whole bundle in the app while working fine in a
 * normal browser. This Vite plugin rewrites such assignments in dependencies
 * to `Object.defineProperty`, which is allowed.
 */
import type { Plugin } from "vite";

/** Object.prototype members a library might shadow by plain assignment. */
const SHADOWED = ["toString", "valueOf", "constructor", "toLocaleString", "hasOwnProperty", "isPrototypeOf"];
const PATTERN = new RegExp(`(?<![\\w$.])([A-Za-z_$][\\w$]*)\\.(${SHADOWED.join("|")})\\s*=(?!=)\\s*([A-Za-z_$][\\w$]*)\\s*([;,])`, "g");

/** Rewrites `x.toString = y;` into a defineProperty call. Returns the code and the number of rewrites. */
export function rewriteShadowingAssignments(code: string): { code: string; count: number } {
  let count = 0;
  const out = code.replace(PATTERN, (_m, obj: string, prop: string, value: string, end: string) => {
    count++;
    const def = `Object.defineProperty(${obj},"${prop}",{value:${value},writable:true,configurable:true,enumerable:true})`;
    // A trailing comma means we were inside a comma expression; keep it.
    return def + end;
  });
  return { code: out, count };
}

/** Finds remaining `x.toString = ...` style assignments the rewrite didn't handle. */
export function findShadowingAssignments(code: string): string[] {
  const re = new RegExp(`(?<![\\w$.])[A-Za-z_$][\\w$]*\\.(?:${SHADOWED.join("|")})\\s*=(?!=)`, "g");
  return code.match(re) ?? [];
}

export function freezeSafe(packages: string[] = ["@xterm/"]): Plugin {
  return {
    name: "nightgrid-freeze-safe",
    enforce: "pre",
    transform(code, id) {
      if (!packages.some((p) => id.includes(`/node_modules/${p}`))) return null;
      const { code: out, count } = rewriteShadowingAssignments(code);
      return count ? { code: out, map: null } : null;
    },
  };
}
