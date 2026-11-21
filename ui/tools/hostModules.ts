/**
 * Host modules (feature store part 2): every module a store component's own
 * files import from the rest of the app is emitted at a stable path
 * (`host/…`, see storeUi.ts `hostPath`) with all its exports kept, and
 * `host/manifest.json` lists them. A component pack built later imports
 * exactly those paths, so it runs on the app's single copy of Svelte, the
 * stores and the shared components instead of bringing its own.
 *
 * The manifest also carries the build time and a hash per command (see
 * `commandHashes`), which the loader checks a pack's requirements against.
 */
import { createHash } from "node:crypto";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import type { Plugin } from "vite";
import { STORE_UI, SVELTE_ENTRIES, commandHashes, hostPath, importsOf, ownerOfFile } from "./storeUi.ts";

export const sha256 = (s: string) => createHash("sha256").update(s).digest("hex");

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    return statSync(p).isDirectory() ? walk(p) : [p];
  });
}

/** Every file a component owns, as absolute paths (tests left out). */
export function ownedFiles(uiSrc: string): string[] {
  const out: string[] = [];
  for (const c of STORE_UI) {
    for (const o of c.owns) {
      const p = join(uiSrc, o);
      for (const f of o.endsWith("/") ? walk(p) : [p]) if (!/\.test\.ts$/.test(f)) out.push(f);
    }
  }
  return out;
}

export const relSrc = (uiSrc: string, abs: string) => relative(uiSrc, abs).replace(/\\/g, "/");

interface Resolver {
  resolve(source: string, importer?: string): Promise<{ id: string; external?: boolean | string } | null>;
}

/** Absolute ids of the host modules (shared by the app build and component builds). */
export async function hostModuleIds(ctx: Resolver, uiSrc: string): Promise<string[]> {
  const ids = new Set<string>();
  const main = join(uiSrc, "main.ts");
  for (const s of SVELTE_ENTRIES) {
    const r = await ctx.resolve(s, main);
    if (r && !r.external) ids.add(r.id);
  }
  for (const file of ownedFiles(uiSrc)) {
    const me = ownerOfFile(relSrc(uiSrc, file));
    for (const spec of importsOf(readFileSync(file, "utf8"))) {
      const r = await ctx.resolve(spec, file);
      if (!r || r.external || !hostPath(r.id, uiSrc)) continue;
      const rel = relSrc(uiSrc, r.id);
      if (!rel.startsWith("..") && ownerOfFile(rel) === me) continue;
      ids.add(r.id);
    }
  }
  return [...ids].sort();
}

export function hostModules(opts: { uiSrc: string; built: number; svelte: string }): Plugin {
  let emitted: Map<string, string> = new Map();
  return {
    name: "nightgrid-host-modules",
    apply: "build",
    async buildStart() {
      emitted = new Map();
      for (const id of await hostModuleIds(this, opts.uiSrc)) {
        const fileName = hostPath(id, opts.uiSrc)!;
        emitted.set(fileName, id);
        this.emitFile({ type: "chunk", id, fileName, preserveSignature: "strict" });
      }
    },
    generateBundle(_o, bundle) {
      const modules: Record<string, string[]> = {};
      for (const fileName of emitted.keys()) {
        const chunk = bundle[fileName];
        if (chunk?.type === "chunk") modules[fileName] = [...chunk.exports].sort();
      }
      const bindings = readFileSync(join(opts.uiSrc, "lib/bindings.ts"), "utf8");
      const manifest = { format: 1, built: opts.built, svelte: opts.svelte, modules, commands: commandHashes(bindings, sha256) };
      this.emitFile({ type: "asset", fileName: "host/manifest.json", source: JSON.stringify(manifest) });
    },
  };
}
