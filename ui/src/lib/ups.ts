/** UPS watch helpers (M10s): wording and tones shared by the Fleet UPS tab and the countdown banner. */
import type { UpsCountdown, UpsReading, UpsWatch } from "./api";
import type { UpsInput_Serialize as UpsForm } from "./bindings";

export type { UpsForm };

/** "45 s", "5 min", "1 h 30 min" (matches the Rust `ups::words`). */
export function upsWords(secs: number): string {
  if (secs < 90) return `${Math.round(secs)} s`;
  if (secs < 3600) return `${Math.floor((secs + 30) / 60)} min`;
  const h = Math.floor(secs / 3600);
  const m = Math.floor(((secs % 3600) + 30) / 60);
  return m ? `${h} h ${m} min` : `${h} h`;
}

/** "1:35" left before the shutdown, from the countdown's deadline. */
export function remaining(c: UpsCountdown, nowMs = Date.now()): number {
  return Math.max(0, Math.round((c.deadline ?? 0) - nowMs / 1000));
}
export function clock(secs: number): string {
  const s = Math.max(0, Math.round(secs));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

export type UpsTone = "ok" | "warn" | "bad" | "unknown";

export function upsTone(r: UpsReading | null | undefined): UpsTone {
  if (!r || r.error) return "unknown";
  if (r.lowBattery || (r.onBattery && (r.charge ?? 100) < 30)) return "bad";
  if (r.onBattery || r.replaceBattery) return "warn";
  return "ok";
}

/** One word for the state: ON MAINS, ON BATTERY, LOW BATTERY, NO READING. */
export function upsState(r: UpsReading | null | undefined): string {
  if (!r) return "NO READING";
  if (r.error) return "ERROR";
  if (r.lowBattery) return "LOW BATTERY";
  if (r.onBattery) return "ON BATTERY";
  return "ON MAINS";
}

/** What will happen, in one sentence, for the watch list and the editor. */
export function triggerText(w: Pick<UpsWatch, "shutdown" | "groupId" | "onBatterySecs" | "lowRuntimeSecs" | "countdownSecs">, group?: string): string {
  if (!w.shutdown || w.groupId == null) return "Alerts only.";
  const when = [w.onBatterySecs > 0 ? `after ${upsWords(w.onBatterySecs)} on battery` : null, w.lowRuntimeSecs > 0 ? `under ${upsWords(w.lowRuntimeSecs)} of runtime` : null, "on low battery"].filter(Boolean);
  return `Shuts down ${group ?? "the group"} ${when.join(", or ")}, after a ${upsWords(w.countdownSecs)} countdown you can cancel.`;
}

/** Same rule as the Rust `ups::valid_target`. */
export const validTarget = (t: string): boolean => t.length > 0 && t.length <= 120 && !t.startsWith("-") && /^[A-Za-z0-9_.@:[\]-]+$/.test(t);

export function blankUps(hostId: number): UpsForm {
  return { id: null, label: "", target: "ups@localhost", hostId, groupId: null, shutdown: false, onBatterySecs: 300, lowRuntimeSecs: 300, countdownSecs: 120, enabled: true };
}

/**
 * Turning on (or retargeting) a group shutdown needs the group's name typed
 * (destructive across hosts, rule 5). Saving an already armed watch for the
 * same group doesn't.
 */
export function needsTyped(before: UpsWatch | null | undefined, after: UpsForm): boolean {
  if (!after.shutdown || after.groupId == null) return false;
  return !before || !before.shutdown || before.groupId !== after.groupId;
}
