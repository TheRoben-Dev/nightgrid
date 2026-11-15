import { describe, expect, it } from "vitest";
import type { Alert } from "../api";
import { ago, fromHhmm, targetLabel, toHhmm, tone } from "./alerts.svelte";

const alert = (over: Partial<Alert> = {}): Alert => ({
  id: 1,
  key: "host:1",
  kind: "hostOffline",
  severity: "critical",
  title: "server01",
  detail: "unreachable",
  hostId: 1,
  since: 0,
  resolved: null,
  ...over,
});

describe("alerts helpers", () => {
  it("formats ages", () => {
    expect(ago(100, 130)).toBe("30s");
    expect(ago(0, 600)).toBe("10m");
    expect(ago(0, 3600 * 2 + 300)).toBe("2h 5m");
    expect(ago(0, 7200)).toBe("2h");
    expect(ago(0, 86400 * 3)).toBe("3d");
    expect(ago(500, 100)).toBe("0s");
  });

  it("converts quiet-hour times", () => {
    expect(toHhmm(22 * 60)).toBe("22:00");
    expect(toHhmm(7 * 60 + 5)).toBe("07:05");
    expect(fromHhmm("07:05")).toBe(425);
    expect(fromHhmm("7:30")).toBe(450);
    expect(fromHhmm("24:00")).toBeNull();
    expect(fromHhmm("12:60")).toBeNull();
    expect(fromHhmm("noon")).toBeNull();
  });

  it("picks the strip tone from the worst active alert", () => {
    expect(tone(null)).toBe("ok");
    expect(tone({ active: [], history: [], quiet: false, enabled: true })).toBe("ok");
    expect(tone({ active: [alert({ severity: "warn" })], history: [], quiet: false, enabled: true })).toBe("warn");
    expect(tone({ active: [alert({ severity: "warn" }), alert({ id: 2 })], history: [], quiet: false, enabled: true })).toBe("bad");
  });

  it("names where an alert leads", () => {
    expect(targetLabel(alert())).toBe("Monitor");
    expect(targetLabel(alert({ kind: "containerDown" }))).toBe("Docker");
    expect(targetLabel(alert({ kind: "certExpiring" }))).toBe("Services");
    expect(targetLabel(alert({ kind: "smartWarning" }))).toBe("Disks");
  });
});
