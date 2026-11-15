<script lang="ts">
  /**
   * Shell history search for the host in the focused terminal (opt-in).
   * Picking a result types it at the prompt; the user presses Enter.
   * Results are not stored anywhere and secret-looking lines never arrive.
   */
  import { api, errorText, type HistoryResult } from "../../api";
  import type { TermSession } from "../../term/session.svelte";
  import { notes } from "../../stores/notes.svelte";
  import { ui } from "../../stores/ui.svelte";

  interface Props {
    session: TermSession;
    hostId: number;
    onclose: () => void;
  }
  let { session, hostId, onclose }: Props = $props();

  let query = $state("");
  let result = $state<HistoryResult | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let active = $state(0);
  let input = $state<HTMLInputElement | null>(null);
  let seq = 0;

  $effect(() => {
    input?.focus();
  });

  $effect(() => {
    const q = query;
    const id = hostId;
    const t = setTimeout(() => void run(id, q), q ? 300 : 0);
    return () => clearTimeout(t);
  });

  async function run(id: number, q: string) {
    const mine = ++seq;
    loading = true;
    try {
      const r = await api.systemHistorySearch(id, q);
      if (mine !== seq) return;
      result = r;
      error = null;
      active = 0;
    } catch (e) {
      if (mine === seq) error = errorText(e);
    } finally {
      if (mine === seq) loading = false;
    }
  }

  function pick(cmd: string) {
    session.paste(cmd);
    onclose();
  }

  function toNote(cmd: string) {
    notes.draft = { id: null, input: { title: cmd.length > 60 ? `${cmd.slice(0, 57)}…` : cmd, body: "```sh\n" + cmd + "\n```", hostId, tags: [], pinned: false, hideFromDrift: false } };
    notes.saveError = null;
    onclose();
    ui.go("notes");
  }

  function onkey(e: KeyboardEvent) {
    const n = result?.entries.length ?? 0;
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
      session.focus();
    } else if (e.key === "ArrowDown" && n) {
      e.preventDefault();
      active = (active + 1) % n;
    } else if (e.key === "ArrowUp" && n) {
      e.preventDefault();
      active = (active - 1 + n) % n;
    } else if (e.key === "Enter" && n) {
      e.preventDefault();
      pick(result!.entries[active].command);
    }
  }

  function when(ts: number | null): string {
    if (ts === null) return "";
    const s = Math.max(0, Date.now() / 1000 - ts);
    return s < 3600 ? `${Math.max(1, Math.floor(s / 60))}m` : s < 86400 ? `${Math.floor(s / 3600)}h` : `${Math.floor(s / 86400)}d`;
  }
</script>

<div class="hist" role="dialog" aria-label="Shell history">
  <div class="top">
    <span class="label">// HISTORY</span>
    <input
      bind:this={input}
      class="input"
      type="search"
      placeholder="Search {session.title}'s shell history"
      aria-label="Search shell history"
      bind:value={query}
      onkeydown={onkey}
    />
    <button type="button" class="btn ghost tiny" aria-label="Close history" onclick={onclose}>×</button>
  </div>
  <div class="body">
    {#if error}
      <p class="error-text pad">{error}</p>
    {:else if result}
      {#if !result.shells.length}
        <p class="muted pad">No bash, zsh or fish history file was found for this user.</p>
      {:else if !result.entries.length}
        <p class="muted pad">{loading ? "Searching…" : "No commands match."}</p>
      {:else}
        <ul role="listbox" aria-label="Matching commands">
          {#each result.entries as e, i (e.command)}
            <li role="option" aria-selected={i === active} class:on={i === active}>
              <button type="button" class="cmd" title="Type it at the prompt (you press Enter)" onclick={() => pick(e.command)} onmouseenter={() => (active = i)}>
                <code>{e.command}</code>
              </button>
              <span class="muted small">{e.shell}{e.when !== null ? ` · ${when(e.when)}` : ""}</span>
              <button type="button" class="btn ghost tiny" title="Start a note with this command" onclick={() => toNote(e.command)}>+ Note</button>
            </li>
          {/each}
        </ul>
      {/if}
    {:else}
      <p class="muted pad">Reading history…</p>
    {/if}
  </div>
  {#if result}
    <div class="foot muted small">
      {result.entries.length}{result.truncated ? "+" : ""} shown{#if result.hidden}&nbsp;· {result.hidden} hidden because they looked like they held a secret{/if} · ↑↓ Enter to type it · Esc closes
    </div>
  {/if}
</div>

<style>
  .hist {
    position: absolute;
    top: 6px;
    right: 6px;
    width: min(560px, calc(100% - 12px));
    max-height: min(420px, calc(100% - 12px));
    display: flex;
    flex-direction: column;
    border: 1px solid var(--mint);
    background: var(--bg-1);
    box-shadow: 0 8px 30px rgba(0, 0, 0, 0.5);
    z-index: 20;
  }
  .top {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 6px 8px;
    border-bottom: 1px solid var(--line);
  }
  .top .label {
    color: var(--cyan);
    white-space: nowrap;
  }
  .top .input {
    flex: 1;
  }
  .body {
    overflow: auto;
    min-height: 0;
    flex: 1;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 2px 8px;
    border-left: 2px solid transparent;
  }
  li.on {
    background: var(--bg-0);
    border-left-color: var(--mint);
  }
  .cmd {
    flex: 1;
    min-width: 0;
    text-align: left;
    background: transparent;
    border: 0;
    padding: 3px 0;
    cursor: pointer;
  }
  .cmd code {
    display: block;
    font-family: var(--font-mono);
    font-size: var(--fs-sm);
    color: var(--text-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .small {
    font-size: var(--fs-xs);
    white-space: nowrap;
  }
  .pad {
    padding: 10px;
    margin: 0;
  }
  .foot {
    padding: 4px 8px;
    border-top: 1px solid var(--line);
  }
</style>
