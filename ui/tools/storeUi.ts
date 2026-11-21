/**
 * Store components whose interface can ship as a signed component pack
 * (feature store part 2). Each lists the files it owns, relative to ui/src:
 * the view it opens and the folders only it uses. Everything an owned file
 * imports from outside its own files is a "host module": the app emits it at
 * a stable path (`/host/…`, see hostModules.ts) so a pack built later shares
 * the running app's single copy of it (stores, components, Svelte itself).
 *
 * A test (ui/tools/storeUi.test.ts) checks that nothing outside a component
 * imports its owned files, so a pack never duplicates state the app holds.
 */

export interface StoreUi {
  /** Store id (core/src/components.rs). */
  id: string;
  /** The module the app loads for the component, relative to ui/src. */
  entry: string;
  /** Files and folders (ending in `/`) it owns, relative to ui/src. */
  owns: string[];
}

export const STORE_UI: StoreUi[] = [
  { id: "proxmox", entry: "lib/views/Proxmox.svelte", owns: ["lib/views/Proxmox.svelte", "lib/components/proxmox/"] },
  { id: "desktops", entry: "lib/views/Desktops.svelte", owns: ["lib/views/Desktops.svelte", "lib/components/desktop/DesktopScreen.svelte"] },
  { id: "security", entry: "lib/views/Security.svelte", owns: ["lib/views/Security.svelte", "lib/components/security/", "lib/stores/security.svelte.ts"] },
  { id: "netmap", entry: "lib/views/Network.svelte", owns: ["lib/views/Network.svelte", "lib/components/TopoMap.svelte"] },
  { id: "automation", entry: "lib/views/Automation.svelte", owns: ["lib/views/Automation.svelte"] },
];

/** Svelte's runtime entry points a compiled component may import. */
export const SVELTE_ENTRIES = [
  "svelte",
  "svelte/internal/client",
  "svelte/internal/disclose-version",
  "svelte/internal/flags/legacy",
  "svelte/internal/flags/async",
  "svelte/internal/flags/tracing",
  "svelte/store",
  "svelte/reactivity",
  "svelte/reactivity/window",
  "svelte/transition",
  "svelte/animate",
  "svelte/easing",
  "svelte/motion",
  "svelte/events",
  "svelte/attachments",
];

/** Whether `rel` (relative to ui/src, forward slashes) is owned by `c`. */
export function owns(c: StoreUi, rel: string): boolean {
  return c.owns.some((o) => (o.endsWith("/") ? rel.startsWith(o) : rel === o));
}

export function ownerOfFile(rel: string): StoreUi | undefined {
  return STORE_UI.find((c) => owns(c, rel));
}

/** Static and dynamic import specifiers in a source file (script blocks of .svelte included). */
export function importsOf(code: string): string[] {
  const out = new Set<string>();
  const re = /(?:^|[\s;}])(?:import|export)\s+(?:type\s+)?(?:[\w*{}\s,$]+\s+from\s+)?["']([^"']+)["']|import\(\s*["']([^"']+)["']\s*\)/g;
  for (const m of code.matchAll(re)) {
    const spec = m[1] ?? m[2];
    if (spec && !/^import\s+type\b/.test(m[0].trim())) out.add(spec);
  }
  // Type-only imports never reach the bundle.
  for (const m of code.matchAll(/import\s+type\s+[\w*{}\s,$]+\s+from\s+["']([^"']+)["']/g)) out.delete(m[1]);
  return [...out];
}

/**
 * The stable path a host module is served at: `host/<path under ui/src>.js`
 * for the app's own files, `host/npm/<package path>.js` for dependencies.
 * Null for anything that can't be shared (virtual modules, queries).
 */
export function hostPath(absId: string, uiSrc: string): string | null {
  if (absId.startsWith("\0") || absId.includes("?")) return null;
  const id = absId.replace(/\\/g, "/");
  const src = uiSrc.replace(/\\/g, "/").replace(/\/$/, "") + "/";
  const nm = id.lastIndexOf("/node_modules/");
  let rel: string;
  if (nm >= 0) rel = "npm/" + id.slice(nm + "/node_modules/".length);
  else if (id.startsWith(src)) rel = id.slice(src.length);
  else return null;
  return "host/" + rel.replace(/\.(ts|js|mjs|svelte\.ts|svelte\.js)$/, (m) => (m.startsWith(".svelte") ? ".svelte" : "")) + ".js";
}

/**
 * A hash per command in `bindings.ts`: its call line plus every type it
 * names, followed through the types those name. A pack records the hashes
 * of the commands its files call; the app refuses it when any differs, so a
 * pack never calls a command whose arguments or result changed shape.
 */
export function commandHashes(bindings: string, hash: (s: string) => string): Record<string, string> {
  const text = bindings.replace(/\r\n/g, "\n");
  const types = new Map<string, string>();
  for (const m of text.matchAll(/^export type (\w+)\b[^\n]*(?:\n(?!export |\n)[^\n]*)*/gm)) types.set(m[1], m[0]);
  const closure = (start: string) => {
    const seen = new Set<string>();
    const todo = [start];
    const parts: string[] = [];
    while (todo.length) {
      const s = todo.pop()!;
      for (const w of s.match(/\b[A-Z]\w*\b/g) ?? []) {
        if (seen.has(w) || !types.has(w)) continue;
        seen.add(w);
        const def = types.get(w)!;
        parts.push(def);
        todo.push(def);
      }
    }
    return parts.sort().join("\n");
  };
  const out: Record<string, string> = {};
  for (const m of text.matchAll(/^\t(\w+): \(.*__TAURI_INVOKE<.*$/gm)) out[m[1]] = hash(m[0] + "\n" + closure(m[0]));
  return out;
}

/** `api.fooBar(` and `commands.fooBar(` calls in a file. */
export function callsOf(code: string): string[] {
  return [...new Set([...code.matchAll(/\b(?:api|commands)\.(\w+)\s*\(/g)].map((m) => m[1]))];
}

/**
 * The host modules a built pack file imports and the names it takes from
 * each (`default` for a default import, `*` for a namespace import or a bare
 * side-effect import, which need the module but no particular name).
 */
export function hostImports(code: string, prefix: string): Map<string, Set<string>> {
  const out = new Map<string, Set<string>>();
  const add = (path: string, names: string[]) => {
    const hp = path.slice(prefix.length);
    const set = out.get(hp) ?? new Set<string>();
    for (const n of names) set.add(n);
    out.set(hp, set);
  };
  const esc = prefix.replace(/[.*+?^${}()|[\]\\/]/g, "\\$&");
  const re = new RegExp(`(?:import|export)\\s*([^"';]*?)\\s*(?:from\\s*)?["'](${esc}host/[^"']+)["']`, "g");
  for (const m of code.matchAll(re)) {
    const clause = m[1].trim();
    const names: string[] = [];
    if (!clause || clause.startsWith("*")) names.push("*");
    else {
      const braces = /\{([^}]*)\}/.exec(clause);
      const before = clause.split("{")[0].replace(/,\s*$/, "").trim();
      if (before && !before.startsWith("*")) names.push("default");
      if (before.includes("*")) names.push("*");
      for (const part of braces?.[1].split(",") ?? []) {
        const n = part.trim().split(/\s+as\s+/)[0].trim();
        if (n) names.push(n);
      }
    }
    add(m[2], names);
  }
  return out;
}
