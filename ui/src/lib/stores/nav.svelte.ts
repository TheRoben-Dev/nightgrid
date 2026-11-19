/**
 * Back/forward history (D-186). Views report their sub-state (tab, host,
 * article) with `track`; the app calls `observe` from an effect, so every
 * change of view, rail tab, sub-tab or host becomes a step. `back` and
 * `forward` put the app where it was.
 */
import { untrack } from "svelte";
import { emptyNav, settle, step, visit, type NavLoc, type NavState } from "../navhistory";
import type { SectionId } from "../sections";
import { ui } from "./ui.svelte";

type Dim = string | number | null;

interface Tracker {
  view: SectionId;
  key: string;
  get: () => Dim;
  set: (v: Dim) => void;
  /** The view fills this in by itself (a default host): no step for that. */
  auto: boolean;
}

class NavStore {
  private trackers = $state.raw<Tracker[]>([]);
  private state = $state.raw<NavState>(emptyNav());
  /** Set while a step is being applied, so its changes don't add steps. */
  private restoring = false;

  get canBack(): boolean {
    return this.state.back.length > 0;
  }
  get canForward(): boolean {
    return this.state.forward.length > 0;
  }

  /**
   * Reports a piece of a view's state. `auto` marks values the view picks by
   * itself the first time (a remembered host). Call once per store.
   */
  track(view: SectionId, key: string, get: () => Dim, set: (v: Dim) => void, auto = false) {
    this.trackers = [...this.trackers.filter((t) => !(t.view === view && t.key === key)), { view, key, get, set, auto }];
  }

  /** Where the app is now. Reads reactive state, so effects re-run on change. */
  here(): NavLoc {
    const section = ui.section;
    const dims: NavLoc["dims"] = {};
    for (const t of this.trackers) if (t.view === section) dims[t.key] = t.get() ?? null;
    return { section, dims };
  }

  /** Records the current place (called from an effect in App). */
  observe() {
    const loc = this.here();
    const auto = new Set(this.trackers.filter((t) => t.view === loc.section && t.auto).map((t) => t.key));
    // Only `loc` is a dependency; the history itself is written, not watched.
    untrack(() => {
      this.state = this.restoring ? settle(this.state, loc) : visit(this.state, loc, auto);
    });
  }

  back() {
    this.go("back");
  }
  forward() {
    this.go("forward");
  }

  private go(dir: "back" | "forward") {
    const r = step(this.state, dir, (l) => ui.allow(l.section as SectionId));
    if (!r) return;
    this.state = r.state;
    this.restoring = true;
    ui.go(r.to.section as SectionId);
    for (const t of this.trackers) {
      if (t.view !== r.to.section || !(t.key in r.to.dims)) continue;
      const v = r.to.dims[t.key];
      if (v !== (t.get() ?? null)) t.set(v);
    }
    // Effects run in a microtask; a timer lands after them, so the restore's
    // own changes settle the current place instead of adding steps.
    setTimeout(() => {
      this.restoring = false;
    }, 0);
  }
}

export const nav = new NavStore();

nav.track(
  "settings",
  "tab",
  () => ui.settingsTab,
  (v) => {
    if (typeof v === "string") ui.settingsTab = v as typeof ui.settingsTab;
  },
);
