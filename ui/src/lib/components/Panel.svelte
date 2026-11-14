<script lang="ts">
  import type { Snippet } from "svelte";
  import { hexId } from "../hex";
  import { scramble } from "../scramble";

  interface Props {
    title: string;
    /** Prefix glyph for the label: `//` for sections, `>` for lists. */
    prefix?: "//" | ">";
    count?: number;
    /** Run the glitch reveal on the title (first open only). */
    reveal?: boolean;
    children: Snippet;
    actions?: Snippet;
  }

  let { title, prefix = "//", count, reveal = false, children, actions }: Props = $props();
  const id = $derived(hexId(title));
</script>

<section class="panel">
  <span class="bracket tl" aria-hidden="true"></span>
  <span class="bracket br" aria-hidden="true"></span>
  <header>
    <h2 class="label">
      <span class="prefix" aria-hidden="true">{prefix}</span>
      {#if reveal}<span use:scramble>{title}</span>{:else}<span>{title}</span>{/if}
      {#if count !== undefined}<span class="count">[{count}]</span>{/if}
    </h2>
    <div class="right">
      {#if actions}{@render actions()}{/if}
      <span class="hex" aria-hidden="true">{id}</span>
    </div>
  </header>
  <div class="body">
    {@render children()}
  </div>
</section>

<style>
  .panel {
    position: relative;
    background: color-mix(in srgb, var(--bg-1) 92%, transparent);
    border: 1px solid var(--line);
    min-width: 0;
    display: flex;
    flex-direction: column;
    transition: border-color var(--dur) var(--ease);
  }

  .panel:focus-within {
    border-color: color-mix(in srgb, var(--mint) 45%, var(--line));
  }

  /* Corner brackets, shown on focus. */
  .bracket {
    position: absolute;
    width: 10px;
    height: 10px;
    border-color: var(--accent);
    border-style: solid;
    opacity: 0;
    transition: opacity var(--dur) var(--ease);
    pointer-events: none;
  }
  .bracket.tl {
    top: -3px;
    left: -3px;
    border-width: 2px 0 0 2px;
  }
  .bracket.br {
    bottom: -3px;
    right: -3px;
    border-width: 0 2px 2px 0;
  }
  .panel:focus-within .bracket {
    opacity: 1;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--gap);
    padding: 8px 12px 7px;
    border-bottom: 1px solid var(--line);
    background-image: linear-gradient(90deg, color-mix(in srgb, var(--mint) 6%, transparent), transparent 60%);
  }

  h2 {
    margin: 0;
    display: flex;
    gap: 6px;
    color: var(--text-0);
  }

  .prefix {
    color: var(--cyan);
  }

  .count {
    color: var(--text-1);
  }

  .right {
    display: flex;
    align-items: center;
    gap: var(--gap);
  }

  .hex {
    font-size: var(--fs-xs);
    color: color-mix(in srgb, var(--text-1) 70%, transparent);
    letter-spacing: 0.08em;
  }

  .body {
    padding: 12px;
    flex: 1;
    min-height: 0;
  }
</style>
