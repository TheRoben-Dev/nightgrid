/**
 * Glitch/scramble text decode-in. Used on view switches, panel and dialog
 * titles, tab labels, notices and state changes (and later for helper
 * replies and help articles). It settles fast so text stays readable, and is
 * never used on live numbers, logs or terminal output. No-op when effects
 * are off (reduce effects, plain theme or prefers-reduced-motion).
 */
const GLYPHS = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789#%&/<>[]{}=+*";

export function effectsEnabled(): boolean {
  if (typeof document === "undefined") return false;
  return document.documentElement.dataset.effects !== "off";
}

export function scrambleFrame(target: string, progress: number, rand: () => number = Math.random): string {
  const revealed = Math.floor(target.length * Math.min(Math.max(progress, 0), 1));
  let out = target.slice(0, revealed);
  for (let i = revealed; i < target.length; i++) {
    const ch = target[i];
    out += ch === " " || ch === "\n" ? ch : GLYPHS[Math.floor(rand() * GLYPHS.length)];
  }
  return out;
}

/** Decode time: quick for labels, capped for long text. */
export const scrambleDuration = (len: number) => Math.min(650, 220 + len * 12);

/**
 * Svelte action that owns the element's text: `<span use:scramble={title}></span>`.
 * Re-runs the decode whenever the text changes. Leave the element empty; the
 * action writes the text (the final text is set at once when effects are off).
 */
export function scramble(node: HTMLElement, text: string | null | undefined) {
  let raf = 0;
  let current = "";
  const run = (target: string) => {
    cancelAnimationFrame(raf);
    current = target;
    if (!effectsEnabled() || !target.trim()) {
      node.textContent = target;
      return;
    }
    const duration = scrambleDuration(target.length);
    const start = performance.now();
    const tick = (now: number) => {
      const p = (now - start) / duration;
      node.textContent = p >= 1 ? target : scrambleFrame(target, p);
      if (p < 1) raf = requestAnimationFrame(tick);
    };
    node.textContent = scrambleFrame(target, 0);
    raf = requestAnimationFrame(tick);
  };
  run(text ?? "");
  return {
    update(next: string | null | undefined) {
      if ((next ?? "") !== current) run(next ?? "");
    },
    destroy() {
      cancelAnimationFrame(raf);
    },
  };
}
