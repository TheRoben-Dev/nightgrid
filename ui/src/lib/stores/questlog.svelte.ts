/**
 * The quest log (1.3, D-175): Library guides started as quests, one per
 * guide and host, with the steps done so far. The top bar lists the active
 * ones with their current step; finishing a step, an objective or the whole
 * guide shows a quest-update toast. Quests on a connected host re-run their
 * read-only checks now and then, so work done in a terminal counts too.
 * Kept in the database (and the profile export); the guide text stays in
 * the app, loaded only once there is a quest to show.
 */
import { api, errorText, type Quest } from "../api";
import { questEvents, questProgress, type QuestEvent, type QuestProgress, type QuestStep } from "../kb/quest";
import { hosts } from "./hosts.svelte";
import { library } from "./library.svelte";
import { quests as checks } from "./quests.svelte";

type Lib = typeof import("../kb/library");

export interface QuestToast extends Omit<QuestEvent, "kind"> {
  kind: QuestEvent["kind"] | "started";
  id: number;
  questId: number;
  articleId: string;
  hostId: number | null;
  quest: string;
  host: string | null;
}

/** How often active quests on connected hosts are checked again. */
const AUTO_EVERY_MS = 3 * 60_000;
const TOAST_MS = 7_000;
const MAX_TOASTS = 3;

class QuestLogStore {
  list = $state<Quest[]>([]);
  open = $state(false);
  error = $state<string | null>(null);
  toasts = $state<QuestToast[]>([]);
  /** The quest the Library should show when it opens this guide (set from the log or a toast). */
  want = $state<{ articleId: string; hostId: number | null } | null>(null);
  /** The Library module, once loaded (it holds every article's text). */
  lib = $state.raw<Lib | null>(null);
  private started = false;
  private seq = 0;
  private steps = new Map<string, QuestStep[]>();
  private timer: ReturnType<typeof setInterval> | null = null;

  get active(): Quest[] {
    return this.list.filter((q) => q.completedAt == null);
  }

  start() {
    if (this.started) return;
    this.started = true;
    void this.refresh();
    this.timer = setInterval(() => {
      if (typeof document === "undefined" || document.visibilityState === "visible") void this.autoVerify();
    }, AUTO_EVERY_MS);
  }

  stop() {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    this.started = false;
  }

  async refresh() {
    try {
      this.list = await api.questsList();
      this.error = null;
      if (this.list.length) await this.loadLib();
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async loadLib(): Promise<Lib> {
    return (this.lib ??= await import("../kb/library"));
  }

  /** A guide's steps (empty until the Library module is loaded, or for an unknown article). */
  stepsOf(articleId: string): QuestStep[] {
    const lib = this.lib;
    if (!lib) return [];
    let s = this.steps.get(articleId);
    if (!s) {
      const a = lib.article(articleId);
      s = a ? lib.stepList(a) : [];
      this.steps.set(articleId, s);
    }
    return s;
  }

  progress(q: Quest): QuestProgress {
    return questProgress(this.stepsOf(q.articleId), q.ticks);
  }

  hostName(q: Quest): string | null {
    return q.hostId == null ? null : (hosts.byId(q.hostId)?.name ?? "removed host");
  }

  find(articleId: string, hostId: number | null): Quest | undefined {
    return this.list.find((q) => q.articleId === articleId && q.hostId === hostId);
  }

  forArticle(articleId: string): Quest[] {
    return this.list.filter((q) => q.articleId === articleId);
  }

  private put(q: Quest) {
    const rest = this.list.filter((x) => x.id !== q.id);
    this.list = q.completedAt == null ? [q, ...rest] : [...rest.filter((x) => x.completedAt == null), q, ...rest.filter((x) => x.completedAt != null)];
  }

  /** Starts a guide as a quest on a host (or none). `seed` are steps already ticked. */
  async begin(articleId: string, title: string, hostId: number | null, seed: string[] = []): Promise<Quest | null> {
    try {
      await this.loadLib();
      const q = await api.questStart(articleId, hostId, title, seed);
      this.put(q);
      this.error = null;
      const p = this.progress(q);
      this.toast(q, { kind: "started", label: "NEW QUEST", title, next: p.current?.text ?? null });
      if (hostId != null) hosts.remember("library", hostId);
      return q;
    } catch (e) {
      this.error = errorText(e);
      return null;
    }
  }

  /** Saves a quest's ticks, with a toast for whatever got done. */
  async setTicks(q: Quest, ticks: string[]) {
    const steps = this.stepsOf(q.articleId);
    const known = new Set(steps.map((s) => s.key));
    // Keys from an older version of the guide are kept only while they still name a step.
    const next = [...new Set(ticks)].filter((k) => !known.size || known.has(k)).sort();
    const events = questEvents(steps, q.ticks, next);
    const complete = questProgress(steps, next).complete;
    const optimistic = { ...q, ticks: next };
    this.put(optimistic);
    try {
      const saved = await api.questTicks(q.id, next, complete);
      this.put(saved);
      this.error = null;
      for (const e of events) this.toast(saved, e);
    } catch (e) {
      this.put(q);
      this.error = errorText(e);
    }
  }

  tick(q: Quest, key: string, on: boolean) {
    const has = q.ticks.includes(key);
    if (on === has) return;
    void this.setTicks(q, on ? [...q.ticks, key] : q.ticks.filter((k) => k !== key));
  }

  reset(q: Quest) {
    void this.setTicks(q, []);
  }

  async abandon(q: Quest) {
    try {
      await api.questAbandon(q.id);
      this.list = this.list.filter((x) => x.id !== q.id);
      this.toasts = this.toasts.filter((t) => t.questId !== q.id);
    } catch (e) {
      this.error = errorText(e);
    }
  }

  /** Steps with a check that aren't done yet. */
  pendingChecks(q: Quest): QuestStep[] {
    return this.stepsOf(q.articleId).filter((s) => s.check && !q.ticks.includes(s.key));
  }

  /**
   * Runs the guide's read-only checks on the quest's host and completes the
   * steps that pass. Never unticks: a check that fails later is the user's
   * to look at, not a reason to take progress away.
   */
  async verify(q: Quest): Promise<void> {
    if (q.hostId == null) return;
    const lib = await this.loadLib();
    const a = lib.article(q.articleId);
    if (!a) return;
    await checks.check(a, q.hostId);
    this.applyRun(q.id);
  }

  /** Completes the quest's steps that its host's last check run verified. */
  applyRun(questId: number) {
    const q = this.list.find((x) => x.id === questId);
    if (!q || q.hostId == null || q.completedAt != null) return;
    const run = checks.run(q.articleId, q.hostId);
    if (!run) return;
    const pass = this.pendingChecks(q).filter((s) => run.results[s.check as string]?.state === "pass");
    if (pass.length) void this.setTicks(q, [...q.ticks, ...pass.map((s) => s.key)]);
  }

  /** Re-checks active quests on hosts that are already connected (never opens a connection). */
  async autoVerify() {
    if (checks.busy) return;
    for (const q of this.active) {
      if (q.hostId == null || !hosts.pings[q.hostId]?.connected) continue;
      if (!this.pendingChecks(q).length) continue;
      await this.verify(q);
    }
  }

  /** Opens the quest's guide in the Library, on the quest's host. */
  show(articleId: string, hostId: number | null) {
    this.want = { articleId, hostId };
    this.open = false;
    library.open(articleId);
  }

  toast(q: Quest, e: Omit<QuestToast, "id" | "questId" | "articleId" | "hostId" | "quest" | "host">) {
    const t: QuestToast = { ...e, id: ++this.seq, questId: q.id, articleId: q.articleId, hostId: q.hostId, quest: q.title, host: this.hostName(q) };
    this.toasts = [...this.toasts, t].slice(-MAX_TOASTS);
    setTimeout(() => this.dismiss(t.id), TOAST_MS);
  }

  dismiss(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }
}

export const questLog = new QuestLogStore();
