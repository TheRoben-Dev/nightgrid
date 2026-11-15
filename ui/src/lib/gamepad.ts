/**
 * Controller navigation for handhelds (Ally, Steam Deck) and any gamepad
 * the webview exposes through the Gamepad API (standard mapping).
 *
 * D-pad / left stick moves focus to the nearest focusable element in that
 * direction, A clicks it, B sends Escape (closes dialogs and menus), the
 * bumpers switch views and Start opens the command palette. Polling runs only
 * while a controller is connected and the window is visible.
 */

export type Dir = "up" | "down" | "left" | "right";

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Standard-mapping button indices. */
export const BTN = { A: 0, B: 1, X: 2, Y: 3, LB: 4, RB: 5, SELECT: 8, START: 9, UP: 12, DOWN: 13, LEFT: 14, RIGHT: 15 } as const;

export type PadAction = { kind: "move"; dir: Dir } | { kind: "activate" } | { kind: "back" } | { kind: "view"; step: 1 | -1 } | { kind: "palette" } | { kind: "rail" };

const STICK = 0.6;
/** First repeat after holding a direction, then every REPEAT_MS. */
const DELAY_MS = 380;
const REPEAT_MS = 110;

/**
 * Index of the best candidate in `dir` from `from`, or -1. Candidates must lie
 * ahead in that direction; the score favours small distance along the axis
 * and penalises sideways offset more, so moving down in a column stays in it.
 */
export function pickNext(from: Box, candidates: Box[], dir: Dir): number {
  const cx = from.x + from.w / 2;
  const cy = from.y + from.h / 2;
  let best = -1;
  let bestScore = Infinity;
  candidates.forEach((c, i) => {
    const ox = c.x + c.w / 2;
    const oy = c.y + c.h / 2;
    let along: number;
    let side: number;
    switch (dir) {
      case "down":
        along = c.y - (from.y + from.h) + 1;
        side = overlap(from.x, from.w, c.x, c.w) ? 0 : Math.abs(ox - cx);
        if (oy <= cy) return;
        break;
      case "up":
        along = from.y - (c.y + c.h) + 1;
        side = overlap(from.x, from.w, c.x, c.w) ? 0 : Math.abs(ox - cx);
        if (oy >= cy) return;
        break;
      case "right":
        along = c.x - (from.x + from.w) + 1;
        side = overlap(from.y, from.h, c.y, c.h) ? 0 : Math.abs(oy - cy);
        if (ox <= cx) return;
        break;
      case "left":
        along = from.x - (c.x + c.w) + 1;
        side = overlap(from.y, from.h, c.y, c.h) ? 0 : Math.abs(oy - cy);
        if (ox >= cx) return;
        break;
    }
    const score = Math.max(along, 0) + side * 2;
    if (score < bestScore) {
      bestScore = score;
      best = i;
    }
  });
  return best;
}

const overlap = (a: number, al: number, b: number, bl: number) => a < b + bl && b < a + al;

/**
 * Turns one gamepad snapshot into actions: presses fire once, held
 * directions repeat. Pure apart from the state object it's given.
 */
export class PadReader {
  private prev: boolean[] = [];
  private held: Dir | null = null;
  private nextRepeat = 0;

  read(buttons: readonly boolean[], axes: readonly number[], now: number): PadAction[] {
    const out: PadAction[] = [];
    const pressed = (i: number) => !!buttons[i] && !this.prev[i];
    if (pressed(BTN.A)) out.push({ kind: "activate" });
    if (pressed(BTN.B)) out.push({ kind: "back" });
    if (pressed(BTN.LB)) out.push({ kind: "view", step: -1 });
    if (pressed(BTN.RB)) out.push({ kind: "view", step: 1 });
    if (pressed(BTN.START)) out.push({ kind: "palette" });
    if (pressed(BTN.SELECT)) out.push({ kind: "rail" });

    const [ax = 0, ay = 0] = axes;
    const dir: Dir | null =
      buttons[BTN.UP] || ay < -STICK
        ? "up"
        : buttons[BTN.DOWN] || ay > STICK
          ? "down"
          : buttons[BTN.LEFT] || ax < -STICK
            ? "left"
            : buttons[BTN.RIGHT] || ax > STICK
              ? "right"
              : null;
    if (dir !== this.held) {
      this.held = dir;
      if (dir) {
        out.push({ kind: "move", dir });
        this.nextRepeat = now + DELAY_MS;
      }
    } else if (dir && now >= this.nextRepeat) {
      out.push({ kind: "move", dir });
      this.nextRepeat = now + REPEAT_MS;
    }
    this.prev = [...buttons];
    return out;
  }
}

const FOCUSABLE =
  'button:not([disabled]), a[href], input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"]), [role="button"], [role="radio"], [role="tab"]';

function boxOf(el: Element): Box | null {
  const r = el.getBoundingClientRect();
  if (r.width === 0 && r.height === 0) return null;
  if (r.bottom < 0 || r.right < 0 || r.top > innerHeight || r.left > innerWidth) return null;
  return { x: r.left, y: r.top, w: r.width, h: r.height };
}

/** Focusable elements inside the open dialog if there is one, else the page. */
function candidates(): HTMLElement[] {
  const dialogs = document.querySelectorAll<HTMLDialogElement>("dialog[open]");
  const scope: ParentNode = dialogs.length ? dialogs[dialogs.length - 1] : document;
  return [...scope.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => !el.closest("[inert], [aria-hidden='true']"));
}

/** Moves focus in `dir`. Text inputs keep left/right for the caret. */
export function moveFocus(dir: Dir): void {
  const active = document.activeElement as HTMLElement | null;
  if (active && (dir === "left" || dir === "right") && active.matches("input:not([type=checkbox]):not([type=radio]), textarea")) return;
  const els = candidates();
  if (!els.length) return;
  const from = active && active !== document.body ? boxOf(active) : null;
  if (!from) {
    els.find((el) => boxOf(el))?.focus();
    return;
  }
  const visible: { el: HTMLElement; box: Box }[] = [];
  for (const el of els) {
    if (el === active) continue;
    const box = boxOf(el);
    if (box) visible.push({ el, box });
  }
  const i = pickNext(from, visible.map((v) => v.box), dir);
  if (i >= 0) {
    visible[i].el.focus();
    visible[i].el.scrollIntoView({ block: "nearest", inline: "nearest" });
  } else {
    // Nothing on screen that way: scroll the nearest scroller a step.
    const step = dir === "down" || dir === "right" ? 120 : -120;
    const scroller = active?.closest<HTMLElement>("[data-scroll], main, .content") ?? document.scrollingElement;
    if (dir === "up" || dir === "down") scroller?.scrollBy({ top: step });
  }
}

function sendKey(key: string) {
  const target = (document.activeElement as HTMLElement | null) ?? document.body;
  target.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
}

export interface PadHooks {
  enabled: () => boolean;
  view: (step: 1 | -1) => void;
  palette: () => void;
  rail: () => void;
}

/** Starts listening for controllers. Returns a stop function. */
export function startGamepad(hooks: PadHooks): () => void {
  if (typeof navigator === "undefined" || !("getGamepads" in navigator)) return () => {};
  const reader = new PadReader();
  let raf = 0;
  const root = document.documentElement;

  const apply = (a: PadAction) => {
    root.dataset.pad = "on";
    switch (a.kind) {
      case "move":
        moveFocus(a.dir);
        break;
      case "activate": {
        const el = document.activeElement as HTMLElement | null;
        if (el && el !== document.body) el.click();
        break;
      }
      case "back": {
        const dlg = [...document.querySelectorAll<HTMLDialogElement>("dialog[open]")].pop();
        if (dlg) dlg.dispatchEvent(new Event("cancel", { cancelable: true }));
        else sendKey("Escape");
        break;
      }
      case "view":
        if (!document.querySelector("dialog[open]")) hooks.view(a.step);
        break;
      case "palette":
        hooks.palette();
        break;
      case "rail":
        hooks.rail();
        break;
    }
  };

  const tick = () => {
    raf = 0;
    const pad = [...navigator.getGamepads()].find((p) => p && p.connected);
    if (!pad) return;
    if (hooks.enabled() && document.visibilityState === "visible" && document.hasFocus()) {
      const acts = reader.read(
        pad.buttons.map((b) => b.pressed),
        pad.axes,
        performance.now(),
      );
      acts.forEach(apply);
    }
    raf = requestAnimationFrame(tick);
  };
  const start = () => {
    if (!raf) raf = requestAnimationFrame(tick);
  };
  // Mouse or keyboard use hides the controller focus ring again.
  const off = () => delete root.dataset.pad;
  addEventListener("gamepadconnected", start);
  addEventListener("pointerdown", off);
  start();
  return () => {
    removeEventListener("gamepadconnected", start);
    removeEventListener("pointerdown", off);
    if (raf) cancelAnimationFrame(raf);
  };
}
