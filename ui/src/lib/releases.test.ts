import { describe, expect, it } from "vitest";
import changelog from "../../../CHANGELOG.md?raw";
import { between, cmpVersion, parseChangelog, releaseLabel, splitUpdateNotes, whatsNew } from "./releases";

const SAMPLE = `# Changelog

Intro.

## [Unreleased]

### Added
- not out yet

## [1.3.0] "Cobalt Signal" - 2026-12-03

### Added
- three

## [1.2.1] "Pale Haze" - 2026-12-02

### Fixed
- two

## [1.2.0] "Velvet Relay" - 2026-12-01 (prerelease)

- one
`;

describe("release notes (D-204)", () => {
  const list = parseChangelog(SAMPLE);

  it("parses released sections newest first, skipping Unreleased", () => {
    expect(list.map((r) => r.version)).toEqual(["1.3.0", "1.2.1", "1.2.0"]);
    expect(list[0]).toMatchObject({ codename: "Cobalt Signal", date: "2026-12-03", tag: null, body: "### Added\n- three" });
    expect(list[2].tag).toBe("prerelease");
    expect(releaseLabel(list[1])).toBe("v1.2.1 · Pale Haze");
  });

  it("compares versions", () => {
    expect(cmpVersion("1.10.0", "1.9.9")).toBeGreaterThan(0);
    expect(cmpVersion("v1.2.0", "1.2.0")).toBe(0);
    expect(between(list, "1.2.0", "1.3.0").map((r) => r.version)).toEqual(["1.3.0", "1.2.1"]);
  });

  it("shows what's new only after an update", () => {
    expect(whatsNew(list, "1.3.0", null, false)).toEqual([]);
    expect(whatsNew(list, "1.3.0", null, true).map((r) => r.version)).toEqual(["1.3.0"]);
    expect(whatsNew(list, "1.3.0", "1.3.0", true)).toEqual([]);
    expect(whatsNew(list, "1.2.1", "1.3.0", true)).toEqual([]);
    expect(whatsNew(list, "1.3.0", "1.2.0", true).map((r) => r.version)).toEqual(["1.3.0", "1.2.1"]);
    expect(whatsNew(list, "1.3.0", "1.0.0", true, 2)).toHaveLength(2);
  });

  it("splits the codename line from update notes", () => {
    expect(splitUpdateNotes("Codename: Cobalt Signal\n\n### Added\n- x")).toEqual({ codename: "Cobalt Signal", notes: "### Added\n- x" });
    expect(splitUpdateNotes("### Fixed\n- y")).toEqual({ codename: null, notes: "### Fixed\n- y" });
    expect(splitUpdateNotes(null)).toEqual({ codename: null, notes: "" });
  });

  it("the bundled changelog has notes and a codename for every release", () => {
    const all = parseChangelog(changelog);
    expect(all.length).toBeGreaterThan(100);
    for (const r of all) {
      expect(r.codename, r.version).toBeTruthy();
      expect(r.body.length, r.version).toBeGreaterThan(0);
    }
    for (let i = 1; i < all.length; i++) expect(cmpVersion(all[i - 1].version, all[i].version), all[i].version).toBeGreaterThan(0);
  });
});
