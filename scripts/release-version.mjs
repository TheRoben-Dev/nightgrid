#!/usr/bin/env node
// Picks the next release version for the release train (D-185) and writes it.
// PRs don't bump versions any more: they add notes under `## [Unreleased]` in
// CHANGELOG.md, and the train calls this once main has been quiet.
//
//   node scripts/release-version.mjs            decide, write files, print JSON
//   node scripts/release-version.mjs --dry-run  decide only
//
// The decision, against the newest `v*` tag:
// - only interface/docs/CI files changed (or nothing) → no release (interface
//   fixes ship as signed patches, Library content as packs);
// - main's version is already above the tag (a PR bumped by hand) → release it;
// - else bump: minor when the Unreleased notes have an "### Added" heading,
//   patch otherwise.
// The Unreleased heading becomes `## [X.Y.Z] - YYYY-MM-DD`.
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import process from "node:process";

const dry = process.argv.includes("--dry-run");
const git = (...a) => execFileSync("git", a, { encoding: "utf8" }).trim();

/** Paths that never need an app release (same list as ui-patch.yml). */
export const PATCHABLE = /^(ui\/|docs\/|scripts\/|\.github\/|[^/]+\.md$|vite\.config\.ts$|svelte\.config\.js$|eslint\.config\.js$|tsconfig[^/]*\.json$)/;

const parse = (v) => v.replace(/^v/, "").split(/[-+]/)[0].split(".").map(Number);
const cmp = (a, b) => {
  const [x, y] = [parse(a), parse(b)];
  for (let i = 0; i < 3; i++) if ((x[i] || 0) !== (y[i] || 0)) return (x[i] || 0) - (y[i] || 0);
  return 0;
};

export function nextVersion(last, current, unreleased) {
  if (cmp(current, last) > 0) return current;
  const [ma, mi, pa] = parse(last);
  return /^###\s+Added/m.test(unreleased) ? `${ma}.${mi + 1}.0` : `${ma}.${mi}.${pa + 1}`;
}

const cargo = readFileSync("Cargo.toml", "utf8");
const VERSION_RE = /^(\[workspace\.package\][\s\S]*?^version\s*=\s*")([^"]+)(")/m;
const current = VERSION_RE.exec(cargo)?.[2];
if (!current) throw new Error("no workspace version in Cargo.toml");

let lastTag = "";
try {
  lastTag = git("describe", "--tags", "--abbrev=0", "--match", "v[0-9]*");
} catch {
  /* no release yet */
}
const last = lastTag ? lastTag.slice(1) : "0.0.0";

const changed = lastTag ? git("diff", "--name-only", lastTag, "HEAD").split("\n").filter(Boolean) : ["(first release)"];
const appChanges = changed.filter((p) => !PATCHABLE.test(p));
const changelog = readFileSync("CHANGELOG.md", "utf8");
const UNRELEASED_RE = /^## \[Unreleased\][^\n]*\n([\s\S]*?)(?=^## \[|(?![\s\S]))/m;
const unreleased = UNRELEASED_RE.exec(changelog)?.[1] ?? "";

const bumpedByHand = cmp(current, last) > 0;
if (!appChanges.length && !bumpedByHand) {
  console.log(JSON.stringify({ release: false, last, reason: "no app changes since the last release" }));
  process.exit(0);
}

const version = nextVersion(last, current, unreleased);
const tag = `v${version}`;
if (!dry) {
  writeFileSync("Cargo.toml", cargo.replace(VERSION_RE, `$1${version}$3`));
  const pkg = readFileSync("package.json", "utf8");
  writeFileSync("package.json", pkg.replace(/("version":\s*")[^"]+(")/, `$1${version}$2`));
  let lock = readFileSync("Cargo.lock", "utf8");
  for (const name of ["nightgrid", "nightgrid-core", "nightgrid-llm"]) {
    lock = lock.replace(new RegExp(`(name = "${name}"\\nversion = ")[^"]+(")`), `$1${version}$2`);
  }
  writeFileSync("Cargo.lock", lock);
  const date = new Date().toISOString().slice(0, 10);
  if (UNRELEASED_RE.test(changelog)) {
    writeFileSync("CHANGELOG.md", changelog.replace(/^## \[Unreleased\][^\n]*$/m, `## [${version}] - ${date}`));
  }
}
console.log(JSON.stringify({ release: true, last, version, tag, changed: appChanges.length, bumpedByHand }));
