/**
 * ASCII host icons: presets and the same rules the backend applies
 * (`core/src/hosts.rs` `normalize_icon`), so the editor can warn early.
 */

export const ICON_MAX_LINES = 8;
export const ICON_MAX_COLS = 32;

export interface IconPreset {
  id: string;
  label: string;
  art: string;
}

export const ICON_PRESETS: IconPreset[] = [
  {
    id: "server",
    label: "Server",
    art: ["┌──────────┐", "│▪ ━━━━  ● │", "├──────────┤", "│▪ ━━━━  ● │", "└──────────┘"].join("\n"),
  },
  {
    id: "nas",
    label: "NAS",
    art: ["┌────────┐", "│▐▌▐▌▐▌▐▌│", "│▐▌▐▌▐▌▐▌│", "├────────┤", "│ ●  ▪▪▪ │", "└────────┘"].join("\n"),
  },
  {
    id: "pi",
    label: "Pi / SBC",
    art: ["┌┬┬┬┬┬┬┬┬┐", "│ ▣   ▤▤ │", "│ Pi  ▤▤ │", "└──▭──▭──┘"].join("\n"),
  },
  {
    id: "router",
    label: "Router",
    art: [" ╲  │  ╱", "┌───────┐", "│● ● ● ●│", "└───────┘"].join("\n"),
  },
  {
    id: "desktop",
    label: "Desktop",
    art: ["┌─────────┐", "│ >_      │", "│         │", "└────┬────┘", "   ══╧══"].join("\n"),
  },
  {
    id: "tower",
    label: "Tower",
    art: ["┌─────┐", "│ ▓▓▓ │", "│ ─── │", "│ ░░░ │", "│   ● │", "└─────┘"].join("\n"),
  },
  {
    id: "cloud",
    label: "Cloud / VPS",
    art: ["    .--.", " .-(    ).", "(___.__)__)"].join("\n"),
  },
  {
    id: "term",
    label: "Terminal",
    art: ["╭────────╮", "│ $ _    │", "╰────────╯"].join("\n"),
  },
];

const charOk = (c: string): boolean => {
  const n = c.codePointAt(0)!;
  return (
    (n >= 0x20 && n <= 0x7e) ||
    (n >= 0xa0 && n <= 0xff) ||
    (n >= 0x2190 && n <= 0x21ff) ||
    (n >= 0x2500 && n <= 0x25ff) ||
    (n >= 0x2800 && n <= 0x28ff)
  );
};

/** Same cleanup as the backend: newlines, tabs, trailing spaces, edges, common indent. */
export function normalizeIcon(text: string | null): string | null {
  if (!text) return null;
  let lines = text
    .replace(/\r\n?/g, "\n")
    .replace(/\t/g, "    ")
    .split("\n")
    .map((l) => l.trimEnd());
  while (lines.length && !lines[0]) lines.shift();
  while (lines.length && !lines[lines.length - 1]) lines.pop();
  if (!lines.length) return null;
  const indent = Math.min(...lines.filter(Boolean).map((l) => l.length - l.trimStart().length));
  lines = lines.map((l) => (l ? l.slice(indent) : l));
  return lines.join("\n");
}

/** Why the backend would refuse this icon, or null when it's fine. */
export function iconProblem(text: string | null): string | null {
  const art = normalizeIcon(text);
  if (!art) return null;
  for (const c of art) {
    if (c !== "\n" && !charOk(c)) return `"${c}" can't be used; stick to ASCII, box drawing and block characters`;
  }
  const lines = art.split("\n");
  if (lines.length > ICON_MAX_LINES) return `At most ${ICON_MAX_LINES} lines (now ${lines.length})`;
  const wide = Math.max(...lines.map((l) => [...l].length));
  if (wide > ICON_MAX_COLS) return `At most ${ICON_MAX_COLS} characters per line (longest is ${wide})`;
  return null;
}
