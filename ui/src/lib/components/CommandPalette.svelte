<script lang="ts">
  import { tick } from "svelte";
  import { DEFAULT_KEYMAP, formatChord } from "../keys";
  import { fuzzyFilter } from "../palette/fuzzy";
  import { commandText, type PaletteCommand } from "../palette/types";

  interface Props {
    open: boolean;
    commands: PaletteCommand[];
    onclose: () => void;
  }
  let { open, commands, onclose }: Props = $props();

  let query = $state("");
  let selected = $state(0);
  let input: HTMLInputElement | undefined = $state();
  let list: HTMLUListElement | undefined = $state();
  let returnFocus: Element | null = null;

  const results = $derived(fuzzyFilter(commands, query, commandText));

  $effect(() => {
    if (open) {
      returnFocus = document.activeElement;
      query = "";
      selected = 0;
      tick().then(() => input?.focus());
    }
  });

  $effect(() => {
    // Keep selection in range as results change.
    if (selected >= results.length) selected = Math.max(0, results.length - 1);
  });

  function close() {
    onclose();
    if (returnFocus instanceof HTMLElement) returnFocus.focus();
  }

  async function runAt(i: number) {
    const cmd = results[i];
    if (!cmd) return;
    close();
    await cmd.run();
  }

  function move(delta: number) {
    if (!results.length) return;
    selected = (selected + delta + results.length) % results.length;
    tick().then(() => list?.querySelector(`[data-index="${selected}"]`)?.scrollIntoView({ block: "nearest" }));
  }

  function onkeydown(e: KeyboardEvent) {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        move(1);
        break;
      case "ArrowUp":
        e.preventDefault();
        move(-1);
        break;
      case "Enter":
        e.preventDefault();
        runAt(selected);
        break;
      case "Escape":
        e.preventDefault();
        close();
        break;
      case "Tab":
        // Single focusable control: keep focus in the dialog.
        e.preventDefault();
        break;
    }
  }

  function highlight(text: string, positions: number[]) {
    const set = new Set(positions);
    return [...text].map((ch, i) => ({ ch, hit: set.has(i) }));
  }
</script>

{#if open}
  <div class="scrim" role="presentation" onclick={close}></div>
  <div class="palette chamfer" role="dialog" aria-modal="true" aria-label="Command palette">
    <div class="head">
      <span class="label"><span class="cyan">//</span> COMMAND.PALETTE</span>
      <span class="label muted">[{results.length}]</span>
    </div>
    <div class="qfield">
      <span class="prompt" aria-hidden="true">&gt;</span>
      <input
        bind:this={input}
        bind:value={query}
        {onkeydown}
        type="text"
        spellcheck="false"
        autocomplete="off"
        placeholder="Type a command, section or theme"
        role="combobox"
        aria-expanded="true"
        aria-controls="palette-list"
        aria-activedescendant={results.length ? `palette-opt-${selected}` : undefined}
      />
    </div>
    <ul id="palette-list" role="listbox" bind:this={list}>
      {#each results as cmd, i (cmd.id)}
        <li
          id="palette-opt-{i}"
          data-index={i}
          role="option"
          aria-selected={i === selected}
          class:selected={i === selected}
          onmousemove={() => (selected = i)}
          onclick={() => runAt(i)}
          onkeydown={() => {}}
        >
          <span class="group">{cmd.group}</span>
          <span class="title">
            {#each highlight(cmd.title, cmd.match.positions.map((p) => p - (cmd.group.length + 1))) as part, j (j)}{#if part.hit}<mark
                  >{part.ch}</mark
                >{:else}{part.ch}{/if}{/each}
          </span>
          {#if cmd.action}<kbd>{formatChord(DEFAULT_KEYMAP[cmd.action])}</kbd>{/if}
        </li>
      {:else}
        <li class="none" role="presentation">NO MATCH // try “theme”, “docker” or “effects”</li>
      {/each}
    </ul>
    <div class="foot label">
      <span><kbd>↑</kbd><kbd>↓</kbd> select</span>
      <span><kbd>Enter</kbd> run</span>
      <span><kbd>Esc</kbd> close</span>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 900;
    background: color-mix(in srgb, var(--bg-0) 70%, transparent);
  }
  .palette {
    position: fixed;
    z-index: 901;
    top: 12vh;
    left: 50%;
    transform: translateX(-50%);
    width: min(640px, calc(100vw - 32px));
    max-height: 64vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-1);
    border: 1px solid color-mix(in srgb, var(--mint) 40%, var(--line));
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
    animation: in var(--dur) var(--ease);
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: translate(-50%, -6px);
    }
  }
  .head {
    display: flex;
    justify-content: space-between;
    padding: 10px 14px 6px 18px;
  }
  .cyan {
    color: var(--cyan);
  }
  .qfield {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 10px;
    margin: 0 12px 8px;
    padding: 0 10px;
    border: 1px solid var(--line);
    background: var(--bg-0);
  }
  .qfield:focus-within {
    border-color: var(--accent);
    box-shadow: var(--glow-accent);
  }
  .prompt {
    color: var(--accent);
  }
  input {
    flex: 1;
    min-width: 0;
    height: 38px;
    padding: 0;
    text-align: left;
    -webkit-appearance: none;
    appearance: none;
    border: 0;
    outline: 0;
    background: transparent;
    font-size: var(--fs-lg);
    box-shadow: none;
  }
  input:focus-visible {
    outline: 0;
    box-shadow: none;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0 6px 6px;
    overflow-y: auto;
  }
  li {
    display: grid;
    grid-template-columns: 84px 1fr auto;
    align-items: center;
    gap: 10px;
    min-height: 32px;
    padding: 0 10px;
    border: 1px solid transparent;
    cursor: pointer;
  }
  li.selected {
    border-color: var(--magenta);
    background: color-mix(in srgb, var(--magenta) 14%, transparent);
  }
  .group {
    font-family: var(--font-display);
    font-weight: 600;
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
  }
  li.selected .group {
    color: var(--magenta);
  }
  mark {
    background: transparent;
    color: var(--accent);
  }
  .none {
    display: block;
    padding: 14px 10px;
    color: var(--text-1);
    cursor: default;
  }
  .foot {
    display: flex;
    gap: 16px;
    padding: 8px 18px;
    border-top: 1px solid var(--line);
    font-size: var(--fs-xs);
  }
  .foot kbd {
    margin-right: 3px;
  }
</style>
