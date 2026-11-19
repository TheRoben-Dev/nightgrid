/**
 * Back/forward navigation (D-186): a browser-style history of where you were
 * in the app (view, rail tab, sub-tab, host pick) and the inputs that walk it
 * (mouse side buttons, Cmd+[ / Cmd+], Alt+Left / Alt+Right, trackpad swipe).
 * Everything here is pure so it can be tested without a window.
 */

/** A place in the app: a view plus the sub-state views report (tab, host…). */
export interface NavLoc {
  section: string;
  /** Sub-state by key ("tab", "host", "article"…), as the view reported it. */
  dims: Record<string, string | number | null>;
}

export interface NavState {
  back: NavLoc[];
  current: NavLoc | null;
  forward: NavLoc[];
}

/** How many places back are kept. */
export const NAV_LIMIT = 50;

export const emptyNav = (): NavState => ({ back: [], current: null, forward: [] });

export function sameLoc(a: NavLoc, b: NavLoc): boolean {
  if (a.section !== b.section) return false;
  const keys = new Set([...Object.keys(a.dims), ...Object.keys(b.dims)]);
  for (const k of keys) if ((a.dims[k] ?? null) !== (b.dims[k] ?? null)) return false;
  return true;
}

/**
 * Whether `next` only fills in what `cur` didn't know yet: a view that just
 * loaded and reported its tab, or a host the view picked by itself. Those
 * update the current place instead of adding a step back.
 */
export function fillsIn(cur: NavLoc, next: NavLoc, autoKeys: ReadonlySet<string>): boolean {
  if (cur.section !== next.section) return false;
  for (const k of new Set([...Object.keys(cur.dims), ...Object.keys(next.dims)])) {
    const a = cur.dims[k];
    const b = next.dims[k];
    if ((a ?? null) === (b ?? null)) continue;
    if (!(k in cur.dims)) continue;
    if (autoKeys.has(k) && (a === null || a === undefined)) continue;
    return false;
  }
  return true;
}

/** Records that the app is now at `next`. */
export function visit(s: NavState, next: NavLoc, autoKeys: ReadonlySet<string> = new Set()): NavState {
  if (!s.current) return { ...s, current: next };
  if (sameLoc(s.current, next)) return s;
  if (fillsIn(s.current, next, autoKeys)) return { ...s, current: next };
  return { back: [...s.back, s.current].slice(-NAV_LIMIT), current: next, forward: [] };
}

/** Replaces the current place without adding a step (after a restore settles). */
export const settle = (s: NavState, next: NavLoc): NavState => ({ ...s, current: next });

/**
 * One step back (or forward), skipping places that can't be shown any more
 * (`ok` false: a view turned off). Returns null when there is nowhere to go.
 */
export function step(s: NavState, dir: "back" | "forward", ok: (l: NavLoc) => boolean = () => true): { state: NavState; to: NavLoc } | null {
  if (!s.current) return null;
  const from = dir === "back" ? [...s.back] : [...s.forward];
  const other = dir === "back" ? [...s.forward] : [...s.back];
  // The stacks are in time order: the nearest place is last in `back`, first in `forward`.
  const take = () => (dir === "back" ? from.pop() : from.shift());
  const pushOther = (l: NavLoc) => (dir === "back" ? other.unshift(l) : other.push(l));
  let to = take();
  while (to && (!ok(to) || sameLoc(to, s.current))) to = take();
  if (!to) return null;
  pushOther(s.current);
  const state = dir === "back" ? { back: from, current: to, forward: other } : { back: other, current: to, forward: from };
  return { state, to };
}

// ---------------------------------------------------------------- inputs

type KeyLike = Pick<KeyboardEvent, "key" | "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">;

/**
 * The navigation a key press asks for. macOS: Cmd+[ and Cmd+] (by key or by
 * physical key, so non-US layouts work). Windows/Linux: Alt+Left and
 * Alt+Right. Everywhere: the BrowserBack/BrowserForward keys some mice and
 * keyboards send.
 */
export function navKey(e: KeyLike, mac: boolean): "back" | "forward" | null {
  if (e.key === "BrowserBack") return "back";
  if (e.key === "BrowserForward") return "forward";
  if (e.shiftKey) return null;
  if (mac) {
    if (!e.metaKey || e.ctrlKey || e.altKey) return null;
    if (e.key === "[" || e.code === "BracketLeft") return "back";
    if (e.key === "]" || e.code === "BracketRight") return "forward";
    return null;
  }
  if (!e.altKey || e.ctrlKey || e.metaKey) return null;
  if (e.key === "ArrowLeft") return "back";
  if (e.key === "ArrowRight") return "forward";
  return null;
}

/**
 * Whether a key chord belongs to the focused element instead: text fields
 * and editors use Alt+Left/Right to move by word, and shells read Alt
 * chords. Cmd+[ on macOS never reaches a shell, so the terminal keeps only
 * the Alt chords (the same rule as the app's other shortcuts).
 */
export function keyBelongsToTarget(t: { editable: boolean; terminal: boolean; vm: boolean }, mac: boolean): boolean {
  if (t.vm) return true;
  if (t.editable) return true;
  if (t.terminal) return !mac;
  return false;
}

/** Mouse buttons 3 and 4 are the side buttons ("back", "forward"). */
export function navButton(button: number): "back" | "forward" | null {
  return button === 3 ? "back" : button === 4 ? "forward" : null;
}

/**
 * Two-finger horizontal swipe on a trackpad. Collects one gesture's wheel
 * deltas and fires once when it is clearly sideways and long enough; the
 * next one needs the fingers to stop first (a pause of `gapMs`).
 */
export class SwipeTracker {
  private sum = 0;
  private vert = 0;
  private last = -Infinity;
  private fired = false;
  constructor(
    private threshold = 160,
    private gapMs = 250,
  ) {}

  /** Feeds one wheel event (pixels); returns a direction once per gesture. */
  feed(dx: number, dy: number, at: number): "back" | "forward" | null {
    if (at - this.last > this.gapMs) {
      this.sum = 0;
      this.vert = 0;
      this.fired = false;
    }
    this.last = at;
    if (this.fired) return null;
    this.sum += dx;
    this.vert += Math.abs(dy);
    if (Math.abs(this.sum) < this.threshold || Math.abs(this.sum) < this.vert * 2) return null;
    this.fired = true;
    // Fingers moving right scroll left (negative deltaX): back, as in Safari.
    return this.sum < 0 ? "back" : "forward";
  }

  /** Ends the gesture early (it started over something that scrolls sideways). */
  cancel(at: number) {
    this.last = at;
    this.fired = true;
  }
}
