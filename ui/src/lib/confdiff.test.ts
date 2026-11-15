import { describe, expect, it } from "vitest";
import type { DriftReport } from "./api";
import { compareVersions, counts, filterFiles, groupFiles, pkgChange, reportText } from "./confdiff";

const report: DriftReport = {
  left: { label: "baseline", taken: 1, manager: "apt", files: 3, packages: 3 },
  right: { label: "lab", taken: 2, manager: "apt", files: 3, packages: 3 },
  files: [
    { path: "/etc/fstab", change: "changed", secret: false },
    { path: "/etc/ssh/sshd_config", change: "changed", secret: false },
    { path: "/etc/ssh/sshd_config.d/x.conf", change: "added", secret: false },
    { path: "/srv/app/.env", change: "removed", secret: true },
  ],
  packages: [
    { name: "openssl", left: "3.0.9-1", right: "3.0.13-1" },
    { name: "curl", left: null, right: "8.5" },
    { name: "telnet", left: "0.17", right: null },
    { name: "zlib", left: "1.3", right: "1.2.13" },
  ],
  hidden: 2,
  mixedManagers: false,
};

describe("config drift", () => {
  it("orders versions", () => {
    expect(compareVersions("3.0.9-1", "3.0.13-1")).toBe(-1);
    expect(compareVersions("1.3", "1.2.13")).toBe(1);
    expect(compareVersions("2:1.0", "2:1.0")).toBe(0);
    expect(compareVersions("1.0", "1.0.1")).toBe(-1);
  });

  it("names package changes", () => {
    expect(report.packages.map(pkgChange)).toEqual(["upgraded", "added", "removed", "downgraded"]);
  });

  it("groups, filters and counts", () => {
    expect(groupFiles(report.files).map((g) => g.dir)).toEqual(["/etc", "/etc/ssh", "/etc/ssh/sshd_config.d", "/srv/app"]);
    expect(filterFiles(report.files, "SSH")).toHaveLength(2);
    expect(counts(report)).toEqual({ changed: 2, added: 1, removed: 1, packages: 4 });
  });

  it("writes text for DRIFT", () => {
    const t = reportText(report, 2);
    expect(t).toContain("Comparing baseline (left) with lab (right).");
    expect(t).toContain("- changed: /etc/fstab");
    expect(t).toContain("…and 2 more files");
    expect(t).toContain("- openssl: 3.0.9-1 → 3.0.13-1");
    expect(t).toContain("(2 host-specific files left out)");
  });
});
