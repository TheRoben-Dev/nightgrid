import { describe, expect, it } from "vitest";
import { fmtKwh, fmtW, money, presetOf, profileProblem, totals, upsCompare, utcOffsetMins, windowText } from "./powercost";
import type { HostPower, UpsPower } from "./api";

const hp = (avgW: number | null, idle: number[] = []): HostPower => ({
  hostId: 1, name: "server01", profile: null, hours: 100, avgCpu: 10, gpuW: null, avgW, peakW: null, kwhMonth: avgW == null ? null : (avgW * 730) / 1000,
  idle: idle.map((k) => ({ from: 23, to: 7, hours: 8, share: 1, kwhMonth: k })),
});

describe("power & cost", () => {
  it("formats", () => {
    expect(fmtW(null)).toBe("--");
    expect(fmtW(7.25)).toBe("7.3 W");
    expect(fmtW(140.4)).toBe("140 W");
    expect(fmtKwh(12.34)).toBe("12.3 kWh");
    expect(money(10, 0, "€")).toBeNull();
    expect(money(10, 0.3, "€")).toBe("€3.00");
    expect(money(100, 2, "kr")).toBe("200 kr");
    expect(money(1, 0.25, "")).toBe("0.25");
    expect(windowText({ from: 23, to: 7, hours: 8, share: 1, kwhMonth: 1 })).toBe("23:00–07:00");
    expect(windowText({ from: 0, to: 0, hours: 24, share: 1, kwhMonth: 1 })).toBe("all day");
    expect(utcOffsetMins(new Date())).toBe(-new Date().getTimezoneOffset());
  });

  it("adds up", () => {
    const t = totals([hp(10, [2]), hp(null), hp(30, [1, 0.5])]);
    expect(t.watts).toBe(40);
    expect(t.kwhMonth).toBeCloseTo(29.2);
    expect([t.covered, t.hosts]).toEqual([2, 3]);
    expect(t.savableKwh).toBe(3.5);
  });

  it("compares with a UPS", () => {
    const u: UpsPower = { id: 1, label: "Rack", watts: 100, measured: true, age: 5, hosts: [1], estimatedW: 95 };
    expect(upsCompare(u)).toContain("within 5%");
    expect(upsCompare({ ...u, estimatedW: 50 })).toContain("cover 50%");
    expect(upsCompare({ ...u, estimatedW: 150 })).toContain("50% above");
    expect(upsCompare({ ...u, watts: null })).toBeNull();
  });

  it("checks profiles", () => {
    expect(profileProblem(10, 5)).toContain("less");
    expect(profileProblem(10, 50)).toBeNull();
    expect(presetOf(7, 30)).toBe("mini");
    expect(presetOf(7, 31)).toBe("");
  });
});
