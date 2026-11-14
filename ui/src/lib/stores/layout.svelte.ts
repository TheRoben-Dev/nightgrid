/**
 * User-adjustable workspace sizes (sidebar width, pane splits, panel
 * heights). Stored per machine in the webview's localStorage: they're layout
 * preferences, not settings worth syncing, and hold nothing sensitive.
 */

export interface SizeSpec {
  /** Default value. */
  def: number;
  min: number;
  max: number;
  /** px for absolute sizes, % of the parent for splits. */
  unit: "px" | "%";
}

export const SIZES = {
  /** Left rail width (expanded). */
  rail: { def: 232, min: 184, max: 420, unit: "px" },
  /** Files: left pane share of the two panes. */
  filesSplit: { def: 50, min: 20, max: 80, unit: "%" },
  /** Files: transfer queue height when open. */
  filesQueue: { def: 24, min: 12, max: 70, unit: "%" },
  /** Docker: container detail/log panel width. */
  dockerSide: { def: 52, min: 28, max: 75, unit: "%" },
  /** Docker stacks: compose output console height. */
  stacksConsole: { def: 40, min: 15, max: 80, unit: "%" },
} satisfies Record<string, SizeSpec>;

export type SizeKey = keyof typeof SIZES;

const STORAGE_KEY = "nightgrid.layout.v1";

export const clampSize = (key: SizeKey, v: number): number => {
  const s: SizeSpec = SIZES[key];
  return Math.round(Math.min(s.max, Math.max(s.min, v)) * 10) / 10;
};

function load(): Partial<Record<SizeKey, number>> {
  try {
    const raw = globalThis.localStorage?.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    const out: Partial<Record<SizeKey, number>> = {};
    for (const k of Object.keys(SIZES) as SizeKey[]) {
      const v = parsed[k];
      if (typeof v === "number" && Number.isFinite(v)) out[k] = clampSize(k, v);
    }
    return out;
  } catch {
    return {};
  }
}

class LayoutStore {
  sizes = $state<Partial<Record<SizeKey, number>>>(load());
  /** True while a splitter is being dragged (turns off size transitions). */
  dragging = $state(false);

  get(key: SizeKey): number {
    return this.sizes[key] ?? SIZES[key].def;
  }

  set(key: SizeKey, v: number) {
    this.sizes[key] = clampSize(key, v);
    this.save();
  }

  reset(key?: SizeKey) {
    if (key) delete this.sizes[key];
    else this.sizes = {};
    this.save();
  }

  get customized(): boolean {
    return Object.keys(this.sizes).length > 0;
  }

  private save() {
    try {
      globalThis.localStorage?.setItem(STORAGE_KEY, JSON.stringify(this.sizes));
    } catch {
      /* storage unavailable: sizes still apply for this run */
    }
  }
}

export const layout = new LayoutStore();
