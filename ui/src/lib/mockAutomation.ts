/**
 * Browser-only stand-in for automation: in-memory snippets and playbooks,
 * a simplified inspect (the real template and danger rules live in Rust)
 * and runs that print canned output, for `pnpm dev`, tests and screenshots.
 */
import type {
  AutomationEvent_Serialize as AutomationEvent,
  Playbook_Serialize as Playbook,
  PlaybookInput_Deserialize as PlaybookInput,
  RunInspect_Serialize as RunInspect,
  RunSpec,
  Snippet,
  SnippetInput_Deserialize as SnippetInput,
  Step_Serialize as Step,
  Variable_Serialize as Variable,
} from "./bindings";

let nextId = 1;
let snippets: Snippet[] = [];
let playbooks: Playbook[] = [];

const BUILTINS = ["host", "hostname", "user"];
const RE = /\{\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*(?:=([^}\n]*))?\}\}/g;

export function mockVariables(texts: string[]): Variable[] {
  const out: Variable[] = [];
  for (const t of texts)
    for (const m of t.matchAll(RE)) {
      if (BUILTINS.includes(m[1])) continue;
      const v = out.find((x) => x.name === m[1]);
      if (!v) out.push({ name: m[1], default: m[2] ?? null });
      else if (v.default === null && m[2] !== undefined) v.default = m[2];
    }
  return out;
}

const quote = (v: string) => (/^[A-Za-z0-9_\-./:@%+,=]+$/.test(v) ? v : `'${v.replace(/'/g, `'\\''`)}'`);

function render(t: string, values: Record<string, string>, q: boolean): { text: string; missing: string[] } {
  const missing: string[] = [];
  const text = t.replace(RE, (_, name: string, def?: string) => {
    const v = values[name] || def;
    if (v === undefined) {
      if (!missing.includes(name)) missing.push(name);
      return `{{${name}}}`;
    }
    return q ? quote(v) : v;
  });
  return { text, missing };
}

const DANGER: [RegExp, string][] = [
  [/(^|[;&|]\s*|sudo\s+)rm\s/, "rm"],
  [/\bdocker\s+(system\s+|image\s+|volume\s+)?prune\b/, "docker prune"],
  [/\bcompose\s+down\b/, "docker compose down"],
  [/\bdocker\s+(rm|stop|restart|kill)\b/, "docker $1"],
  [/\b(reboot|shutdown|poweroff)\b/, "$1"],
  [/\bsystemctl\s+(stop|restart|disable|mask)\b/, "systemctl $1"],
  [/\b(apt|apt-get|dnf)\s+(-y\s+)?(upgrade|full-upgrade|dist-upgrade)\b/, "$1 upgrade"],
  [/\b(kill|pkill|killall)\s/, "$1"],
  [/[^>2&]>\s*(?!\/dev\/)[^\s>&]/, "overwrite (>)"],
];

function danger(cmd: string): string[] {
  const out: string[] = [];
  for (const [re, label] of DANGER) {
    const m = cmd.match(re);
    if (m) {
      const l = label.replace(/\$(\d)/g, (_, i) => m[Number(i)] ?? "");
      if (!out.includes(l)) out.push(l);
    }
  }
  return out;
}

const texts = (s: Step): string[] =>
  s.type === "command" ? [s.command] : s.type === "upload" ? [s.localPath, s.remotePath] : s.type === "waitPort" ? [s.host] : [s.target];

function label(s: Step): string {
  switch (s.type) {
    case "command":
      return `${s.sudo ? "# " : "$ "}${s.command.split("\n")[0]}${s.command.trim().includes("\n") ? " …" : ""}`;
    case "upload":
      return `upload → ${s.remotePath}`;
    case "waitPort":
      return `wait for ${s.host}:${s.port} (${s.timeoutSecs}s)`;
    case "docker":
      return `docker ${s.action} ${s.target}`;
  }
}

function stepDanger(s: Step): string[] {
  if (s.type === "command") return danger(s.command);
  if (s.type === "upload") return [`overwrite ${s.remotePath}`];
  if (s.type === "docker" && (s.action === "stop" || s.action === "restart")) return [`docker ${s.action} ${s.target}`];
  return [];
}

function plan(spec: RunSpec): { name: string; steps: Step[] } {
  if (spec.kind === "command") return { name: "command", steps: [{ type: "command", command: spec.command, sudo: spec.sudo }] };
  if (spec.kind === "snippet") {
    const s = snippets.find((x) => x.id === spec.id);
    if (!s) throw new Error("snippet not found");
    return { name: s.name, steps: [{ type: "command", command: s.command, sudo: s.sudo }] };
  }
  const p = playbooks.find((x) => x.id === spec.id);
  if (!p) throw new Error("playbook not found");
  return { name: p.name, steps: p.steps };
}

function renderStep(s: Step, v: Record<string, string>): { step: Step; missing: string[] } {
  const r = (t: string, q = false) => render(t, v, q);
  switch (s.type) {
    case "command": {
      const c = r(s.command, true);
      return { step: { ...s, command: c.text }, missing: c.missing };
    }
    case "upload": {
      const a = r(s.localPath);
      const b = r(s.remotePath);
      return { step: { ...s, localPath: a.text, remotePath: b.text }, missing: [...a.missing, ...b.missing] };
    }
    case "waitPort": {
      const h = r(s.host);
      return { step: { ...s, host: h.text }, missing: h.missing };
    }
    case "docker": {
      const t = r(s.target);
      return { step: { ...s, target: t.text }, missing: t.missing };
    }
  }
}

const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));
const runs = new Map<number, { stopped: boolean }>();

function normSnippet(i: SnippetInput, id: number): Snippet {
  if (!i.name?.trim()) throw new Error("invalid value for name: give it a name (up to 80 characters)");
  if (!i.command?.trim()) throw new Error("invalid value for command: enter a command");
  return {
    id,
    name: i.name.trim(),
    command: i.command.trimEnd(),
    description: (i.description ?? "").trim(),
    scope: i.scope ?? { kind: "global" },
    sudo: !!i.sudo,
  };
}

function normPlaybook(i: PlaybookInput, id: number): Playbook {
  if (!i.name?.trim()) throw new Error("invalid value for name: give it a name (up to 80 characters)");
  if (!i.steps?.length) throw new Error("invalid value for steps: a playbook has 1 to 50 steps");
  return {
    id,
    name: i.name.trim(),
    description: (i.description ?? "").trim(),
    steps: i.steps as Step[],
    stopOnError: i.stopOnError ?? true,
  };
}

snippets = [
  normSnippet({ name: "Disk usage", command: "df -h -x tmpfs -x devtmpfs", description: "Mounted filesystems" }, nextId++),
  normSnippet({ name: "Tail container logs", command: "docker logs --tail {{lines=100}} {{container}}" }, nextId++),
  normSnippet({ name: "Restart a service", command: "systemctl restart {{unit}}", sudo: true }, nextId++),
  normSnippet({ name: "Clean up Docker", command: "docker system prune -f", description: "Stopped containers, dangling images" }, nextId++),
];
playbooks = [
  normPlaybook(
    {
      name: "Deploy app",
      description: "Pull, restart, wait until it answers",
      steps: [
        { type: "command", command: "cd /opt/{{app=web}} && git pull --ff-only", sudo: false },
        { type: "docker", action: "pull", target: "ghcr.io/example/{{app=web}}:latest" },
        { type: "docker", action: "restart", target: "{{app=web}}" },
        { type: "waitPort", host: "127.0.0.1", port: 8080, timeoutSecs: 60 },
      ],
      stopOnError: true,
    },
    nextId++,
  ),
];

export const mockAutomation = {
  snippets: async (): Promise<Snippet[]> => [...snippets].sort((a, b) => a.name.localeCompare(b.name)),
  saveSnippet: async (id: number | null, i: SnippetInput): Promise<Snippet> => {
    const s = normSnippet(i, id ?? nextId++);
    snippets = id ? snippets.map((x) => (x.id === id ? s : x)) : [...snippets, s];
    return s;
  },
  deleteSnippet: async (id: number): Promise<null> => {
    snippets = snippets.filter((s) => s.id !== id);
    return null;
  },
  playbooks: async (): Promise<Playbook[]> => [...playbooks].sort((a, b) => a.name.localeCompare(b.name)),
  savePlaybook: async (id: number | null, i: PlaybookInput): Promise<Playbook> => {
    const p = normPlaybook(i, id ?? nextId++);
    playbooks = id ? playbooks.map((x) => (x.id === id ? p : x)) : [...playbooks, p];
    return p;
  },
  deletePlaybook: async (id: number): Promise<null> => {
    playbooks = playbooks.filter((p) => p.id !== id);
    return null;
  },
  inspect: async (spec: RunSpec, values: Record<string, string>, hostName: string | null): Promise<RunInspect> => {
    const p = plan(spec);
    const v = hostName ? { ...values, host: hostName, hostname: hostName, user: "admin" } : values;
    const multi = p.steps.length > 1;
    const destructive: string[] = [];
    const missing: string[] = [];
    const steps: string[] = [];
    let preview: string | null = null;
    p.steps.forEach((s, i) => {
      const r = renderStep(s, v);
      for (const m of r.missing) if (!missing.includes(m) && !BUILTINS.includes(m)) missing.push(m);
      steps.push(label(r.step));
      if (!multi && r.step.type === "command") preview = r.step.command;
      const found = r.missing.length ? stepDanger(s) : stepDanger(r.step);
      if (!r.missing.length && s.type === "command") found.push(...stepDanger(s));
      for (const d of new Set(found)) {
        const t = multi ? `step ${i + 1}: ${d}` : d;
        if (!destructive.includes(t)) destructive.push(t);
      }
    });
    return {
      name: p.name,
      variables: mockVariables(p.steps.flatMap(texts)),
      destructive,
      needsSudo: p.steps.some((s) => s.type === "command" && s.sudo),
      steps,
      preview,
      missing,
    };
  },
  run: async (spec: RunSpec, hostIds: number[], values: Record<string, string>, on: (e: AutomationEvent) => void): Promise<number> => {
    const p = plan(spec);
    const id = nextId++;
    const state = { stopped: false };
    runs.set(id, state);
    void (async () => {
      await Promise.all(
        hostIds.map(async (hostId, n) => {
          const t0 = Date.now();
          on({ kind: "hostStart", hostId });
          await delay(150 + n * 120);
          let ok = true;
          let code: number | null = null;
          for (const [i, s0] of p.steps.entries()) {
            if (state.stopped) break;
            const s = renderStep(s0, values).step;
            const ts = Date.now();
            on({ kind: "stepStart", hostId, step: i, label: label(s) });
            const lines =
              s.type === "command"
                ? [`$ ${s.command}`, "Filesystem      Size  Used Avail Use% Mounted on", "/dev/sda1        98G   41G   52G  45% /"]
                : [`${label(s)}: done`];
            for (const text of lines) {
              await delay(80);
              on({ kind: "lines", hostId, lines: [{ stream: s.type === "command" ? "out" : "info", text }] });
            }
            code = hostId % 3 === 0 && s.type === "command" ? 1 : 0;
            if (code) on({ kind: "lines", hostId, lines: [{ stream: "err", text: "error: something went wrong" }] });
            on({ kind: "stepEnd", hostId, step: i, code, error: null, ms: Date.now() - ts });
            if (code) {
              ok = false;
              if (p.steps.length > 1) break;
            }
          }
          on({ kind: "hostEnd", hostId, ok: ok && !state.stopped, code, error: null, needs: null, ms: Date.now() - t0, stopped: state.stopped });
        }),
      );
      on({ kind: "done", stopped: state.stopped });
      runs.delete(id);
    })();
    return id;
  },
  stop: async (id: number): Promise<null> => {
    const r = runs.get(id);
    if (r) r.stopped = true;
    return null;
  },
};
