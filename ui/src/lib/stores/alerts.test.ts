import { describe, expect, it } from "vitest";
import type { Alert, AlertsSnapshot, Silence } from "../api";
import { ago, alertCue, customSecs, fromHhmm, groupByCause, loud, silenceWhen, targetLabel, toHhmm, tone } from "./alerts.svelte";

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
  causedBy: null,
  silenced: null,
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
    expect(tone({ active: [], history: [], quiet: false, enabled: true, upsCountdown: null, logNotes: [], logQuiet: [], silences: [] })).toBe("ok");
    expect(tone({ active: [alert({ severity: "warn" })], history: [], quiet: false, enabled: true, upsCountdown: null, logNotes: [], logQuiet: [], silences: [] })).toBe("warn");
    expect(tone({ active: [alert({ severity: "warn" }), alert({ id: 2 })], history: [], quiet: false, enabled: true, upsCountdown: null, logNotes: [], logQuiet: [], silences: [] })).toBe("bad");
  });

  it("names where an alert leads", () => {
    expect(targetLabel(alert())).toBe("Monitor");
    expect(targetLabel(alert({ kind: "containerDown" }))).toBe("Docker");
    expect(targetLabel(alert({ kind: "certExpiring" }))).toBe("Services");
    expect(targetLabel(alert({ kind: "smartWarning" }))).toBe("Disks");
  });
});

describe("groupByCause", () => {
  it("lists folded alerts under their cause and keeps orphans on top", () => {
    const host = alert({ id: 1, key: "host:1" });
    const box = alert({ id: 2, key: "docker:1:web", kind: "containerDown", causedBy: 1 });
    const svc = alert({ id: 3, key: "svc:4", kind: "serviceDown", causedBy: 1 });
    const orphan = alert({ id: 4, key: "disk:2:/", kind: "diskFull", causedBy: 99 });
    const g = groupByCause([host, box, svc, orphan]);
    expect(g.map((x) => x.alert.id)).toEqual([1, 4]);
    expect(g[0].folded.map((x) => x.id)).toEqual([2, 3]);
    expect(g[1].folded).toEqual([]);
  });
});

describe("acknowledged and muted alerts (D-196)", () => {
  const snap = (active: Alert[]): AlertsSnapshot => ({ active, history: [], quiet: false, enabled: true, upsCountdown: null, logNotes: [], logQuiet: [], silences: [] });
  const nas = alert({ id: 7, key: "disk:2:/volume1", kind: "diskFull", severity: "warn", silenced: 1 });

  it("don't badge, colour the strip or play a sound", () => {
    expect(loud([nas, alert()]).map((a) => a.id)).toEqual([1]);
    expect(tone(snap([nas]))).toBe("ok");
    expect(tone(snap([nas, alert({ id: 2, severity: "warn" })]))).toBe("warn");
    expect(alertCue(snap([]), snap([nas]))).toBeNull();
    expect(alertCue(snap([nas]), snap([]))).toBeNull();
    expect(alertCue(snap([]), snap([alert()]))).toBe("alertCritical");
  });

  it("say how long they last", () => {
    const base: Silence = { id: 1, mode: "ack", key: "disk:2:/", kind: "diskFull", hostId: 2, label: "DISK FULL · nas", severity: "warn", until: null, untilClear: true, created: 0 };
    expect(silenceWhen(base, 100)).toBe("until it clears, unless it gets worse");
    expect(silenceWhen({ ...base, untilClear: false }, 100)).toBe("for good, unless it gets worse");
    expect(silenceWhen({ ...base, untilClear: false, until: 100 + 3 * 3600 }, 100)).toBe("3h left, unless it gets worse");
    expect(silenceWhen({ ...base, mode: "mute", severity: null, untilClear: false }, 100)).toBe("for good");
  });

  it("checks custom durations", () => {
    expect(customSecs(2, 3600)).toBe(7200);
    expect(customSecs(0, 3600)).toBeNull();
    expect(customSecs(53, 7 * 86400)).toBeNull();
  });
});
