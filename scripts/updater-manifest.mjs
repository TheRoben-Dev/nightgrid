#!/usr/bin/env node
// Builds latest.json for the Tauri updater from the signed bundles in a
// release's artifact folder. Usage:
//   node scripts/updater-manifest.mjs <dist-dir> <tag> <out-file>
// Exits 0 without writing anything when there are no .sig files (unsigned
// build: the release goes out without self-update support).
import { readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import process from "node:process";
import { notesFor, releasesIn } from "./codenames.mjs";

const [dist, tag, out] = process.argv.slice(2);
if (!dist || !tag || !out) {
  console.error("usage: updater-manifest.mjs <dist-dir> <tag> <out-file>");
  process.exit(2);
}
const version = tag.replace(/^v/, "");
const REPO = process.env.GITHUB_REPOSITORY ?? "TheRoben-Dev/nightgrid";

const walk = (d) => readdirSync(d).flatMap((n) => (statSync(join(d, n)).isDirectory() ? walk(join(d, n)) : [join(d, n)]));
const files = walk(dist);

// Updater bundle → manifest keys. Specific `{os}-{arch}-{bundle}` keys first,
// plain `{os}-{arch}` as a fallback for macOS/Windows; Linux only updates the
// AppImage (deb/rpm are left to the package manager).
const KINDS = [
  { test: (f) => f.endsWith(".app.tar.gz"), keys: ["darwin-aarch64-app", "darwin-aarch64"] },
  { test: (f) => /-setup\.exe$/.test(f), keys: ["windows-x86_64-nsis", "windows-x86_64"] },
  { test: (f) => f.endsWith(".AppImage"), keys: ["linux-x86_64-appimage"] },
];

const platforms = {};
for (const f of files) {
  const kind = KINDS.find((k) => k.test(f));
  if (!kind) continue;
  const sigFile = files.find((s) => s === `${f}.sig`);
  if (!sigFile) continue;
  const entry = {
    signature: readFileSync(sigFile, "utf8").trim(),
    url: `https://github.com/${REPO}/releases/download/${tag}/${encodeURIComponent(basename(f))}`,
  };
  for (const k of kind.keys) platforms[k] = entry;
}

if (!Object.keys(platforms).length) {
  console.log("No signed updater bundles found; skipping latest.json.");
  process.exit(0);
}

// Release notes: this version's CHANGELOG section, as plain text, led by
// its codename (D-204) so the update prompt can show it.
let notes = `Nightgrid ${version}`;
try {
  const log = readFileSync("CHANGELOG.md", "utf8");
  const body = notesFor(log, version);
  const name = releasesIn(log).find((r) => r.version === version)?.codename;
  notes = `${name ? `Codename: ${name}\n\n` : ""}${body || notes}`;
} catch {
  /* no changelog: keep the default line */
}

const manifest = { version, notes, pub_date: new Date().toISOString(), platforms };
writeFileSync(out, JSON.stringify(manifest, null, 2) + "\n");
console.log(`Wrote ${out} for ${version}: ${Object.keys(platforms).join(", ")}`);
