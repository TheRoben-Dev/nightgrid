<script lang="ts">
  import { onMount } from "svelte";
  import type { ConnectError } from "../../api";
  import { terms } from "../../stores/terms.svelte";
  import { settings } from "../../stores/settings.svelte";
  import { ui } from "../../stores/ui.svelte";
  import { authNotices } from "../../stores/authNotices.svelte";
  import type { TermSession } from "../../term/session.svelte";
  import AuthPrompt from "./AuthPrompt.svelte";
  import HostKeyPrompt from "./HostKeyPrompt.svelte";

  interface Props {
    session: TermSession;
    tabId: number;
    active: boolean;
    /** More than one pane in the tab (shows the pane header). */
    multi: boolean;
  }
  let { session, tabId, active, multi }: Props = $props();

  let host: HTMLDivElement | undefined = $state();
  let query = $state("");
  let searchInput: HTMLInputElement | undefined = $state();

  onMount(() => {
    // Hold on to what this pane was mounted with: when a pane closes, its
    // props can already point at nothing by the time this tears down, and a
    // throw here would stop Svelte mounting the pane that takes its place.
    const s = session;
    const h = host;
    if (!h) return;
    s.attach(h);
    const ro = new ResizeObserver(() => s.refit());
    ro.observe(h);
    if (active) s.focus();
    return () => {
      ro.disconnect();
      // Keep the xterm element alive; the store disposes it when the pane closes.
      s.detach(h);
    };
  });

  $effect(() => {
    if (session.searchOpen) queueMicrotask(() => searchInput?.focus());
  });

  type KeyPrompt = Extract<ConnectError, { kind: "hostKeyUnknown" } | { kind: "hostKeyChanged" }>;
  type SecretPrompt = Extract<ConnectError, { kind: "passwordRequired" } | { kind: "passphraseRequired" }>;
  const keyPrompt = $derived(
    session.prompt?.kind === "hostKeyUnknown" || session.prompt?.kind === "hostKeyChanged"
      ? (session.prompt as KeyPrompt)
      : null,
  );
  const secretPrompt = $derived(
    session.prompt?.kind === "passwordRequired" || session.prompt?.kind === "passphraseRequired"
      ? (session.prompt as SecretPrompt)
      : null,
  );

  const STATE_LABEL: Record<string, string> = {
    idle: "IDLE",
    connecting: "LINKING",
    connected: "ONLINE",
    reconnecting: "RELINK",
    disconnected: "OFFLINE",
    prompt: "WAITING",
    failed: "FAILED",
    exited: "ENDED",
  };

  function closeFind() {
    session.searchOpen = false;
    session.search.clearDecorations();
    session.focus();
  }

  function find(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      closeFind();
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      if (e.shiftKey) session.search.findPrevious(query);
      else session.search.findNext(query);
    }
  }
</script>

<div class="pane" class:active class:multi class:solid={settings.value.termSolidBg} onfocusin={() => terms.markActive(tabId, session.paneId)}>
  {#if multi}
    <div class="phead">
      <span class="dot {session.state}" aria-hidden="true"></span>
      <span class="ptitle">{session.title}</span>
      <span class="pstate">{STATE_LABEL[session.state]}</span>
      <button type="button" class="pclose" aria-label="Close pane" onclick={() => terms.closePane(session.paneId)}>×</button>
    </div>
  {/if}
  <div class="surface" bind:this={host}></div>

  {#if session.searchOpen}
    <div class="find">
      <input
        bind:this={searchInput}
        class="input"
        placeholder="find…"
        aria-label="Find in terminal"
        bind:value={query}
        onkeydown={find}
        oninput={() => session.search.findNext(query, { incremental: true })}
      />
      <button type="button" class="fbtn" title="Previous (Shift+Enter)" aria-label="Previous match" onclick={() => session.search.findPrevious(query)}>↑</button>
      <button type="button" class="fbtn" title="Next (Enter)" aria-label="Next match" onclick={() => session.search.findNext(query)}>↓</button>
      <button type="button" class="fbtn" title="Close (Esc)" aria-label="Close find" onclick={closeFind}>×</button>
    </div>
  {/if}

  {#if keyPrompt}
    <div class="overlay" class:red={keyPrompt.kind === "hostKeyChanged"}>
      <HostKeyPrompt prompt={keyPrompt} onaccept={() => session.acceptHostKey()} oncancel={() => session.cancelPrompt()} />
    </div>
  {:else if secretPrompt}
    <div class="overlay">
      <AuthPrompt
        prompt={secretPrompt}
        canRemember={session.origin.kind === "ssh" && ui.keychain}
        onsubmit={(s, r) => session.provideSecret(s, r)}
        oncancel={() => session.cancelPrompt()}
      />
    </div>
  {:else if session.state === "connecting"}
    {@const waiting = session.origin.kind === "ssh" ? authNotices.forHost(session.origin.hostId) : undefined}
    <div class="badge">{waiting?.notice.url ? "WAITING FOR APPROVAL" : "LINKING"}<span class="blink">_</span></div>
  {:else if session.state === "reconnecting"}
    <div class="banner warn">
      <span>Connection lost · reconnecting (attempt {session.attempt}){session.message ? ` · ${session.message}` : ""}</span>
      <button type="button" class="btn ghost" onclick={() => session.retry()}>Retry now</button>
    </div>
  {:else if session.state === "disconnected" || session.state === "failed"}
    <div class="banner bad">
      <span>{session.message ?? "Disconnected."}</span>
      <button type="button" class="btn ghost" onclick={() => session.retry()}>Reconnect</button>
      <button type="button" class="btn ghost" onclick={() => terms.closePane(session.paneId)}>Close</button>
    </div>
  {:else if session.state === "exited"}
    <div class="banner">
      <span>Session ended{session.exitCode !== null ? ` (exit ${session.exitCode})` : ""}.</span>
      <button type="button" class="btn ghost" onclick={() => session.start()}>Restart</button>
      <button type="button" class="btn ghost" onclick={() => terms.closePane(session.paneId)}>Close</button>
    </div>
  {/if}
</div>

<style>
  .pane {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    border: 1px solid transparent;
  }
  .pane.solid {
    background: var(--bg-0);
  }
  .pane.multi {
    border-color: var(--line);
  }
  .pane.multi.active {
    border-color: color-mix(in srgb, var(--magenta) 70%, var(--line));
  }
  .phead {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 22px;
    padding: 0 4px 0 8px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .active .phead {
    color: var(--text-0);
    background: color-mix(in srgb, var(--magenta) 8%, transparent);
  }
  .ptitle {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pstate {
    letter-spacing: 0.12em;
    font-size: 10px;
  }
  .pclose {
    border: 0;
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
    width: 20px;
  }
  .pclose:hover {
    color: var(--text-0);
  }
  .surface {
    flex: 1;
    min-height: 0;
    padding: 6px 0 0 8px;
    overflow: hidden;
  }
  .surface :global(.term-surface) {
    width: 100%;
    height: 100%;
  }
  /* xterm.css paints the viewport black; the theme colours the scroll area instead (transparent unless "solid"). */
  .surface :global(.xterm .xterm-viewport) {
    background-color: transparent;
  }
  .dot {
    width: 6px;
    height: 6px;
    border: 1px solid var(--text-1);
  }
  .dot.connected {
    background: var(--mint);
    border-color: var(--mint);
  }
  .dot.reconnecting,
  .dot.connecting,
  .dot.prompt {
    border-color: var(--warn);
  }
  .dot.failed,
  .dot.disconnected {
    border-color: var(--danger);
  }
  .overlay {
    position: absolute;
    z-index: 10;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px;
    background: color-mix(in srgb, var(--bg-0) 82%, transparent);
    overflow: auto;
  }
  .overlay.red {
    background: color-mix(in srgb, var(--danger) 12%, var(--bg-0));
  }
  .badge {
    position: absolute;
    z-index: 10;
    top: 30px;
    right: 14px;
    font-family: var(--font-display);
    letter-spacing: 0.2em;
    font-size: var(--fs-xs);
    color: var(--warn);
  }
  .blink {
    animation: blink 1s steps(2) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0;
    }
  }
  .banner {
    position: absolute;
    z-index: 10;
    left: 10px;
    right: 10px;
    bottom: 10px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px 6px 12px;
    border: 1px solid var(--line);
    background: var(--bg-1);
    font-size: var(--fs-sm);
  }
  .banner span {
    flex: 1;
  }
  .banner.warn {
    border-color: var(--warn);
  }
  .banner.bad {
    border-color: var(--danger);
  }
  .find {
    position: absolute;
    z-index: 10;
    top: 30px;
    right: 14px;
    width: 280px;
    display: flex;
    gap: 2px;
  }
  .find .input {
    flex: 1;
    min-width: 0;
  }
  .fbtn {
    width: 26px;
    border: 1px solid var(--line);
    background: var(--bg-1);
    color: var(--text-1);
    cursor: pointer;
  }
  .fbtn:hover {
    color: var(--text-0);
    border-color: var(--mint);
  }
</style>
