import type { SectionId } from "../sections";

class UiStore {
  section = $state<SectionId>("overview");
  paletteOpen = $state(false);
  /** Breadcrumb tail after the section, e.g. ["plex"]. Filled by views. */
  crumbs = $state<string[]>([]);

  go(id: SectionId) {
    this.section = id;
    this.crumbs = [];
  }
}

export const ui = new UiStore();
