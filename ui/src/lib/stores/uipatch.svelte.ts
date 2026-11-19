/**
 * Signed interface patches (D-185). Interface-only fixes between releases
 * arrive as a small signed pack built for this exact app version; the
 * backend fetches and verifies it and the app switches to it on the next
 * start. Without one (offline, turned off, nothing newer) the interface
 * built into the app is used, exactly as before.
 */
import { api, errorText, type UiPatchStatus } from "../api";
import { settings } from "./settings.svelte";

/** Wait after launch, then every few hours while the app stays open. */
const LAUNCH_DELAY_MS = 30_000;
const EVERY_MS = 6 * 60 * 60 * 1000;

class UiPatchStore {
  status = $state<UiPatchStatus | null>(null);
  checking = $state(false);
  error = $state<string | null>(null);
  /** A downloaded patch is waiting for a restart and the notice wasn't hidden. */
  notice = $state(false);
  private timer: ReturnType<typeof setInterval> | null = null;

  /**
   * On launch: tell the backend this interface is up (a patch that never
   * gets here is set aside and the built-in interface comes back), then check
   * quietly when enabled.
   */
  async start() {
    try {
      await api.uiPatchReady();
      this.status = await api.uiPatchStatus();
    } catch (e) {
      console.warn("interface patch status failed", e);
    }
    setTimeout(() => void this.check(false), LAUNCH_DELAY_MS);
    this.timer ??= setInterval(() => void this.check(false), EVERY_MS);
  }

  /** `manual` checks even when patches are off and shows errors. */
  async check(manual: boolean) {
    if (this.checking || (!manual && !settings.value.uiPatches)) return;
    this.checking = true;
    if (manual) this.error = null;
    try {
      const s = await api.uiPatchCheck();
      this.status = s;
      if (manual) this.error = s.error;
      if (s.updated && settings.value.uiPatches) this.notice = true;
    } catch (e) {
      if (manual) this.error = errorText(e);
    } finally {
      this.checking = false;
    }
  }

  /** Deletes the downloaded patch: the next start uses the built-in interface. */
  async useBuiltIn() {
    try {
      this.status = await api.uiPatchRemove();
      this.notice = false;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  restart() {
    void api.uiPatchRestart();
  }
}

export const uiPatch = new UiPatchStore();

/** "Nov 19, 18:40" for a patch's build time. */
export function patchLabel(built: number): string {
  return new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" }).format(built * 1000);
}
