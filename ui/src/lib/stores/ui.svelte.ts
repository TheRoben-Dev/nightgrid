import type { SectionId } from "../sections";

class UiStore {
  section = $state<SectionId>("overview");
  paletteOpen = $state(false);
  /** Breadcrumb tail after the section, e.g. ["plex"]. Filled by views. */
  crumbs = $state<string[]>([]);
  /** Whether the OS keychain works (from app_info); gates "remember" options. */
  keychain = $state(false);

  /** Settings tab shown. */
  settingsTab = $state<"general" | "features">("general");
  /** A turned-off view someone tried to open; Settings → Features points at it. */
  featureAsked = $state<string | null>(null);
  /** Whether a section is on (set by the app from Settings → Features). */
  allow: (id: SectionId) => boolean = () => true;

  go(id: SectionId) {
    if (!this.allow(id)) {
      this.featureAsked = id;
      this.settingsTab = "features";
      id = "settings";
    }
    this.section = id;
    this.crumbs = [];
  }
}

export const ui = new UiStore();
