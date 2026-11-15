/**
 * Browser-only stand-in for the DRIFT helper: a fake download and canned,
 * streamed replies (with a "show me" action), so the panel can be exercised
 * in `pnpm dev`, tests and screenshots without a model.
 */
import type { ChatTurn, DownloadProgress, HelperEvent, HelperReply, HelperStatus } from "./bindings";

const SIZE = 2_497_281_120;
let have = 0;
let ready = false;
let downloading = false;
let cancel = false;
let stop = false;

const status = (): HelperStatus => ({
  model: "Qwen3 4B Instruct (2507), Q4_K_M",
  license: "Apache-2.0",
  bytes: SIZE,
  state: ready ? "ready" : have > 0 ? "partial" : "missing",
  have: ready ? SIZE : have,
  downloading,
  loaded: ready,
  gpu: true,
});

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
  status: async (): Promise<HelperStatus> => status(),
  async download(on: (p: DownloadProgress) => void): Promise<boolean> {
    downloading = true;
    cancel = false;
    while (have < SIZE) {
      if (cancel) {
        downloading = false;
        return false;
      }
      await wait(60);
      have = Math.min(SIZE, have + SIZE / 40);
      on({ done: have, total: SIZE });
    }
    downloading = false;
    ready = true;
    return true;
  },
  cancelDownload: async () => {
    cancel = true;
  },
  deleteModel: async () => {
    have = 0;
    ready = false;
    return null;
  },
  unload: async () => {},
  async ask(messages: ChatTurn[], on: (e: HelperEvent) => void): Promise<HelperReply> {
    if (!ready) throw new Error("Download the model first (Settings → Helper).");
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
  /** Test hook. */
  reset(r = false) {
    have = r ? SIZE : 0;
    ready = r;
    downloading = false;
  },
};
