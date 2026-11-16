<script lang="ts">
  import { tunnels } from "../stores/tunnels.svelte";
  import { DEFAULT_KEYMAP, formatChord } from "../keys";
  import { sectionById } from "../sections";
  import { terms } from "../stores/terms.svelte";
  import { ui } from "../stores/ui.svelte";
  import { helper } from "../stores/helper.svelte";
  import { settings } from "../stores/settings.svelte";
  import Drift from "./helper/Drift.svelte";

  const node = $derived.by(() => {
    const o = ui.section === "terminal" ? terms.currentSession?.origin : undefined;
    return o?.kind === "ssh" ? o.name.toUpperCase() : "LOCAL";
  });
  const segments = $derived([`NODE::${node}`, sectionById(ui.section).label.toUpperCase(), ...ui.crumbs]);
</script>

<header class="topbar">
  <!-- Segmented breadcrumb: completed segments in mint, current in magenta. -->
  <ol class="crumbs" aria-label="Breadcrumb">
    {#each segments as seg, i (i)}
      <li class:current={i === segments.length - 1} aria-current={i === segments.length - 1 ? "location" : undefined}>
        <span class="bar" aria-hidden="true"></span>
        <span class="text">{seg}</span>
      </li>
    {/each}
  </ol>

  <button type="button" class="search" data-guide="palette" onclick={() => (ui.paletteOpen = true)} aria-haspopup="dialog">
    <span class="prompt" aria-hidden="true">&gt;_</span>
    <span class="placeholder">Search or run a command</span>
    <kbd>{formatChord(DEFAULT_KEYMAP["palette.open"])}</kbd>
  </button>

  <div class="indicators" aria-label="Connections">
    <span class="ind" title="Active SSH sessions"><span class="glyph" aria-hidden="true">◇</span> SSH <b>{terms.stats.ssh}</b></span>
    {#if settings.on("tunnels")}
      <button type="button" class="ind tun" class:on={tunnels.activeCount > 0} title="Active tunnels" onclick={() => ui.go("tunnels")}><span class="glyph" aria-hidden="true">⇄</span> TUN <b>{tunnels.activeCount}</b></button>
    {/if}
    {#if settings.on("drift")}<button
      type="button"
      class="ind drift"
      class:on={helper.open}
      data-guide="helper"
      title="DRIFT helper ({formatChord(DEFAULT_KEYMAP['helper.toggle'])})"
      aria-pressed={helper.open}
      onclick={() => helper.toggle()}><Drift mood={helper.drift} icon scale={1} /> DRIFT</button
      >{/if}
  </div>
</header>

<style>
  .drift {
    all: unset;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }
  .drift.on {
    color: var(--magenta);
  }
  .drift:focus-visible {
    outline: 1px solid var(--accent);
  }
  .tun {
    all: unset;
    cursor: pointer;
  }
  .tun:focus-visible {
    outline: 1px solid var(--accent);
  }
  .tun.on b {
    color: var(--ok);
  }
  .topbar {
    height: var(--topbar-h);
    flex: none;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 0 12px;
    border-bottom: 1px solid var(--line);
    background: color-mix(in srgb, var(--bg-0) 85%, transparent);
  }

  .crumbs {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    gap: 4px;
    flex: 1;
    min-width: 0;
    max-width: 560px;
  }
  .crumbs li {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .bar {
    height: 3px;
    background: color-mix(in srgb, var(--mint) 70%, transparent);
  }
  .current .bar {
    background: var(--magenta);
    box-shadow: var(--glow-active);
  }
  .text {
    font-family: var(--font-display);
    font-weight: 600;
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .current .text {
    color: var(--text-0);
  }

  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: var(--hit);
    width: min(360px, 32vw);
    padding: 0 8px 0 10px;
    border: 1px solid var(--line);
    background: var(--bg-1);
    color: var(--text-1);
    cursor: pointer;
    text-align: left;
  }
  .search:hover {
    border-color: color-mix(in srgb, var(--mint) 40%, var(--line));
    color: var(--text-0);
  }
  .prompt {
    color: var(--accent);
  }
  .placeholder {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .indicators {
    display: flex;
    gap: 14px;
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
    color: var(--text-1);
  }
  .ind b {
    color: var(--text-0);
    font-weight: 500;
  }
  .glyph {
    color: var(--cyan);
  }
</style>
