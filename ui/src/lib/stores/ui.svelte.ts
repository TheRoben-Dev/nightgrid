import type { SectionId } from "../sections";

class UiStore {
  section = $state<SectionId>("overview");
  paletteOpen = $state(false);
  /** Breadcrumb tail after the section, e.g. ["plex"]. Filled by views. */
  crumbs = $state<string[]>([]);
  /** Whether the OS keychain works (from app_info); gates "remember" options. */
  keychain = $state(false);

  go(id: SectionId) {
    this.section = id;
    this.crumbs = [];
  }
}

export const ui = new UiStore();
