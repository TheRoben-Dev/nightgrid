/** Library quests (M12e): a guide's checkable steps, verified on a host. */
import type { QuestResult } from "../api";

export interface QuestSummary {
  pass: number;
  fail: number;
  unknown: number;
  total: number;
}

export function questSummary(rs: QuestResult[]): QuestSummary {
  return {
    pass: rs.filter((r) => r.state === "pass").length,
    fail: rs.filter((r) => r.state === "fail").length,
    unknown: rs.filter((r) => r.state === "unknown").length,
    total: rs.length,
  };
}

export const STATE_MARK: Record<QuestResult["state"], string> = { pass: "✓", fail: "✗", unknown: "?" };
export const STATE_WORD: Record<QuestResult["state"], string> = { pass: "VERIFIED", fail: "NOT YET", unknown: "CAN'T TELL" };

/** "Harden SSH: 3 of 5 steps verified on server01", for DRIFT and the header. */
export const questLine = (title: string, host: string, s: QuestSummary): string =>
  `${title}: ${s.pass} of ${s.total} checkable steps verified on ${host}${s.fail ? `, ${s.fail} not done yet` : ""}${s.unknown ? `, ${s.unknown} couldn't be checked` : ""}`;

/** One `- [ ]` step of a guide, for the quest log (1.4). */
export interface QuestStep {
  /** Markdown block-item key, as ticks use. */
  key: string;
  text: string;
  /** Index of its objective (the guide's `##` part). */
  part: number;
  partTitle: string;
  /** The read-only check that verifies it, if any. */
  check: string | null;
}

export interface QuestProgress {
  done: number;
  total: number;
  /** The first step not done yet; null when every step is. */
  current: QuestStep | null;
  /** Objectives (parts) done, of how many. */
  objectives: { done: number; total: number };
  complete: boolean;
}

function byPart(steps: QuestStep[]): Map<number, QuestStep[]> {
  const m = new Map<number, QuestStep[]>();
  for (const s of steps) m.set(s.part, [...(m.get(s.part) ?? []), s]);
  return m;
}

export function questProgress(steps: QuestStep[], ticks: Iterable<string>): QuestProgress {
  const t = new Set(ticks);
  const parts = [...byPart(steps).values()];
  const done = steps.filter((s) => t.has(s.key)).length;
  return {
    done,
    total: steps.length,
    current: steps.find((s) => !t.has(s.key)) ?? null,
    objectives: { done: parts.filter((p) => p.every((s) => t.has(s.key))).length, total: parts.length },
    complete: steps.length > 0 && done === steps.length,
  };
}

/** What a toast says after steps were done: the biggest news first. */
export interface QuestEvent {
  kind: "updated" | "objective" | "complete";
  /** QUEST UPDATED, OBJECTIVE COMPLETE, QUEST COMPLETE. */
  label: string;
  /** The objective or quest that finished, or the step that was done. */
  title: string;
  /** What's next, if anything. */
  next: string | null;
}

/**
 * The events between two sets of ticks: one for the quest finishing, else one
 * per objective that finished, else one "updated" for new steps. Unticking
 * makes no event.
 */
export function questEvents(steps: QuestStep[], before: Iterable<string>, after: Iterable<string>): QuestEvent[] {
  const b = new Set(before);
  const a = new Set(after);
  const fresh = steps.filter((s) => a.has(s.key) && !b.has(s.key));
  if (!fresh.length) return [];
  const next = steps.find((s) => !a.has(s.key)) ?? null;
  if (!next) return [{ kind: "complete", label: "QUEST COMPLETE", title: "Every objective done", next: null }];
  const finished = [...byPart(steps).values()].filter((p) => p.every((s) => a.has(s.key)) && !p.every((s) => b.has(s.key)));
  if (finished.length)
    return finished.map((p) => ({ kind: "objective", label: "OBJECTIVE COMPLETE", title: p[0].partTitle, next: next.text }));
  const last = fresh[fresh.length - 1];
  return [{ kind: "updated", label: "QUEST UPDATED", title: fresh.length > 1 ? `${fresh.length} steps done` : last.text, next: next.text }];
}
