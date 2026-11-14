/**
 * Keyboard shortcut parsing and matching. Shortcuts are strings like
 * "Mod+K", "Mod+Shift+P", "Escape". `Mod` is Cmd on macOS and Ctrl elsewhere.
 * All app shortcuts live in DEFAULT_KEYMAP so they can be rebound later.
 */

export interface Chord {
  mod: boolean;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
  key: string; // lower-case `KeyboardEvent.key`, or a named key
}

export const isMac = (): boolean =>
  typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

export function parseChord(spec: string): Chord {
  const parts = spec.split("+").map((p) => p.trim());
  const keyPart = parts.pop();
  if (!keyPart) throw new Error(`empty shortcut: ${spec}`);
  const chord: Chord = { mod: false, ctrl: false, alt: false, shift: false, meta: false, key: keyPart.toLowerCase() };
  for (const p of parts) {
    switch (p.toLowerCase()) {
      case "mod":
        chord.mod = true;
        break;
      case "ctrl":
      case "control":
        chord.ctrl = true;
        break;
      case "alt":
      case "option":
        chord.alt = true;
        break;
      case "shift":
        chord.shift = true;
        break;
      case "cmd":
      case "meta":
      case "super":
        chord.meta = true;
        break;
      default:
        throw new Error(`unknown modifier "${p}" in ${spec}`);
    }
  }
  return chord;
}

type KeyLike = Pick<KeyboardEvent, "key" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">;

export function matches(e: KeyLike, spec: string | Chord, mac = isMac()): boolean {
  const c = typeof spec === "string" ? parseChord(spec) : spec;
  const wantCtrl = c.ctrl || (c.mod && !mac);
  const wantMeta = c.meta || (c.mod && mac);
  return (
    e.key.toLowerCase() === c.key &&
    e.ctrlKey === wantCtrl &&
    e.metaKey === wantMeta &&
    e.altKey === c.alt &&
    e.shiftKey === c.shift
  );
}

/** Human-readable label for a shortcut, e.g. "⌘K" on macOS, "Ctrl+K" elsewhere. */
export function formatChord(spec: string, mac = isMac()): string {
  const c = parseChord(spec);
  const key = c.key.length === 1 ? c.key.toUpperCase() : c.key[0].toUpperCase() + c.key.slice(1);
  if (mac) {
    return [c.ctrl && "⌃", c.alt && "⌥", c.shift && "⇧", (c.mod || c.meta) && "⌘", key].filter(Boolean).join("");
  }
  return [(c.mod || c.ctrl) && "Ctrl", c.alt && "Alt", c.shift && "Shift", c.meta && "Win", key]
    .filter(Boolean)
    .join("+");
}

export const DEFAULT_KEYMAP = {
  "palette.open": "Mod+K",
  "rail.toggle": "Mod+B",
  "nav.settings": "Mod+,",
  "nav.overview": "Mod+1",
  "nav.terminal": "Mod+2",
  "nav.files": "Mod+3",
  "nav.docker": "Mod+4",
  "nav.services": "Mod+5",
  "nav.tunnels": "Mod+6",
  "nav.automation": "Mod+7",
  "effects.toggle": "Mod+Shift+E",
} as const;

export type ActionId = keyof typeof DEFAULT_KEYMAP;
