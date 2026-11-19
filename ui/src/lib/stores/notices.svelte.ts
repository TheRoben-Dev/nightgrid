/**
 * The notifications inbox (1.1, D-167): every notice Nightgrid sent, newest
 * first, with an unread count for the bell. New notices arrive as events.
 */
import { sound } from "./sound.svelte";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, inTauri, type Notice } from "../api";
import { target } from "../notices";
import { ui } from "./ui.svelte";
import type { SectionId } from "../sections";

const LIMIT = 200;

/** Alerts have their own sounds; drills sound like a finished task. */
export function noticeCue(n: Pick<Notice, "source" | "level">): "notice" | "taskDone" | "taskFail" | null {
  if (n.source === "alert") return null;
  if (n.source === "drill") return n.level === "ok" || n.level === "info" ? "taskDone" : "taskFail";
  return "notice";
}

class NoticesStore {
  list = $state<Notice[]>([]);
  open = $state(false);
  error = $state<string | null>(null);
  /** Tab to open when a view is reached from a notice (read by the view). */
  wantTab = $state<{ view: string; tab: string } | null>(null);
  private started = false;

  get unread() {
    return this.list.filter((n) => !n.read).length;
  }

  start() {
    if (this.started) return;
    this.started = true;
    if (inTauri()) {
      void listen<Notice>("notify://notice", (e) => {
        if (this.list.some((n) => n.id === e.payload.id)) return;
        this.list = [e.payload, ...this.list].slice(0, LIMIT);
        const cue = noticeCue(e.payload);
        if (cue) sound.play(cue);
      });
    }
    void this.refresh();
  }

  async refresh() {
    try {
      this.list = await api.noticesList(LIMIT);
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async markRead(ids: number[] | null) {
    this.list = this.list.map((n) => (ids === null || ids.includes(n.id) ? { ...n, read: true } : n));
    try {
      await api.noticesRead(ids);
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async clear() {
    this.list = [];
    try {
      await api.noticesClear();
    } catch (e) {
      this.error = errorText(e);
    }
  }

  /** Opens what the notice is about and marks it read. */
  follow(n: Notice) {
    void this.markRead([n.id]);
    const t = target(n);
    this.open = false;
    if (!t) return;
    if (t.tab) this.wantTab = { view: t.view, tab: t.tab };
    ui.go(t.view as SectionId);
  }

  /** The tab a view should switch to, once (views call this on mount). */
  takeTab(view: string): string | null {
    const w = this.wantTab;
    if (!w || w.view !== view) return null;
    this.wantTab = null;
    return w.tab;
  }
}

export const notices = new NoticesStore();
