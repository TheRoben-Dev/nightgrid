#!/usr/bin/env node
// Release codenames (D-204). Every release gets a two-word codename on top of
// its version, in the cyberpunk mood but made only of plain English words:
// no names, places, characters, gangs, products or slang from any franchise.
//
// The release train calls `pickCodename` when it names the version
// (scripts/release-version.mjs) and writes it into the CHANGELOG heading:
//
//   ## [1.23.0] "Cobalt Signal" - 2026-11-21
//
// The pick is deterministic (a hash of the version over the combinations
// still free), never repeats a codename, avoids the words of the last twelve
// and gives every word a turn before any comes back. To add words, append to the
// lists below; ui/tools/codenames.test.ts checks them against DENY.
//
//   node scripts/codenames.mjs title v1.23.0    → v1.23.0 · Cobalt Signal
//   node scripts/codenames.mjs notes v1.23.0    → that version's notes (markdown)
//   node scripts/codenames.mjs backfill         → name every unnamed release
import { readFileSync, writeFileSync } from "node:fs";
import process from "node:process";
import { fileURLToPath } from "node:url";

export const ADJECTIVES = [
  "Neon", "Static", "Midnight", "Ultraviolet", "Infrared", "Cobalt", "Velvet", "Hollow",
  "Silent", "Synthetic", "Electric", "Magnetic", "Analog", "Binary", "Quantum", "Fractured",
  "Flickering", "Glitched", "Wired", "Acid", "Feral", "Sodium", "Mercury", "Obsidian",
  "Crimson", "Ionic", "Copper", "Cold", "Hushed", "Rusted", "Pale", "Violet",
  "Tungsten", "Carbon", "Silver", "Lunar", "Polar", "Vapor", "Holographic", "Laser",
  "Plasma", "Overclocked", "Encrypted", "Liquid", "Distant", "Restless", "Sleepless", "Scattered",
  "Humming", "Shattered", "Wireless", "Orbital", "Molten", "Frozen", "Amber", "Indigo",
];

export const NOUNS = [
  "Signal", "Circuit", "Lattice", "Pulse", "Relay", "Cipher", "Vector", "Echo",
  "Mirage", "Spectrum", "Parallax", "Frequency", "Rain", "Haze", "Glow", "Current",
  "Mainframe", "Firewall", "Packet", "Socket", "Kernel", "Daemon", "Syntax", "Voltage",
  "Wavelength", "Transit", "Skyline", "Alley", "Arcade", "Monsoon", "Smog", "Rooftop",
  "Afterglow", "Undertow", "Satellite", "Beacon", "Diode", "Prism", "Mosaic", "Lantern",
  "Reverie", "Paradox", "Monolith", "Cascade", "Tempest", "Synapse", "Helix", "Shard",
  "Ember", "Flux", "Vortex", "Static", "Bandwidth", "Checksum", "Cursor", "Sector",
  "Underpass", "Monorail", "Billboard", "Hologram", "Overpass", "Antenna", "Switchboard", "Neon",
];

/**
 * Words and phrases a codename must never contain: franchise names, places,
 * characters, gangs and slang (Cyberpunk 2077/2020/Red/Edgerunners, Gibson's
 * books, Blade Runner, Ghost in the Shell, Akira, The Matrix, Shadowrun, Deus
 * Ex, Tron, Snow Crash, Altered Carbon, Hackers …), games or films that are
 * just two plain words, and Nightgrid's own names. Matched as whole words,
 * case-insensitive.
 */
export const DENY = [
  // Cyberpunk 2077 / 2020 / Red / Edgerunners
  "cyberpunk", "night city", "netrunner", "netrunning", "braindance", "arasaka", "militech", "kang tao", "zetatech",
  "biotechnica", "edgerunner", "edgerunners", "choom", "choomba", "preem", "gonk", "nova", "delta", "chrome",
  "chromed", "ripperdoc", "cyberpsycho", "cyberpsychosis", "sandevistan", "kerenzikov", "kiroshi", "relic",
  "soulkiller", "blackwall", "afterlife", "watson", "pacifica", "dogtown", "heywood", "westbrook", "badlands",
  "maelstrom", "voodoo", "tyger", "valentinos", "mox", "trauma", "netwatch", "nomad", "nomads", "fixer", "techie",
  "solo", "silverhand", "mantis", "phantom", "liberty", "red", "samurai", "eddies", "flatline", "flatlined",
  "cyberware", "cyberdeck", "deck", "jack", "agent", "construct",
  // Gibson, Blade Runner, Ghost in the Shell, Akira, The Matrix and others
  "neuromancer", "wintermute", "sprawl", "burning", "overdrive", "ice", "cyberspace", "deckard", "tyrell",
  "replicant", "nexus", "blade", "runner", "electric sheep", "electric dreams", "tears", "akira", "tetsuo",
  "ghost", "shell", "major", "matrix", "zion", "neo", "trinity", "morpheus", "tron", "grid", "deus", "shadowrun",
  "metaverse", "snow crash", "altered", "stack", "hackers", "headroom", "uplink", "outrun", "ghostrunner",
  "cloudpunk", "transistor", "dredd", "mega city", "judge", "robocop", "genesis", "neon demon", "midnight rain",
  "static shock", "quantum break", "binary domain", "hard reset", "kill switch", "zero day", "ion storm",
  "cobalt strike", "neon white", "neon abyss", "neon genesis", "velvet underground", "silent hill",
  // Nightgrid's own names
  "nightgrid", "drift",
];

const words = (name) => name.toLowerCase().split(/\s+/);

/** Whether a name contains a denied word or phrase (whole words only). */
export function denied(name) {
  const w = ` ${words(name).join(" ")} `;
  return DENY.some((d) => w.includes(` ${d} `));
}

/** FNV-1a, so the pick for a version never changes between runs or machines. */
function hash(s) {
  let h = 0x811c9dc5;
  for (const c of s) {
    h ^= c.codePointAt(0);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h;
}

let combos = null;
/** Every allowed adjective + noun pair, in list order. */
function allowed() {
  if (!combos) {
    combos = [];
    for (const a of ADJECTIVES)
      for (const n of NOUNS) {
        const name = `${a} ${n}`;
        if (a !== n && !denied(name)) combos.push({ name, ws: words(name) });
      }
  }
  return combos;
}

/**
 * The codename for `version`, given the codenames already used, oldest first.
 * Never one already used or sharing a word with the last `spread`; among the
 * rest, only those whose words were used least, so every word gets a turn
 * before any comes back.
 */
export function pickCodename(version, used = [], spread = 12) {
  const taken = new Set(used.map((u) => u.toLowerCase()));
  const recent = new Set(used.slice(-spread).flatMap(words));
  const uses = new Map();
  for (const w of used.flatMap(words)) uses.set(w, (uses.get(w) ?? 0) + 1);
  let best = Infinity;
  let free = [];
  for (const { name, ws } of allowed()) {
    if (taken.has(name.toLowerCase()) || ws.some((w) => recent.has(w))) continue;
    const score = ws.reduce((t, w) => t + (uses.get(w) ?? 0), 0);
    if (score < best) [best, free] = [score, []];
    if (score === best) free.push(name);
  }
  if (!free.length) throw new Error("out of codenames: add words to scripts/codenames.mjs");
  return free[hash(version) % free.length];
}

/** `## [1.2.3] "Name" - 2026-01-01 (rest)` → its parts; null for other lines. */
export const HEADING_RE = /^## \[(\d+\.\d+\.\d+)\](?: "([^"]+)")?(?: - (\d{4}-\d{2}-\d{2}))?(.*)$/;

/** Released versions in the changelog, newest first, with their codenames. */
export function releasesIn(changelog) {
  const out = [];
  for (const line of changelog.split("\n")) {
    const m = HEADING_RE.exec(line);
    if (m) out.push({ version: m[1], codename: m[2] ?? null, date: m[3] ?? null });
  }
  return out;
}

/** Codenames used so far, oldest first. */
export const usedCodenames = (changelog) =>
  releasesIn(changelog)
    .reverse()
    .map((r) => r.codename)
    .filter(Boolean);

/** The heading line for a new release. */
export const heading = (version, codename, date) => `## [${version}] "${codename}" - ${date}`;

export const UNRELEASED_RE = /^## \[Unreleased\][^\n]*\n([\s\S]*?)(?=^## \[|(?![\s\S]))/m;

/** Every release has notes in the app (D-204): one with none gets a line. */
export const NO_NOTES = "### Changed\n- Small fixes and upkeep behind the scenes.\n";

/** Turns the Unreleased section into the release's, adding one if missing or empty. */
export function nameRelease(changelog, head) {
  const m = UNRELEASED_RE.exec(changelog);
  if (m && m[1].trim()) return changelog.replace(/^## \[Unreleased\][^\n]*$/m, head);
  if (m) return changelog.replace(UNRELEASED_RE, `${head}\n\n${NO_NOTES}\n`);
  const first = changelog.search(/^## \[/m);
  const at = first < 0 ? changelog.length : first;
  return `${changelog.slice(0, at)}${head}\n\n${NO_NOTES}\n${changelog.slice(at)}`;
}

/** Names every release in the changelog that has no codename yet, oldest first. */
export function backfill(changelog) {
  const lines = changelog.split("\n");
  const used = [];
  const order = lines.map((l, i) => [HEADING_RE.exec(l), i]).filter(([m]) => m).reverse();
  for (const [m, i] of order) {
    const [, version, codename, date, rest] = m;
    if (codename) {
      used.push(codename);
      continue;
    }
    const name = pickCodename(version, used);
    used.push(name);
    lines[i] = `## [${version}] "${name}"${date ? ` - ${date}` : ""}${rest}`;
  }
  return lines.join("\n");
}

/** One version's section body (markdown, without its heading). */
export function notesFor(changelog, version) {
  const lines = changelog.split("\n");
  const start = lines.findIndex((l) => HEADING_RE.exec(l)?.[1] === version);
  if (start < 0) return "";
  let end = lines.findIndex((l, i) => i > start && l.startsWith("## ["));
  if (end < 0) end = lines.length;
  return lines.slice(start + 1, end).join("\n").trim();
}

/** Release title: `v1.2.3 · Name`, or just the tag before codenames. */
export function title(changelog, tag) {
  const v = tag.replace(/^v/, "");
  const r = releasesIn(changelog).find((x) => x.version === v);
  return r?.codename ? `v${v} · ${r.codename}` : `v${v}`;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const [cmd, arg] = process.argv.slice(2);
  const log = readFileSync("CHANGELOG.md", "utf8");
  if (cmd === "title" && arg) console.log(title(log, arg));
  else if (cmd === "notes" && arg) console.log(notesFor(log, arg.replace(/^v/, "")));
  else if (cmd === "backfill") writeFileSync("CHANGELOG.md", backfill(log));
  else {
    console.error("usage: codenames.mjs title <tag> | notes <tag> | backfill");
    process.exit(2);
  }
}
