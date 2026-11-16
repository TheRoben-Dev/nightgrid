<script lang="ts">
  /**
   * One guest screen (M10n): draws the patches the backend decodes and sends
   * keyboard and mouse input back. Keys go to the guest only while the screen
   * has focus (click it); the toolbar covers what a host keyboard can't send.
   */
  import { api, type VncInput } from "../../api";
  import { readClipboard, writeClipboard } from "../../clipboard";
  import { desktops, type DesktopTab } from "../../stores/desktops.svelte";
  import { buttonMask, drawPatch, keysymFor, KEYSYM, toGuest, typeText } from "../../vnc";
  import Modal from "../Modal.svelte";

  let { tab }: { tab: DesktopTab } = $props();

  let canvas = $state<HTMLCanvasElement | null>(null);
  let wrap = $state<HTMLDivElement | null>(null);
  let fit = $state(true);
  let focused = $state(false);
  let typing = $state(false);
  let typed = $state("");
  let note = $state<string | null>(null);

  // Frame loop: long-poll for patches while the session is open.
  $effect(() => {
    const sid = tab.session;
    const c = canvas;
    if (sid == null || !c) return;
    let alive = true;
    // A new canvas (the view was reopened) starts blank: ask for the whole screen.
    void api.desktopInput(sid, [{ type: "refresh" }]).catch(() => {});
    (async () => {
      while (alive) {
        try {
          const buf = await api.desktopFrame(sid);
          if (!alive) break;
          if (buf.byteLength) drawPatch(c, buf);
        } catch {
          break;
        }
      }
    })();
    return () => {
      alive = false;
    };
  });

  // Input is batched per animation frame and sent in order, one batch at a time.
  let queue: VncInput[] = [];
  let inflight: Promise<unknown> = Promise.resolve();
  let scheduled = false;
  let pointer: { x: number; y: number } | null = null;
  let mask = 0;

  function flush() {
    scheduled = false;
    const sid = tab.session;
    if (pointer) {
      queue.push({ type: "pointer", x: pointer.x, y: pointer.y, buttons: mask });
      pointer = null;
    }
    if (sid == null || !queue.length) {
      queue = [];
      return;
    }
    const batch = queue;
    queue = [];
    inflight = inflight.then(() => api.desktopInput(sid, batch)).catch(() => {});
  }

  function send(...events: VncInput[]) {
    queue.push(...events);
    if (!scheduled) {
      scheduled = true;
      requestAnimationFrame(flush);
    }
  }

  function guestPoint(e: MouseEvent) {
    const c = canvas;
    if (!c) return null;
    return toGuest(c.getBoundingClientRect(), { width: c.width, height: c.height }, e.clientX, e.clientY);
  }

  function onMove(e: PointerEvent) {
    const p = guestPoint(e);
    if (!p) return;
    mask = buttonMask(e.buttons);
    pointer = p;
    send();
  }

  function onButton(e: PointerEvent) {
    canvas?.focus();
    const p = guestPoint(e);
    if (!p) return;
    mask = buttonMask(e.buttons);
    pointer = null;
    send({ type: "pointer", x: p.x, y: p.y, buttons: mask });
    e.preventDefault();
  }

  let wheelAcc = 0;
  function onWheel(e: WheelEvent) {
    e.preventDefault();
    const p = guestPoint(e);
    if (!p) return;
    wheelAcc += e.deltaMode === 1 ? e.deltaY * 40 : e.deltaY;
    while (Math.abs(wheelAcc) >= 50) {
      const b = wheelAcc < 0 ? 8 : 16;
      wheelAcc -= Math.sign(wheelAcc) * 50;
      send({ type: "pointer", x: p.x, y: p.y, buttons: mask | b }, { type: "pointer", x: p.x, y: p.y, buttons: mask });
    }
  }

  // Keys held down, by physical key, so the release matches the press.
  let held: Record<string, number> = {};

  function onKey(e: KeyboardEvent, down: boolean) {
    if (tab.session == null) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.isComposing) return;
    const id = e.code || e.key;
    if (down) {
      const k = held[id] ?? keysymFor(e);
      if (k === null) return;
      held[id] = k;
      send({ type: "key", keysym: k, down: true });
    } else {
      const k = held[id] ?? keysymFor(e);
      delete held[id];
      if (k !== null) send({ type: "key", keysym: k, down: false });
    }
  }

  function releaseAll() {
    focused = false;
    for (const k of Object.values(held)) send({ type: "key", keysym: k, down: false });
    held = {};
    if (mask) {
      mask = 0;
      if (pointer) send();
    }
  }

  function combo(...keys: number[]) {
    send(...keys.map((k) => ({ type: "key" as const, keysym: k, down: true })), ...keys.reverse().map((k) => ({ type: "key" as const, keysym: k, down: false })));
    canvas?.focus();
  }

  async function sendClipboard() {
    const text = await readClipboard().catch(() => "");
    if (!text) {
      note = "The clipboard is empty.";
      return;
    }
    send({ type: "clipboard", text });
    note = "Sent to the guest's clipboard (needs a clipboard agent in the guest; use Type text otherwise).";
  }

  function typeIt() {
    if (typed) send(...typeText(typed).map((k) => ({ type: "key" as const, ...k })));
    typed = "";
    typing = false;
    canvas?.focus();
  }

  function fullscreen() {
    if (document.fullscreenElement) void document.exitFullscreen();
    else void wrap?.requestFullscreen?.();
  }
</script>

<div class="screen" bind:this={wrap}>
  <div class="bar">
    <span class="title" title={tab.title}>{tab.title}</span>
    {#if tab.status === "connected"}<span class="muted small">{tab.width}×{tab.height}</span>{/if}
    <span class="state s-{tab.status}">{tab.status.toUpperCase()}</span>
    <span class="spacer"></span>
    {#if tab.status === "connected"}
      <button type="button" class="btn ghost tiny" data-guide="desktop-cad" title="Send Ctrl+Alt+Del" onclick={() => combo(KEYSYM.ctrl, KEYSYM.alt, KEYSYM.del)}>Ctrl+Alt+Del</button>
      <button type="button" class="btn ghost tiny" data-guide="desktop-type" title="Type text into the guest key by key" onclick={() => (typing = true)}>Type text…</button>
      <button type="button" class="btn ghost tiny" title="Put this computer's clipboard on the guest's" onclick={sendClipboard}>Send clipboard</button>
      {#if tab.clipboard}
        <button type="button" class="btn ghost tiny" title="Copy what the guest put on its clipboard" onclick={() => tab.clipboard && void writeClipboard(tab.clipboard)}>Copy guest clipboard</button>
      {/if}
      <button type="button" class="btn ghost tiny" onclick={() => (fit = !fit)}>{fit ? "1:1" : "Fit"}</button>
      <button type="button" class="btn ghost tiny" onclick={fullscreen}>Full</button>
      <button type="button" class="btn ghost tiny" title="Redraw the whole screen" onclick={() => send({ type: "refresh" })}>Redraw</button>
      <button type="button" class="btn ghost tiny" onclick={() => desktops.disconnect(tab.key)}>Disconnect</button>
    {:else if tab.status === "closed"}
      <button type="button" class="btn primary tiny" onclick={() => desktops.connect(tab.key)}>Reconnect</button>
    {/if}
  </div>
  {#if note}
    <p class="note muted small" role="status">{note}<button type="button" class="x" aria-label="Dismiss" onclick={() => (note = null)}>×</button></p>
  {/if}

  <div class="view" class:fit class:live={tab.status === "connected"}>
    {#if tab.status === "connecting"}
      <p class="center muted">Opening the display of {tab.vm.name} on {tab.hostName}…</p>
    {:else if tab.status === "closed"}
      <div class="center">
        <p class="label">{tab.error ? "DISCONNECTED" : "CLOSED"}</p>
        {#if tab.error}<p class="error-text">{tab.error}</p>{/if}
      </div>
    {/if}
    <canvas
      bind:this={canvas}
      class:hidden={tab.status !== "connected"}
      class:focused
      tabindex="0"
      data-vm-screen
      aria-label="Screen of {tab.vm.name}. Click to send keys to the guest."
      onpointermove={onMove}
      onpointerdown={onButton}
      onpointerup={onButton}
      onwheel={onWheel}
      oncontextmenu={(e) => e.preventDefault()}
      onkeydown={(e) => onKey(e, true)}
      onkeyup={(e) => onKey(e, false)}
      onfocus={() => (focused = true)}
      onblur={releaseAll}
    ></canvas>
  </div>
  {#if tab.status === "connected"}
    <p class="hint muted small">{focused ? "Keys go to the guest. Click outside the screen to get them back." : "Click the screen to type into the guest."}</p>
  {/if}
</div>

<Modal open={typing} title="TYPE INTO {tab.vm.name.toUpperCase()}" width={520} onclose={() => (typing = false)}>
  <p class="muted">Each character is sent as a key press, so it works in installers, login screens and guests without a clipboard agent. It assumes the guest's keyboard layout matches the characters.</p>
  <textarea class="input" rows="4" bind:value={typed} spellcheck="false" autocomplete="off"></textarea>
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (typing = false)}>Cancel</button>
    <button type="button" class="btn primary" disabled={!typed} onclick={typeIt}>Type it</button>
  {/snippet}
</Modal>

<style>
  .screen {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
    background: var(--bg-0);
  }
  .bar {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .title {
    color: var(--text-0);
    font-size: var(--fs-sm);
    letter-spacing: 0.04em;
    max-width: 30ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .spacer {
    flex: 1;
  }
  .state {
    letter-spacing: 0.1em;
    color: var(--text-1);
  }
  .s-connected {
    color: var(--ok);
  }
  .s-connecting {
    color: var(--cyan);
  }
  .note {
    margin: 0;
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .x {
    background: none;
    border: 0;
    color: inherit;
    cursor: pointer;
  }
  .view {
    flex: 1;
    min-height: 0;
    overflow: auto;
    position: relative;
    border: 1px solid var(--line);
    background: #000;
  }
  .view.fit {
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .view.fit canvas {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
  canvas {
    display: block;
    image-rendering: auto;
    outline: none;
    cursor: default;
  }
  canvas.focused {
    box-shadow: 0 0 0 1px var(--magenta);
  }
  canvas.hidden {
    display: none;
  }
  .center {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 20px;
    text-align: center;
    margin: 0;
  }
  .hint {
    margin: 0;
  }
  textarea {
    width: 100%;
    font-family: var(--font-mono);
  }
</style>
