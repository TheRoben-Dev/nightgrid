<script lang="ts">
  /**
   * Release log (D-204): every Nightgrid release with its codename and notes,
   * from the CHANGELOG bundled with this version, so it works offline.
   */
  import Markdown from "../Markdown.svelte";
  import { releases } from "../../stores/releases.svelte";
  import { cmpVersion } from "../../releases";

  const PAGE = 12;
  let shown = $state(PAGE);
  let open = $state<string | null>(null);
  let q = $state("");

  $effect(() => {
    void releases.load().then(() => {
      if (open === null) open = releases.current ?? releases.list[0]?.version ?? null;
    });
  });

  const filtered = $derived.by(() => {
    const t = q.trim().toLowerCase();
    if (!t) return releases.list;
    return releases.list.filter((r) => r.version.includes(t) || (r.codename ?? "").toLowerCase().includes(t) || r.body.toLowerCase().includes(t));
  });
  const visible = $derived(filtered.slice(0, shown));
  const isNewer = (v: string) => !!releases.current && cmpVersion(v, releases.current) > 0;
</script>

<div class="log" data-guide="settings-releases">
  <div class="bar">
    <input type="search" class="input" placeholder="Search versions, codenames, notes" aria-label="Search releases" bind:value={q} oninput={() => (shown = PAGE)} />
    <span class="muted small">{filtered.length} releases</span>
    <button type="button" class="btn ghost tiny" disabled={!releases.running} onclick={() => void releases.replay()}>Show What's new</button>
  </div>
  {#if !releases.loaded}
    <p class="muted small">Loading…</p>
  {:else if !visible.length}
    <p class="muted small">No release matches.</p>
  {:else}
    <ul>
      {#each visible as r (r.version)}
        <li class:open={open === r.version} class:cur={r.version === releases.current}>
          <button type="button" class="row" aria-expanded={open === r.version} onclick={() => (open = open === r.version ? null : r.version)}>
            <span class="caret" aria-hidden="true">{open === r.version ? "▾" : "▸"}</span>
            <span class="v">v{r.version}</span>
            <span class="cn">{r.codename ?? "—"}</span>
            {#if r.version === releases.current}<span class="chip">INSTALLED</span>{:else if isNewer(r.version)}<span class="chip new">NEWER</span>{/if}
            {#if r.tag}<span class="tag muted small">{r.tag}</span>{/if}
            <span class="d muted small">{r.date ?? ""}</span>
          </button>
          {#if open === r.version}
            <div class="notes">
              {#if r.body}<Markdown src={r.body} />{:else}<p class="muted small">No notes for this release.</p>{/if}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
    {#if filtered.length > shown}
      <button type="button" class="btn ghost tiny more" onclick={() => (shown += PAGE * 2)}>Show older ({filtered.length - shown})</button>
    {/if}
  {/if}
</div>

<style>
  .log {
    display: grid;
    gap: 8px;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .bar input {
    flex: 1;
    min-width: 200px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    border-top: 1px solid var(--line);
  }
  li {
    border-bottom: 1px solid var(--line);
  }
  li.cur {
    background: linear-gradient(90deg, color-mix(in srgb, var(--mint) 7%, transparent), transparent 60%);
  }
  .row {
    display: flex;
    align-items: baseline;
    gap: 10px;
    width: 100%;
    padding: 6px 4px;
    background: none;
    border: 0;
    color: var(--text-0);
    text-align: left;
    cursor: pointer;
    font: inherit;
  }
  .row:hover .cn {
    color: var(--mint);
  }
  .caret {
    width: 1em;
    color: var(--text-1);
  }
  .v {
    font-family: var(--font-mono);
    color: var(--cyan);
    min-width: 6.5em;
  }
  .cn {
    font-family: var(--font-display);
    letter-spacing: 0.1em;
    text-transform: uppercase;
    font-size: var(--fs-sm);
  }
  .chip {
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    padding: 0 6px;
    border: 1px solid var(--mint);
    color: var(--mint);
  }
  .chip.new {
    border-color: var(--magenta);
    color: var(--magenta);
  }
  .d {
    margin-left: auto;
    font-family: var(--font-mono);
  }
  .notes {
    padding: 4px 8px 12px 28px;
    font-size: var(--fs-sm);
    line-height: 1.55;
  }
  .more {
    justify-self: start;
  }
</style>
