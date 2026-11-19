<script lang="ts">
  /**
   * Fleet → LOGS (M10u): one search over the journal and container logs of
   * several hosts, merged newest first, with saved searches that can alert.
   */
  import { api, errorText, type LogSearchQuery, type LogSearchResult, type SavedLogSearch } from "../../api";
  import { blankQuery, highlight, hitsText, SINCE, sourceCounts, splitSource } from "../../logsearch";
  import { hosts } from "../../stores/hosts.svelte";
  import ExplainButton from "../helper/ExplainButton.svelte";

  let q = $state<LogSearchQuery>(blankQuery());
  let result = $state<LogSearchResult | null>(null);
  /** The query the result is for (highlighting uses it). */
  let ran = $state<LogSearchQuery | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let only = $state<string | null>(null);
  let saved = $state<SavedLogSearch[]>([]);
  let current = $state<SavedLogSearch | null>(null);
  let saveName = $state<string | null>(null);
  let more = $state(false);

  const hostName = (id: number) => hosts.hosts.find((h) => h.id === id)?.name ?? `host ${id}`;

  async function loadSaved() {
    try {
      saved = await api.logSearchesList();
    } catch (e) {
      error = errorText(e);
    }
  }
  $effect(() => {
    void loadSaved();
  });

  async function search(e?: SubmitEvent) {
    e?.preventDefault();
    if (busy) return;
    busy = true;
    error = null;
    only = null;
    const query = $state.snapshot(q) as LogSearchQuery;
    try {
      result = await api.logSearch(query);
      ran = query;
    } catch (err) {
      error = errorText(err);
    }
    busy = false;
  }

  function toggleHost(id: number) {
    q.hosts = q.hosts.includes(id) ? q.hosts.filter((h) => h !== id) : [...q.hosts, id];
  }

  function load(s: SavedLogSearch) {
    current = s;
    q = { ...blankQuery(), ...s.query };
    more = !!(s.query.unit || s.query.container || s.query.priority != null);
    void search();
  }

  async function save(alert: boolean) {
    const name = (saveName ?? current?.name ?? "").trim();
    if (!name) return;
    error = null;
    try {
      const s = await api.logSearchSave({ id: saveName != null ? null : (current?.id ?? null), name, query: $state.snapshot(q) as LogSearchQuery, alert });
      current = s;
      saveName = null;
      await loadSaved();
    } catch (e) {
      error = errorText(e);
    }
  }

  async function setAlert(s: SavedLogSearch, alert: boolean) {
    error = null;
    try {
      await api.logSearchSave({ id: s.id, name: s.name, query: s.query, alert });
      await loadSaved();
      if (current?.id === s.id) current = saved.find((x) => x.id === s.id) ?? null;
    } catch (e) {
      error = errorText(e);
    }
  }

  async function remove(s: SavedLogSearch) {
    error = null;
    try {
      await api.logSearchDelete(s.id);
      if (current?.id === s.id) current = null;
      await loadSaved();
    } catch (e) {
      error = errorText(e);
    }
  }

  const shown = $derived(result ? (only ? result.hits.filter((h) => h.source === only) : result.hits) : []);
  const counts = $derived(result ? sourceCounts(result.hits) : []);
  const fmt = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", second: "2-digit" });
</script>

<div class="logs" data-guide="fleet-logs">
  <form class="row" onsubmit={search}>
    <input class="input grow" bind:value={q.text} placeholder={q.regex ? "regular expression, e.g. status=5\\d\\d" : "text to find (empty: every line)"} spellcheck="false" autocomplete="off" aria-label="Search text" />
    <button type="button" class="opt" class:on={q.regex} title="Regular expression" aria-pressed={q.regex} onclick={() => (q.regex = !q.regex)}>.*</button>
    <button type="button" class="opt" class:on={q.caseSensitive} title="Match case" aria-pressed={q.caseSensitive} onclick={() => (q.caseSensitive = !q.caseSensitive)}>Aa</button>
    <select class="input since" bind:value={q.sinceMins} aria-label="How far back">
      {#each SINCE as s (s.mins)}<option value={s.mins}>last {s.label}</option>{/each}
    </select>
    <button type="submit" class="btn primary" data-guide="fleet-logs-search" disabled={busy || (!q.journal && !q.containers)}>{busy ? "Searching…" : "Search"}</button>
  </form>

  <div class="row small">
    <button type="button" class="chip" class:on={!q.hosts.length} onclick={() => (q.hosts = [])}>ALL HOSTS</button>
    {#each hosts.hosts as h (h.id)}
      <button type="button" class="chip" class:on={q.hosts.includes(h.id)} onclick={() => toggleHost(h.id)}>{h.name}</button>
    {/each}
    <span class="sep"></span>
    <label class="check"><input type="checkbox" bind:checked={q.journal} /> journal</label>
    <label class="check"><input type="checkbox" bind:checked={q.containers} /> containers</label>
    <button type="button" class="btn ghost tiny" onclick={() => (more = !more)}>{more ? "Fewer filters" : "More filters"}</button>
  </div>
  {#if more}
    <div class="row small">
      <input class="input" value={q.unit ?? ""} oninput={(e) => (q.unit = e.currentTarget.value || null)} placeholder="unit, e.g. nginx.service" aria-label="Unit" spellcheck="false" />
      <input class="input" value={q.container ?? ""} oninput={(e) => (q.container = e.currentTarget.value || null)} placeholder="container name contains" aria-label="Container" spellcheck="false" />
      <select class="input" value={q.priority ?? ""} onchange={(e) => (q.priority = e.currentTarget.value === "" ? null : Number(e.currentTarget.value))} aria-label="Journal priority">
        <option value="">any priority</option>
        <option value="3">errors and worse</option>
        <option value="4">warnings and worse</option>
        <option value="6">info and worse</option>
      </select>
    </div>
  {/if}

  <div class="row small saved" data-guide="fleet-logs-saved">
    <span class="label">SAVED</span>
    {#each saved as s (s.id)}
      <span class="sv" class:cur={current?.id === s.id}>
        <button type="button" class="name" onclick={() => load(s)}>{s.name}</button>
        <button type="button" class="bell" class:on={s.alert} title={s.alert ? "Alerts on: new matches raise LOG MATCH (checked every 5 min)" : "Alert on new matches"} aria-pressed={s.alert} onclick={() => void setAlert(s, !s.alert)}>{s.alert ? "◉" : "○"}</button>
        <button type="button" class="x" title="Delete saved search" aria-label="Delete {s.name}" onclick={() => void remove(s)}>×</button>
      </span>
    {/each}
    {#if saveName != null}
      <input class="input" bind:value={saveName} placeholder="name" aria-label="Saved search name" maxlength="60" />
      <button type="button" class="btn tiny" disabled={!saveName.trim()} onclick={() => void save(false)}>Save</button>
      <button type="button" class="btn ghost tiny" onclick={() => (saveName = null)}>Cancel</button>
    {:else}
      {#if current}<button type="button" class="btn ghost tiny" onclick={() => void save(current?.alert ?? false)}>Update “{current.name}”</button>{/if}
      <button type="button" class="btn ghost tiny" onclick={() => (saveName = "")}>Save as…</button>
    {/if}
  </div>

  {#if error}<p class="bad small" role="alert">{error}</p>{/if}

  {#if result}
    <div class="row small summary">
      <span>{result.hits.length}{result.truncated ? "+" : ""} line{result.hits.length === 1 ? "" : "s"} from {result.hosts.length - result.errors.length} of {result.hosts.length} host{result.hosts.length === 1 ? "" : "s"}</span>
      {#each result.errors as e (e.hostId)}<span class="bad" title={e.message}>{hostName(e.hostId)}: {e.message}</span>{/each}
      <span class="spacer"></span>
      {#if shown.length}
        <ExplainButton source="log search" text={() => hitsText(shown.slice(0, 200), hostName)} question="What do these log lines say is going wrong, and what should I check first?" />
      {/if}
    </div>
    {#if counts.length > 1}
      <div class="row small">
        <button type="button" class="chip" class:on={!only} onclick={() => (only = null)}>ALL</button>
        {#each counts.slice(0, 20) as c (c.source)}
          <button type="button" class="chip" class:on={only === c.source} onclick={() => (only = only === c.source ? null : c.source)}>{splitSource(c.source).name} <span class="muted">{c.n}</span></button>
        {/each}
      </div>
    {/if}
    <div class="hits" role="log">
      {#each shown as h, i (i)}
        {@const src = splitSource(h.source)}
        <div class="hit">
          <span class="ts">{h.ts ? fmt.format(new Date(h.ts * 1000)) : "?"}</span>
          <span class="host">{hostName(h.hostId)}</span>
          <span class="src" title={h.source}><span class="k">{src.kind === "container" ? "CTR" : "JRN"}</span> {src.name}</span>
          <span class="text">{#each highlight(h.text, ran ?? q) as p, j (j)}{#if p.hit}<mark>{p.t}</mark>{:else}{p.t}{/if}{/each}</span>
        </div>
      {:else}
        <p class="muted small">No matching lines.</p>
      {/each}
    </div>
  {:else if !busy}
    <p class="muted small">
      Searches the journal over SSH and container logs through Docker on the hosts you pick, connecting where needed. Lines that look like they hold a password or token are hidden. Save a search and turn on its alert (◉) to get a LOG MATCH alert when new lines match.
    </p>
  {/if}
</div>

<style>
  .logs {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  .row {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .grow {
    flex: 1 1 240px;
    min-width: 0;
  }
  .since {
    width: auto;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .opt {
    background: none;
    border: 1px solid var(--line);
    color: var(--text-1);
    font-family: var(--font-mono);
    padding: 4px 8px;
    cursor: pointer;
  }
  .opt.on {
    color: var(--bg-0);
    background: var(--cyan);
    border-color: var(--cyan);
  }
  .chip {
    background: none;
    border: 1px solid var(--line);
    color: var(--text-1);
    padding: 0 var(--chip-px);
    font: inherit;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    min-height: var(--chip-h);
    font-size: var(--fs-xs);
    white-space: nowrap;
  }
  .chip.on {
    color: var(--text-0);
    border-color: var(--cyan);
  }
  .sep {
    width: 1px;
    height: 16px;
    background: var(--line);
  }
  .check {
    display: inline-flex;
    gap: 4px;
    align-items: center;
  }
  .saved .label {
    color: var(--cyan);
    letter-spacing: var(--tracking-label);
  }
  .sv {
    display: inline-flex;
    align-items: center;
    border: 1px solid var(--line);
  }
  .sv.cur {
    border-color: var(--magenta);
  }
  .sv button {
    background: none;
    border: 0;
    color: var(--text-1);
    font: inherit;
    cursor: pointer;
    padding: 1px 6px;
  }
  .sv .name {
    color: var(--text-0);
  }
  .sv .bell.on {
    color: var(--warn);
  }
  .sv .x:hover {
    color: var(--magenta);
  }
  .summary .bad,
  .bad {
    color: var(--danger);
  }
  .spacer {
    flex: 1;
  }
  .muted {
    color: var(--text-1);
  }
  .hits {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border: 1px solid var(--line);
    background: var(--bg-0);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
  }
  .hit {
    display: grid;
    grid-template-columns: 150px 110px 150px 1fr;
    gap: 10px;
    padding: 2px 8px;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
  }
  .hit:hover {
    background: var(--bg-1);
  }
  .ts {
    color: var(--text-1);
    white-space: nowrap;
  }
  .host {
    color: var(--cyan);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .src {
    color: var(--text-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .k {
    color: var(--mint);
    letter-spacing: 0.06em;
  }
  .text {
    color: var(--text-0);
    white-space: pre-wrap;
    word-break: break-word;
  }
  mark {
    background: color-mix(in srgb, var(--yellow, var(--warn)) 35%, transparent);
    color: var(--text-0);
  }
  @media (max-width: 760px) {
    .hit {
      grid-template-columns: 1fr 1fr;
    }
    .text {
      grid-column: 1 / -1;
    }
  }
</style>
