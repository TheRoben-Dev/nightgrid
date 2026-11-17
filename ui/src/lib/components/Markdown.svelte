<script lang="ts">
  /**
   * Renders the markdown subset from helper/md.ts as text nodes (never HTML),
   * with headings. `code` renders extra controls beside each code block
   * (copy, send to terminal …).
   */
  import type { Snippet } from "svelte";
  import { parseMd, type Block, type Inline } from "../helper/md";
  import { SECTIONS, type SectionId } from "../sections";

  interface Props {
    src: string;
    code?: Snippet<[Extract<Block, { kind: "code" }>]>;
    /** Runbook steps: `- [ ]` checkboxes (ticked for this reading only) and `[label](view:id)` links. */
    steps?: boolean;
    onview?: (view: string) => void;
    /** `[label](kb:shelf/slug)` links to library articles. */
    onarticle?: (id: string) => void;
    /** Kept checklist state (library guides): `block-item` keys, saved by the caller. */
    ticks?: Record<string, boolean>;
    ontick?: (key: string, on: boolean) => void;
    /** Library figures: resolves `![caption](fig:name)` to a bundled image URL. Without it, no images. */
    figure?: (name: string) => string | undefined;
    /** The caller shows a leading figure itself (the Library's briefing banner). */
    hideLead?: boolean;
  }
  let { src, code, steps = false, onview, onarticle, ticks, ontick, figure, hideLead = false }: Props = $props();

  const blocks = $derived(parseMd(src, { headings: true, steps, figures: !!figure }));
  let ticked = $state<Record<string, boolean>>({});
  // A different note (or an edit) starts the checklist over.
  $effect(() => {
    void src;
    ticked = {};
  });
  const isTicked = (key: string, initial: boolean | null) => (ticks ? (ticks[key] ?? !!initial) : (ticked[key] ?? !!initial));
  function tick(key: string, on: boolean) {
    if (ontick) ontick(key, on);
    else ticked[key] = on;
  }
  const view = (id: string) => SECTIONS.find((s) => s.id === (id as SectionId));
  /** A figure opened full size (click to zoom, click or Esc to close). */
  let zoom = $state<{ url: string; alt: string } | null>(null);
</script>

{#snippet inl(parts: Inline[])}{#each parts as t, ti (ti)}{#if t.kind === "code"}<code>{t.text}</code>{:else if t.kind === "bold"}<b>{t.text}</b
      >{:else if t.kind === "em"}<i>{t.text}</i>{:else if t.kind === "view"}{#if t.view.startsWith("kb:")}{#if onarticle}<button type="button" class="vlink" title="Open the article" onclick={() => onarticle(t.view.slice(3))}>{t.text} ▤</button>{:else}{t.text}{/if}{:else if onview && view(t.view)}<button
          type="button"
          class="vlink"
          title="Open {view(t.view)?.label}"
          onclick={() => onview(t.view)}>{t.text} →</button
        >{:else}{t.text}{/if}{:else}{t.text}{/if}{/each}{/snippet}

{#snippet item(parts: Inline[], key: string, check: boolean | null | undefined)}
  {#if check === null || check === undefined}<li>{@render inl(parts)}</li>
  {:else}
    <li class="task" class:done={isTicked(key, check)}>
      <label><input type="checkbox" checked={isTicked(key, check)} onchange={(e) => tick(key, e.currentTarget.checked)} /> <span>{@render inl(parts)}</span></label>
    </li>
  {/if}
{/snippet}

<div class="md">
  {#each blocks as b, bi (bi)}
    {#if b.kind === "h"}
      {#if b.level <= 1}<h2>{@render inl(b.text)}</h2>{:else if b.level === 2}<h3>{@render inl(b.text)}</h3>{:else}<h4>{@render inl(b.text)}</h4>{/if}
    {:else if b.kind === "p"}
      <p>{#each b.lines as line, li (li)}{#if li}<br />{/if}{@render inl(line)}{/each}</p>
    {:else if b.kind === "ul"}
      <ul class:tasks={!!b.checks}>{#each b.items as it, ii (ii)}{@render item(it, `${bi}-${ii}`, b.checks?.[ii])}{/each}</ul>
    {:else if b.kind === "ol"}
      <ol start={b.start ?? 1}>{#each b.items as it, ii (ii)}{@render item(it, `${bi}-${ii}`, b.checks?.[ii])}{/each}</ol>
    {:else if b.kind === "fig"}
      {@const url = bi === 0 && hideLead ? undefined : figure?.(b.name)}
      {#if url}
        <figure class="fig" class:lead={bi === 0}>
          <button type="button" class="frame" title="Show full size" onclick={() => (zoom = { url, alt: b.alt })}><img src={url} alt={b.alt} loading="lazy" decoding="async" draggable="false" /></button>
          {#if b.alt}<figcaption><span class="tag" aria-hidden="true">{bi === 0 ? "◢ BRIEFING" : "◢ INTEL"}</span> {b.alt}</figcaption>{/if}
        </figure>
      {/if}
    {:else if b.kind === "code"}
      <div class="code">
        <pre><code>{b.text}</code></pre>
        {#if code}<div class="acts">{@render code(b)}</div>{/if}
      </div>
    {/if}
  {/each}
</div>

<svelte:window onkeydown={(e) => zoom && e.key === "Escape" && (zoom = null)} />

{#if zoom}
  <button type="button" class="zoom" aria-label="Close the figure" onclick={() => (zoom = null)}>
    <img src={zoom.url} alt={zoom.alt} />
    {#if zoom.alt}<span class="zcap">{zoom.alt}</span>{/if}
  </button>
{/if}

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
  ul.tasks {
    list-style: none;
    padding-left: 4px;
  }
  .task label {
    display: inline-flex;
    gap: 8px;
    align-items: baseline;
    cursor: pointer;
  }
  .task.done span {
    color: var(--text-1);
    text-decoration: line-through;
  }
  .vlink {
    background: none;
    border: 0;
    border-bottom: 1px dashed var(--cyan);
    padding: 0;
    color: var(--cyan);
    font: inherit;
    cursor: pointer;
  }
  .vlink:hover {
    color: var(--magenta);
    border-color: var(--magenta);
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
  /* Figures: framed like an intel card in a quest log. */
  .fig {
    margin: 4px 0 14px;
    max-width: 760px;
  }
  .fig .frame {
    position: relative;
    display: block;
    width: 100%;
    cursor: zoom-in;
    font: inherit;
    color: inherit;
    border: 1px solid var(--line);
    background: #07080b;
    padding: 0;
    line-height: 0;
    clip-path: polygon(0 0, calc(100% - 14px) 0, 100% 14px, 100% 100%, 14px 100%, 0 calc(100% - 14px));
  }
  .fig .frame::before,
  .fig .frame::after {
    content: "";
    position: absolute;
    width: 18px;
    height: 18px;
    pointer-events: none;
  }
  .fig .frame::before {
    top: 0;
    left: 0;
    border-top: 2px solid var(--warn);
    border-left: 2px solid var(--warn);
  }
  .fig .frame::after {
    right: 0;
    bottom: 0;
    border-right: 2px solid var(--cyan);
    border-bottom: 2px solid var(--cyan);
  }
  .fig img {
    display: block;
    width: 100%;
    height: auto;
    user-select: none;
  }
  .zoom {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 3vh 3vw;
    border: 0;
    background: color-mix(in srgb, #000 82%, transparent);
    cursor: zoom-out;
  }
  .zoom img {
    max-width: 94vw;
    max-height: 86vh;
    width: auto;
    height: auto;
    min-width: min(94vw, 960px);
    object-fit: contain;
    border: 1px solid var(--line);
  }
  .zcap {
    font-size: var(--fs-sm);
    color: var(--text-1);
  }
  .fig figcaption {
    margin-top: 5px;
    font-size: var(--fs-xs);
    color: var(--text-1);
    line-height: 1.4;
  }
  .fig .tag {
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
    color: var(--warn);
    margin-right: 4px;
  }
  .fig.lead {
    max-width: none;
    margin-bottom: 16px;
  }
  .acts {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
    padding: 4px 6px;
    border-top: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  }
</style>
