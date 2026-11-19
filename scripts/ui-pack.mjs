#!/usr/bin/env node
// Builds a signed-interface patch (D-183): the built frontend (dist/) as
// gzipped JSON, pinned to the app version in Cargo.toml and the hash of the
// typed command interface (ui/src/lib/bindings.ts). CI signs the .gz with the
// release key and publishes it to the rolling `ui-latest` release. Usage:
//   node scripts/ui-pack.mjs <dist-dir> <out-dir>
// UI_BUILT, UI_APP_VERSION and UI_API_HASH override the pinned values (tests).
// UI_NOTES: newline-separated notes shown in Settings.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, relative, sep } from "node:path";
import process from "node:process";
import { gzipSync } from "node:zlib";

const [dist, out] = process.argv.slice(2);
if (!dist || !out) {
  console.error("usage: ui-pack.mjs <dist-dir> <out-dir>");
  process.exit(2);
}

/** Same rule as core/src/uipack.rs `path_ok`. */
const SEG = /^[A-Za-z0-9\-_.@~]+$/;
const pathOk = (p) => p.length <= 200 && p.split("/").length <= 6 && p.split("/").every((s) => SEG.test(s) && !s.startsWith("."));

function walk(dir) {
  return readdirSync(dir).flatMap((n) => {
    const p = join(dir, n);
    return statSync(p).isDirectory() ? walk(p) : [p];
  });
}

export function apiHash(text) {
  return createHash("sha256").update(text.replace(/\r\n/g, "\n")).digest("hex");
}

const git = (...args) => {
  try {
    return execFileSync("git", args, { encoding: "utf8" }).trim();
  } catch {
    return "";
  }
};

const files = {};
let bytes = 0;
for (const f of walk(dist).sort()) {
  const rel = relative(dist, f).split(sep).join("/");
  if (rel.endsWith(".map")) continue;
  if (!pathOk(rel)) throw new Error(`can't pack ${rel}: name not allowed`);
  const data = readFileSync(f);
  files[rel] = data.toString("base64");
  bytes += data.length;
}
if (!files["index.html"]) throw new Error(`${dist} has no index.html`);

const cargo = readFileSync("Cargo.toml", "utf8");
const appVersion = process.env.UI_APP_VERSION || /^\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m.exec(cargo)?.[1];
if (!appVersion) throw new Error("no workspace version in Cargo.toml");
const hash = process.env.UI_API_HASH || apiHash(readFileSync("ui/src/lib/bindings.ts", "utf8"));
const built = Number(process.env.UI_BUILT || git("log", "-1", "--format=%ct"));
if (!Number.isInteger(built) || built <= 0) throw new Error("no build time");
const commit = git("rev-parse", "--short=12", "HEAD");
const notes = (process.env.UI_NOTES || "").split("\n").map((s) => s.trim()).filter(Boolean).slice(0, 20);

const pack = { format: 1, appVersion, apiHash: hash, built, commit, notes, files };
const gz = gzipSync(JSON.stringify(pack), { level: 9 });
mkdirSync(out, { recursive: true });
writeFileSync(join(out, "ui-pack.json.gz"), gz);
writeFileSync(join(out, "ui-pack.meta.json"), `${JSON.stringify({ format: 1, appVersion, apiHash: hash, built, bytes: gz.length })}\n`);
console.log(`ui pack for ${appVersion} (api ${hash.slice(0, 12)}, built ${built}): ${Object.keys(files).length} files, ${bytes} bytes → ${gz.length} gzipped`);
