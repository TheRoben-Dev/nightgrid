/**
 * The running DRIFT tour (M10q): which step is shown, and the watchers that
 * move it on when the user does what the step asks. Watching only listens;
 * nothing here clicks, types or switches views.
 */
import { tourById, type Tour, type TourStep } from "../helper/tours";
import { railTarget } from "../sections";
import { settingsTabFor } from "../settingsTabs";
import { settings } from "./settings.svelte";
import { ui } from "./ui.svelte";

class TourStore {
  tour = $state<Tour | null>(null);
  index = $state(0);
  /** The step's control isn't on screen (another view, a closed panel). */
  missing = $state(false);
  private stopWatch: (() => void) | null = null;

  get step(): TourStep | null {
    return this.tour?.steps[this.index] ?? null;
  }

  /** The step's control as it is in the page: a view that shares a sidebar row points at that row. */
  get targetId(): string | undefined {
    const t = this.step?.target;
    return t ? (railTarget(t)?.id ?? t) : undefined;
  }

  start(id: string): boolean {
    const t = tourById(id);
    if (!t) return false;
    this.tour = t;
    this.index = 0;
    this.watch();
    if (id === "first-run" && !settings.value.tourOffered) void settings.update({ tourOffered: true });
    return true;
  }

  stop() {
    this.stopWatch?.();
    this.stopWatch = null;
    this.tour = null;
    this.index = 0;
    this.missing = false;
  }

  next() {
    if (!this.tour) return;
    if (this.index + 1 >= this.tour.steps.length) {
      this.stop();
      return;
    }
    this.index++;
    this.watch();
  }

  back() {
    if (!this.tour || this.index === 0) return;
    this.index--;
    this.watch();
  }

  /** Sets up what ends the current step. Polls: cheap, and robust to re-renders. */
  private watch() {
    this.stopWatch?.();
    const step = this.step;
    if (!step) return;
    const wait = step.wait;
    const at = this.index;
    const target = this.targetId;
    // A click on a shared sidebar row opens the view this step asks for.
    const rt = step.target ? railTarget(step.target) : null;
    if (rt) ui.lastInRail = { ...ui.lastInRail, [rt.row]: rt.view };
    // A step on a Settings control opens the tab that holds it.
    const tab = step.target ? settingsTabFor(step.target) : null;
    if (tab) ui.settingsTab = tab;
    const done = () => {
      if (this.step === step && this.index === at) this.next();
    };
    let onClick: ((e: Event) => void) | null = null;
    if (wait.kind === "click" && target) {
      onClick = (e: Event) => {
        const el = document.querySelector(`[data-guide="${target}"]`);
        // Let the click do its work first, then move on.
        if (el && e.target instanceof Node && el.contains(e.target)) setTimeout(done, 80);
      };
      addEventListener("click", onClick, true);
    }
    const timer = setInterval(() => {
      if (target) this.missing = !document.querySelector(`[data-guide="${target}"]`);
      else this.missing = false;
      if (wait.kind === "view" && ui.section === wait.view) done();
      else if (wait.kind === "appear" && document.querySelector(wait.selector)) done();
      else if (wait.kind === "gone" && !document.querySelector(wait.selector)) done();
    }, 200);
    this.stopWatch = () => {
      clearInterval(timer);
      if (onClick) removeEventListener("click", onClick, true);
    };
  }
}

export const tour = new TourStore();
