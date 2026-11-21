/**
 * Component patches (feature store part 2, D-207): looks for newer signed
 * packs of installed store components after launch and every few hours
 * (with Settings → Updates → interface fixes on), and holds what Settings →
 * STORE shows. Loading a pack is componentUi.ts's job.
 */
import { api, errorText, type ComponentPackInfo } from "../api";
import { componentPacks } from "../componentUi";
import { COMPONENTS } from "../storeCatalog";
import { settings } from "./settings.svelte";

const LAUNCH_DELAY_MS = 25_000;
const EVERY_MS = 6 * 60 * 60 * 1000;

class ComponentPatchesStore {
  packs = $state<ComponentPackInfo[]>([]);
  checking = $state(false);
  error = $state<string | null>(null);
  checkedAt = $state<string | null>(null);
  /** Components that got a pack this run (used from their next start). */
  fresh = $state<string[]>([]);
  private timer: ReturnType<typeof setInterval> | null = null;

  async start() {
    try {
      this.packs = await api.componentPacks();
    } catch (e) {
      console.warn("component packs unavailable", e);
    }
    setTimeout(() => void this.check(false), LAUNCH_DELAY_MS);
    this.timer ??= setInterval(() => void this.check(false), EVERY_MS);
  }

  private installedIds(): string[] {
    return COMPONENTS.map((c) => c.id).filter((id) => settings.installed(id));
  }

  async check(manual: boolean, ids = this.installedIds()) {
    if (this.checking || (!manual && settings.value.uiPatches === false) || !ids.length) return;
    this.checking = true;
    try {
      const r = await api.componentPacksCheck(ids);
      this.packs = r.packs;
      this.checkedAt = r.checkedAt;
      this.error = r.notConfigured ? null : r.error;
      if (r.updated.length) {
        this.fresh = [...new Set([...this.fresh, ...r.updated])];
        componentPacks.refresh();
      }
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.checking = false;
    }
  }

  /** Removing a component drops its pack too; it downloads again when installed. */
  async remove(id: string) {
    try {
      this.packs = await api.componentPackRemove(id);
      componentPacks.refresh();
    } catch (e) {
      this.error = errorText(e);
    }
  }

  pack(id: string): ComponentPackInfo | undefined {
    return this.packs.find((p) => p.id === id);
  }
}

export const componentPatches = new ComponentPatchesStore();
