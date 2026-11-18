import { sound } from "./sound.svelte";
import { railFor, type SectionId } from "../sections";
import type { SettingsTab } from "../settingsTabs";

class UiStore {
  section = $state<SectionId>("overview");
  paletteOpen = $state(false);
  /** Breadcrumb tail after the section, e.g. ["plex"]. Filled by views. */
  crumbs = $state<string[]>([]);
  /** Whether the OS keychain works (from app_info); gates "remember" options. */
  keychain = $state(false);

  /** Settings tab shown. */
  settingsTab = $state<SettingsTab>("general");
  /** A turned-off view someone tried to open; Settings → Features points at it. */
  featureAsked = $state<string | null>(null);
  /** Whether a section is on (set by the app from Settings → Features). */
  allow: (id: SectionId) => boolean = () => true;

  /** The view last open in each sidebar entry, so the entry returns to it. */
  lastInRail = $state<Record<string, SectionId>>({});

  go(id: SectionId) {
    if (!this.allow(id)) {
      this.featureAsked = id;
      this.settingsTab = "features";
      id = "settings";
    }
    if (id !== this.section) sound.play("tick");
    this.section = id;
    this.crumbs = [];
    const entry = railFor(id);
    if (entry && entry.members.length > 1) this.lastInRail = { ...this.lastInRail, [entry.id]: id };
  }
}

export const ui = new UiStore();
