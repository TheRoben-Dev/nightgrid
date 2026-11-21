#!/usr/bin/env node
// Packs built store components (feature store part 2, D-207) for the rolling
// `components-latest` release. Usage:
//   node scripts/component-pack.mjs <built-dir> <out-dir> [published-index.json]
// <built-dir> is what ui/tools/buildComponents.ts wrote (<id>/manifest.json,
// <id>/files/…). A component is packed only when its build is newer than the
// one in the published index (its files changed on main since); the others
// keep their published entry. Writes <out>/<id>.json.gz for each new pack
// (CI signs them) and <out>/index.json for all, plus <out>/packed.txt.
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import process from "node:process";
import { gzipSync } from "node:zlib";

const [built, out, publishedPath] = process.argv.slice(2);
if (!built || !out) {
  console.error("usage: component-pack.mjs <built-dir> <out-dir> [published-index.json]");
  process.exit(2);
}
mkdirSync(out, { recursive: true });

const MAX_CONTENT = 16 * 1024 * 1024;
const git = (...a) => {
  try {
    return execFileSync("git", a, { encoding: "utf8" }).trim();
  } catch {
    return "";
  }
};
const version = /^\[workspace\.package\][^[]*?^version\s*=\s*"([^"]+)"/ms.exec(readFileSync("Cargo.toml", "utf8"))?.[1];
if (!version) throw new Error("no workspace version in Cargo.toml");

let published = { format: 1, components: [] };
if (publishedPath && existsSync(publishedPath)) {
  try {
    const p = JSON.parse(readFileSync(publishedPath, "utf8"));
    if (p.format === 1 && Array.isArray(p.components)) published = p;
  } catch {
    console.warn("published index unreadable: packing everything");
  }
}
const before = new Map(published.components.map((c) => [c.id, c]));

const walk = (d) => readdirSync(d).flatMap((n) => (statSync(join(d, n)).isDirectory() ? walk(join(d, n)) : [join(d, n)]));
// The owned paths of each component, for its notes (ui/tools/storeUi.ts, one entry per line).
const owns = {};
for (const line of readFileSync("ui/tools/storeUi.ts", "utf8").split("\n")) {
  const m = /^\s*\{ id: "([a-z0-9-]+)", entry: "[^"]+", owns: \[([^\]]*)\] \},?$/.exec(line);
  if (m) owns[m[1]] = [...m[2].matchAll(/"([^"]+)"/g)].map((x) => `ui/src/${x[1]}`);
}

const index = { format: 1, components: [] };
const packed = [];
for (const id of readdirSync(built).sort()) {
  const mf = join(built, id, "manifest.json");
  if (!existsSync(mf)) continue;
  const m = JSON.parse(readFileSync(mf, "utf8"));
  const prev = before.get(id);
  if (prev && prev.built >= m.built) {
    index.components.push(prev);
    continue;
  }
  const files = {};
  let bytes = 0;
  for (const f of walk(join(built, id, "files"))) {
    const data = readFileSync(f);
    bytes += data.length;
    files[relative(join(built, id, "files"), f).replace(/\\/g, "/")] = data.toString("base64");
  }
  if (bytes > MAX_CONTENT) throw new Error(`${id} is ${bytes} bytes, over the ${MAX_CONTENT} ceiling`);
  const since = prev ? [`--since=@${prev.built + 1}`] : ["-5"];
  const notes = git("log", "--no-merges", "--format=%s", ...since, "--", ...(owns[id] ?? []))
    .split("\n")
    .filter(Boolean)
    .slice(0, 5);
  const pack = {
    format: 1,
    id,
    built: m.built,
    minApp: version,
    commit: m.commit,
    notes,
    entry: m.entry,
    style: !!m.style && "style.css" in files,
    requires: m.requires,
    files,
  };
  const gz = gzipSync(JSON.stringify(pack), { level: 9 });
  writeFileSync(join(out, `${id}.json.gz`), gz);
  index.components.push({ id, built: m.built, minApp: version, bytes: gz.length });
  packed.push(id);
  console.log(`${id}: packed build ${m.built} (${gz.length} bytes gzipped)${prev ? `, was ${prev.built}` : ""}`);
}
// Components no longer built (removed from the store) drop out of the index.
writeFileSync(join(out, "index.json"), JSON.stringify(index, null, 1));
writeFileSync(join(out, "packed.txt"), packed.join("\n") + (packed.length ? "\n" : ""));
if (!packed.length) console.log("nothing newer than what's published");
