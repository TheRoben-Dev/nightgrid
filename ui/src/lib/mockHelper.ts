/**
 * Browser-only stand-in for the DRIFT helper: a fake download and canned,
 * streamed replies (with a "show me" action), so the panel can be exercised
 * in `pnpm dev`, tests and screenshots without a model.
 */
import type { ChatTurn, DownloadProgress, HelperEvent, HelperModels, HelperReply, HelperStatus, ModelOption } from "./bindings";

/** The same three models as `core/src/helper/model.rs`. */
const MODELS = [
  { id: "qwen3-4b-instruct-2507-q4km", name: "Qwen3 4B Instruct (2507), Q4_K_M", tier: "light", params: "4B parameters", blurb: "Quick and small. Good for questions about the app and short log explanations.", bytes: 2_497_281_120, need: 3_497_281_120 },
  { id: "qwen3-8b-q4km", name: "Qwen3 8B, Q4_K_M", tier: "balanced", params: "8B parameters", blurb: "Twice the size: follows the facts it's given more closely and explains errors better. A little slower.", bytes: 5_027_784_512, need: 6_527_784_512 },
  { id: "qwen3-30b-a3b-instruct-2507-q4km", name: "Qwen3 30B-A3B Instruct (2507), Q4_K_M", tier: "best", params: "30B parameters, 3B active", blurb: "The most knowledgeable, and quick for its size because only part of it works on each word. Needs a lot of memory.", bytes: 18_556_686_752, need: 19_856_686_752 },
];
/** A pretend 16 GB Apple Silicon Mac: Metal can use about 11 GB. */
const MEM = 17_179_869_184;
const GOOD = (MEM / 3) * 2 - 1e9;
const MAX = (MEM / 3) * 2;

const have: Record<string, number> = {};
const ready: Record<string, boolean> = {};
let downloadingId: string | null = null;
let cancel = false;
let stop = false;
/** Set by the caller from settings (api.ts passes it in). */
let activeId = MODELS[0].id;
let loadedId: string | null = null;

const spec = (id: string) => MODELS.find((m) => m.id === id) ?? MODELS[0];

const status = (): HelperStatus => {
  const m = spec(activeId);
  return {
    id: m.id,
    model: m.name,
    license: "Apache-2.0",
    bytes: m.bytes,
    state: ready[m.id] ? "ready" : have[m.id] ? "partial" : "missing",
    have: ready[m.id] ? m.bytes : (have[m.id] ?? 0),
    downloading: downloadingId !== null,
    downloadingId,
    loaded: loadedId === m.id,
    gpu: true,
    context: loadedId === m.id ? 8192 : 0,
  };
};

function fit(need: number): string {
  return need <= GOOD ? "good" : need <= MAX ? "tight" : "no";
}

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

function canned(question: string): string {
  const q = question.toLowerCase();
  if (q.includes("backup") || q.includes("export"))
    return "Backups live in Settings. Pick a passphrase, type it twice and press **Export…**; tick *Include saved passwords and tokens* if the other machine should get them too.\n\n[[show settings]]\n[[point settings-backup: Export and import are here]]";
  if (q.includes("host"))
    return "Press **+** at the top of the sidebar, fill in the address and user, and pick *SSH agent* if your keys are loaded there.\n\n[[point hosts-add: Add a host here]]";
  return "In this browser preview I can't run a model, but in the app I'd answer from the user guide and what's on your screen.\n\n```sh\ndocker compose logs --tail 50\n```";
}

export const mockHelper = {
  setActive(id: string) {
    activeId = spec(id).id;
  },
  status: async (): Promise<HelperStatus> => status(),
  async models(): Promise<HelperModels> {
    const models: ModelOption[] = MODELS.map((m) => ({
      ...m,
      license: "Apache-2.0",
      state: ready[m.id] ? "ready" : have[m.id] ? "partial" : "missing",
      have: ready[m.id] ? m.bytes : (have[m.id] ?? 0),
      fit: fit(m.need),
      reason:
        fit(m.need) === "no"
          ? `needs about ${(m.need / 1e9).toFixed(1)} GB; this computer has 17 GB in all`
          : `needs about ${(m.need / 1e9).toFixed(1)} GB, about ${Math.round(GOOD / 1e9)} GB of unified memory the GPU can use is free for it`,
    }));
    return {
      hardware: { os: "macos", arch: "aarch64", cpu: "Apple M2 Pro", cores: 10, memory: MEM, unified: true, gpus: [{ name: "Apple M2 Pro GPU (shares memory)", vram: 0 }], gpuBuild: true },
      recommended: [...models].reverse().find((m) => m.fit === "good")?.id ?? MODELS[0].id,
      active: activeId,
      models,
    };
  },
  async download(id: string, on: (p: DownloadProgress) => void): Promise<boolean> {
    if (downloadingId) throw new Error("A model is already downloading.");
    const size = spec(id).bytes;
    downloadingId = id;
    cancel = false;
    have[id] ??= 0;
    while (have[id] < size) {
      if (cancel) {
        downloadingId = null;
        return false;
      }
      await wait(60);
      have[id] = Math.min(size, have[id] + size / 40);
      on({ done: have[id], total: size });
    }
    downloadingId = null;
    ready[id] = true;
    return true;
  },
  cancelDownload: async () => {
    cancel = true;
  },
  deleteModel: async (id: string) => {
    have[id] = 0;
    ready[id] = false;
    if (loadedId === id) loadedId = null;
    return null;
  },
  unload: async () => {
    loadedId = null;
  },
  async ask(messages: ChatTurn[], on: (e: HelperEvent) => void): Promise<HelperReply> {
    if (!ready[activeId]) throw new Error(`Download ${spec(activeId).name} first (Settings → DRIFT).`);
    loadedId = activeId;
    stop = false;
    const last = messages[messages.length - 1]?.content ?? "";
    const q = last.slice(last.lastIndexOf("Question:") + 9);
    on({ kind: "thinking" });
    await wait(300);
    const text = canned(q);
    let out = "";
    for (const piece of text.match(/.{1,6}/gs) ?? []) {
      if (stop) break;
      await wait(25);
      out += piece;
      on({ kind: "text", text: piece });
    }
    return { text: out, finish: stop ? "stopped" : "done", promptTokens: Math.round(last.length / 4), tokens: Math.round(out.length / 4), seconds: 1.2 };
  },
  stop: async () => {
    stop = true;
  },
  /** Test hook: `r` marks the active model downloaded. */
  reset(r = false) {
    for (const k of Object.keys(have)) delete have[k];
    for (const k of Object.keys(ready)) delete ready[k];
    activeId = MODELS[0].id;
    loadedId = null;
    downloadingId = null;
    if (r) ready[activeId] = true;
  },
};
