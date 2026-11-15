/**
 * Terminal key hints (M10j): the keys of the program in front of a terminal.
 * Data only, so the hint strip and DRIFT read the same maps. The program comes
 * from the SSH shell hook (the command line, reduced to safe characters) or,
 * for local terminals, the PTY's foreground process name.
 */

export interface KeyHint {
  keys: string;
  does: string;
}

export interface KeyMap {
  id: string;
  /** Shown in the strip and to DRIFT. */
  name: string;
  hints: KeyHint[];
}

export const KEYMAPS: KeyMap[] = [
  {
    id: "nano",
    name: "nano",
    hints: [
      { keys: "Ctrl+O Enter", does: "save" },
      { keys: "Ctrl+X", does: "exit" },
      { keys: "Ctrl+W", does: "search" },
      { keys: "Ctrl+K / Ctrl+U", does: "cut / paste line" },
      { keys: "Alt+U", does: "undo" },
      { keys: "Ctrl+_", does: "go to line" },
    ],
  },
  {
    id: "vim",
    name: "vim",
    hints: [
      { keys: "i", does: "insert" },
      { keys: "Esc", does: "normal mode" },
      { keys: ":w Enter", does: "save" },
      { keys: ":q! Enter", does: "quit without saving" },
      { keys: ":wq Enter", does: "save and quit" },
      { keys: "/text", does: "search" },
      { keys: "u", does: "undo" },
    ],
  },
  {
    id: "less",
    name: "pager (less)",
    hints: [
      { keys: "Space / b", does: "page down / up" },
      { keys: "/text", does: "search" },
      { keys: "n / N", does: "next / previous match" },
      { keys: "g / G", does: "top / bottom" },
      { keys: "F", does: "follow new lines" },
      { keys: "q", does: "quit" },
    ],
  },
  {
    id: "follow",
    name: "following output",
    hints: [
      { keys: "Ctrl+C", does: "stop following" },
      { keys: "Enter", does: "blank lines to mark a spot" },
    ],
  },
  {
    id: "htop",
    name: "htop",
    hints: [
      { keys: "F3 or /", does: "search" },
      { keys: "F4 or \\", does: "filter" },
      { keys: "F5 or t", does: "tree" },
      { keys: "F6", does: "sort by" },
      { keys: "F9 or k", does: "kill (asks)" },
      { keys: "q", does: "quit" },
    ],
  },
  {
    id: "top",
    name: "top",
    hints: [
      { keys: "P / M", does: "sort by CPU / memory" },
      { keys: "1", does: "per-CPU" },
      { keys: "k", does: "kill (asks)" },
      { keys: "q", does: "quit" },
    ],
  },
  {
    id: "btop",
    name: "btop",
    hints: [
      { keys: "f", does: "filter" },
      { keys: "← →", does: "sort column" },
      { keys: "k", does: "kill (asks)" },
      { keys: "Esc", does: "menu" },
      { keys: "q", does: "quit" },
    ],
  },
  {
    id: "tmux",
    name: "tmux",
    hints: [
      { keys: "Ctrl+B d", does: "detach" },
      { keys: "Ctrl+B c", does: "new window" },
      { keys: "Ctrl+B n / p", does: "next / previous window" },
      { keys: 'Ctrl+B % / "', does: "split" },
      { keys: "Ctrl+B [", does: "scroll (q to leave)" },
    ],
  },
  {
    id: "screen",
    name: "screen",
    hints: [
      { keys: "Ctrl+A d", does: "detach" },
      { keys: "Ctrl+A c", does: "new window" },
      { keys: "Ctrl+A n / p", does: "next / previous window" },
      { keys: "Ctrl+A Esc", does: "scroll (Esc to leave)" },
    ],
  },
  {
    id: "watch",
    name: "watch",
    hints: [{ keys: "Ctrl+C", does: "stop" }],
  },
  {
    id: "mc",
    name: "Midnight Commander",
    hints: [
      { keys: "Tab", does: "other panel" },
      { keys: "F5 / F6", does: "copy / move" },
      { keys: "F4", does: "edit" },
      { keys: "F8", does: "delete (asks)" },
      { keys: "F10", does: "quit" },
    ],
  },
  {
    id: "ssh",
    name: "nested ssh",
    hints: [
      { keys: "Enter ~ .", does: "drop a stuck connection" },
      { keys: "exit", does: "leave" },
    ],
  },
];

const BY_ID = new Map(KEYMAPS.map((m) => [m.id, m]));
export const keymap = (id: string | null): KeyMap | null => (id ? (BY_ID.get(id) ?? null) : null);

const ALIASES: Record<string, string> = {
  nano: "nano",
  pico: "nano",
  vi: "vim",
  vim: "vim",
  nvim: "vim",
  view: "vim",
  vimdiff: "vim",
  less: "less",
  more: "less",
  most: "less",
  man: "less",
  htop: "htop",
  top: "top",
  btop: "btop",
  bpytop: "btop",
  tmux: "tmux",
  screen: "screen",
  watch: "watch",
  mc: "mc",
  ssh: "ssh",
};

/** Words that run the next word: `sudo htop`, `env X=1 vim`. */
const WRAPPERS = new Set(["sudo", "doas", "env", "exec", "time", "nice", "nohup", "command", "builtin", "caffeinate", "stdbuf", "ionice"]);
/** Wrapper options that take a value (`sudo -u bob`, `nice -n 10`). */
const TAKES_VALUE: Record<string, string[]> = {
  sudo: ["-u", "-g", "-C", "-p", "-h", "-U", "-r", "-t", "-D"],
  doas: ["-u", "-C"],
  env: ["-u", "-C", "-S"],
  nice: ["-n"],
  ionice: ["-c", "-n", "-p"],
};

function words(segment: string): string[] {
  const w = segment.trim().split(/\s+/).filter(Boolean);
  let i = 0;
  while (i < w.length) {
    const base = w[i].split("/").pop() ?? w[i];
    if (/^[A-Za-z_][A-Za-z0-9_]*=/.test(w[i])) {
      i++;
    } else if (WRAPPERS.has(base)) {
      i++;
      while (i < w.length && w[i].startsWith("-")) i += TAKES_VALUE[base]?.includes(w[i]) ? 2 : 1;
    } else break;
  }
  return w.slice(i);
}

const has = (w: string[], ...flags: string[]) => w.some((x) => flags.includes(x) || (/^-[a-zA-Z]+$/.test(x) && flags.some((f) => f.length === 2 && x.includes(f[1]))));

function classify(w: string[]): string | null {
  if (!w.length) return null;
  const prog = (w[0].split("/").pop() ?? w[0]).toLowerCase();
  const sub = w[1];
  switch (prog) {
    case "journalctl":
      return has(w, "-f", "--follow") ? "follow" : "less";
    case "tail":
      return has(w, "-f", "-F", "--follow") ? "follow" : null;
    case "docker":
    case "podman":
    case "kubectl":
      if (sub === "logs" || (sub === "compose" && w[2] === "logs")) return has(w, "-f", "--follow") ? "follow" : null;
      if (sub === "stats" || sub === "events") return "watch";
      return null;
    case "git":
      return ["log", "diff", "show", "blame"].includes(sub ?? "") ? "less" : null;
    case "systemctl":
      return sub === "status" ? "less" : null;
    case "dmesg":
      return has(w, "-w", "--follow") ? "follow" : null;
  }
  return ALIASES[prog] ?? null;
}

/**
 * The key map for a command line (from the shell hook) or a process name
 * (local terminals). A pager at the end of a pipeline wins: `dmesg | less`.
 */
export function programFromCommand(line: string | null): string | null {
  if (!line) return null;
  const segments = line.split("|");
  const last = classify(words(segments[segments.length - 1]));
  if (segments.length > 1 && last === "less") return last;
  return classify(words(segments[0]));
}

/** The OSC 7767 payload: `<token>;<command line>`. `undefined` = not ours. */
export function parseHint(payload: string, token: string): string | null | undefined {
  const i = payload.indexOf(";");
  if (i < 0 || payload.slice(0, i) !== token) return undefined;
  const cmd = payload.slice(i + 1).replace(/[^A-Za-z0-9 ._/+:=|-]/g, "").trim();
  return cmd || null;
}

/** Plain-text maps for DRIFT's guide index. */
export function keymapDocs(): { id: string; title: string; text: string }[] {
  return KEYMAPS.map((m) => ({
    id: `keys-${m.id}`,
    title: `Keys in ${m.name}`,
    text: `Keys in ${m.name} (shown in the terminal's key hint strip): ${m.hints.map((h) => `${h.keys}: ${h.does}`).join("; ")}.`,
  }));
}
