// @vitest-environment node
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  ADJECTIVES,
  DENY,
  NOUNS,
  backfill,
  denied,
  heading,
  nameRelease,
  notesFor,
  pickCodename,
  releasesIn,
  title,
  usedCodenames,
} from "../../scripts/codenames.mjs";

const changelog = readFileSync(new URL("../../CHANGELOG.md", import.meta.url), "utf8");

describe("release codenames (D-204)", () => {
  it("word lists are plain, unique words with nothing from the deny list", () => {
    for (const list of [ADJECTIVES, NOUNS]) {
      expect(new Set(list).size).toBe(list.length);
      for (const w of list) {
        expect(w).toMatch(/^[A-Z][a-z]+$/);
        expect(denied(w), w).toBe(false);
      }
    }
    for (const d of DENY) expect(d).toBe(d.toLowerCase());
  });

  it("deny matching is by whole word, case-insensitive", () => {
    expect(denied("Neon Ghost")).toBe(true);
    expect(denied("ELECTRIC SHEEP")).toBe(true);
    expect(denied("Ghostly Rain")).toBe(false);
    expect(denied("Cobalt Signal")).toBe(false);
  });

  it("picks are deterministic, fresh and spread out", () => {
    expect(pickCodename("9.9.9", [])).toBe(pickCodename("9.9.9", []));
    const used: string[] = [];
    for (let i = 0; i < 300; i++) {
      const name = pickCodename(`2.${i}.0`, used);
      expect(used).not.toContain(name);
      expect(denied(name)).toBe(false);
      const words = name.split(" ");
      for (const prev of used.slice(-12)) for (const w of prev.split(" ")) expect(words).not.toContain(w);
      used.push(name);
    }
    // Every adjective has had a turn before any comes back.
    const firstAdjs = used.slice(0, ADJECTIVES.length).map((n) => n.split(" ")[0]);
    expect(new Set(firstAdjs).size).toBeGreaterThan(ADJECTIVES.length - 8);
  });

  it("every release in CHANGELOG.md has a unique, allowed codename", () => {
    const all = releasesIn(changelog);
    expect(all.length).toBeGreaterThan(100);
    for (const r of all) {
      expect(r.codename, r.version).toBeTruthy();
      expect(denied(r.codename!), r.codename!).toBe(false);
    }
    const names = usedCodenames(changelog);
    expect(new Set(names.map((n) => n.toLowerCase())).size).toBe(names.length);
    // Backfilling again changes nothing.
    expect(backfill(changelog)).toBe(changelog);
  });

  it("names the Unreleased section, or adds one when there are no notes", () => {
    const head = heading("3.1.0", "Cobalt Signal", "2026-12-01");
    const intro = "# Changelog\n\nIntro.\n\n";
    const old = '## [3.0.0] "Pale Haze" - 2026-11-30\n\n### Fixed\n- x\n';
    const withNotes = nameRelease(`${intro}## [Unreleased]\n\n### Added\n- y\n\n${old}`, head);
    expect(withNotes).toBe(`${intro}${head}\n\n### Added\n- y\n\n${old}`);
    for (const src of [`${intro}${old}`, `${intro}## [Unreleased]\n\n${old}`]) {
      const out = nameRelease(src, head);
      expect(releasesIn(out).map((r) => r.version)).toEqual(["3.1.0", "3.0.0"]);
      expect(notesFor(out, "3.1.0")).toContain("Small fixes");
      expect(notesFor(out, "3.0.0")).toBe("### Fixed\n- x");
    }
  });

  it("titles releases with their codename", () => {
    const top = releasesIn(changelog)[0];
    expect(title(changelog, `v${top.version}`)).toBe(`v${top.version} · ${top.codename}`);
    expect(title(changelog, "v99.0.0")).toBe("v99.0.0");
  });
});
