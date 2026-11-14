/** Pure helpers for the file browser (formatting, selection, language detection). */

export function formatSize(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ["KiB", "MiB", "GiB", "TiB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

export const formatSpeed = (bps: number): string => `${formatSize(Math.round(bps))}/s`;

/** `rwxr-xr-x` from permission bits (with setuid/setgid/sticky). */
export function formatMode(mode: number | null | undefined, dir = false): string {
  if (mode == null) return "—";
  const bits = "rwxrwxrwx";
  let s = "";
  for (let i = 0; i < 9; i++) s += mode & (1 << (8 - i)) ? bits[i] : "-";
  const set = (pos: number, flag: number, on: string, off: string) => {
    if (mode & flag) s = s.slice(0, pos) + (s[pos] === "x" ? on : off) + s.slice(pos + 1);
  };
  set(2, 0o4000, "s", "S");
  set(5, 0o2000, "s", "S");
  set(8, 0o1000, "t", "T");
  return (dir ? "d" : "-") + s;
}

export function formatTime(secs: number | null | undefined, now = Date.now()): string {
  if (secs == null) return "—";
  const d = new Date(secs * 1000);
  const pad = (x: number) => String(x).padStart(2, "0");
  const sameYear = d.getFullYear() === new Date(now).getFullYear();
  const date = `${sameYear ? "" : d.getFullYear() + "-"}${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  return `${date} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export const isHidden = (name: string): boolean => name.startsWith(".");

export interface Mods {
  toggle: boolean; // Ctrl/Cmd
  range: boolean; // Shift
}

/**
 * Next selection after clicking `path` in `order` (visible rows). Returns the
 * new selection and anchor (the row Shift ranges extend from).
 */
export function selectClick(
  order: string[],
  selected: string[],
  anchor: string | null,
  path: string,
  mods: Mods,
): { selected: string[]; anchor: string } {
  if (mods.range && anchor && order.includes(anchor)) {
    const a = order.indexOf(anchor);
    const b = order.indexOf(path);
    const [lo, hi] = a < b ? [a, b] : [b, a];
    const range = order.slice(lo, hi + 1);
    const base = mods.toggle ? selected.filter((p) => !range.includes(p)) : [];
    return { selected: [...base, ...range], anchor };
  }
  if (mods.toggle) {
    const has = selected.includes(path);
    return { selected: has ? selected.filter((p) => p !== path) : [...selected, path], anchor: path };
  }
  return { selected: [path], anchor: path };
}

/** Moves the cursor by `delta` rows (clamped); used by arrow keys. */
export function moveCursor(order: string[], cursor: string | null, delta: number): string | null {
  if (!order.length) return null;
  const i = cursor ? order.indexOf(cursor) : -1;
  const next = i < 0 ? (delta > 0 ? 0 : order.length - 1) : Math.min(order.length - 1, Math.max(0, i + delta));
  return order[next];
}

export type LangKey =
  | "yaml"
  | "json"
  | "javascript"
  | "typescript"
  | "python"
  | "markdown"
  | "html"
  | "css"
  | "xml"
  | "shell"
  | "nginx"
  | "dockerfile"
  | "toml"
  | "properties"
  | "plain";

/** Editor language from a file name (compose files, .env, nginx confs, ...). */
export function langFor(name: string): LangKey {
  const n = name.toLowerCase();
  if (n === "dockerfile" || n.startsWith("dockerfile.") || n.endsWith(".dockerfile") || n === "containerfile")
    return "dockerfile";
  if (n === ".env" || n.startsWith(".env.") || n.endsWith(".env")) return "properties";
  if (n === "nginx.conf" || n.endsWith(".nginx") || n.endsWith(".vhost")) return "nginx";
  if ([".bashrc", ".zshrc", ".profile", ".bash_profile", ".bash_aliases"].includes(n)) return "shell";
  const ext = n.includes(".") ? n.slice(n.lastIndexOf(".") + 1) : "";
  const map: Record<string, LangKey> = {
    yml: "yaml",
    yaml: "yaml",
    json: "json",
    jsonc: "json",
    js: "javascript",
    mjs: "javascript",
    cjs: "javascript",
    ts: "typescript",
    py: "python",
    md: "markdown",
    markdown: "markdown",
    html: "html",
    htm: "html",
    css: "css",
    xml: "xml",
    svg: "xml",
    sh: "shell",
    bash: "shell",
    zsh: "shell",
    conf: "properties",
    ini: "properties",
    cfg: "properties",
    properties: "properties",
    service: "properties",
    timer: "properties",
    toml: "toml",
  };
  return map[ext] ?? "plain";
}
