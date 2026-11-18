/** When a UI sound may play (D-178). Pure, so it's tested without audio. */
import type { SoundCategory, SoundId } from "./sounds";

export interface GateSettings {
  on: boolean;
  volume: number;
  muted: readonly string[];
  quietHours: boolean;
  quietStart: number;
  quietEnd: number;
  unfocusedMute: boolean;
}

/** Whether `min` (minutes after midnight) is inside the quiet window, which may wrap midnight. */
export function inQuiet(min: number, start: number, end: number): boolean {
  if (start === end) return false;
  return start < end ? min >= start && min < end : min >= start || min < end;
}

/** The same sound again within this many ms is dropped. */
export const REPEAT_MS = 250;
/** Any two sounds closer than this: only the louder-ranked one plays. */
export const GAP_MS = 90;

/** Higher wins when two sounds land together. */
export const RANK: Record<SoundCategory, number> = {
  alerts: 7,
  quests: 6,
  actions: 5,
  tasks: 4,
  notices: 3,
  drift: 2,
  dialogs: 1,
  navigation: 0,
};

export interface Last {
  id: SoundId;
  category: SoundCategory;
  at: number;
}

export function allowed(
  s: GateSettings,
  id: SoundId,
  category: SoundCategory,
  ctx: { now: Date; focused: boolean; last: Last | null; lastSame: number | null },
): boolean {
  if (!s.on || s.volume <= 0 || s.muted.includes(category)) return false;
  if (s.unfocusedMute && !ctx.focused) return false;
  if (s.quietHours && inQuiet(ctx.now.getHours() * 60 + ctx.now.getMinutes(), s.quietStart, s.quietEnd)) return false;
  const t = ctx.now.getTime();
  if (ctx.lastSame != null && t - ctx.lastSame < REPEAT_MS) return false;
  if (ctx.last && t - ctx.last.at < GAP_MS && RANK[ctx.last.category] >= RANK[category] && ctx.last.id !== id) return false;
  return true;
}

/** 0–100 on the slider → gain. Squared, so low settings stay usable. */
export const gainOf = (volume: number) => Math.pow(Math.max(0, Math.min(100, volume)) / 100, 2);
