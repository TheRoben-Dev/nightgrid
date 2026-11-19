/**
 * Library quests (M12e): runs a guide's `{check: …}` steps on a host, read
 * only, and keeps the last result per guide and host for this session (and
 * for DRIFT, which reports progress from it). Nothing is stored.
 */
import { sound } from "./sound.svelte";
import { api, errorText, type QuestResult } from "../api";
import { stepChecks, type Article } from "../kb/library";
import { questLine, questSummary } from "../kb/quest";
import { hosts } from "./hosts.svelte";

export interface QuestRun {
  articleId: string;
  title: string;
  hostId: number;
  host: string;
  at: number;
  /** By check id. */
  results: Record<string, QuestResult>;
}

/** Every check passing is a completed quest; more passes than last time is a step. */
export function questCue(prev: QuestRun | undefined, rs: QuestResult[]): "questComplete" | "questStep" | null {
  const pass = rs.filter((r) => r.state === "pass").length;
  const before = prev ? Object.values(prev.results).filter((r) => r.state === "pass").length : 0;
  if (rs.length && pass === rs.length) return before === rs.length ? null : "questComplete";
  return pass > before ? "questStep" : null;
}

class QuestStore {
  runs = $state<Record<string, QuestRun>>({});
  busy = $state<string | null>(null);
  error = $state<string | null>(null);

  key = (articleId: string, hostId: number) => `${articleId}|${hostId}`;

  run(articleId: string, hostId: number | null | undefined): QuestRun | null {
    return hostId == null ? null : (this.runs[this.key(articleId, hostId)] ?? null);
  }

  async check(a: Article, hostId: number) {
    const checks = [...new Set(Object.values(stepChecks(a)))];
    const h = hosts.byId(hostId);
    if (!checks.length || !h) return;
    const k = this.key(a.id, hostId);
    this.busy = k;
    this.error = null;
    hosts.remember("library", hostId);
    try {
      const rs = await api.questCheck(hostId, checks);
      const cue = questCue(this.runs[k], rs);
      if (cue) sound.play(cue);
      this.runs = { ...this.runs, [k]: { articleId: a.id, title: a.title, hostId, host: h.name, at: Date.now(), results: Object.fromEntries(rs.map((r) => [r.id, r])) } };
    } catch (e) {
      this.error = errorText(e);
    } finally {
      if (this.busy === k) this.busy = null;
    }
  }

  /** Guide progress on a host, one line per guide checked this session (for DRIFT). */
  linesFor(hostId: number): string[] {
    return Object.values(this.runs)
      .filter((r) => r.hostId === hostId)
      .map((r) => {
        const rs = Object.values(r.results);
        const missing = rs.filter((x) => x.state === "fail").map((x) => `${x.id} (${x.detail})`);
        return `${questLine(r.title, r.host, questSummary(rs))}${missing.length ? `. Not done: ${missing.slice(0, 6).join("; ")}` : ""}`;
      });
  }
}

export const quests = new QuestStore();
