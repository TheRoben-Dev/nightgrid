<script lang="ts">
  /**
   * Library (M11): the built-in knowledge base inside Knowledge. Shelves
   * (Using Nightgrid, How-tos, Homelab basics, Linux skills, Self-hosted
   * apps, Guides), topics across them, full-text search, and step-by-step
   * articles read like a quest log: parts are objectives, ticks are kept.
   * Everything is bundled and works offline; newer signed content can
   * replace the bundled shelves between releases (D-174, stores/kbcontent).
   */
  import { writeClipboard } from "../clipboard";
  import Markdown from "../components/Markdown.svelte";
  import Panel from "../components/Panel.svelte";
  import { CATEGORIES, SHELVES, categoryOf, type Shelf } from "../kb/catalog";
  import type { Article } from "../kb/library";
  import { FIG_LINE } from "../helper/md";
  import { figure } from "../kb/figures";
  import { ARTICLES, article, parts, search, stepChecks } from "../kb/library";
  import { questSummary } from "../kb/quest";
  import { hosts } from "../stores/hosts.svelte";
  import { quests } from "../stores/quests.svelte";
  import { scramble } from "../scramble";
  import { kbContent, kbVersionLabel } from "../stores/kbcontent.svelte";
  import { library } from "../stores/library.svelte";
  import { terms } from "../stores/terms.svelte";
  import { ui } from "../stores/ui.svelte";

  const shelfLabel = (s: Shelf) => SHELVES.find((x) => x.id === s)?.label ?? s;
  /** Shelves whose articles have `- [ ]` steps worth ticking. */
  const tracked = (a: Article) => a.shelf !== "nightgrid" && a.shelf !== "basics";
  const inShelf = (a: Article) => library.shelf === "all" || a.shelf === library.shelf;
  const inCat = (a: Article) => library.cat === "all" || categoryOf(a.id) === library.cat;
  // Newer downloaded content, if any, before the first read; `rev` changes when the text does.
  $effect(() => {
    void kbContent.ensure();
  });
  const all = $derived.by(() => {
    void kbContent.rev;
    return [...ARTICLES];
  });
  const hits = $derived.by(() => {
    void kbContent.rev;
    return library.query.trim() ? search(library.query, 60) : null;
  });
  const listed = $derived(
    hits
      ? hits.filter((h) => inShelf(h.article) && inCat(h.article)).map((h) => ({ a: h.article, snippet: h.snippet }))
      : all.filter((a) => inShelf(a) && inCat(a)).map((a) => ({ a, snippet: a.summary })),
  );
  /** Counts beside each filter, given the other filter. */
  const shelfCount = $derived(new Map(SHELVES.map((s) => [s.id, all.filter((a) => a.shelf === s.id && inCat(a)).length])));
  const catCount = $derived(new Map(CATEGORIES.map((c) => [c.id, all.filter((a) => categoryOf(a.id) === c.id && inShelf(a)).length])));
  const catLabel = (id: string) => CATEGORIES.find((c) => c.id === categoryOf(id))?.label ?? "";
  const cur = $derived.by(() => {
    void kbContent.rev;
    return library.current ? (article(library.current) ?? null) : null;
  });
  /** A figure on the article's first line is its briefing banner, shown above the objectives. */
  const lead = $derived.by(() => {
    const m = cur ? FIG_LINE.exec(cur.body.split("\n", 1)[0]) : null;
    const url = m ? figure(m[2]) : undefined;
    return m && url ? { url, alt: m[1].trim() } : null;
  });
  const objectives = $derived(cur ? parts(cur) : []);
  const ticks = $derived(cur ? (library.progress[cur.id] ?? {}) : {});

  function done(keys: string[], t: Record<string, boolean>) {
    return keys.filter((k) => t[k]).length;
  }
  function progressOf(id: string): { done: number; total: number } | null {
    const a = article(id);
    if (!a || !tracked(a)) return null;
    const keys = parts(a).flatMap((p) => p.keys);
    if (!keys.length) return null;
    return { done: done(keys, library.progress[id] ?? {}), total: keys.length };
  }
  const total = $derived(objectives.reduce((n, p) => n + p.keys.length, 0));
  /** Steps a host check can verify, by block-item key. */
  const checkable = $derived(cur ? stepChecks(cur) : {});
  const nCheckable = $derived(Object.keys(checkable).length);
  let questHost = $state<number | null>(hosts.preferred("library")?.id ?? null);
  $effect(() => {
    if (questHost != null && !hosts.byId(questHost)) questHost = hosts.preferred("library")?.id ?? null;
  });
  const qrun = $derived(cur ? quests.run(cur.id, questHost) : null);
  const qsum = $derived(qrun ? questSummary(Object.values(qrun.results)) : null);
  const verified = $derived.by(() => {
    const r = qrun;
    if (!r) return undefined;
    const out: Record<string, { state: "pass" | "fail" | "unknown"; detail: string; host: string }> = {};
    for (const [k, id] of Object.entries(checkable)) {
      const x = r.results[id];
      if (x) out[k] = { state: x.state, detail: x.detail, host: r.host };
    }
    return out;
  });
  function tickVerified() {
    if (!cur || !verified) return;
    for (const [k, v] of Object.entries(verified)) if (v.state === "pass" && !ticks[k]) library.tick(cur.id, k, true);
  }
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
      <div class="tags" role="group" aria-label="Shelf">
        <button type="button" class="chip" class:on={library.shelf === "all"} aria-pressed={library.shelf === "all"} onclick={() => (library.shelf = "all")}>ALL</button>
        {#each SHELVES as s (s.id)}
          <button type="button" class="chip" class:on={library.shelf === s.id} aria-pressed={library.shelf === s.id} title={s.what} onclick={() => (library.shelf = s.id)}>{s.label} <span class="n">{shelfCount.get(s.id)}</span></button>
        {/each}
      </div>
      <select class="input topic" aria-label="Topic" data-guide="library-topic" bind:value={library.cat}>
        <option value="all">All topics</option>
        {#each CATEGORIES as c (c.id)}
          <option value={c.id} disabled={!catCount.get(c.id)}>{c.label} ({catCount.get(c.id)})</option>
        {/each}
      </select>
      <ul class="list">
        {#each listed as { a, snippet } (a.id)}
          {@const p = progressOf(a.id)}
          <li>
            <button type="button" class="item" class:sel={a.id === library.current} onclick={() => library.read(a.id)}>
              <span class="t">{a.title}</span>
              <span class="shelf">{shelfLabel(a.shelf)} · {catLabel(a.id)}{#if p && p.done} · {p.done}/{p.total}{/if}</span>
              <span class="snip muted">{snippet}</span>
            </button>
          </li>
        {:else}
          <li class="muted empty">
            Nothing in the library matches{#if library.shelf !== "all" || library.cat !== "all"} with these filters{/if}. Try fewer words, or ask DRIFT.
            {#if library.shelf !== "all" || library.cat !== "all"}<button type="button" class="btn ghost tiny" onclick={() => ((library.shelf = "all"), (library.cat = "all"))}>CLEAR FILTERS</button>{/if}
          </li>
        {/each}
      </ul>
    </div>
  </Panel>

  <section class="main" data-guide="library-article" bind:this={reader}>
    {#if cur}
      <header class="head">
        <span class="kicker">
          <button type="button" class="btn ghost tiny" onclick={() => (library.trail.length ? library.back() : library.read(null))}>← {library.trail.length ? "BACK" : "SHELVES"}</button>
          {shelfLabel(cur.shelf)} · {catLabel(cur.id)}
        </span>
        <h1 aria-label={cur.title}><span use:scramble={cur.title}></span></h1>
        {#if flash}<span class="flash" role="status">{flash}</span>{/if}
      </header>

      {#if lead}
        <figure class="brief">
          <img src={lead.url} alt={lead.alt} decoding="async" draggable="false" />
          {#if lead.alt}<figcaption><span class="btag" aria-hidden="true">◢ BRIEFING</span> {lead.alt}</figcaption>{/if}
        </figure>
      {/if}

      {#if tracked(cur) && objectives.length}
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
          {#if nCheckable && hosts.hosts.length}
            <div class="verify" data-guide="library-verify">
              <span class="qlabel">VERIFY ON</span>
              <select class="input tiny" bind:value={questHost} aria-label="Host to check">
                {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
              </select>
              <button type="button" class="btn tiny" disabled={questHost == null || quests.busy !== null} title="Runs {nCheckable} read-only checks on the host: nothing is changed and sudo is never used" onclick={() => cur && questHost != null && void quests.check(cur, questHost)}
                >{quests.busy && cur && questHost != null && quests.busy === quests.key(cur.id, questHost) ? "Checking…" : qrun ? "Check again" : "Check"}</button
              >
              {#if qsum}
                <span class="qcount" title="Checked {new Date(qrun?.at ?? 0).toLocaleTimeString()}">{qsum.pass}/{qsum.total} verified{qsum.unknown ? ` · ${qsum.unknown} unclear` : ""}</span>
                {#if qsum.pass}<button type="button" class="btn ghost tiny" onclick={tickVerified}>Tick verified</button>{/if}
              {:else}
                <span class="muted small">{nCheckable} step{nCheckable === 1 ? "" : "s"} can be checked</span>
              {/if}
            </div>
            {#if quests.error}<p class="error-text small">{quests.error}</p>{/if}
          {/if}
        </aside>
      {/if}

      <Markdown
        src={cur.body}
        steps
        onview={(v) => ui.go(v as never)}
        onarticle={(id) => library.read(id)}
        {figure}
        hideLead={!!lead}
        {verified}
        ticks={tracked(cur) ? ticks : undefined}
        ontick={tracked(cur) ? (k, on) => library.tick(cur.id, k, on) : undefined}
      >
        {#snippet code(b)}
          <button type="button" class="btn ghost tiny" onclick={() => void copy(b.text)}>Copy</button>
          <button type="button" class="btn tiny" title="Types it into the focused terminal; you press Enter" onclick={() => send(b.text)}>Send to terminal</button>
        {/snippet}
      </Markdown>
    {:else}
      <div class="shelves">
        <h1>THE LIBRARY</h1>
        <p class="muted">Built in and offline: how each part of Nightgrid works, how-tos for common jobs, the Linux and homelab background they assume, install guides for self-hosted apps, and step-by-step guides for whole setups. Search covers every article; DRIFT reads them too.</p>
        <p class="muted small" data-guide="library-version">
          Content {kbVersionLabel(kbContent.version)} · {kbContent.source === "update" ? "updated since this app was released" : "bundled with this app"}{#if kbContent.checking} · checking for newer content…{/if}
        </p>
        <section class="shelfcard">
          <h2>TOPICS <span class="muted small">· {all.length} articles</span></h2>
          <div class="topics">
            <button type="button" class="chip" class:on={library.cat === "all"} aria-pressed={library.cat === "all"} onclick={() => (library.cat = "all")}>ALL</button>
            {#each CATEGORIES as c (c.id)}
              <button type="button" class="chip" class:on={library.cat === c.id} aria-pressed={library.cat === c.id} disabled={!catCount.get(c.id)} onclick={() => (library.cat = library.cat === c.id ? "all" : c.id)}>{c.label} <span class="n">{catCount.get(c.id)}</span></button>
            {/each}
          </div>
        </section>
        {#each SHELVES.filter((s) => library.shelf === "all" || library.shelf === s.id) as s (s.id)}
          {@const shelfArticles = all.filter((a) => a.shelf === s.id && inCat(a))}
          {#if shelfArticles.length}
          <section class="shelfcard">
            <h2>{s.label} <span class="muted small">· {s.what} · {shelfArticles.length}</span></h2>
            <div class="cards">
              {#each shelfArticles as a (a.id)}
                {@const p = progressOf(a.id)}
                <button type="button" class="card" onclick={() => library.read(a.id)}>
                  <span class="t">{a.title}</span>
                  {#if s.id !== "nightgrid"}<span class="snip muted">{a.summary}</span>{/if}
                  {#if p && p.done}<span class="bar small" aria-label={`${p.done} of ${p.total} steps done`}><i style:width={`${(p.done / p.total) * 100}%`}></i></span>{/if}
                </button>
              {/each}
            </div>
          </section>
          {/if}
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
  .chip:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .chip .n {
    opacity: 0.6;
  }
  .topics {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .topics .chip {
    font-size: var(--fs-sm);
    padding: 3px 8px;
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
  /* A guide's briefing banner (its leading figure), above the objectives. */
  .brief {
    margin: 0 0 14px;
  }
  .brief img {
    display: block;
    width: 100%;
    height: auto;
    border: 1px solid color-mix(in srgb, var(--warn) 45%, var(--line));
    border-left: 3px solid var(--warn);
    background: #07080b;
    clip-path: polygon(0 0, calc(100% - 18px) 0, 100% 18px, 100% 100%, 0 100%);
    user-select: none;
  }
  .brief figcaption {
    margin-top: 5px;
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .btag {
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
    color: var(--warn);
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
  .verify {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px dashed var(--line);
  }
  .verify .input.tiny {
    padding: 2px 6px;
    font-size: var(--fs-xs);
    max-width: 180px;
  }
</style>
