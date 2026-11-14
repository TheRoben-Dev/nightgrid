/**
 * One terminal pane: an xterm.js instance plus its backend session (local PTY
 * or SSH shell). The xterm DOM lives in `el`, which outlives the Svelte
 * component showing it, so panes keep scrollback while layouts change.
 */
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { WebglAddon } from "@xterm/addon-webgl";
import { Terminal } from "@xterm/xterm";
import { api, errorText, type ConnectError, type PromptKind, type TermMessage, type TermState } from "../api";
import { openExternal, readClipboard, writeClipboard } from "../clipboard";
import { actionFor } from "../keys";
import { settings } from "../stores/settings.svelte";
import { resyncStyles } from "./cspStyles";
import { xtermTheme } from "./theme";

export type PaneState = TermState | "idle" | "prompt" | "failed" | "exited";

export type Origin =
  | { kind: "local" }
  /** `container` set: a shell inside that container (`docker exec -it`). */
  | { kind: "ssh"; hostId: number; name: string; container?: { id: string; name: string } };

function b64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

export class TermSession {
  readonly paneId: number;
  readonly origin: Origin;
  state = $state<PaneState>("idle");
  /** Status detail (host label, error text). */
  message = $state<string | null>(null);
  attempt = $state(0);
  exitCode = $state<number | null>(null);
  /** Something the user must answer before connecting (host key, password). */
  prompt = $state<ConnectError | null>(null);
  searchOpen = $state(false);

  readonly el: HTMLDivElement;
  readonly term: Terminal;
  private readonly fit = new FitAddon();
  readonly search = new SearchAddon();
  private backendId: number | null = null;
  private generation = 0;
  private opened = false;
  private resizeTimer: ReturnType<typeof setTimeout> | null = null;
  private disposed = false;

  constructor(paneId: number, origin: Origin) {
    this.paneId = paneId;
    this.origin = origin;
    this.el = document.createElement("div");
    this.el.className = "term-surface";
    const s = settings.value;
    this.term = new Terminal({
      allowProposedApi: true,
      fontFamily: '"JetBrains Mono", ui-monospace, "SF Mono", Menlo, Consolas, monospace',
      fontSize: s.termFontSize,
      scrollback: s.termScrollback,
      cursorBlink: s.termCursorBlink,
      cursorStyle: "bar",
      macOptionIsMeta: true,
      theme: xtermTheme(),
    });
    this.term.loadAddon(this.fit);
    this.term.loadAddon(this.search);
    this.term.loadAddon(new Unicode11Addon());
    this.term.unicode.activeVersion = "11";
    this.term.loadAddon(new WebLinksAddon((_e, uri) => void openExternal(uri)));

    this.term.onData((d) => {
      if (this.backendId !== null) void api.termWrite(this.backendId, d);
    });
    this.term.onResize(({ cols, rows }) => this.sendResize(cols, rows));
    this.term.onSelectionChange(() => {
      if (settings.value.termCopyOnSelect && this.term.hasSelection()) void writeClipboard(this.term.getSelection());
    });
    this.term.attachCustomKeyEventHandler((e) => this.onKey(e));
    this.el.addEventListener("contextmenu", (e) => {
      if (!settings.value.termRightClickPaste) return;
      e.preventDefault();
      void this.pasteClipboard();
    });
  }

  get title(): string {
    if (this.origin.kind === "local") return "local";
    return this.origin.container ? `${this.origin.name} ▸ ${this.origin.container.name}` : this.origin.name;
  }

  /** Puts the terminal into `host` (first call opens xterm). */
  attach(host: HTMLElement) {
    if (this.disposed) return;
    if (this.el.parentElement !== host) host.appendChild(this.el);
    if (!this.opened) {
      this.term.open(this.el);
      try {
        const gl = new WebglAddon();
        gl.onContextLoss(() => gl.dispose());
        this.term.loadAddon(gl);
      } catch {
        /* no WebGL: xterm keeps its DOM renderer */
      }
      this.opened = true;
    }
    resyncStyles(this.el);
    this.refit();
  }

  /** Takes the terminal out of `host` without disposing it. */
  detach(host: HTMLElement) {
    if (this.el.parentElement === host) host.removeChild(this.el);
  }

  refit() {
    if (!this.opened || !this.el.isConnected || this.el.clientWidth === 0) return;
    try {
      this.fit.fit();
    } catch {
      /* not laid out yet */
    }
  }

  focus() {
    this.term.focus();
  }

  applySettings() {
    const s = settings.value;
    this.term.options.fontSize = s.termFontSize;
    this.term.options.scrollback = s.termScrollback;
    this.term.options.cursorBlink = s.termCursorBlink;
    this.term.options.theme = xtermTheme();
    this.refit();
  }

  private onKey(e: KeyboardEvent): boolean {
    if (e.type !== "keydown") return true;
    // App shortcuts that apply inside the terminal never reach the shell.
    if (actionFor(e, true)) return false;
    const mod = e.ctrlKey && e.shiftKey && !e.altKey && !e.metaKey;
    if (mod && e.key.toLowerCase() === "c" && this.term.hasSelection()) {
      void writeClipboard(this.term.getSelection());
      return false;
    }
    if (mod && e.key.toLowerCase() === "v") {
      void this.pasteClipboard();
      return false;
    }
    return true;
  }

  async pasteClipboard() {
    try {
      const text = await readClipboard();
      if (text) this.term.paste(text);
    } catch {
      /* clipboard unavailable */
    }
  }

  private sendResize(cols: number, rows: number) {
    if (this.resizeTimer) clearTimeout(this.resizeTimer);
    this.resizeTimer = setTimeout(() => {
      if (this.backendId !== null) void api.termResize(this.backendId, cols, rows);
    }, 60);
  }

  private listener(gen: number) {
    return (m: TermMessage) => {
      if (gen !== this.generation || this.disposed) return;
      switch (m.kind) {
        case "data":
          this.term.write(b64ToBytes(m.b64));
          break;
        case "status":
          this.state = m.state;
          this.attempt = m.attempt;
          this.message = m.message;
          break;
        case "exit":
          this.exitCode = m.code;
          this.state = "exited";
          this.term.write(`\r\n\x1b[2m[process exited${m.code !== null ? ` with code ${m.code}` : ""}]\x1b[0m\r\n`);
          this.backendId = null;
          break;
      }
    };
  }

  /** Opens (or re-opens) the backend session. */
  async start() {
    if (this.disposed) return;
    this.closeBackend();
    const gen = ++this.generation;
    this.prompt = null;
    this.exitCode = null;
    this.state = "connecting";
    this.message = null;
    const { cols, rows } = this.term;
    try {
      if (this.origin.kind === "local") {
        this.backendId = await api.termOpenLocal(cols, rows, this.listener(gen));
      } else {
        const o = this.origin;
        const res = o.container
          ? await api.termOpenDockerExec(o.hostId, o.container.id, cols, rows, this.listener(gen))
          : await api.termOpenSsh(o.hostId, cols, rows, this.listener(gen));
        if (gen !== this.generation) return;
        if (res.kind === "opened") {
          this.backendId = res.id;
        } else if (res.error.kind === "authFailed" || res.error.kind === "other") {
          this.state = "failed";
          this.message = res.error.message;
        } else if (res.error.kind === "network") {
          this.state = "failed";
          this.message = `Cannot reach ${res.error.host}: ${res.error.message}`;
        } else {
          this.state = "prompt";
          this.prompt = res.error;
        }
      }
      if (this.disposed && this.backendId !== null) this.closeBackend();
    } catch (e) {
      if (gen !== this.generation) return;
      this.state = "failed";
      this.message = errorText(e);
    }
  }

  /** Accepts the pending host key and reconnects. */
  async acceptHostKey() {
    const p = this.prompt;
    if (!p || (p.kind !== "hostKeyUnknown" && p.kind !== "hostKeyChanged")) return;
    await api.sshTrustHostKey(p.host, p.port, p.key.openssh, p.kind === "hostKeyChanged");
    await this.start();
  }

  /** Supplies a password/passphrase and reconnects. */
  async provideSecret(secret: string, remember: boolean) {
    const p = this.prompt;
    if (!p || (p.kind !== "passwordRequired" && p.kind !== "passphraseRequired")) return;
    const kind: PromptKind = p.kind === "passwordRequired" ? "password" : "passphrase";
    await api.sshProvideSecret(p.target, kind, secret, remember);
    await this.start();
  }

  /** "Retry now" while the backend is backing off; otherwise a fresh start. */
  retry() {
    if (this.state === "reconnecting" && this.backendId !== null) void api.termReconnect(this.backendId);
    else void this.start();
  }

  cancelPrompt() {
    this.prompt = null;
    this.state = "failed";
    this.message = "Connection cancelled.";
  }

  private closeBackend() {
    if (this.backendId !== null) {
      void api.termClose(this.backendId);
      this.backendId = null;
    }
  }

  dispose() {
    this.disposed = true;
    this.generation++;
    this.closeBackend();
    this.term.dispose();
    this.el.remove();
  }
}
