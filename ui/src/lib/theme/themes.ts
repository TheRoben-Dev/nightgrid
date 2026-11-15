/**
 * Theme definitions. This file is the source of truth for colour tokens;
 * `styles/tokens.css` carries the NIGHTGRID values as the pre-JS default so the
 * first paint is already themed (a test keeps the two in sync).
 */

export type ThemeId = "nightgrid" | "ice" | "bloodline" | "custom";

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
  /** Starts from the old PLAIN DARK palette; every slot is pickable from `SWATCHES`. */
  custom: {
    id: "custom",
    label: "CUSTOM",
    effects: true,
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

export const TOKEN_KEYS = Object.keys(THEMES.nightgrid.tokens) as (keyof ThemeTokens)[];

/** The custom theme as saved in settings: token name → #RRGGBB. */
export type CustomTokens = Partial<Record<string, string>>;

export function isThemeId(v: unknown): v is ThemeId {
  return typeof v === "string" && v in THEMES;
}

/**
 * Curated choices for the custom theme. Slots that only work together are
 * picked as a set (the four surfaces, the two text shades). Tests check every
 * combination still meets the contrast rules the built-in themes do.
 */
export interface SwatchSet<K extends keyof ThemeTokens> {
  label: string;
  values: Pick<ThemeTokens, K>;
}

export type SurfaceKey = "bg-0" | "bg-1" | "bg-2" | "line";
export type TextKey = "text-0" | "text-1";
export type ColourKey = "accent" | "cyan" | "magenta" | "danger" | "ok" | "warn" | "mint";

export const SURFACES: SwatchSet<SurfaceKey>[] = [
  { label: "Void", values: { "bg-0": "#07080B", "bg-1": "#0D0F14", "bg-2": "#141821", line: "#1F2633" } },
  { label: "Deep ice", values: { "bg-0": "#050A12", "bg-1": "#0A1220", "bg-2": "#111C2E", line: "#1B2A40" } },
  { label: "Oxblood", values: { "bg-0": "#0B0606", "bg-1": "#140B0B", "bg-2": "#1E1111", line: "#33201F" } },
  { label: "Graphite", values: { "bg-0": "#111214", "bg-1": "#18191C", "bg-2": "#202226", line: "#2C2F35" } },
  { label: "Moss", values: { "bg-0": "#060A07", "bg-1": "#0C130E", "bg-2": "#131C16", line: "#1F2D23" } },
  { label: "Plum", values: { "bg-0": "#0A0710", "bg-1": "#110C19", "bg-2": "#181222", line: "#2A2140" } },
  { label: "Ink", values: { "bg-0": "#000000", "bg-1": "#0A0A0A", "bg-2": "#141414", line: "#262626" } },
];

export const TEXTS: SwatchSet<TextKey>[] = [
  { label: "Cool grey", values: { "text-0": "#E6EDF3", "text-1": "#8B98A9" } },
  { label: "Frost", values: { "text-0": "#EAF6FF", "text-1": "#8FA6BE" } },
  { label: "Warm", values: { "text-0": "#F3E9E6", "text-1": "#A99390" } },
  { label: "Neutral", values: { "text-0": "#E4E6EA", "text-1": "#9BA1AB" } },
  { label: "Paper", values: { "text-0": "#F2EDE4", "text-1": "#A39C8F" } },
  { label: "Phosphor", values: { "text-0": "#D8FFE4", "text-1": "#86B596" } },
];

export const COLOURS: Record<ColourKey, { label: string; hint: string; values: string[] }> = {
  accent: { label: "Accent", hint: "Primary buttons, focus, highlights", values: ["#F3E600", "#7DF9FF", "#FFB000", "#E8D44D", "#5CFFB0", "#FF7AC6", "#C6F432", "#C3A6FF"] },
  cyan: { label: "Info", hint: "Links, info, running", values: ["#00E5FF", "#00C8FF", "#FF6B3D", "#5CC8E0", "#7AA2F7", "#4DD0B5"] },
  magenta: { label: "Active", hint: "Selected items, active tab", values: ["#FF2A6D", "#B98CFF", "#FF2A55", "#E06C9F", "#FF5CF1", "#C084FC"] },
  danger: { label: "Danger", hint: "Errors, destructive actions", values: ["#FF003C", "#FF4D6A", "#FF1E3C", "#F0556A", "#FF5555"] },
  ok: { label: "OK", hint: "Healthy, success", values: ["#2BFF88", "#4DFFC3", "#9BE564", "#5FD38D", "#A6E22E"] },
  warn: { label: "Warning", hint: "Degraded, pending", values: ["#FF9F1C", "#FFC857", "#E9A23B", "#FFB86C"] },
  mint: { label: "Neutral highlight", hint: "Completed steps, hairlines", values: ["#BDF5EC", "#D6F4FF", "#F3D9C9", "#C9D3DD", "#E0D4FF"] },
};

const sameSet = <K extends keyof ThemeTokens>(sets: SwatchSet<K>[], c: CustomTokens | ThemeTokens) =>
  sets.find((s) => (Object.entries(s.values) as [K, string][]).every(([k, v]) => (c as CustomTokens)[k]?.toUpperCase() === v));

/**
 * Resolves saved custom tokens onto the CUSTOM base. Anything that isn't one
 * of the curated choices (hand-edited settings, an old profile) falls back to
 * the base value for that slot or set, so the palette always stays legible.
 */
export function resolveCustom(c: CustomTokens = {}): ThemeTokens {
  const t = { ...THEMES.custom.tokens };
  const surface = sameSet(SURFACES, c);
  if (surface) Object.assign(t, surface.values);
  const text = sameSet(TEXTS, c);
  if (text) Object.assign(t, text.values);
  for (const [k, slot] of Object.entries(COLOURS) as [ColourKey, (typeof COLOURS)[ColourKey]][]) {
    const v = c[k]?.toUpperCase();
    if (v && slot.values.includes(v)) t[k] = v;
  }
  return t;
}

/** Which curated set the tokens use, for highlighting the picker. */
export const surfaceOf = (t: ThemeTokens) => sameSet(SURFACES, t)?.label ?? null;
export const textOf = (t: ThemeTokens) => sameSet(TEXTS, t)?.label ?? null;

/** Writes a theme's tokens onto an element (normally `document.documentElement`). */
export function applyTheme(el: HTMLElement, id: ThemeId, custom?: CustomTokens): void {
  const theme = THEMES[id];
  const tokens = id === "custom" ? resolveCustom(custom) : theme.tokens;
  for (const [k, v] of Object.entries(tokens)) {
    el.style.setProperty(`--${k}`, v);
  }
  el.dataset.theme = id;
  el.dataset.themeEffects = String(theme.effects);
}
