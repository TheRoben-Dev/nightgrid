import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { STORE_UI, commandHashes, hostImports, hostPath, importsOf, ownerOfFile } from "./storeUi";

const SRC = resolve(__dirname, "../src");
const walk = (d: string): string[] => readdirSync(d).flatMap((n) => (statSync(join(d, n)).isDirectory() ? walk(join(d, n)) : [join(d, n)]));
const rel = (abs: string) => relative(SRC, abs).replace(/\\/g, "/");

describe("store component ownership", () => {
  it("components exist and own their entry", () => {
    for (const c of STORE_UI) {
      expect(ownerOfFile(c.entry)?.id, c.id).toBe(c.id);
      for (const o of c.owns) expect(statSync(join(SRC, o)), o).toBeTruthy();
    }
    const catalog = readFileSync(join(SRC, "lib/storeCatalog.ts"), "utf8");
    for (const c of STORE_UI) expect(catalog, c.id).toContain(`id: "${c.id}"`);
  });

  it("nothing outside a component imports its files (views.ts loads entries lazily)", () => {
    const bad: string[] = [];
    for (const file of walk(SRC).filter((f) => /\.(ts|svelte)$/.test(f) && !f.endsWith(".test.ts"))) {
      const me = ownerOfFile(rel(file));
      const code = readFileSync(file, "utf8");
      for (const spec of importsOf(code)) {
        if (!spec.startsWith(".")) continue;
        const target = rel(resolve(dirname(file), spec));
        const owner = STORE_UI.find((c) => c.owns.some((o) => (o.endsWith("/") ? target.startsWith(o) : target === o || `${target}.ts` === o)));
        if (!owner || owner === me) continue;
        if (rel(file) === "lib/views.ts" && target === owner.entry) continue;
        bad.push(`${rel(file)} → ${target}`);
      }
    }
    expect(bad).toEqual([]);
  });
});

describe("pack helpers", () => {
  it("names host modules stably", () => {
    expect(hostPath("/r/ui/src/lib/stores/hosts.svelte.ts", "/r/ui/src")).toBe("host/lib/stores/hosts.svelte.js");
    expect(hostPath("/r/ui/src/lib/components/Panel.svelte", "/r/ui/src")).toBe("host/lib/components/Panel.svelte.js");
    expect(hostPath("/r/ui/src/lib/api.ts", "/r/ui/src/")).toBe("host/lib/api.js");
    expect(hostPath("/r/node_modules/.pnpm/svelte@5/node_modules/svelte/src/index-client.js", "/r/ui/src")).toBe("host/npm/svelte/src/index-client.js");
    expect(hostPath("\0virtual", "/r/ui/src")).toBeNull();
    expect(hostPath("/r/ui/src/a.svelte?svelte&type=style", "/r/ui/src")).toBeNull();
    expect(hostPath("/elsewhere/x.ts", "/r/ui/src")).toBeNull();
  });

  it("finds imports but not type-only ones", () => {
    const code = `import { a } from "./a";\nimport type { T } from "./t";\nimport B from "../b.svelte";\nconst m = import("./lazy");\nexport { c } from "./c";`;
    expect(importsOf(code).sort()).toEqual(["../b.svelte", "./a", "./c", "./lazy"]);
  });

  it("reads what a built file takes from the host", () => {
    const code = `import "../../../host/x.js";import * as e from "../../../host/svelte.js";import d, { a as b, c } from "../../../host/m.js";import { f } from "./chunk.js";`;
    const m = hostImports(code, "../../../");
    expect([...m.get("host/x.js")!]).toEqual(["*"]);
    expect([...m.get("host/svelte.js")!]).toEqual(["*"]);
    expect([...m.get("host/m.js")!].sort()).toEqual(["a", "c", "default"]);
    expect(m.size).toBe(3);
  });

  it("hashes a command with the types it uses", () => {
    const b = (shape: string) =>
      `export const commands = {\n\tfoo: (x: Arg, options?: InvokeOptions) => __TAURI_INVOKE<Out>("foo", { x }, options),\n\tbar: (options?: InvokeOptions) => __TAURI_INVOKE<string>("bar", undefined, options),\n};\n\nexport type Arg = { a: number };\n\nexport type Out = { inner: Inner };\n\nexport type Inner = ${shape};\n`;
    const h = (s: string) => String(s.length) + s.slice(-20);
    const one = commandHashes(b("{ x: string }"), h);
    const two = commandHashes(b("{ x: number, y: string }"), h);
    expect(Object.keys(one).sort()).toEqual(["bar", "foo"]);
    expect(one.foo).not.toBe(two.foo);
    expect(one.bar).toBe(two.bar);
  });
});
