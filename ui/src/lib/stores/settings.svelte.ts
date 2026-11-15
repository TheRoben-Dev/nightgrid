import { metrics } from "./metrics.svelte";
import { api, DEFAULT_SETTINGS, type AppSettings } from "../api";
import { applyTheme, isThemeId, THEMES, type ThemeId } from "../theme/themes";

export const UI_SCALE_STEPS = [90, 100, 110, 120, 130, 140, 150] as const;
export const TERM_FONT_STEPS = [10, 11, 12, 13, 14, 15, 16, 18, 20, 24] as const;
export const TERM_SCROLLBACK_STEPS = [1000, 2000, 5000, 10000, 20000, 50000, 100000] as const;
export const METRICS_INTERVAL_STEPS = [1, 2, 5, 10, 30] as const;
export const METRICS_BACKGROUND_STEPS = [15, 30, 60, 120, 300] as const;
export const METRICS_RETENTION_STEPS = [1, 3, 7, 14, 30, 90] as const;
export const SERVICES_INTERVAL_STEPS = [15, 30, 60, 120, 300, 900] as const;

class SettingsStore {
  value = $state<AppSettings>({ ...DEFAULT_SETTINGS });
  loaded = $state(false);
  error = $state<string | null>(null);
  /** OS-level `prefers-reduced-motion`. */
  osReducedMotion = $state(false);

  get themeId(): ThemeId {
    return isThemeId(this.value.theme) ? this.value.theme : "nightgrid";
  }

  /** Effects are on only if the theme allows them and neither the user nor the OS reduced them. */
  get effects(): boolean {
    return THEMES[this.themeId].effects && !this.value.reduceEffects && !this.osReducedMotion;
  }

  async load() {
    try {
      this.value = { ...DEFAULT_SETTINGS, ...(await api.settingsGet()) };
      this.error = null;
    } catch (e) {
      this.error = String((e as { message?: string })?.message ?? e);
    } finally {
      this.loaded = true;
    }
  }

  async update(patch: Partial<AppSettings>) {
    const prev = this.value;
    this.value = { ...prev, ...patch };
    try {
      this.value = await api.settingsSet(this.value);
      this.error = null;
      if (Object.keys(patch).some((k) => k.startsWith("metrics"))) metrics.sync();
    } catch (e) {
      this.value = prev;
      this.error = String((e as { message?: string })?.message ?? e);
    }
  }

  /** Push current settings onto the document root. */
  apply(root: HTMLElement = document.documentElement) {
    applyTheme(root, this.themeId);
    root.dataset.effects = this.effects ? "on" : "off";
    root.style.setProperty("--ui-scale", String(this.value.uiScale / 100));
    root.dataset.handheld = this.value.handheld ? "on" : "off";
  }

  /** The handheld preset: big targets, larger scale, icon-only sidebar. */
  setHandheld(on: boolean) {
    return this.update(
      on
        ? { handheld: true, railCollapsed: true, uiScale: Math.max(this.value.uiScale, 130) }
        : { handheld: false, uiScale: this.value.uiScale >= 130 ? 100 : this.value.uiScale },
    );
  }

  watchReducedMotion() {
    if (typeof matchMedia === "undefined") return () => {};
    const mq = matchMedia("(prefers-reduced-motion: reduce)");
    const set = () => (this.osReducedMotion = mq.matches);
    set();
    mq.addEventListener("change", set);
    return () => mq.removeEventListener("change", set);
  }
}

export const settings = new SettingsStore();
