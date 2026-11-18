<script lang="ts">
  /**
   * Knowledge → Starters (D-177): ready-made runbooks for common alerts.
   * Read one, then add it as a runbook note (it's yours to edit after).
   */
  import type { StarterRunbook } from "../../api";
  import { CATALOG } from "../../kb/catalog";
  import { runbookAdded, runbookInput } from "../../starters";
  import { KIND_LABEL } from "../../stores/alerts.svelte";
  import { library } from "../../stores/library.svelte";
  import { notes } from "../../stores/notes.svelte";
  import { starters } from "../../stores/starters.svelte";
  import { ui } from "../../stores/ui.svelte";
  import Markdown from "../Markdown.svelte";

  let openId = $state<string | null>(null);
  let confirming = $state(false);
  let busy = $state(false);

  $effect(() => {
    void starters.load();
  });
  $effect(() => {
    if (starters.focus) {
      openId = starters.focus;
      starters.focus = null;
    }
  });

  const list = $derived(starters.catalog?.runbooks ?? []);
  const cur = $derived(list.find((r) => r.id === openId) ?? null);
  const articleTitle = (id: string) => CATALOG.find((e) => e.id === id)?.title ?? id;
  const kinds = (r: StarterRunbook) => (r.alertKinds.length ? r.alertKinds.map((k) => KIND_LABEL[k]).join(", ") : "no alert kind yet (open it from Knowledge)");

  async function add(r: StarterRunbook) {
    busy = true;
    const n = await notes.add(runbookInput(r));
    busy = false;
    confirming = false;
    if (n) notes.starters = false;
  }
</script>

<div class="sr" data-guide="notes-starter-runbooks">
  {#if !cur}
    <h2 class="label">STARTER RUNBOOKS</h2>
    <p class="muted small">What to do when a common alert fires, step by step, with commands you can send to a terminal. Add one and it shows as <strong>▤ Runbook</strong> on matching alerts; edit it to fit your setup. Ready-made server setups are in Fleet → BLUEPRINTS → STARTERS.</p>
    {#if starters.error}<p class="bad small" role="alert">{starters.error}</p>{/if}
    <ul class="list">
      {#each list as r (r.id)}
        {@const have = runbookAdded(notes.notes, r)}
        <li>
          <button type="button" class="item" onclick={() => ((openId = r.id), (confirming = false))}>
            <span class="t">{r.title.replace(/^Runbook:\s*/, "")}{#if have}<span class="added">ADDED</span>{/if}</span>
            <span class="muted small">{r.summary}</span>
            <span class="kinds small">{kinds(r)}</span>
          </button>
        </li>
      {/each}
    </ul>
  {:else}
    {@const have = runbookAdded(notes.notes, cur)}
    <div class="row">
      <button type="button" class="btn ghost tiny" onclick={() => (openId = null)}>← All starter runbooks</button>
    </div>
    <h2 class="title">{cur.title}</h2>
    <p class="muted small">For alerts: {kinds(cur)} · tags {cur.tags.map((t) => `#${t}`).join(" ")}</p>
    <div class="chips">
      {#each cur.library as a (a)}<button type="button" class="chip" onclick={() => library.open(a)}>▤ {articleTitle(a)}</button>{/each}
    </div>
    <div class="actions">
      {#if have}
        <span class="ok small">Already in your notes.</span>
        <button type="button" class="btn tiny" onclick={() => ((notes.selected = have.id), (notes.starters = false))}>Open it</button>
      {:else if confirming}
        <span class="small">Add “{cur.title}” to your notes as a runbook{cur.alertKinds.length ? ` for ${kinds(cur)} alerts` : ""}?</span>
        <button type="button" class="btn primary tiny" disabled={busy} onclick={() => void add(cur)}>Yes, add it</button>
        <button type="button" class="btn ghost tiny" onclick={() => (confirming = false)}>Cancel</button>
      {:else}
        <button type="button" class="btn primary tiny" onclick={() => (confirming = true)}>Add to my runbooks…</button>
      {/if}
    </div>
    <div class="preview">
      <p class="label">PREVIEW</p>
      <Markdown src={cur.body} steps onview={(v) => ui.go(v as never)} onarticle={(id) => library.open(id)} />
    </div>
  {/if}
</div>

<style>
  .sr {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 4px 2px;
    overflow: auto;
    min-height: 0;
  }
  .label {
    margin: 0;
    color: var(--cyan);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
  }
  .title {
    margin: 0;
    font-size: var(--fs-lg, 18px);
  }
  .small {
    font-size: var(--fs-xs);
    margin: 0;
  }
  .muted {
    color: var(--text-1);
  }
  .ok {
    color: var(--ok, #4ade80);
  }
  .bad {
    color: var(--danger);
  }
  .list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 8px;
  }
  .item {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 3px;
    text-align: left;
    background: var(--bg-1);
    border: 1px solid var(--line);
    color: var(--text-0);
    padding: 8px 10px;
    font: inherit;
    cursor: pointer;
  }
  .item:hover,
  .item:focus-visible {
    border-color: var(--cyan);
  }
  .t {
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .added {
    color: var(--ok, #4ade80);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
  }
  .kinds {
    color: var(--magenta);
    margin-top: auto;
  }
  .row,
  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .chip {
    background: none;
    border: 1px solid var(--line);
    color: var(--text-1);
    padding: 1px 8px;
    font: inherit;
    font-size: var(--fs-xs);
    cursor: pointer;
  }
  .chip:hover {
    color: var(--text-0);
    border-color: var(--cyan);
  }
  .preview {
    border-top: 1px solid var(--line);
    padding-top: 8px;
  }
</style>
