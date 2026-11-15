import { describe, expect, it } from "vitest";
import type { Host, UpdatesReport } from "./api";
import { agoText, confirmPhrase, pendingImages, rowTone, securityCount, totals, updateRows, updatesText, type HostUpdates, type ImageRow } from "./fleet";

const host = (id: number, name: string, groupId: number | null = null) => ({ id, name, groupId }) as unknown as Host;
const rep = (n: number, o: Partial<UpdatesReport> = {}): UpdatesReport => ({
  manager: "apt",
  updates: Array.from({ length: n }, (_, i) => ({ name: `p${i}`, current: "1", available: "2", repo: i === 0 && o.note === "sec" ? "jammy-security" : "jammy-updates" })),
  rebootRequired: false,
  rebootReasons: [],
  note: null,
  ...o,
});
const ok = (r: UpdatesReport): HostUpdates => ({ status: "ok", report: r, error: null, checkedAt: 1 });

describe("fleet updates", () => {
  it("counts security packages", () => {
    expect(securityCount(rep(3, { note: "sec" }))).toBe(1);
    expect(securityCount(null)).toBe(0);
  });

  it("tones and sorts rows", () => {
    expect(rowTone(undefined)).toBe("unknown");
    expect(rowTone({ status: "checking", report: null, error: null, checkedAt: null })).toBe("busy");
    expect(rowTone(ok(rep(0)))).toBe("ok");
    expect(rowTone(ok(rep(2)))).toBe("warn");
    expect(rowTone(ok(rep(0, { rebootRequired: true })))).toBe("bad");
    const hs = [host(1, "alpha"), host(2, "beta"), host(3, "gamma"), host(4, "delta")];
    const rows = updateRows(hs, {
      1: ok(rep(0)),
      2: ok(rep(5)),
      3: ok(rep(1, { note: "sec" })),
      4: { status: "error", report: null, error: "offline", checkedAt: 1 },
    });
    expect(rows.map((r) => r.host.name)).toEqual(["gamma", "beta", "delta", "alpha"]);
    expect(rows[1].upgradable).toBe(true);
    expect(rows[3].upgradable).toBe(false);
    expect(totals(rows)).toEqual({ hosts: 2, packages: 6, security: 1, reboot: 0, failed: 1 });
  });

  it("asks for the right confirmation phrase", () => {
    const all = [host(1, "a", 7), host(2, "b", 7), host(3, "c")];
    const groups = [{ id: 7, name: "lab" }];
    expect(confirmPhrase([all[0]], groups, all)).toBe("a");
    expect(confirmPhrase([all[0], all[1]], groups, all)).toBe("lab");
    expect(confirmPhrase([all[0], all[2]], groups, all)).toBe("upgrade 2 hosts");
  });

  it("lists moved images and writes DRIFT text", () => {
    const img = (hostId: number, status: "available" | "current", image: string) =>
      ({ hostId, stack: { name: "media" }, update: { image, status, current: { version: "1.0" }, latest: { version: "1.1" } } }) as unknown as ImageRow;
    const imgs = [img(2, "available", "b"), img(1, "current", "x"), img(1, "available", "a")];
    expect(pendingImages(imgs).map((i) => i.update.image)).toEqual(["a", "b"]);
    const rows = updateRows([host(1, "alpha")], { 1: ok(rep(1, { rebootRequired: true, rebootReasons: ["kernel"] })) });
    const t = updatesText(rows, imgs);
    expect(t).toContain("alpha (apt): 1 pending; reboot required (kernel)");
    expect(t).toContain("stack media: a 1.0 → 1.1");
  });

  it("says how long ago", () => {
    expect(agoText(null, 0)).toBe("never");
    expect(agoText(0, 10_000)).toBe("just now");
    expect(agoText(0, 5 * 60_000)).toBe("5 min ago");
    expect(agoText(0, 3 * 3600_000)).toBe("3 h ago");
  });
});
