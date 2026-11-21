/**
 * Transmissions (D-202): signed notices from Nightgrid's developer about
 * something people have to fix on their own end. The backend fetches the
 * feed from GitHub Releases, verifies it with the release key and filters it
 * for this system, version and date; this store drops acknowledged ones and
 * ones aimed at features that are off, and queues the rest for the
 * INCOMING TRANSMISSION dialog.
 */
import { api, errorText, type Broadcast, type BroadcastStatus } from "../api";
import { ui } from "./ui.svelte";
import { library } from "./library.svelte";
import { settings } from "./settings.svelte";
import { SETTINGS_TABS, type SettingsTab } from "../settingsTabs";
import { SECTIONS, type SectionId } from "../sections";
import { openExternal } from "../clipboard";

/** Wait after launch, then every few hours while the app stays open. */
const LAUNCH_DELAY_MS = 15_000;
const EVERY_MS = 3 * 60 * 60 * 1000;
/** Acknowledged ids kept at most (ids no longer in the feed are dropped first). */
const KEEP_DISMISSED = 200;

/** Transmissions still to show: not acknowledged and aimed at something that's on. */
export function pending(list: readonly Broadcast[], dismissed: readonly string[], on: (feature: string) => boolean): Broadcast[] {
  return list.filter((b) => !dismissed.includes(b.id) && (b.features.length === 0 || b.features.some(on)));
}

/** Adds an acknowledged id; forgets ids the feed no longer carries. */
export function acknowledged(dismissed: readonly string[], id: string, feed: readonly Broadcast[] | null): string[] {
  const known = feed?.map((b) => b.id) ?? null;
  const kept = dismissed.filter((d) => d !== id && (!known || known.includes(d)));
  return [...kept, id].slice(-KEEP_DISMISSED);
}

export type LinkTarget = { kind: "kb"; id: string } | { kind: "view"; view: SectionId; tab: SettingsTab | null } | { kind: "web"; url: string };

/** Where a transmission's button leads (null for anything this version doesn't know). */
export function linkTarget(to: string): LinkTarget | null {
  if (to.startsWith("kb:")) return { kind: "kb", id: to.slice(3) };
  if (to.startsWith("https://")) return { kind: "web", url: to };
  if (to.startsWith("view:")) {
    const [view, tab] = to.slice(5).split("/");
    if (!SECTIONS.some((s) => s.id === view)) return null;
    const settingsTab = view === "settings" && SETTINGS_TABS.some((t) => t.id === tab) ? (tab as SettingsTab) : null;
    return { kind: "view", view: view as SectionId, tab: settingsTab };
  }
  return null;
}

class BroadcastStore {
  status = $state<BroadcastStatus | null>(null);
  checking = $state(false);
  error = $state<string | null>(null);
  /** Put off with "Later" until the next start. */
  later = $state<string[]>([]);
  /** Settings → "Show all" replays every current transmission, acknowledged or not. */
  replay = $state<Broadcast[] | null>(null);
  private timer: ReturnType<typeof setInterval> | null = null;

  /** All current transmissions (acknowledged included). */
  get all(): Broadcast[] {
    return this.status?.notices ?? [];
  }

  /** Waiting to be shown, most urgent first. */
  get queue(): Broadcast[] {
    if (this.replay) return this.replay;
    if (!settings.value.broadcasts) return [];
    return pending(this.all, settings.value.dismissedBroadcasts ?? [], (f) => settings.on(f)).filter((b) => !this.later.includes(b.id));
  }

  get current(): Broadcast | null {
    return this.queue[0] ?? null;
  }

  async start() {
    try {
      this.status = await api.broadcastStatus();
    } catch (e) {
      console.warn("transmission status failed", e);
    }
    setTimeout(() => void this.check(false), LAUNCH_DELAY_MS);
    this.timer ??= setInterval(() => void this.check(false), EVERY_MS);
  }

  /** `manual` checks even when transmissions are off and shows errors. */
  async check(manual: boolean) {
    if (this.checking || (!manual && !settings.value.broadcasts)) return;
    this.checking = true;
    if (manual) this.error = null;
    try {
      const s = await api.broadcastCheck();
      this.status = s;
      if (manual) this.error = s.error;
    } catch (e) {
      if (manual) this.error = errorText(e);
    } finally {
      this.checking = false;
    }
  }

  /** "Understood": never shown again (unless replayed from Settings). */
  acknowledge(b: Broadcast) {
    if (this.replay) {
      this.replay = this.replay.filter((x) => x.id !== b.id);
      if (!this.replay.length) this.replay = null;
    }
    if ((settings.value.dismissedBroadcasts ?? []).includes(b.id)) return;
    void settings.update({ dismissedBroadcasts: acknowledged(settings.value.dismissedBroadcasts ?? [], b.id, this.status ? this.all : null) });
  }

  /** "Later": back on the next start. */
  snooze(b: Broadcast) {
    if (this.replay) {
      this.replay = null;
      return;
    }
    this.later = [...this.later, b.id];
  }

  showAll() {
    this.replay = this.all.length ? [...this.all] : null;
  }

  /** Follows a transmission's button. */
  follow(b: Broadcast) {
    const t = b.link ? linkTarget(b.link.to) : null;
    if (!t) return;
    if (t.kind === "web") {
      void openExternal(t.url);
      return;
    }
    this.snooze(b);
    if (t.kind === "kb") library.open(t.id);
    else {
      if (t.tab) ui.settingsTab = t.tab;
      ui.go(t.view);
    }
  }
}

export const broadcasts = new BroadcastStore();
