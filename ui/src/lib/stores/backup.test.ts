import { describe, expect, it } from "vitest";
import { mockBackup } from "../mockBackup";
import { passphraseProblem, summaryLines } from "./backup";

describe("backup helpers", () => {
  it("checks the export passphrase", () => {
    expect(passphraseProblem("short", "short")).toMatch(/8 characters/);
    expect(passphraseProblem("long enough", "long enougj")).toMatch(/match/);
    expect(passphraseProblem("long enough", "long enough")).toBeNull();
  });

  it("summarises a backup without empty kinds", () => {
    const lines = summaryLines({
      appVersion: "0.9.0",
      createdAt: 0,
      hosts: 1,
      groups: 0,
      tags: 0,
      snippets: 2,
      playbooks: 0,
      services: 0,
      tunnels: 0,
      integrations: 0,
      bookmarks: 0,
      settings: true,
      secrets: 3,
    });
    expect(lines).toEqual(["1 host", "2 snippets", "3 saved secrets", "app settings"]);
  });
});

describe("browser mock", () => {
  it("needs a picked file before importing", async () => {
    await expect(mockBackup.import("correct horse", "merge")).rejects.toThrow(/Open a backup/);
    await expect(mockBackup.open("nope", false)).rejects.toThrow(/wrong passphrase/);
    const p = await mockBackup.open("correct horse", false);
    expect(p?.summary.hosts).toBeGreaterThan(0);
    expect((await mockBackup.import("correct horse", "merge")).skipped).toBeGreaterThan(0);
  });
});
