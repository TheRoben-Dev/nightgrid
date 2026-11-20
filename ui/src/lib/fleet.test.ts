import { describe, expect, it } from "vitest";
import type { Host, MountForecast, UpdatesReport } from "./api";
import {
  agoText,
  confirmPhrase,
  diskRows,
  disksText,
  diskTone,
  fillText,
  fillTone,
  pendingImages,
  rateText,
  rowTone,
  securityCount,
  size,
  sparkPath,
  totals,
  updateRows,
  updatesText,
  usedPct,
  type HostDisks,
  type HostUpdates,
  type ImageRow,
} from "./fleet";

const host = (id: number, name: string, groupId: number | null = null) => ({ id, name, groupId }) as unknown as Host;
const rep = (n: number, o: Partial<UpdatesReport> = {}): UpdatesReport => ({
  manager: "apt",
  updates: Array.from({ length: n }, (_, i) => ({ name: `p${i}`, current: "1", available: "2", repo: i === 0 && o.note === "sec" ? "jammy-security" : "jammy-updates", held: false })),
  rebootRequired: false,
  rebootReasons: [],
  note: null,
  held: [],
  holdSupport: "host",
  ...o,
});
const ok = (r: UpdatesReport): HostUpdates => ({ status: "ok", report: r, error: null, checkedAt: 1 });

describe("fleet updates", () => {
  it("leaves held hosts and packages out (D-195)", () => {
    const r = rep(3, { note: "sec" });
    r.updates[0].held = true;
    expect(securityCount(r)).toBe(0);
    const hold = { id: 1, hostId: 2, kind: "host" as const, target: "", stack: "", version: null, until: null, note: "", createdAt: 0 };
    const rows = updateRows([host(1, "a"), host(2, "b")], { 1: ok(r), 2: ok(rep(4)) }, (id) => (id === 2 ? hold : null));
    const a = rows.find((x) => x.host.id === 1)!;
    const b = rows.find((x) => x.host.id === 2)!;
    expect([a.count, a.heldCount, a.upgradable, a.tone]).toEqual([2, 1, true, "warn"]);
    expect([b.count, b.heldCount, b.upgradable, b.tone]).toEqual([0, 4, false, "held"]);
    expect(rows[1].host.id).toBe(2);
    expect(totals(rows)).toMatchObject({ hosts: 1, packages: 2 });
    expect(updatesText(rows, [])).toContain("b (apt): updates held by the user");
  });

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

describe("fleet disks", () => {
  const f = (o: Partial<MountForecast>): MountForecast => ({ mount: "/", size: 100e9, used: 50e9, at: 0, perDay: null, daysLeft: null, spanDays: 0, daily: [], ...o });

  it("words the forecast", () => {
    expect(fillText(f({}))).toBe("learning (needs a day of history)");
    expect(fillText(f({ perDay: 0, spanDays: 3 }))).toBe("not growing");
    expect(fillText(f({ perDay: 1e9, daysLeft: 12.7, spanDays: 3 }))).toBe("full in about 12 days");
    expect(fillText(f({ perDay: 1e9, daysLeft: 0.25, spanDays: 3 }))).toBe("full in about 6 hours");
    expect(rateText(31e9)).toBe("+31.0 GB/day");
    expect(rateText(-1.2e9)).toBe("−1.2 GB/day");
    expect(rateText(1e6)).toBe("flat");
    expect(size(3_710e9)).toBe("3.7 TB");
    expect(fillTone(f({ perDay: 1e9, daysLeft: 2 }), 14)).toBe("bad");
    expect(fillTone(f({ perDay: 1e9, daysLeft: 9 }), 14)).toBe("warn");
    expect(fillTone(f({ perDay: 1e9, daysLeft: 40 }), 14)).toBe("ok");
    expect(usedPct(f({}))).toBe(50);
  });

  it("draws a sparkline and ranks hosts", () => {
    expect(sparkPath([{ ts: 0, used: 1 }], 10, 10)).toBe("");
    expect(sparkPath([{ ts: 0, used: 0 }, { ts: 10, used: 10 }], 100, 20)).toBe("M0.0,19.0 L100.0,1.0");
    // A tiny wobble on a big range stays near the middle.
    expect(sparkPath([{ ts: 0, used: 0 }, { ts: 10, used: 10 }], 100, 20, 1000)).toBe("M0.0,10.1 L100.0,9.9");
    const d = (o: Partial<HostDisks>): HostDisks => ({ status: "ok", error: null, pools: null, smart: null, smartNote: null, forecasts: [], checkedAt: 1, ...o });
    const rows = diskRows(
      [host(1, "a"), host(2, "b"), host(3, "c")],
      {
        1: d({}),
        2: d({ pools: { health: "bad", pools: [] } }),
        3: d({ forecasts: [f({ perDay: 1e9, daysLeft: 9 })] }),
      },
      14,
    );
    expect(rows.map((r) => [r.host.name, r.tone])).toEqual([
      ["b", "bad"],
      ["c", "warn"],
      ["a", "ok"],
    ]);
    expect(diskTone(undefined, 14)).toBe("unknown");
    expect(disksText(rows)).toContain("c:\n  - /: 50.0 GB of 100 GB (50%), +1.0 GB/day, full in about 9 days");
  });
});
