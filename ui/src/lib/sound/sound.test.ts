import { describe, expect, it } from "vitest";
import { allowed, gainOf, inQuiet, type GateSettings } from "./gate";
import { duration, render, wav, SAMPLE_RATE } from "./synth";
import { CATEGORIES, SOUNDS, fileName, soundById } from "./sounds";
import { alertCue } from "../stores/alerts.svelte";
import { noticeCue } from "../stores/notices.svelte";
import { questCue } from "../stores/quests.svelte";
import { DEFAULT_SETTINGS, type Alert, type AlertsSnapshot, type QuestResult } from "../api";

describe("sound recipes", () => {
  it("are short, unique, in a known category and render clean", () => {
    expect(new Set(SOUNDS.map((s) => s.id)).size).toBe(SOUNDS.length);
    for (const s of SOUNDS) {
      expect(CATEGORIES.some((c) => c.id === s.category)).toBe(true);
      expect(duration(s.recipe)).toBeLessThan(1.5);
      const pcm = render(s.recipe);
      let peak = 0;
      for (const x of pcm) {
        expect(Number.isFinite(x)).toBe(true);
        peak = Math.max(peak, Math.abs(x));
      }
      expect(peak).toBeGreaterThan(0.05);
      expect(peak).toBeLessThanOrEqual(s.recipe.level + 1e-6);
      expect(Math.abs(pcm[0])).toBeLessThan(0.01);
      expect(Math.abs(pcm[pcm.length - 1])).toBeLessThan(0.01);
    }
  });

  it("render the same samples every time", () => {
    const r = soundById("driftReply").recipe;
    expect(render(r)).toEqual(render(r));
  });

  it("every category has a sound to test", () => {
    for (const c of CATEGORIES) expect(SOUNDS.some((s) => s.category === c.id)).toBe(true);
  });

  it("writes a valid WAV", () => {
    const pcm = render(soundById("ok").recipe);
    const b = wav(pcm);
    const v = new DataView(b.buffer);
    expect(String.fromCharCode(...b.slice(0, 4))).toBe("RIFF");
    expect(v.getUint32(24, true)).toBe(SAMPLE_RATE);
    expect(b.length).toBe(44 + pcm.length * 2);
    expect(fileName("alertCritical")).toBe("alert-critical.wav");
  });
});

describe("sound gate", () => {
  const base: GateSettings = { on: true, ...DEFAULT_SETTINGS.sound };
  const at = (h: number, m = 0) => new Date(2026, 10, 18, h, m);
  const ctx = (o: Partial<Parameters<typeof allowed>[3]> = {}) => ({ now: at(12), focused: true, last: null, lastSame: null, ...o });

  it("defaults: on, view tick muted", () => {
    expect(allowed(base, "ok", "actions", ctx())).toBe(true);
    expect(allowed(base, "tick", "navigation", ctx())).toBe(false);
  });

  it("respects the master switch, volume, focus and categories", () => {
    expect(allowed({ ...base, on: false }, "ok", "actions", ctx())).toBe(false);
    expect(allowed({ ...base, volume: 0 }, "ok", "actions", ctx())).toBe(false);
    expect(allowed(base, "ok", "actions", ctx({ focused: false }))).toBe(false);
    expect(allowed({ ...base, unfocusedMute: false }, "ok", "actions", ctx({ focused: false }))).toBe(true);
    expect(allowed({ ...base, muted: ["alerts"] }, "alertWarn", "alerts", ctx())).toBe(false);
  });

  it("quiet hours wrap midnight", () => {
    expect(inQuiet(23 * 60, 22 * 60, 7 * 60)).toBe(true);
    expect(inQuiet(3 * 60, 22 * 60, 7 * 60)).toBe(true);
    expect(inQuiet(12 * 60, 22 * 60, 7 * 60)).toBe(false);
    expect(inQuiet(13 * 60, 12 * 60, 14 * 60)).toBe(true);
    expect(inQuiet(5, 5, 5)).toBe(false);
    const q = { ...base, quietHours: true };
    expect(allowed(q, "ok", "actions", ctx({ now: at(23, 30) }))).toBe(false);
    expect(allowed(q, "ok", "actions", ctx({ now: at(9) }))).toBe(true);
  });

  it("drops repeats and lesser sounds that land together", () => {
    const t = at(12).getTime();
    expect(allowed(base, "ok", "actions", ctx({ lastSame: t - 100 }))).toBe(false);
    expect(allowed(base, "ok", "actions", ctx({ lastSame: t - 1000 }))).toBe(true);
    const lastAlert = { id: "alertCritical" as const, category: "alerts" as const, at: t - 20 };
    expect(allowed(base, "notice", "notices", ctx({ last: lastAlert }))).toBe(false);
    const lastNotice = { id: "notice" as const, category: "notices" as const, at: t - 20 };
    expect(allowed(base, "alertWarn", "alerts", ctx({ last: lastNotice }))).toBe(true);
  });

  it("volume is a curve", () => {
    expect(gainOf(0)).toBe(0);
    expect(gainOf(100)).toBe(1);
    expect(gainOf(50)).toBeCloseTo(0.25);
    expect(gainOf(250)).toBe(1);
  });
});

describe("what plays when", () => {
  const alert = (id: number, severity: "warning" | "critical", causedBy: number | null = null) => ({ id, severity, causedBy }) as unknown as Alert;
  const snap = (active: Alert[]) => ({ active, history: [] }) as unknown as AlertsSnapshot;

  it("alerts: worst new one, then clears; nothing at launch or for folded ones", () => {
    expect(alertCue(null, snap([alert(1, "critical")]))).toBeNull();
    expect(alertCue(snap([]), snap([alert(1, "warning")]))).toBe("alertWarn");
    expect(alertCue(snap([]), snap([alert(1, "warning"), alert(2, "critical")]))).toBe("alertCritical");
    expect(alertCue(snap([alert(1, "warning")]), snap([]))).toBe("alertClear");
    expect(alertCue(snap([alert(1, "critical")]), snap([alert(1, "critical"), alert(2, "critical", 1)]))).toBeNull();
    expect(alertCue(snap([alert(1, "warning")]), snap([alert(1, "warning")]))).toBeNull();
  });

  it("notices: alerts are silent here, drills sound like tasks", () => {
    expect(noticeCue({ source: "alert", level: "critical" })).toBeNull();
    expect(noticeCue({ source: "drill", level: "ok" })).toBe("taskDone");
    expect(noticeCue({ source: "drill", level: "critical" })).toBe("taskFail");
    expect(noticeCue({ source: "rotation", level: "warn" })).toBe("notice");
  });

  it("quests: a step when more pass, complete once when all pass", () => {
    const r = (states: string[]) => states.map((state, i) => ({ id: `c${i}`, state, detail: "" })) as unknown as QuestResult[];
    const run = (rs: QuestResult[]) => ({ articleId: "a", title: "A", hostId: 1, host: "h", at: 0, results: Object.fromEntries(rs.map((x) => [x.id, x])) });
    expect(questCue(undefined, r(["fail", "fail"]))).toBeNull();
    expect(questCue(undefined, r(["pass", "fail"]))).toBe("questStep");
    expect(questCue(run(r(["pass", "fail"])), r(["pass", "fail"]))).toBeNull();
    expect(questCue(run(r(["pass", "fail"])), r(["pass", "pass"]))).toBe("questComplete");
    expect(questCue(run(r(["pass", "pass"])), r(["pass", "pass"]))).toBeNull();
  });
});
