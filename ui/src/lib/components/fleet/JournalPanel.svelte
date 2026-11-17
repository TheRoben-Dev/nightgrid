<script lang="ts">
  /**
   * Fleet → JOURNAL (0.10.81): every change made from Nightgrid across all
   * hosts, newest first, with line diffs for text edits, plus entries you
   * write yourself. DRIFT reads the same journal.
   */
  import { api, errorText, type ChangeArea, type ChangeEntry, type ChangeQuery } from "../../api";
  import { AREA_CODE, AREAS, areaLabel, byDay, dayLabel, diffCounts, diffLines, RANGES, rangeSince, timeOf, type RangeId } from "../../journal";
  import { hosts } from "../../stores/hosts.svelte";
  import { helper } from "../../stores/helper.svelte";
  import { settings } from "../../stores/settings.svelte";
  import Modal from "../Modal.svelte";
  import { SvelteSet } from "svelte/reactivity";

  let text = $state("");
  let hostId = $state<number | null>(null);
  let areas = $state<ChangeArea[]>([]);
  let range = $state<RangeId>("30d");
  let failedOnly = $state(false);
  let entries = $state<ChangeEntry[]>([]);
  let loaded = $state(false);
  let error = $state<string | null>(null);
  const open = new SvelteSet<number>();
  let exported = $state<string | null>(null);

  // Writing or editing a hand-written entry.
  let editing = $state<{ id: number | null; hostId: number | null; title: string; detail: string; when: string } | null>(null);
  let confirmClear = $state(false);

  const query = (): ChangeQuery => ({
    text: text.trim(),
    hostId,
    areas: [...areas],
    since: rangeSince(range, Math.floor(Date.now() / 1000)),
    until: null,
    failedOnly,
    limit: 0,
  });

  async function load() {
    error = null;
    try {
      entries = await api.journalQuery(query());
    } catch (e) {
      error = errorText(e);
    }
    loaded = true;
  }

  // Reload when a filter changes (typing waits a moment).
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    void [hostId, areas.length, range, failedOnly];
    void load();
  });
  function typed() {
    clearTimeout(timer);
    timer = setTimeout(() => void load(), 250);
  }

  function toggleArea(a: ChangeArea) {
    areas = areas.includes(a) ? areas.filter((x) => x !== a) : [...areas, a];
  }
  function toggle(id: number) {
    if (open.has(id)) open.delete(id);
    else open.add(id);
  }

  /** `datetime-local` value for Unix seconds, in local time. */
  const localInput = (at: number) => {
    const off = new Date(at * 1000).getTimezoneOffset() * 60;
    return new Date((at - off) * 1000).toISOString().slice(0, 16);
  };

  function newNote() {
    editing = { id: null, hostId, title: "", detail: "", when: localInput(Math.floor(Date.now() / 1000)) };
  }
  function editNote(e: ChangeEntry) {
    editing = { id: e.id, hostId: e.hostId, title: e.title, detail: e.detail, when: localInput(e.at) };
  }

  async function saveNote() {
    if (!editing || !editing.title.trim()) return;
    error = null;
    try {
      if (editing.id == null) {
        const at = editing.when ? Math.floor(new Date(editing.when).getTime() / 1000) : null;
        await api.journalNote(editing.hostId, editing.title, editing.detail, at);
      } else {
        await api.journalEdit(editing.id, editing.title, editing.detail);
      }
      editing = null;
      await load();
    } catch (e) {
      error = errorText(e);
    }
  }

  async function deleteNote(id: number) {
    error = null;
    try {
      await api.journalDelete(id);
      editing = null;
      await load();
    } catch (e) {
      error = errorText(e);
    }
  }

  async function clearAll() {
    error = null;
    try {
      await api.journalClear();
      confirmClear = false;
      await load();
    } catch (e) {
      error = errorText(e);
    }
  }

  async function exportMd() {
    error = null;
    try {
      exported = await api.journalExport(query());
    } catch (e) {
      error = errorText(e);
    }
  }

  function askDrift(e: ChangeEntry) {
    const text = [`${areaLabel(e.area)}${e.host ? ` on ${e.host}` : ""}: ${e.title}`, e.detail, e.diff ?? ""].filter(Boolean).join("\n");
    void helper.explain({ source: "journal entry", text, host: e.host || null, question: "Something may have broken after this change. What did it do, what could it break, and how do I check or undo it?" });
  }

  const now = $derived(Math.floor(Date.now() / 1000));
  const days = $derived(byDay(entries));
  const failedCount = $derived(entries.filter((e) => !e.ok).length);
</script>

<div class="journal" data-guide="fleet-journal">
  <div class="row">
    <input class="input grow" bind:value={text} oninput={typed} placeholder="search titles, details, hosts and diffs" spellcheck="false" autocomplete="off" aria-label="Search the journal" />
    <select class="input" bind:value={hostId} aria-label="Host">
      <option value={null}>all hosts</option>
      {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
    </select>
    <div class="seg" role="group" aria-label="How far back">
      {#each RANGES as r (r.id)}
        <button type="button" class:on={range === r.id} aria-pressed={range === r.id} onclick={() => (range = r.id)}>{r.label}</button>
      {/each}
    </div>
    <button type="button" class="btn primary" data-guide="journal-note" onclick={newNote}>+ Entry</button>
  </div>
  <div class="row small">
    <button type="button" class="chip" class:on={!areas.length} onclick={() => (areas = [])}>ALL</button>
    {#each AREAS as a (a.id)}
      <button type="button" class="chip" class:on={areas.includes(a.id)} aria-pressed={areas.includes(a.id)} onclick={() => toggleArea(a.id)}>{a.label}</button>
    {/each}
    <span class="sep"></span>
    <label class="check"><input type="checkbox" bind:checked={failedOnly} /> failed only</label>
  </div>
  <div class="row small summary">
    <span>{entries.length}{entries.length >= 1000 ? "+" : ""} entr{entries.length === 1 ? "y" : "ies"}{#if failedCount}&nbsp;· <span class="bad">{failedCount} failed</span>{/if}</span>
    <span class="spacer"></span>
    {#if exported}<span class="muted" title={exported}>Saved {exported.split(/[\\/]/).pop()}</span>{/if}
    <button type="button" class="btn ghost tiny" disabled={!entries.length} onclick={() => void exportMd()}>Export Markdown</button>
    <button type="button" class="btn ghost tiny" onclick={() => (confirmClear = true)}>Clear…</button>
  </div>
  {#if error}<p class="bad small" role="alert">{error}</p>{/if}

  <div class="list" role="log" aria-label="Journal entries">
    {#each days as d (d.key)}
      <h3 class="day label">{dayLabel(d.key, now)}</h3>
      {#each d.entries as e (e.id)}
        {@const c = diffCounts(e.diff)}
        <div class="entry" class:failed={!e.ok} class:manual={e.manual}>
          <span class="ts">{timeOf(e.at)}</span>
          <span class="code" title={areaLabel(e.area)}>{AREA_CODE[e.area]}</span>
          <div class="body">
            <div class="head">
              <span class="title">{e.title}</span>
              {#if e.host}<span class="host">{e.host}</span>{/if}
              {#if !e.ok}<span class="tag bad">FAILED</span>{/if}
              {#if e.diff}
                <button type="button" class="diffbtn" aria-expanded={open.has(e.id)} onclick={() => toggle(e.id)}>
                  {#if c.added || c.removed}<span class="add">+{c.added}</span> <span class="del">−{c.removed}</span>{:else}diff{/if}
                  {open.has(e.id) ? "▾" : "▸"}
                </button>
              {/if}
              <span class="spacer"></span>
              {#if e.manual}
                <button type="button" class="btn ghost tiny" onclick={() => editNote(e)}>Edit</button>
              {:else if settings.on("drift")}
                <button type="button" class="btn ghost tiny ask" title="Ask DRIFT what this change could have broken" onclick={() => askDrift(e)}>Ask DRIFT</button>
              {/if}
            </div>
            {#if e.detail}<p class="detail">{e.detail}</p>{/if}
            {#if e.diff && open.has(e.id)}
              <pre class="diff">{#each diffLines(e.diff) as l, i (i)}<span class={l.kind}>{l.kind === "gap" ? `… ${l.text}` : `${l.kind === "add" ? "+" : l.kind === "del" ? "−" : " "} ${l.text}`}
</span>{/each}</pre>
            {/if}
          </div>
        </div>
      {/each}
    {:else}
      {#if loaded}
        <div class="empty">
          <p class="label">{text || areas.length || hostId != null || failedOnly ? "NOTHING MATCHES" : "NOTHING RECORDED YET"}</p>
          <p class="muted small">
            Deploys, compose runs, rollbacks, container actions, file and crontab edits, firewall and proxy changes, updates, power actions, Proxmox changes and SSH keys pushed from Nightgrid land here on their own, with a line diff where text changed. Add your own entries for changes made elsewhere (“swapped the UPS battery”). Lines that look like they hold a secret are hidden, and `.env` vault changes are recorded by name only.
          </p>
        </div>
      {/if}
    {/each}
  </div>
</div>

<Modal open={editing != null} title={editing?.id != null ? "EDIT ENTRY" : "NEW JOURNAL ENTRY"} width={520} onclose={() => (editing = null)}>
  {#if editing}
    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        void saveNote();
      }}
    >
      <label>What changed <input class="input" bind:value={editing.title} maxlength="200" placeholder="e.g. Moved the NAS to the new switch" required /></label>
      <label>Details <textarea class="input" rows="4" bind:value={editing.detail} maxlength="4000" placeholder="optional"></textarea></label>
      {#if editing.id == null}
        <div class="two">
          <label>Host
            <select class="input" bind:value={editing.hostId}>
              <option value={null}>none (whole lab)</option>
              {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
            </select>
          </label>
          <label>When <input class="input" type="datetime-local" bind:value={editing.when} /></label>
        </div>
      {/if}
    </form>
  {/if}
  {#snippet footer()}
    {#if editing?.id != null}
      {@const id = editing.id}
      <button type="button" class="btn ghost danger" onclick={() => void deleteNote(id)}>Delete</button>
    {/if}
    <span class="spacer"></span>
    <button type="button" class="btn ghost" onclick={() => (editing = null)}>Cancel</button>
    <button type="button" class="btn primary" disabled={!editing?.title.trim()} onclick={() => void saveNote()}>Save</button>
  {/snippet}
</Modal>

<Modal open={confirmClear} title="CLEAR THE JOURNAL" tone="danger" width={460} onclose={() => (confirmClear = false)}>
  <p>Forget every journal entry, including the ones you wrote? Hosts aren't touched, and the per-host timelines keep their own history.</p>
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (confirmClear = false)}>Cancel</button>
    <button type="button" class="btn danger" onclick={() => void clearAll()}>Clear journal</button>
  {/snippet}
</Modal>

<style>
  .journal {
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
  .small {
    font-size: var(--fs-xs);
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
  }
  .seg button {
    border: 0;
    background: none;
    color: var(--text-1);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    padding: 3px 8px;
    cursor: pointer;
  }
  .seg button + button {
    border-left: 1px solid var(--line);
  }
  .seg button.on {
    color: var(--magenta);
    box-shadow: inset 0 -2px 0 var(--magenta);
  }
  .chip {
    background: none;
    border: 1px solid var(--line);
    color: var(--text-1);
    padding: 1px 8px;
    font: inherit;
    cursor: pointer;
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
  .spacer {
    flex: 1;
  }
  .muted {
    color: var(--text-1);
  }
  .bad {
    color: var(--danger);
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border-top: 1px solid var(--line);
    padding-right: 4px;
  }
  .day {
    position: sticky;
    top: 0;
    margin: 0;
    padding: 6px 0 4px;
    background: var(--bg-0);
    color: var(--cyan);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    z-index: 1;
  }
  .entry {
    display: grid;
    grid-template-columns: 44px 40px 1fr;
    gap: 8px;
    padding: 5px 0;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
    font-size: var(--fs-sm);
  }
  .entry.failed {
    box-shadow: inset 2px 0 0 var(--danger);
  }
  .entry.manual .code {
    color: var(--magenta);
  }
  .ts {
    color: var(--text-1);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    padding-left: 6px;
  }
  .code {
    color: var(--cyan);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
  }
  .body {
    min-width: 0;
  }
  .ask {
    opacity: 0;
  }
  .entry:hover .ask,
  .ask:focus-visible {
    opacity: 1;
  }
  .head {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .title {
    color: var(--text-0);
    overflow-wrap: anywhere;
  }
  .host {
    color: var(--text-1);
    border: 1px solid var(--line);
    padding: 0 5px;
    font-size: var(--fs-xs);
  }
  .tag.bad {
    color: var(--bg-0);
    background: var(--danger);
    padding: 0 4px;
    font-size: var(--fs-xs);
  }
  .diffbtn {
    background: none;
    border: 1px solid var(--line);
    color: var(--text-1);
    font: inherit;
    font-size: var(--fs-xs);
    font-family: var(--font-mono);
    padding: 0 5px;
    cursor: pointer;
  }
  .add {
    color: var(--ok, #4ade80);
  }
  .del {
    color: var(--danger);
  }
  .detail {
    margin: 2px 0 0;
    color: var(--text-1);
    font-size: var(--fs-xs);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .diff {
    margin: 6px 0 2px;
    padding: 6px 8px;
    background: var(--bg-1);
    border: 1px solid var(--line);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    overflow: auto;
    max-height: 320px;
    white-space: pre;
  }
  .diff .add {
    color: var(--ok, #4ade80);
  }
  .diff .del {
    color: var(--danger);
  }
  .diff .gap {
    color: var(--text-1);
    font-style: italic;
  }
  .empty {
    padding: 20px 4px;
    max-width: 70ch;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .form label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
</style>
