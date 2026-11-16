/** Browser-only stand-in for custom command widgets (M10x): canned outputs per command. */
import type { CmdReading_Serialize as CmdReading, CmdWidget_Serialize as CmdWidget } from "./bindings";
import { commandHint } from "./cmdwidgets";

let nextId = 3;
let widgets: CmdWidget[] = [
  { id: 1, hostId: 1, name: "Root used", command: "df -P / | tail -1", parse: "regex", pattern: "(\\d+)%", unit: "%", warn: 80, crit: 90, below: false, intervalSecs: 60 },
  { id: 2, hostId: 1, name: "Failed units", command: "systemctl --failed --no-legend --plain", parse: "lines", pattern: "", unit: "", warn: 1, crit: 3, below: false, intervalSecs: 300 },
];

function output(cmd: string): [number, string] {
  if (cmd.includes("df")) return [0, `/dev/sda2 959786032 ${Math.round(700e6 + Math.random() * 1e7)} 210000000 ${76 + Math.round(Math.random() * 6)}% /`];
  if (cmd.includes("loadavg")) return [0, `${(Math.random() * 3).toFixed(2)} 0.80 0.71 2/412 12345`];
  if (cmd.includes("--failed")) return [0, ""];
  if (cmd.includes("who")) return [0, "roben pts/0 2026-11-16 08:12\nroben pts/1 2026-11-16 09:30"];
  if (cmd.includes("docker ps")) return [0, "a1b2c3\nd4e5f6\n0718aa\n99ccdd"];
  if (cmd.includes("zpool")) return [0, "all pools are healthy"];
  if (cmd.includes("grep -q")) return [Math.random() < 0.8 ? 0 : 1, ""];
  if (cmd.startsWith("nope")) return [127, ""];
  return [0, String(Math.round(Math.random() * 100))];
}

function read(w: CmdWidget, exit: number, out: string): CmdReading {
  const at = Math.floor(Date.now() / 1000);
  const base = { exit, output: out || (exit === 127 ? `sh: ${w.command.split(" ")[0]}: not found` : ""), at };
  const lvl = (v: number) => {
    const trips = (t: number | null) => t != null && (w.below ? v <= t : v >= t);
    return trips(w.crit) ? "crit" : trips(w.warn) ? "warn" : w.warn != null || w.crit != null ? "ok" : "info";
  };
  if (w.parse === "exit") return exit === 0 ? { ...base, value: 0, text: "OK", level: "ok" } : { ...base, value: exit, text: `FAIL (${exit})`, level: "crit" };
  if (exit !== 0 && !out) return { ...base, value: null, text: `exit ${exit}: ${base.output}`, level: "error" };
  let v: number | null;
  let text = "";
  if (w.parse === "lines") v = out.split("\n").filter((l) => l.trim()).length;
  else if (w.parse === "number") v = Number(/-?\d+(\.\d+)?/.exec(out)?.[0] ?? NaN);
  else if (w.parse === "regex") {
    const m = new RegExp(w.pattern).exec(out);
    if (!m) return { ...base, value: null, text: "no match", level: "error" };
    text = (m[1] ?? m[0]).trim();
    v = /^-?\d/.test(text) ? Number(text) : null;
    if (v == null) return { ...base, value: null, text, level: "info" };
  } else return { ...base, value: null, text: out.split("\n").find((l) => l.trim()) ?? "", level: exit === 0 ? "info" : "crit" };
  if (v == null || Number.isNaN(v)) return { ...base, value: null, text: "no number in the output", level: "error" };
  return { ...base, value: v, text: text || String(Math.round(v * 100) / 100), level: lvl(v) };
}

async function exec(w: CmdWidget): Promise<CmdReading> {
  const hint = commandHint(w.command);
  if (hint) throw new Error(hint);
  await new Promise((r) => setTimeout(r, 250));
  const [exit, out] = output(w.command.trim());
  return read(w, exit, out);
}

export const mockCmdWidgets = {
  list: async (): Promise<CmdWidget[]> => widgets,
  save: async (w: CmdWidget): Promise<CmdWidget> => {
    const hint = commandHint(w.command);
    if (hint) throw new Error(hint);
    const saved = { ...w, name: w.name.trim(), command: w.command.trim(), id: w.id || nextId++ };
    widgets = [...widgets.filter((x) => x.id !== saved.id), saved].sort((a, b) => a.name.localeCompare(b.name));
    return saved;
  },
  remove: async (id: number): Promise<boolean> => {
    const n = widgets.length;
    widgets = widgets.filter((w) => w.id !== id);
    return widgets.length < n;
  },
  run: async (id: number): Promise<CmdReading> => {
    const w = widgets.find((x) => x.id === id);
    if (!w) throw new Error("that widget was deleted");
    return exec(w);
  },
  tryRun: exec,
};
