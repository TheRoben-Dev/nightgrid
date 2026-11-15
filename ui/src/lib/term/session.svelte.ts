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
import { api, errorText, type ConnectError, type PromptKind, type TermMessage, type TermState, type VmKind } from "../api";
import { openExternal, readClipboard, writeClipboard } from "../clipboard";
import { actionFor } from "../keys";
import { files } from "../stores/files.svelte";
import { settings } from "../stores/settings.svelte";
import { resyncStyles } from "./cspStyles";
import { xtermTheme } from "./theme";

export type PaneState = TermState | "idle" | "prompt" | "failed" | "exited";

export type Origin =
  | { kind: "local" }
  /** `container` set: a shell inside that container (`docker exec -it`). */
  | {
      kind: "ssh";
      hostId: number;
      name: string;
      container?: { id: string; name: string };
      /** A guest's console (`virsh console`, `qm terminal`, `pct enter`). */
      vm?: { kind: VmKind; id: string; name: string; asRoot: boolean };
    };

/** OSC number the remote edit helper uses (see core `edit_hook`). */
export const EDIT_OSC = 7766;

function newToken(): string {
  const b = new Uint8Array(16);
  crypto.getRandomValues(b);
  return Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
}

/** `token;base64(path)` → absolute path, or null if it isn't ours. */
export function parseEditRequest(data: string, token: string): string | null {
  const i = data.indexOf(";");
  if (i < 0 || data.slice(0, i) !== token) return null;
  try {
    const path = new TextDecoder("utf-8", { fatal: true }).decode(b64ToBytes(data.slice(i + 1).trim()));
    return path.startsWith("/") ? path : null;
  } catch {
    return null;
  }
}

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
  private fitFrame = 0;
  private gl: WebglAddon | null = null;
  /** Size the backend PTY was last told (null until the session is open). */
  private sentSize: { cols: number; rows: number } | null = null;
  private readonly onDocEvent = () => this.redraw();
  private disposed = false;
  /** Per-start secret for the "edit in Nightgrid" hook (null when off). */
  private editToken: string | null = null;
  /** Brings this pane to the front (set by the terminal store). */
  reveal: (() => void) | null = null;

  constructor(paneId: number, origin: Origin) {
    this.paneId = paneId;
    this.origin = origin;
    this.el = document.createElement("div");
    this.el.className = "term-surface";
    const s = settings.value;
    this.term = new Terminal({
      allowProposedApi: true,
      // "NG Symbols" (bundled Nerd Font symbols) only covers icon code points, so text stays in JetBrains Mono.
      fontFamily: '"NG Symbols", "JetBrains Mono", ui-monospace, "SF Mono", Menlo, Consolas, monospace',
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
    this.term.onResize(() => this.sendResize());
    this.term.onSelectionChange(() => {
      if (settings.value.termCopyOnSelect && this.term.hasSelection()) void writeClipboard(this.term.getSelection());
    });
    this.term.attachCustomKeyEventHandler((e) => this.onKey(e));
    this.term.parser.registerOscHandler(EDIT_OSC, (data) => this.onEditRequest(data));
    // Repaint when the window comes back or a font (e.g. the icon font) finishes loading.
    document.addEventListener("visibilitychange", this.onDocEvent);
    document.fonts?.addEventListener?.("loadingdone", this.onDocEvent);
    this.el.addEventListener("contextmenu", (e) => {
      if (!settings.value.termRightClickPaste) return;
      e.preventDefault();
      void this.pasteClipboard();
    });
  }

  get title(): string {
    if (this.origin.kind === "local") return "local";
    const sub = this.origin.container?.name ?? this.origin.vm?.name;
    return sub ? `${this.origin.name} ▸ ${sub}` : this.origin.name;
  }

  /** Puts the terminal into `host` (first call opens xterm). */
  attach(host: HTMLElement) {
    if (this.disposed) return;
    const moved = this.opened && this.el.parentElement !== host;
    if (this.el.parentElement !== host) host.appendChild(this.el);
    if (!this.opened) {
      this.term.open(this.el);
      this.loadWebgl();
      this.opened = true;
    }
    resyncStyles(this.el);
    this.refit();
    // Splitting re-parents the pane; WebKit can drop the canvas contents on a move.
    if (moved) requestAnimationFrame(() => this.redraw());
  }

  private loadWebgl() {
    try {
      const gl = new WebglAddon();
      // WebKit caps live WebGL contexts and may drop one (many panes, sleep, GPU switch):
      // fall back to xterm's DOM renderer and repaint instead of going blank.
      gl.onContextLoss(() => {
        gl.dispose();
        if (this.gl === gl) this.gl = null;
        this.redraw();
      });
      this.term.loadAddon(gl);
      this.gl = gl;
    } catch {
      /* no WebGL: xterm keeps its DOM renderer */
    }
  }

  /** Repaints every row (and rebuilds the glyph atlas). */
  redraw() {
    if (!this.opened || this.disposed || document.visibilityState === "hidden") return;
    try {
      this.gl?.clearTextureAtlas();
      this.term.refresh(0, this.term.rows - 1);
    } catch {
      /* disposed mid-frame */
    }
  }

  /** Takes the terminal out of `host` without disposing it. */
  detach(host: HTMLElement) {
    if (this.el.parentElement === host) host.removeChild(this.el);
  }

  /** Fits to the pane once layout has settled (one fit per frame, however many resizes). */
  refit() {
    if (this.fitFrame) return;
    this.fitFrame = requestAnimationFrame(() => {
      this.fitFrame = 0;
      if (!this.opened || this.disposed || !this.el.isConnected || this.el.clientWidth === 0 || this.el.clientHeight === 0) return;
      try {
        this.fit.fit();
      } catch {
        /* not laid out yet */
      }
      this.syncSize();
    });
  }

  /** Types `text` at the prompt as a bracketed paste; the user presses Enter. */
  paste(text: string) {
    this.term.paste(text);
    this.focus();
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

  /**
   * `nano FILE` / `$EDITOR` on the host printed `OSC 7766 ; token ; b64(path)`.
   * Opens the file in an editor tab; closing the tab answers with the token so
   * the remote helper returns. Anything without our token is ignored.
   */
  private onEditRequest(data: string): boolean {
    const token = this.editToken;
    if (!token || this.origin.kind !== "ssh") return true;
    const path = parseEditRequest(data, token);
    if (!path) return true;
    const { hostId, name } = this.origin;
    files.openFromTerminal(hostId, path, name, () => {
      if (this.backendId !== null && this.editToken === token) void api.termWrite(this.backendId, token + "\r");
      this.reveal?.();
    });
    return true;
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

  private sendResize() {
    if (this.resizeTimer) clearTimeout(this.resizeTimer);
    this.resizeTimer = setTimeout(() => this.syncSize(), 60);
  }

  /**
   * Tells the backend the terminal's current size if it differs from what it
   * was last told. Called after fits and once the session opens, so a resize
   * that happened while connecting (a split, a sidebar toggle) isn't lost.
   */
  private syncSize() {
    if (this.backendId === null) return;
    const { cols, rows } = this.term;
    if (this.sentSize?.cols === cols && this.sentSize.rows === rows) return;
    this.sentSize = { cols, rows };
    void api.termResize(this.backendId, cols, rows);
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
        this.sentSize = { cols, rows };
        this.syncSize();
      } else {
        const o = this.origin;
        this.editToken = !o.container && !o.vm && settings.value.termEditInApp ? newToken() : null;
        const res = o.container
          ? await api.termOpenDockerExec(o.hostId, o.container.id, cols, rows, this.listener(gen))
          : o.vm
            ? await api.termOpenVmConsole(o.hostId, o.vm.kind, o.vm.id, o.vm.asRoot, cols, rows, this.listener(gen))
            : await api.termOpenSsh(o.hostId, cols, rows, this.editToken, this.listener(gen));
        if (gen !== this.generation) return;
        if (res.kind === "opened") {
          this.backendId = res.id;
          this.sentSize = { cols, rows };
          this.syncSize();
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
      this.sentSize = null;
      this.backendId = null;
    }
  }

  dispose() {
    this.disposed = true;
    this.generation++;
    if (this.fitFrame) cancelAnimationFrame(this.fitFrame);
    document.removeEventListener("visibilitychange", this.onDocEvent);
    document.fonts?.removeEventListener?.("loadingdone", this.onDocEvent);
    this.closeBackend();
    this.term.dispose();
    this.el.remove();
  }
}
