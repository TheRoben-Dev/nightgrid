<script lang="ts">
  /**
   * Renders the markdown subset from helper/md.ts as text nodes (never HTML),
   * with headings. `code` renders extra controls beside each code block
   * (copy, send to terminal …).
   */
  import type { Snippet } from "svelte";
  import { parseMd, type Block, type Inline } from "../helper/md";

  interface Props {
    src: string;
    code?: Snippet<[Extract<Block, { kind: "code" }>]>;
  }
  let { src, code }: Props = $props();

  const blocks = $derived(parseMd(src, { headings: true }));
</script>

{#snippet inl(parts: Inline[])}{#each parts as t, ti (ti)}{#if t.kind === "code"}<code>{t.text}</code>{:else if t.kind === "bold"}<b>{t.text}</b
      >{:else if t.kind === "em"}<i>{t.text}</i>{:else}{t.text}{/if}{/each}{/snippet}

<div class="md">
  {#each blocks as b, bi (bi)}
    {#if b.kind === "h"}
      {#if b.level <= 1}<h2>{@render inl(b.text)}</h2>{:else if b.level === 2}<h3>{@render inl(b.text)}</h3>{:else}<h4>{@render inl(b.text)}</h4>{/if}
    {:else if b.kind === "p"}
      <p>{#each b.lines as line, li (li)}{#if li}<br />{/if}{@render inl(line)}{/each}</p>
    {:else if b.kind === "ul"}
      <ul>{#each b.items as item, ii (ii)}<li>{@render inl(item)}</li>{/each}</ul>
    {:else if b.kind === "ol"}
      <ol start={b.start ?? 1}>{#each b.items as item, ii (ii)}<li>{@render inl(item)}</li>{/each}</ol>
    {:else if b.kind === "code"}
      <div class="code">
        <pre><code>{b.text}</code></pre>
        {#if code}<div class="acts">{@render code(b)}</div>{/if}
      </div>
    {/if}
  {/each}
</div>

<style>
  .md {
    font-size: var(--fs-sm);
    line-height: 1.55;
    color: var(--text-0);
  }
  .md > :first-child {
    margin-top: 0;
  }
  h2,
  h3,
  h4 {
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
    color: var(--cyan);
    margin: 18px 0 6px;
    font-weight: 600;
  }
  h2 {
    font-size: var(--fs-md, 15px);
  }
  h3 {
    font-size: var(--fs-sm);
  }
  h4 {
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  p {
    margin: 0 0 10px;
  }
  ul,
  ol {
    margin: 0 0 10px;
    padding-left: 22px;
  }
  li {
    margin: 2px 0;
  }
  li::marker {
    color: var(--mint);
  }
  code {
    font-family: var(--font-mono);
    color: var(--cyan);
  }
  .code {
    position: relative;
    margin: 0 0 12px;
    border: 1px solid var(--line);
    border-left: 2px solid var(--mint);
    background: var(--bg-0);
  }
  pre {
    margin: 0;
    padding: 8px 10px;
    overflow-x: auto;
    font-size: var(--fs-sm);
  }
  pre code {
    color: var(--text-0);
    white-space: pre;
  }
  .acts {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
    padding: 4px 6px;
    border-top: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  }
</style>
