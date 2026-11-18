#!/usr/bin/env node
// Builds the Library content pack (D-174): docs/kb articles and figures plus
// their catalog lines, as gzipped JSON, with a small meta file beside it.
// CI signs the .gz with the release key and publishes both to the rolling
// `kb-latest` release. Usage:
//   node scripts/kb-pack.mjs <out-dir>
// KB_VERSION overrides the version (default: the content's last commit time).
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { gzipSync } from "node:zlib";

const out = process.argv[2];
if (!out) {
  console.error("usage: kb-pack.mjs <out-dir>");
  process.exit(2);
}

const SHELVES = ["basics", "guides", "howto", "apps", "linux", "errors"];
const KB = "docs/kb";
const MAX_CONTENT = 25 * 1024 * 1024;

/** The content version: the newest commit touching the Library (same rule as vite.config.ts). */
function kbVersion() {
  if (process.env.KB_VERSION) return Number(process.env.KB_VERSION);
  const t = execFileSync("git", ["log", "-1", "--format=%ct", "--", "docs/kb", "ui/src/lib/kb/catalog.ts"], { encoding: "utf8" }).trim();
  return Number(t || 0);
}

const files = {};
let bytes = 0;
for (const shelf of SHELVES) {
  for (const n of readdirSync(join(KB, shelf)).sort()) {
    if (!/^[\w-]+\.md$/.test(n) || /^readme\.md$/i.test(n)) continue;
    const text = readFileSync(join(KB, shelf, n), "utf8");
    files[`${shelf}/${n}`] = text;
    bytes += text.length;
  }
}
for (const n of readdirSync(join(KB, "figures")).sort()) {
  if (!/^[a-z0-9-]+\.svg$/.test(n)) continue;
  const text = readFileSync(join(KB, "figures", n), "utf8");
  files[`figures/${n}`] = text;
  bytes += text.length;
}
if (bytes > MAX_CONTENT) throw new Error(`Library is ${bytes} bytes, over the 25 MB ceiling`);

// Catalog lines for the packed shelves, read from catalog.ts (one entry per line).
const LINE = /^\s*\{ id: ("[^"]*"), shelf: ("[a-z]+"), title: ("(?:[^"\\]|\\.)*"), cat: ("[a-z]+") \},?$/;
const catalog = [];
for (const line of readFileSync("ui/src/lib/kb/catalog.ts", "utf8").split("\n")) {
  const m = LINE.exec(line);
  if (!m) continue;
  const [id, shelf, title, cat] = m.slice(1).map((s) => JSON.parse(s));
  if (SHELVES.includes(shelf)) catalog.push({ id, shelf, title, cat });
}
const articles = Object.keys(files).filter((k) => k.endsWith(".md"));
const listed = new Set(catalog.map((c) => `${c.id}.md`));
const missing = articles.filter((a) => !listed.has(a));
const orphan = [...listed].filter((a) => !(a in files));
if (missing.length || orphan.length) throw new Error(`catalog out of step: missing ${missing.join(", ") || "-"}; no file for ${orphan.join(", ") || "-"}`);

const cargo = readFileSync("Cargo.toml", "utf8");
const minApp = /^\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m.exec(cargo)?.[1];
if (!minApp) throw new Error("no workspace version in Cargo.toml");
let commit = "";
try {
  commit = execFileSync("git", ["rev-parse", "--short=12", "HEAD"], { encoding: "utf8" }).trim();
} catch {
  /* not a git checkout */
}

const version = kbVersion();
if (!Number.isInteger(version) || version <= 0) throw new Error("no content version");
const pack = { format: 1, version, minApp, commit, catalog, files };
const gz = gzipSync(JSON.stringify(pack), { level: 9 });
mkdirSync(out, { recursive: true });
writeFileSync(join(out, "kb-pack.json.gz"), gz);
writeFileSync(join(out, "kb-pack.meta.json"), `${JSON.stringify({ format: 1, version, minApp, bytes: gz.length })}\n`);
console.log(`kb pack ${version} (app ≥ ${minApp}): ${articles.length} articles, ${Object.keys(files).length - articles.length} figures, ${bytes} bytes → ${gz.length} gzipped`);
