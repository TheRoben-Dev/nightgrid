<script lang="ts">
  /**
   * Notes and runbooks (M10e): markdown notes, optionally tied to a host.
   * Code blocks in a note can be typed into a terminal on that host (the
   * user presses Enter) or copied. Notes go into the encrypted backup.
   */
  import { onMount } from "svelte";
  import { writeClipboard } from "../clipboard";
  import ExplainButton from "../components/helper/ExplainButton.svelte";
  import Markdown from "../components/Markdown.svelte";
  import Panel from "../components/Panel.svelte";
  import { scramble } from "../scramble";
  import { hosts } from "../stores/hosts.svelte";
  import { hasSecret, notes, parseTags } from "../stores/notes.svelte";
  import { terms } from "../stores/terms.svelte";
  import { formatChord } from "../keys";
  import { KIND_LABEL } from "../stores/alerts.svelte";
  import { services } from "../stores/services.svelte";
  import { ui } from "../stores/ui.svelte";
  import { runbookScope } from "../stores/notes.svelte";
  import type { AlertKind } from "../api";

  const KINDS = Object.keys(KIND_LABEL) as AlertKind[];
  function toggleKind(k: AlertKind) {
    if (!notes.draft) return;
    const ks = notes.draft.input.alertKinds;
    notes.draft.input.alertKinds = ks.includes(k) ? ks.filter((x) => x !== k) : [...ks, k];
  }
  const serviceName = (id: number | null) => (id === null ? null : (services.services.find((s) => s.id === id)?.name ?? `service ${id}`));

  onMount(() => {
    void notes.load();
    if (!services.loaded) void services.refresh();
  });

  const cur = $derived(notes.current);
  const draft = $derived(notes.draft);
  const hostName = (id: number | null) => (id === null ? null : (hosts.byId(id)?.name ?? `host ${id}`));
  let tagText = $state("");
  let confirmDelete = $state(false);
  let flash = $state<string | null>(null);
  let flashTimer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    // Fresh tag text whenever a different draft opens.
    void notes.draft?.id;
    tagText = notes.draft ? notes.draft.input.tags.join(", ") : "";
  });
  $effect(() => {
    void notes.selected;
    confirmDelete = false;
  });

  function say(msg: string) {
    flash = msg;
    if (flashTimer) clearTimeout(flashTimer);
    flashTimer = setTimeout(() => (flash = null), 2500);
  }

  function send(text: string) {
    const title = terms.send(text, cur?.hostId ?? null);
    say(title ? `Typed into ${title}. Press Enter there to run it.` : "Opened a terminal.");
  }

  async function copy(text: string) {
    await writeClipboard(text);
    say("Copied.");
  }

  function ago(ts: number): string {
    const s = Math.max(0, Date.now() / 1000 - ts);
    if (s < 60) return "just now";
    if (s < 3600) return `${Math.floor(s / 60)}m ago`;
    if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
    return `${Math.floor(s / 86400)}d ago`;
  }

  /** Mod+S in any editor field submits the form. */
  function saveKey(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      (e.currentTarget as HTMLElement).closest("form")?.requestSubmit();
    }
  }

  async function onSave(e: SubmitEvent) {
    e.preventDefault();
    if (!notes.draft) return;
    notes.draft.input.tags = parseTags(tagText);
    await notes.save();
  }

  const secret = $derived(draft ? hasSecret(draft.input) : false);
</script>

<div class="notes">
  <Panel title="NOTES" prefix=">" count={notes.notes.length} guide="notes-list">
    {#snippet actions()}
      <button type="button" class="btn tiny" onclick={() => notes.startNew()} data-guide="notes-new">New</button>
    {/snippet}
    <div class="side">
      <input class="input" type="search" placeholder="Search notes" aria-label="Search notes" bind:value={notes.query} />
      <select class="input" aria-label="Filter by host" bind:value={notes.host}>
        <option value="all">All hosts</option>
        <option value="none">Not tied to a host</option>
        {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
      </select>
      {#if notes.tags.length}
        <div class="tags" role="group" aria-label="Filter by tag">
          {#each notes.tags as t (t)}
            <button type="button" class="chip" class:on={notes.tag === t} aria-pressed={notes.tag === t} onclick={() => (notes.tag = notes.tag === t ? null : t)}
              >#{t}</button
            >
          {/each}
        </div>
      {/if}
      {#if notes.error}<p class="error-text">{notes.error}</p>{/if}
      <ul class="list">
        {#each notes.visible as n (n.id)}
          <li>
            <button
              type="button"
              class="item"
              class:sel={n.id === notes.selected && !draft}
              onclick={() => {
                notes.cancel();
                notes.selected = n.id;
              }}
            >
              <span class="t">{#if n.pinned}<span class="pin" title="Pinned">◆</span>{/if}{n.title}</span>
              <span class="meta muted">
                {#if n.hostId !== null}<span class="host">{hostName(n.hostId)}</span>{/if}
                {#each n.tags as t (t)}<span>#{t}</span>{/each}
                <span class="when">{ago(n.updatedAt)}</span>
              </span>
            </button>
          </li>
        {:else}
          <li class="muted empty">{notes.loaded ? (notes.notes.length ? "No notes match." : "No notes yet. Write down how things are set up, or a runbook for a fix you'll need again.") : "Loading…"}</li>
        {/each}
      </ul>
    </div>
  </Panel>

  <section class="main" data-guide="notes-view">
    {#if draft}
      <form class="editor" onsubmit={onSave}>
        <div class="row">
          <input class="input title" placeholder="Title" aria-label="Title" maxlength="120" bind:value={draft.input.title} onkeydown={saveKey} />
          <label class="pinbox"><input type="checkbox" bind:checked={draft.input.pinned} /> Pin</label>
          <label class="pinbox" title="DRIFT, the local helper, can search your notes to answer questions. Tick to keep this one out."
            ><input type="checkbox" bind:checked={draft.input.hideFromDrift} /> Hide from DRIFT</label
          >
        </div>
        <div class="row">
          <select class="input" aria-label="Host" bind:value={draft.input.hostId}>
            <option value={null}>Not tied to a host</option>
            {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
          </select>
          <input class="input grow" placeholder="Tags, e.g. docker, backup" aria-label="Tags" bind:value={tagText} onkeydown={saveKey} />
        </div>
        <fieldset class="runbook" data-guide="notes-runbook">
          <label class="pinbox"
            ><input type="checkbox" bind:checked={draft.input.runbook} /> Runbook for alerts
            <span class="muted small">(offered on matching alerts)</span></label
          >
          {#if draft.input.runbook}
            <div class="kinds" role="group" aria-label="Alert kinds">
              {#each KINDS as k (k)}
                <button type="button" class="chip kind" class:sel={draft.input.alertKinds.includes(k)} aria-pressed={draft.input.alertKinds.includes(k)} onclick={() => toggleKind(k)}>{KIND_LABEL[k]}</button>
              {/each}
            </div>
            <div class="row">
              <select class="input" aria-label="Only for this service" bind:value={draft.input.serviceId}>
                <option value={null}>Any service</option>
                {#each services.services as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
              </select>
              <span class="muted small grow">
                {#if !draft.input.alertKinds.length && draft.input.hostId === null && draft.input.serviceId === null}Pick alert kinds, a host or a service, or it won't show on any alert.{:else}Shows on: {runbookScope(draft.input, hostName, serviceName)}.{/if}
                Steps: <code>- [ ] step</code> becomes a checklist, <code>[Open disks](view:fleet)</code> a link.
              </span>
            </div>
          {/if}
        </fieldset>
        <textarea
          class="input body"
          aria-label="Note text (markdown)"
          placeholder="Markdown. Use ``` code blocks for commands: each gets a button to type it into a terminal on this note's host."
          bind:value={draft.input.body}
          onkeydown={saveKey}
          spellcheck="false"
        ></textarea>
        {#if secret}
          <div class="warn" role="alert">
            <span>This looks like it contains a password or token. Notes never store secrets; keep them in your password manager.</span>
            <button type="button" class="btn tiny" onclick={() => notes.maskDraft()}>Mask them</button>
          </div>
        {/if}
        {#if notes.saveError}<p class="error-text">{notes.saveError}</p>{/if}
        <div class="row end">
          <span class="muted small">Markdown: # headings, - lists, **bold**, `code`, ``` blocks. {formatChord("Mod+S")} saves.</span>
          <span class="spacer"></span>
          <button type="button" class="btn ghost" onclick={() => notes.cancel()}>Cancel</button>
          <button type="submit" class="btn primary" disabled={notes.saving || !draft.input.title.trim()}>{notes.saving ? "Saving…" : "Save"}</button>
        </div>
      </form>
    {:else if cur}
      <article class="reader">
        <header>
          <h1 aria-label={cur.title}><span use:scramble={cur.title}></span></h1>
          <div class="sub">
            {#if cur.runbook}<span class="rb" title="Offered on: {runbookScope(cur, hostName, serviceName)}">RUNBOOK</span>{/if}
            {#if cur.hostId !== null}<span class="host">{hostName(cur.hostId)}</span>{/if}
            {#each cur.tags as t (t)}<span class="muted">#{t}</span>{/each}
            <span class="muted">edited {ago(cur.updatedAt)}</span>
            {#if cur.hideFromDrift}<span class="muted" title="DRIFT doesn't search this note">· hidden from DRIFT</span>{/if}
            <span class="spacer"></span>
            <ExplainButton
              source="note “{cur.title}”"
              text={() => `# ${cur.title}\n\n${cur.body}`}
              question="Walk me through this runbook. Is anything risky, out of date or missing?"
              host={hostName(cur.hostId)}
            />
            <button type="button" class="btn ghost tiny" onclick={() => notes.togglePin(cur)}>{cur.pinned ? "Unpin" : "Pin"}</button>
            <button type="button" class="btn tiny" onclick={() => notes.edit(cur)}>Edit</button>
            {#if confirmDelete}
              <button type="button" class="btn danger tiny" onclick={() => void notes.remove(cur.id)}>Delete “{cur.title}”</button>
              <button type="button" class="btn ghost tiny" onclick={() => (confirmDelete = false)}>Keep</button>
            {:else}
              <button type="button" class="btn ghost tiny" onclick={() => (confirmDelete = true)}>Delete</button>
            {/if}
          </div>
        </header>
        <div class="text">
          {#if cur.body.trim()}
            {#if cur.runbook}<p class="muted small scope">For alerts: {runbookScope(cur, hostName, serviceName)}.</p>{/if}
            <Markdown src={cur.body} steps onview={(v) => ui.go(v as never)}>
              {#snippet code(b)}
                <button type="button" class="btn ghost tiny" onclick={() => void copy(b.text)}>Copy</button>
                <button
                  type="button"
                  class="btn tiny"
                  title={cur.hostId !== null ? `Types it into a terminal on ${hostName(cur.hostId)}; you press Enter` : "Types it into the focused terminal; you press Enter"}
                  onclick={() => send(b.text)}>Send to terminal</button
                >
              {/snippet}
            </Markdown>
          {:else}
            <p class="muted">Empty note.</p>
          {/if}
        </div>
        {#if flash}<p class="flash" role="status">{flash}</p>{/if}
      </article>
    {:else}
      <div class="blank muted">
        <p>Pick a note, or start one with <strong>New</strong>.</p>
        <p class="small">Notes are saved on this computer and included in encrypted backups. Passwords and tokens are refused.</p>
      </div>
    {/if}
  </section>
</div>

<style>
  .notes {
    height: 100%;
    display: grid;
    grid-template-columns: minmax(240px, 320px) 1fr;
    /* One row the height of the view: the list and the editor scroll inside it. */
    grid-template-rows: minmax(0, 1fr);
    gap: 12px;
    min-height: 0;
  }
  .notes > :global(.panel) {
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .notes > :global(.panel .body) {
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
    background: var(--bg-1);
  }
  .item.sel {
    border-left-color: var(--mint);
    background: var(--bg-1);
  }
  .t {
    font-size: var(--fs-sm);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pin {
    color: var(--magenta, var(--mint));
    margin-right: 6px;
    font-size: var(--fs-xs);
  }
  .meta {
    display: flex;
    gap: 6px;
    font-size: var(--fs-xs);
    flex-wrap: wrap;
  }
  .host {
    color: var(--cyan);
  }
  .when {
    margin-left: auto;
  }
  .empty {
    padding: 8px;
    font-size: var(--fs-sm);
  }
  .main {
    min-height: 0;
    border: 1px solid var(--line);
    background: color-mix(in srgb, var(--bg-1) 70%, transparent);
    display: flex;
    flex-direction: column;
  }
  .reader {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
  }
  .reader header {
    padding: 12px 16px 8px;
    border-bottom: 1px solid var(--line);
  }
  h1 {
    margin: 0 0 6px;
    font-family: var(--font-display);
    font-size: var(--fs-lg, 18px);
    letter-spacing: 0.04em;
    color: var(--text-0);
  }
  .sub {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .spacer {
    flex: 1;
  }
  .text {
    padding: 14px 16px;
    overflow: auto;
    flex: 1;
    min-height: 0;
    max-width: 860px;
  }
  .flash {
    margin: 0;
    padding: 6px 16px;
    border-top: 1px solid var(--line);
    color: var(--mint);
    font-size: var(--fs-xs);
  }
  .editor {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    flex: 1;
    min-height: 0;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .row.end {
    flex-wrap: wrap;
  }
  .title {
    flex: 1;
    font-size: var(--fs-md, 15px);
  }
  .grow {
    flex: 1;
  }
  .pinbox {
    display: flex;
    gap: 4px;
    align-items: center;
    font-size: var(--fs-sm);
    white-space: nowrap;
  }
  .body {
    flex: 1;
    min-height: 120px;
    resize: none;
    font-family: var(--font-mono);
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .warn {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 6px 10px;
    border: 1px solid var(--warn, var(--danger));
    color: var(--warn, var(--danger));
    font-size: var(--fs-sm);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .blank {
    margin: auto;
    text-align: center;
    padding: 20px;
  }
  @media (max-width: 760px) {
    .notes {
      grid-template-columns: 1fr;
    }
  }
  .runbook {
    border: 1px solid var(--line);
    padding: 6px 8px;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .kinds {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .chip.kind.sel {
    color: var(--magenta);
    border-color: var(--magenta);
    background: transparent;
  }
  .rb {
    color: var(--magenta);
    letter-spacing: 0.1em;
    font-size: var(--fs-xs);
  }
  .scope {
    margin: 0 0 8px;
  }
</style>
