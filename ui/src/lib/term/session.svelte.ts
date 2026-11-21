/**
 * One terminal pane: an xterm.js instance plus its backend session (local PTY
 * or SSH shell). The xterm DOM lives in `el`, which outlives the Svelte
 * component showing it, so panes keep scrollback while layouts change.
 */
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { parseHint, programFromCommand } from "./keyhints";
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
  /** `shell`: a Windows shell picked from `termLocalShells` (PowerShell, cmd, a WSL distro). */
  | { kind: "local"; shell?: { id: string; name: string } }
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
/** OSC number the shell hook reports the command about to run with (key hints). */
export const HINT_OSC = 7767;
/** "NG Symbols" (bundled Nerd Font symbols) only covers icon code points, so text stays in JetBrains Mono. */
const FONT = '"NG Symbols", "JetBrains Mono", ui-monospace, "SF Mono", Menlo, Consolas, monospace';
/** The same stack spelled differently: assigning it makes xterm measure the font again. */
const FONT_ALT = FONT.replace("Menlo, ", "Menlo,");
/** How often a local terminal's foreground program is checked while visible. */
const FOREGROUND_POLL_MS = 1200;

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
  /** Command line (SSH hook) or process name (local) in front; null at the prompt. */
  command = $state<string | null>(null);
  /** Key map for what's in front (see `keyhints.ts`). */
  readonly hintId = $derived(programFromCommand(this.command));

  readonly el: HTMLDivElement;
  readonly term: Terminal;
  private readonly fit = new FitAddon();
  readonly search = new SearchAddon();
  private backendId: number | null = null;
  private generation = 0;
  private opened = false;
  private resizeTimer: ReturnType<typeof setTimeout> | null = null;
  private fitFrame = 0;
  /** Size the backend PTY was last told (null until the session is open). */
  private sentSize: { cols: number; rows: number } | null = null;
  private readonly onVisible = () => this.redraw();
  private readonly onFonts = () => this.remeasure();
  private disposed = false;
  /** Per-start secret for the "edit in Nightgrid" hook (null when off). */
  private editToken: string | null = null;
  private editOn = false;
  private fgTimer: ReturnType<typeof setInterval> | null = null;
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
      // xterm's DOM renderer: unlike WebGL it survives being moved between
      // panes in WebKit and lets the app backdrop show through.
      allowTransparency: true,
      fontFamily: FONT,
      fontSize: s.termFontSize,
      scrollback: s.termScrollback,
      cursorBlink: s.termCursorBlink,
      cursorStyle: "bar",
      macOptionIsMeta: true,
      theme: xtermTheme(s.termSolidBg),
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
    this.term.parser.registerOscHandler(HINT_OSC, (data) => {
      const token = this.editToken;
      const cmd = token ? parseHint(data, token) : undefined;
      if (cmd !== undefined) this.command = cmd;
      return true;
    });
    // Repaint when the window comes back; measure again when a font finishes loading.
    document.addEventListener("visibilitychange", this.onVisible);
    document.fonts?.addEventListener?.("loadingdone", this.onFonts);
    this.el.addEventListener("contextmenu", (e) => {
      if (!settings.value.termRightClickPaste) return;
      e.preventDefault();
      void this.pasteClipboard();
    });
  }

  get title(): string {
    if (this.origin.kind === "local") return this.origin.shell?.name ?? "local";
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
      this.opened = true;
    }
    resyncStyles(this.el);
    // Fonts may have finished loading while the pane was off screen (another
    // tab), when xterm can't measure; measure again whenever it comes back.
    if (moved) this.remeasure();
    else this.refit();
  }

  /** Repaints every row. */
  redraw() {
    if (!this.opened || this.disposed || document.visibilityState === "hidden") return;
    try {
      this.term.refresh(0, this.term.rows - 1);
    } catch {
      /* disposed mid-frame */
    }
  }

  /**
   * Makes xterm measure the font again (cell size), then fits. Needed after a
   * web font loads: until then cells are sized for the fallback font and text
   * drifts out of its columns.
   */
  remeasure() {
    if (!this.opened || this.disposed) return;
    if (this.el.isConnected && this.el.clientWidth > 0) {
      this.term.options.fontFamily = this.term.options.fontFamily === FONT ? FONT_ALT : FONT;
    }
    this.refit();
  }

  /** Takes the terminal out of `host` without disposing it. */
  detach(host: HTMLElement) {
    if (this.el.parentElement === host) host.removeChild(this.el);
  }

  /**
   * Fits to the pane once layout has settled: two frames later, so a split
   * that just collapsed or a tab that just appeared has its final size. One
   * fit per frame however many resizes; then the PTY size, then a repaint.
   */
  refit() {
    if (this.fitFrame) return;
    this.fitFrame = requestAnimationFrame(() => {
      this.fitFrame = requestAnimationFrame(() => {
        this.fitFrame = 0;
        if (!this.opened || this.disposed || !this.el.isConnected || this.el.clientWidth === 0 || this.el.clientHeight === 0) return;
        try {
          this.fit.fit();
        } catch {
          /* not laid out yet */
        }
        this.syncSize();
        this.redraw();
      });
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
    this.term.options.theme = xtermTheme(s.termSolidBg);
    this.refit();
  }

  /**
   * `nano FILE` / `$EDITOR` on the host printed `OSC 7766 ; token ; b64(path)`.
   * Opens the file in an editor tab; closing the tab answers with the token so
   * the remote helper returns. Anything without our token is ignored.
   */
  private onEditRequest(data: string): boolean {
    const token = this.editToken;
    if (!token || !this.editOn || this.origin.kind !== "ssh") return true;
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
          this.command = null;
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
    this.command = null;
    const { cols, rows } = this.term;
    try {
      if (this.origin.kind === "local") {
        this.backendId = await api.termOpenLocal(cols, rows, this.origin.shell?.id ?? null, this.listener(gen));
        this.sentSize = { cols, rows };
        this.syncSize();
        this.watchForeground();
      } else {
        const o = this.origin;
        const plain = !o.container && !o.vm;
        this.editOn = plain && settings.value.termEditInApp;
        const hints = plain && settings.value.termKeyHints;
        this.editToken = this.editOn || hints ? newToken() : null;
        const res = o.container
          ? await api.termOpenDockerExec(o.hostId, o.container.id, cols, rows, this.listener(gen))
          : o.vm
            ? await api.termOpenVmConsole(o.hostId, o.vm.kind, o.vm.id, o.vm.asRoot, cols, rows, this.listener(gen))
            : await api.termOpenSsh(o.hostId, cols, rows, this.editToken, this.editOn, hints, this.listener(gen));
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

  /** Local terminals: ask the PTY which program is in front, while the pane is on screen. */
  private watchForeground() {
    if (this.fgTimer) clearInterval(this.fgTimer);
    this.fgTimer = setInterval(() => {
      const id = this.backendId;
      if (id === null || !settings.value.termKeyHints || !this.el.isConnected || document.visibilityState === "hidden") return;
      void api.termForeground(id).then(
        (name) => {
          if (this.backendId === id) this.command = name;
        },
        () => {},
      );
    }, FOREGROUND_POLL_MS);
  }

  dispose() {
    if (this.fgTimer) clearInterval(this.fgTimer);
    this.disposed = true;
    this.generation++;
    if (this.fitFrame) cancelAnimationFrame(this.fitFrame);
    document.removeEventListener("visibilitychange", this.onVisible);
    document.fonts?.removeEventListener?.("loadingdone", this.onFonts);
    this.closeBackend();
    this.term.dispose();
    this.el.remove();
  }
}
