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
