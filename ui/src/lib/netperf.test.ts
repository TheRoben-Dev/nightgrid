import { describe, expect, it } from "vitest";
import { best, formatBps, formatMs, pairs, tone } from "./netperf";
import type { NetTest } from "./api";

const t = (id: number, up: number | null, extra: Partial<NetTest> = {}): NetTest => ({
  id,
  fromHost: 1,
  toHost: 2,
  target: "192.168.1.12",
  at: id,
  upBps: up,
  downBps: up == null ? null : up * 0.99,
  retransmits: 0,
  rttMs: 0.4,
  jitterMs: 0.1,
  lossPct: 0,
  error: null,
  ...extra,
});

describe("netperf helpers", () => {
  it("formats rates and times", () => {
    expect(formatBps(941e6)).toBe("941 Mbit/s");
    expect(formatBps(2.35e9)).toBe("2.35 Gbit/s");
    expect(formatBps(54.2e6)).toBe("54.2 Mbit/s");
    expect(formatBps(null)).toBe("--");
    expect(formatMs(0.401)).toBe("0.40 ms");
    expect(formatMs(12.34)).toBe("12.3 ms");
  });

  it("groups runs by pair, newest pair first, oldest run first", () => {
    const list = [t(3, 9e8), { ...t(2, 8e8), toHost: 3 }, t(1, 7e8)];
    const p = pairs(list);
    expect(p.map((x) => x.key)).toEqual(["1>2", "1>3"]);
    expect(p[0].runs.map((r) => r.id)).toEqual([1, 3]);
    expect(p[0].latest.id).toBe(3);
    expect(best(t(1, 5e8))).toBe(5e8);
  });

  it("flags fast ethernet, slowdowns, loss and failures", () => {
    const hist = [t(1, 940e6), t(2, 935e6), t(3, 938e6)];
    expect(tone(t(4, 930e6), hist)).toBe("ok");
    expect(tone(t(4, 94e6), hist)).toBe("warn");
    expect(tone(t(4, 400e6), hist)).toBe("warn");
    expect(tone(t(4, 930e6, { lossPct: 20 }), hist)).toBe("warn");
    expect(tone(t(4, null, { error: "upload: refused" }), hist)).toBe("bad");
  });
});
