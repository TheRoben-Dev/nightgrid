<script lang="ts">
  /**
   * The terminal tab bar's "+" menu: this computer or any saved host, opened
   * as a new tab or split next to the focused pane. On Windows "this
   * computer" is one row per shell: PowerShell, cmd and each WSL distro. Type to filter; arrows and
   * Enter pick; Esc closes.
   */
  import { onMount } from "svelte";
  import HostIcon from "../HostIcon.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { terms } from "../../stores/terms.svelte";
  import { settings } from "../../stores/settings.svelte";
  import type { LocalShell } from "../../api";

  interface Props {
    /** The button it opened from. */
    anchor: DOMRect;
    onclose: () => void;
  }
  let { anchor, onclose }: Props = $props();
  const left = $derived(Math.max(8, Math.min(anchor.left, window.innerWidth - 348)));

  type Where = "tab" | "row" | "col";
  let where = $state<Where>("tab");
  let query = $state("");
  let index = $state(0);
  let input: HTMLInputElement | undefined = $state();
  let box: HTMLDivElement | undefined = $state();

  interface Item {
    id: number | null;
    name: string;
    sub: string;
    host?: (typeof hosts.hosts)[number];
    shell?: LocalShell;
  }

  const localItems = $derived.by((): Item[] => {
    if (!terms.localShells.length) return [{ id: null, name: "This computer", sub: "local shell" }];
    const def = settings.value.termLocalShell;
    return terms.localShells.map((s) => ({
      id: null,
      name: s.name,
      sub: `${s.wsl ? "WSL" : s.detail} · this computer${s.id === def || (!def && s.id === terms.localShells[0]?.id) ? " · default" : ""}`,
      shell: s,
    }));
  });

  const items = $derived.by((): Item[] => {
    const q = query.trim().toLowerCase();
    const all: Item[] = [
      ...localItems,
      ...hosts.hosts
        .slice()
        .sort((a, b) => Number(b.favorite) - Number(a.favorite) || a.name.localeCompare(b.name))
        .map((h) => ({ id: h.id, name: h.name, sub: h.hostname, host: h })),
    ];
    if (!q) return all;
    return all.filter(
      (i) =>
        i.name.toLowerCase().includes(q) ||
        i.sub.toLowerCase().includes(q) ||
        (i.id === null && "local this computer".includes(q)) ||
        i.host?.tags.some((t) => t.toLowerCase().includes(q)),
    );
  });

  $effect(() => {
    void query;
    index = 0;
  });

  function pick(i: Item) {
    const shell = i.shell ? { id: i.shell.id, name: i.shell.name } : undefined;
    if (where === "tab") {
      if (i.id === null) terms.openLocal(shell);
      else terms.openHost(i.id);
    } else terms.split(where, i.id, shell);
    onclose();
    queueMicrotask(() => terms.currentSession?.focus());
  }

  function key(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      index = Math.min(index + 1, items.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      index = Math.max(index - 1, 0);
    } else if (e.key === "Enter" && items[index]) {
      e.preventDefault();
      pick(items[index]);
    }
  }

  onMount(() => {
    input?.focus();
    // Ask again each time: a WSL distro may have been installed meanwhile.
    void terms.loadLocalShells(true);
    const away = (e: PointerEvent) => {
      const t = e.target as Node;
      if (box && !box.contains(t) && !(t instanceof Element && t.closest("[data-newterm]"))) onclose();
    };
    document.addEventListener("pointerdown", away, true);
    return () => document.removeEventListener("pointerdown", away, true);
  });

  const canSplit = $derived(!!terms.current);
</script>

<div class="menu" bind:this={box} role="dialog" aria-label="New terminal" style:top="{anchor.bottom + 4}px" style:left="{left}px">
  <div class="where" role="radiogroup" aria-label="Open in">
    <button type="button" role="radio" aria-checked={where === "tab"} class:on={where === "tab"} onclick={() => (where = "tab")}>New tab</button>
    <button type="button" role="radio" aria-checked={where === "row"} class:on={where === "row"} disabled={!canSplit} onclick={() => (where = "row")}
      >Split right</button
    >
    <button type="button" role="radio" aria-checked={where === "col"} class:on={where === "col"} disabled={!canSplit} onclick={() => (where = "col")}
      >Split down</button
    >
  </div>
  <input bind:this={input} class="input" placeholder="host, address or #tag…" aria-label="Filter hosts" bind:value={query} onkeydown={key} />
  <ul role="listbox" aria-label="Where to open a shell">
    {#each items as it, i (it.id ?? `local:${it.shell?.id ?? ""}`)}
      <li>
        <button type="button" role="option" aria-selected={i === index} class:sel={i === index} onmouseenter={() => (index = i)} onclick={() => pick(it)}>
          {#if it.host}
            <span class="stripe" style:background={it.host.color ?? "transparent"}></span>
            <HostIcon art={it.host.icon} color={it.host.color} />
          {:else}
            <span class="stripe"></span>
            <span class="local" aria-hidden="true">{it.shell?.wsl ? "λ" : it.shell?.id === "cmd" ? "C:" : ">_"}</span>
          {/if}
          <span class="n">{it.name}</span>
          <span class="s muted">{it.sub}</span>
          {#if it.id !== null && terms.shellFor(it.id)}<span class="open muted" title="Already has a terminal">open</span>{/if}
        </button>
      </li>
    {:else}
      <li class="none muted">No host matches.</li>
    {/each}
  </ul>
</div>

<style>
  .menu {
    position: fixed;
    z-index: 40;
    width: min(340px, calc(100vw - 32px));
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    border: 1px solid var(--magenta);
    background: var(--bg-1);
    box-shadow: var(--glow-active);
  }
  .where {
    display: flex;
    gap: 2px;
  }
  .where button {
    flex: 1;
    padding: 3px 0;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    font-size: var(--fs-xs);
    cursor: pointer;
  }
  .where button.on {
    color: var(--bg-0);
    background: var(--mint);
    border-color: var(--mint);
  }
  .where button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    max-height: 300px;
    overflow: auto;
  }
  li button {
    position: relative;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px 5px 10px;
    border: 0;
    background: transparent;
    color: var(--text-0);
    text-align: left;
    cursor: pointer;
    font-size: var(--fs-sm);
  }
  li button.sel {
    background: color-mix(in srgb, var(--magenta) 14%, transparent);
  }
  .stripe {
    position: absolute;
    left: 0;
    top: 2px;
    bottom: 2px;
    width: 3px;
  }
  .local {
    width: 18px;
    color: var(--cyan);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
  }
  .n {
    white-space: nowrap;
  }
  .s {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-xs);
  }
  .open {
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
  .none {
    padding: 6px 8px;
    font-size: var(--fs-sm);
  }
</style>
