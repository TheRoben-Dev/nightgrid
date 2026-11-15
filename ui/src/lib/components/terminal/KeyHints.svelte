<script lang="ts">
  /**
   * One strip under the terminal: the keys of the program in front of the
   * active pane (M10j). Fixed height, so it never resizes the terminal.
   */
  import { keymap } from "../../term/keyhints";
  import type { TermSession } from "../../term/session.svelte";

  interface Props {
    session: TermSession | null;
  }
  let { session }: Props = $props();

  const KEY = "nightgrid.keyhints.hidden";
  const load = (): string[] => {
    try {
      const v = JSON.parse(localStorage.getItem(KEY) ?? "[]");
      return Array.isArray(v) ? v.filter((x) => typeof x === "string") : [];
    } catch {
      return [];
    }
  };
  let hidden = $state<string[]>(load());
  const save = (v: string[]) => {
    hidden = v;
    try {
      localStorage.setItem(KEY, JSON.stringify(v));
    } catch {
      /* private mode: per session only */
    }
  };

  const map = $derived(keymap(session?.hintId ?? null));
  const show = $derived(map && !hidden.includes(map.id) ? map : null);
  const sample = "nano, vim, less, htop, tmux";
</script>

<div class="keys" data-guide="term-key-hints" aria-live="polite">
  {#if show}
    <span class="prog">{show.name.toUpperCase()}</span>
    <ul>
      {#each show.hints as h (h.keys)}
        <li><kbd>{h.keys}</kbd> <span>{h.does}</span></li>
      {/each}
    </ul>
    <button type="button" class="x" title={`Hide hints for ${show.name}`} aria-label={`Hide hints for ${show.name}`} onclick={() => save([...hidden, show.id])}>×</button>
  {:else}
    <span class="idle muted">
      {#if map}Hints for {map.name} are hidden.{:else}Keys show here when {sample} or another known program is in front.{/if}
      {#if hidden.length}<button type="button" class="link" onclick={() => save([])}>Show hidden ({hidden.length})</button>{/if}
    </span>
  {/if}
</div>

<style>
  .keys {
    height: 26px;
    flex: none;
    margin-top: 6px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 8px;
    border: 1px solid var(--line);
    background: var(--bg-1);
    font-size: var(--fs-xs);
    overflow: hidden;
    white-space: nowrap;
  }
  .prog {
    font-family: var(--font-display);
    font-weight: 700;
    letter-spacing: var(--tracking-label);
    color: var(--cyan);
  }
  ul {
    display: flex;
    gap: 14px;
    margin: 0;
    padding: 0;
    list-style: none;
    overflow: hidden;
    flex: 1;
    min-width: 0;
  }
  li {
    color: var(--text-1);
  }
  kbd {
    font-family: var(--font-mono);
    color: var(--text-0);
    border: 1px solid var(--line);
    padding: 0 4px;
    background: var(--bg-2);
  }
  .x {
    border: 0;
    background: none;
    color: var(--text-1);
    cursor: pointer;
    font-size: 14px;
  }
  .x:hover {
    color: var(--text-0);
  }
  .idle {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .link {
    border: 0;
    background: none;
    padding: 0 0 0 8px;
    color: var(--accent);
    cursor: pointer;
    font-size: inherit;
  }
</style>
