<script lang="ts">
  /**
   * Library (M11): the built-in knowledge base inside Knowledge. Three
   * shelves (Using Nightgrid, Homelab basics, Guides), full-text search, and
   * guides read like a quest log: parts are objectives, ticks are kept.
   * Everything is bundled; nothing is fetched.
   */
  import { writeClipboard } from "../clipboard";
  import Markdown from "../components/Markdown.svelte";
  import Panel from "../components/Panel.svelte";
  import { SHELVES, type Shelf } from "../kb/catalog";
  import { ARTICLES, article, parts, search } from "../kb/library";
  import { scramble } from "../scramble";
  import { library } from "../stores/library.svelte";
  import { terms } from "../stores/terms.svelte";
  import { ui } from "../stores/ui.svelte";

  const shelfLabel = (s: Shelf) => SHELVES.find((x) => x.id === s)?.label ?? s;
  const hits = $derived(library.query.trim() ? search(library.query) : null);
  const listed = $derived(
    hits
      ? hits.map((h) => ({ a: h.article, snippet: h.snippet }))
      : ARTICLES.filter((a) => library.shelf === "all" || a.shelf === library.shelf).map((a) => ({ a, snippet: a.summary })),
  );
  const cur = $derived(library.current ? (article(library.current) ?? null) : null);
  const objectives = $derived(cur ? parts(cur) : []);
  const ticks = $derived(cur ? (library.progress[cur.id] ?? {}) : {});

  function done(keys: string[], t: Record<string, boolean>) {
    return keys.filter((k) => t[k]).length;
  }
  function progressOf(id: string): { done: number; total: number } | null {
    const a = article(id);
    if (!a || a.shelf !== "guides") return null;
    const keys = parts(a).flatMap((p) => p.keys);
    return { done: done(keys, library.progress[id] ?? {}), total: keys.length };
  }
  const total = $derived(objectives.reduce((n, p) => n + p.keys.length, 0));
  const doneAll = $derived(objectives.reduce((n, p) => n + done(p.keys, ticks), 0));

  let flash = $state<string | null>(null);
  let flashTimer: ReturnType<typeof setTimeout> | null = null;
  function say(msg: string) {
    flash = msg;
    if (flashTimer) clearTimeout(flashTimer);
    flashTimer = setTimeout(() => (flash = null), 2500);
  }
  async function copy(text: string) {
    await writeClipboard(text);
    say("Copied.");
  }
  function send(text: string) {
    const title = terms.send(text, null);
    say(title ? `Typed into ${title}. Press Enter there to run it.` : "Opened a terminal.");
  }

  let reader = $state<HTMLElement | null>(null);
  $effect(() => {
    void library.current;
    reader?.scrollTo({ top: 0 });
  });
</script>

<div class="lib">
  <Panel title="LIBRARY" prefix=">" count={listed.length} guide="library-list">
    <div class="side">
      <input class="input" type="search" placeholder="Search the library" aria-label="Search the library" data-guide="library-search" bind:value={library.query} />
      {#if !hits}
        <div class="tags" role="group" aria-label="Shelf">
          <button type="button" class="chip" class:on={library.shelf === "all"} aria-pressed={library.shelf === "all"} onclick={() => (library.shelf = "all")}>ALL</button>
          {#each SHELVES as s (s.id)}
            <button type="button" class="chip" class:on={library.shelf === s.id} aria-pressed={library.shelf === s.id} title={s.what} onclick={() => (library.shelf = s.id)}>{s.label}</button>
          {/each}
        </div>
      {/if}
      <ul class="list">
        {#each listed as { a, snippet } (a.id)}
          {@const p = progressOf(a.id)}
          <li>
            <button type="button" class="item" class:sel={a.id === library.current} onclick={() => library.read(a.id)}>
              <span class="t">{a.title}</span>
              <span class="shelf">{shelfLabel(a.shelf)}{#if p && p.done} · {p.done}/{p.total}{/if}</span>
              <span class="snip muted">{snippet}</span>
            </button>
          </li>
        {:else}
          <li class="muted empty">Nothing in the library matches. Try fewer words, or ask DRIFT.</li>
        {/each}
      </ul>
    </div>
  </Panel>

  <section class="main" data-guide="library-article" bind:this={reader}>
    {#if cur}
      <header class="head">
        <span class="kicker">
          <button type="button" class="btn ghost tiny" onclick={() => (library.trail.length ? library.back() : library.read(null))}>← {library.trail.length ? "BACK" : "SHELVES"}</button>
          {shelfLabel(cur.shelf)}
        </span>
        <h1 aria-label={cur.title}><span use:scramble={cur.title}></span></h1>
        {#if flash}<span class="flash" role="status">{flash}</span>{/if}
      </header>

      {#if cur.shelf === "guides" && objectives.length}
        <aside class="quest" aria-label="Objectives" data-guide="library-objectives">
          <div class="qhead">
            <span class="qlabel">OBJECTIVES</span>
            <span class="qcount">{doneAll}/{total}</span>
            <span class="bar" aria-hidden="true"><i style:width={`${total ? (doneAll / total) * 100 : 0}%`}></i></span>
            {#if doneAll}<button type="button" class="btn ghost tiny" onclick={() => library.resetProgress(cur.id)}>RESET</button>{/if}
          </div>
          <ol class="objs">
            {#each objectives as o (o.title)}
              {@const d = done(o.keys, ticks)}
              <li class:complete={d === o.keys.length} class:active={d > 0 && d < o.keys.length}>
                <span class="mark" aria-hidden="true">{d === o.keys.length ? "◆" : d ? "◈" : "◇"}</span>
                <span class="otitle">{o.title}</span>
                <span class="ocount">{d}/{o.keys.length}</span>
              </li>
            {/each}
          </ol>
        </aside>
      {/if}

      <Markdown
        src={cur.body}
        steps
        onview={(v) => ui.go(v as never)}
        onarticle={(id) => library.read(id)}
        ticks={cur.shelf === "guides" ? ticks : undefined}
        ontick={cur.shelf === "guides" ? (k, on) => library.tick(cur.id, k, on) : undefined}
      >
        {#snippet code(b)}
          <button type="button" class="btn ghost tiny" onclick={() => void copy(b.text)}>Copy</button>
          <button type="button" class="btn tiny" title="Types it into the focused terminal; you press Enter" onclick={() => send(b.text)}>Send to terminal</button>
        {/snippet}
      </Markdown>
    {:else}
      <div class="shelves">
        <h1>THE LIBRARY</h1>
        <p class="muted">Built in and offline: how each part of Nightgrid works, the homelab background it assumes, and step-by-step guides for whole setups. Search covers every article; DRIFT reads them too.</p>
        {#each SHELVES as s (s.id)}
          <section class="shelfcard">
            <h2>{s.label} <span class="muted small">· {s.what}</span></h2>
            <div class="cards">
              {#each ARTICLES.filter((a) => a.shelf === s.id) as a (a.id)}
                {@const p = progressOf(a.id)}
                <button type="button" class="card" onclick={() => library.read(a.id)}>
                  <span class="t">{a.title}</span>
                  {#if s.id !== "nightgrid"}<span class="snip muted">{a.summary}</span>{/if}
                  {#if p && p.done}<span class="bar small" aria-label={`${p.done} of ${p.total} steps done`}><i style:width={`${(p.done / p.total) * 100}%`}></i></span>{/if}
                </button>
              {/each}
            </div>
          </section>
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .lib {
    height: 100%;
    display: grid;
    grid-template-columns: minmax(240px, 320px) 1fr;
    grid-template-rows: minmax(0, 1fr);
    gap: 12px;
    min-height: 0;
  }
  .lib > :global(.panel) {
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .lib > :global(.panel .body) {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
    flex: 1;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .chip {
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    font-size: var(--fs-xs);
    padding: 1px 6px;
    cursor: pointer;
  }
  .chip.on {
    color: var(--bg-0);
    background: var(--mint);
    border-color: var(--mint);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow: auto;
    flex: 1;
    min-height: 0;
  }
  .item {
    width: 100%;
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 8px;
    border: 0;
    border-left: 2px solid transparent;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
    background: transparent;
    color: var(--text-0);
    cursor: pointer;
  }
  .item:hover {
    background: color-mix(in srgb, var(--cyan) 6%, transparent);
  }
  .item.sel {
    border-left-color: var(--magenta);
    background: color-mix(in srgb, var(--magenta) 8%, transparent);
  }
  .t {
    font-weight: 600;
  }
  .shelf {
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--cyan);
  }
  .snip {
    font-size: var(--fs-xs);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .empty {
    padding: 8px;
    font-size: var(--fs-xs);
  }
  .main {
    min-height: 0;
    overflow: auto;
    border: 1px solid var(--line);
    padding: 16px 20px 32px;
    background: var(--bg-1);
  }
  .main > :global(.md) {
    max-width: 78ch;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 12px;
    margin-bottom: 12px;
  }
  .kicker {
    flex-basis: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--magenta);
  }
  h1 {
    margin: 0;
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
    font-size: var(--fs-lg, 20px);
    color: var(--text-0);
  }
  .flash {
    font-size: var(--fs-xs);
    color: var(--mint);
  }
  /* Guides: a quest log of objectives (the article's parts). */
  .quest {
    max-width: 78ch;
    border: 1px solid color-mix(in srgb, var(--warn) 55%, var(--line));
    border-left: 3px solid var(--warn);
    background: color-mix(in srgb, var(--warn) 5%, transparent);
    padding: 8px 12px;
    margin-bottom: 16px;
    clip-path: polygon(0 0, calc(100% - 12px) 0, 100% 12px, 100% 100%, 0 100%);
  }
  .qhead {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 6px;
  }
  .qlabel {
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
    color: var(--warn);
    font-size: var(--fs-xs);
  }
  .qcount {
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .bar {
    flex: 1;
    height: 4px;
    background: color-mix(in srgb, var(--line) 70%, transparent);
    position: relative;
    max-width: 240px;
  }
  .bar i {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--warn);
  }
  .bar.small {
    display: block;
    width: 100%;
    max-width: none;
    margin-top: 4px;
  }
  .objs {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: var(--fs-xs);
  }
  .objs li {
    display: grid;
    grid-template-columns: 16px 1fr auto;
    gap: 6px;
    color: var(--text-1);
  }
  .objs li.active {
    color: var(--text-0);
  }
  .objs li.active .mark {
    color: var(--warn);
  }
  .objs li.complete {
    color: var(--text-2);
    text-decoration: line-through;
  }
  .objs li.complete .mark {
    color: var(--mint);
  }
  .ocount {
    color: var(--text-2);
    text-decoration: none;
  }
  .shelves h1 {
    margin-bottom: 6px;
  }
  .shelfcard h2 {
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
    font-size: var(--fs-sm);
    color: var(--cyan);
    margin: 20px 0 8px;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 8px;
  }
  .card {
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 10px;
    border: 1px solid var(--line);
    background: var(--bg-0);
    color: var(--text-0);
    cursor: pointer;
  }
  .card:hover {
    border-color: var(--cyan);
  }
  .small {
    font-size: var(--fs-xs);
  }
</style>
