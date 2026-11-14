import type { ITheme } from "@xterm/xterm";

/** Appends an alpha byte to a #RRGGBB colour; other formats pass through. */
const alpha = (c: string, a: string) => (/^#[0-9a-f]{6}$/i.test(c) ? c + a : c);

/** Builds an xterm theme from the current CSS design tokens. */
export function xtermTheme(root: Element = document.documentElement): ITheme {
  const css = getComputedStyle(root);
  const v = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
  const bg = v("--bg-0", "#07080B");
  const fg = v("--text-0", "#E6EDF3");
  const dim = v("--text-1", "#8B98A9");
  const cyan = v("--cyan", "#00E5FF");
  const magenta = v("--magenta", "#FF2A6D");
  const accent = v("--accent", "#F3E600");
  const ok = v("--ok", "#2BFF88");
  const danger = v("--danger", "#FF003C");
  const mint = v("--mint", "#BDF5EC");
  const line = v("--bg-2", "#141821");
  return {
    background: bg,
    foreground: fg,
    cursor: magenta,
    cursorAccent: bg,
    selectionBackground: alpha(cyan, "55"),
    selectionInactiveBackground: alpha(cyan, "2a"),
    black: line,
    red: danger,
    green: ok,
    yellow: accent,
    blue: cyan,
    magenta,
    cyan: mint,
    white: dim,
    brightBlack: dim,
    brightRed: magenta,
    brightGreen: ok,
    brightYellow: accent,
    brightBlue: cyan,
    brightMagenta: magenta,
    brightCyan: mint,
    brightWhite: fg,
  };
}
