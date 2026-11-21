/**
 * Builds store components' interfaces as separate packs (feature store
 * part 2): `node ui/tools/buildComponents.ts [outDir] [id…]`.
 *
 * Each component's entry is bundled with only its own files; every import
 * of a host module (see hostModules.ts) is left as an import of the host's
 * stable path, relative to where the pack is served
 * (`/components/<id>/<built>/…`). Writes `<outDir>/<id>/files/…` and
 * `<outDir>/<id>/manifest.json` with what the pack needs from the app: the
 * host modules and names it imports, the commands it calls (with their
 * hashes) and the Svelte version it was compiled with.
 */
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { build, type Plugin, type Rolldown } from "vite";
import { freezeSafe } from "./freezeSafe.ts";
import { hostModuleIds, ownedFiles, relSrc, sha256 } from "./hostModules.ts";
import { STORE_UI, callsOf, commandHashes, hostImports, hostPath, ownerOfFile, owns, type StoreUi } from "./storeUi.ts";

const root = fileURLToPath(new URL("../..", import.meta.url));
const uiSrc = join(root, "ui/src");
const git = (...a: string[]) => {
  try {
    return execFileSync("git", a, { cwd: root, encoding: "utf8" }).trim();
  } catch {
    return "";
  }
};

/** Prefix from a pack's files back to the app root (`components/<id>/<built>/`). */
const UP = "../../../";

/** The time of the newest commit touching a component's files (its version). */
export function componentBuilt(c: StoreUi): number {
  return Number(git("log", "-1", "--format=%ct", "--", ...c.owns.map((o) => join("ui/src", o)))) || 0;
}

function hostExternals(c: StoreUi, hostIds: Set<string>, used: Map<string, Set<string>>): Plugin {
  return {
    name: "nightgrid-component-externals",
    enforce: "pre",
    async resolveId(source, importer, options) {
      if (!importer || source.startsWith(UP)) return null;
      const r = await this.resolve(source, importer, { ...options, skipSelf: true });
      if (!r || r.external) return r;
      const rel = relSrc(uiSrc, r.id);
      if (!rel.startsWith("..") && owns(c, rel)) return r;
      const hp = hostPath(r.id, uiSrc);
      if (!hp || !hostIds.has(r.id)) {
        // A virtual module or one only this component reaches: bundle it.
        if (hp && !rel.startsWith("..") && ownerOfFile(rel)) throw new Error(`${c.id} imports ${rel}, owned by another component`);
        return r;
      }
      if (!used.has(hp)) used.set(hp, new Set());
      return { id: UP + hp, external: true };
    },
  };
}

async function buildOne(c: StoreUi, outDir: string, hostIds: Set<string>, svelteVersion: string) {
  const used = new Map<string, Set<string>>();
  const dir = join(outDir, c.id);
  rmSync(dir, { recursive: true, force: true });
  const out = await build({
    configFile: false,
    root: join(root, "ui"),
    logLevel: "warn",
    plugins: [hostExternals(c, hostIds, used), freezeSafe(), svelte({ configFile: join(root, "svelte.config.js") })],
    define: { __KB_VERSION__: "0", __UI_BUILT__: "0" },
    build: {
      outDir: join(dir, "files"),
      emptyOutDir: true,
      target: "safari15",
      minify: "oxc",
      cssCodeSplit: false,
      copyPublicDir: false,
      lib: { entry: join(uiSrc, c.entry), formats: ["es"], fileName: () => "entry.js", cssFileName: "style" },
      rolldownOptions: { output: { chunkFileNames: "chunk-[hash].js", assetFileNames: "[name][extname]" } },
    },
  });
  const outputs = (Array.isArray(out) ? out : [out]) as Rolldown.RolldownOutput[];
  for (const o of outputs) {
    for (const item of o.output) {
      if (item.type !== "chunk") continue;
      for (const [hp, names] of hostImports(item.code, UP)) {
        const set = used.get(hp) ?? new Set<string>();
        for (const n of names) set.add(n);
        used.set(hp, set);
      }
    }
  }
  const bindings = readFileSync(join(uiSrc, "lib/bindings.ts"), "utf8");
  const all = commandHashes(bindings, sha256);
  const calls = new Set(ownedFiles(uiSrc).filter((f) => owns(c, relSrc(uiSrc, f))).flatMap((f) => callsOf(readFileSync(f, "utf8"))));
  const commands = Object.fromEntries([...calls].filter((n) => all[n]).sort().map((n) => [n, all[n]]));
  const manifest = {
    format: 1,
    id: c.id,
    built: componentBuilt(c),
    commit: git("rev-parse", "--short", "HEAD"),
    entry: "entry.js",
    style: true,
    requires: {
      svelte: svelteVersion,
      modules: Object.fromEntries([...used].sort().map(([k, v]) => [k, [...v].sort()])),
      commands,
    },
  };
  writeFileSync(join(dir, "manifest.json"), JSON.stringify(manifest, null, 1));
  return manifest;
}

async function main() {
  const [outArg, ...ids] = process.argv.slice(2);
  const outDir = resolve(outArg ?? join(root, "dist-components"));
  mkdirSync(outDir, { recursive: true });
  const svelteVersion = (JSON.parse(readFileSync(join(root, "node_modules/svelte/package.json"), "utf8")) as { version: string }).version;
  // The same resolution the app build uses, through a throwaway Vite resolver.
  let hostIds = new Set<string>();
  await build({
    configFile: false,
    root: join(root, "ui"),
    logLevel: "silent",
    plugins: [
      {
        name: "collect-host-ids",
        enforce: "pre",
        async buildStart() {
          hostIds = new Set(await hostModuleIds(this, uiSrc));
        },
        resolveId(id) {
          return id.endsWith("virtual:empty") ? "\0empty" : null;
        },
        load(id) {
          return id === "\0empty" ? "export {}" : null;
        },
      },
    ],
    build: { write: false, lib: { entry: "virtual:empty", formats: ["es"], fileName: "x" } },
  });
  for (const c of STORE_UI.filter((c) => !ids.length || ids.includes(c.id))) {
    const m = await buildOne(c, outDir, hostIds, svelteVersion);
    console.log(`${c.id}: built ${m.built}, ${Object.keys(m.requires.modules).length} host modules, ${Object.keys(m.requires.commands).length} commands`);
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();
