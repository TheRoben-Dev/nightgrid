/**
 * Theme definitions. This file is the source of truth for colour tokens;
 * `styles/tokens.css` carries the NIGHTGRID values as the pre-JS default so the
 * first paint is already themed (a test keeps the two in sync).
 */

export type ThemeId = "nightgrid" | "ice" | "bloodline" | "plain";

export interface ThemeTokens {
  "bg-0": string;
  "bg-1": string;
  "bg-2": string;
  line: string;
  "text-0": string;
  "text-1": string;
  accent: string;
  cyan: string;
  magenta: string;
  danger: string;
  ok: string;
  warn: string;
  /** Completed / neutral highlight (filled steps, checked boxes, hairlines on focus). */
  mint: string;
}

export interface Theme {
  id: ThemeId;
  label: string;
  /** Plain themes switch off glow, scanlines and glitch regardless of the effects toggle. */
  effects: boolean;
  tokens: ThemeTokens;
}

export const THEMES: Record<ThemeId, Theme> = {
  nightgrid: {
    id: "nightgrid",
    label: "NIGHTGRID",
    effects: true,
    tokens: {
      "bg-0": "#07080B",
      "bg-1": "#0D0F14",
      "bg-2": "#141821",
      line: "#1F2633",
      "text-0": "#E6EDF3",
      "text-1": "#8B98A9",
      accent: "#F3E600",
      cyan: "#00E5FF",
      magenta: "#FF2A6D",
      danger: "#FF003C",
      ok: "#2BFF88",
      warn: "#FF9F1C",
      mint: "#BDF5EC",
    },
  },
  ice: {
    id: "ice",
    label: "ICE",
    effects: true,
    tokens: {
      "bg-0": "#050A12",
      "bg-1": "#0A1220",
      "bg-2": "#111C2E",
      line: "#1B2A40",
      "text-0": "#EAF6FF",
      "text-1": "#8FA6BE",
      accent: "#7DF9FF",
      cyan: "#00C8FF",
      magenta: "#B98CFF",
      danger: "#FF4D6A",
      ok: "#4DFFC3",
      warn: "#FFC857",
      mint: "#D6F4FF",
    },
  },
  bloodline: {
    id: "bloodline",
    label: "BLOODLINE",
    effects: true,
    tokens: {
      "bg-0": "#0B0606",
      "bg-1": "#140B0B",
      "bg-2": "#1E1111",
      line: "#33201F",
      "text-0": "#F3E9E6",
      "text-1": "#A99390",
      accent: "#FFB000",
      cyan: "#FF6B3D",
      magenta: "#FF2A55",
      danger: "#FF1E3C",
      ok: "#9BE564",
      warn: "#FF9F1C",
      mint: "#F3D9C9",
    },
  },
  plain: {
    id: "plain",
    label: "PLAIN DARK",
    effects: false,
    tokens: {
      "bg-0": "#111214",
      "bg-1": "#18191C",
      "bg-2": "#202226",
      line: "#2C2F35",
      "text-0": "#E4E6EA",
      "text-1": "#9BA1AB",
      accent: "#E8D44D",
      cyan: "#5CC8E0",
      magenta: "#E06C9F",
      danger: "#F0556A",
      ok: "#5FD38D",
      warn: "#E9A23B",
      mint: "#C9D3DD",
    },
  },
};

export const THEME_IDS = Object.keys(THEMES) as ThemeId[];

export function isThemeId(v: unknown): v is ThemeId {
  return typeof v === "string" && v in THEMES;
}

/** Writes a theme's tokens onto an element (normally `document.documentElement`). */
export function applyTheme(el: HTMLElement, id: ThemeId): void {
  const theme = THEMES[id];
  for (const [k, v] of Object.entries(theme.tokens)) {
    el.style.setProperty(`--${k}`, v);
  }
  el.dataset.theme = id;
  el.dataset.themeEffects = String(theme.effects);
}
