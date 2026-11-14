/**
 * Brief glitch/scramble text reveal. Used only on app boot, a panel title's
 * first open and alert banners — never on content being read. No-op when
 * effects are off.
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
    out += ch === " " ? " " : GLYPHS[Math.floor(rand() * GLYPHS.length)];
  }
  return out;
}

/** Svelte action: `<h2 use:scramble>TITLE</h2>`. */
export function scramble(node: HTMLElement, opts: { duration?: number } = {}) {
  const target = node.textContent ?? "";
  if (!effectsEnabled() || !target.trim()) return {};
  const duration = opts.duration ?? 420;
  const start = performance.now();
  let raf = 0;
  const tick = (now: number) => {
    const p = (now - start) / duration;
    node.textContent = p >= 1 ? target : scrambleFrame(target, p);
    if (p < 1) raf = requestAnimationFrame(tick);
  };
  raf = requestAnimationFrame(tick);
  return {
    destroy() {
      cancelAnimationFrame(raf);
      node.textContent = target;
    },
  };
}
