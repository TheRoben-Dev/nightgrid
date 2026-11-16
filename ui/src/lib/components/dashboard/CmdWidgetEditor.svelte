<script lang="ts">
  /**
   * Add or edit a custom command widget (M10x). "Try it" runs the command
   * once without saving, so the parse rule can be checked against real
   * output. Read-only commands only; the backend refuses the rest.
   */
  import { api, errorText } from "../../api";
  import { blank, display, EXAMPLES, INTERVALS, PARSES, parseThreshold, problems, type CmdReading, type CmdWidget } from "../../cmdwidgets";
  import { cmdWidgets } from "../../stores/cmdwidgets.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import Modal from "../Modal.svelte";

  let { onsaved, ondeleted }: { onsaved: (w: CmdWidget, isNew: boolean) => void; ondeleted: (id: number) => void } = $props();

  let form = $state<CmdWidget>(blank(0));
  let warnText = $state("");
  let critText = $state("");
  let tried = $state<CmdReading | null>(null);
  let tryError = $state<string | null>(null);
  let trying = $state(false);
  let saving = $state(false);
  let saveError = $state<string | null>(null);
  let confirmDelete = $state(false);
  let touched = $state(false);

  // Load the form whenever a widget is opened.
  let openedFor: CmdWidget | null = null;
  $effect(() => {
    const e = cmdWidgets.editing;
    if (e && e !== openedFor) {
      openedFor = e;
      form = { ...e, hostId: e.hostId || (hosts.preferred("widget")?.id ?? 0) };
      warnText = e.warn == null ? "" : String(e.warn);
      critText = e.crit == null ? "" : String(e.crit);
      tried = null;
      tryError = null;
      saveError = null;
      confirmDelete = false;
      touched = false;
    } else if (!e) openedFor = null;
  });

  const current = $derived<CmdWidget>({ ...form, warn: parseThreshold(warnText), crit: parseThreshold(critText) });
  const errs = $derived(problems(current));
  const badThreshold = $derived((!!warnText.trim() && current.warn == null) || (!!critText.trim() && current.crit == null));
  const ok = $derived(!Object.keys(errs).length && !badThreshold && current.hostId > 0);
  const isNew = $derived(form.id === 0);

  function example(i: number) {
    const ex = EXAMPLES[i];
    form = { ...form, ...ex.w, pattern: ex.w.pattern ?? "", unit: ex.w.unit ?? "", below: false };
    warnText = ex.w.warn == null ? "" : String(ex.w.warn);
    critText = ex.w.crit == null ? "" : String(ex.w.crit);
    tried = null;
  }

  async function tryIt() {
    touched = true;
    if (errs.command || errs.pattern || !current.hostId) return;
    trying = true;
    tryError = null;
    try {
      tried = await api.cmdWidgetTry({ ...current, name: current.name.trim() || "Try" });
    } catch (e) {
      tried = null;
      tryError = errorText(e);
    } finally {
      trying = false;
    }
  }

  async function save() {
    touched = true;
    if (!ok) return;
    saving = true;
    saveError = null;
    try {
      const wasNew = isNew;
      const saved = await cmdWidgets.save(current);
      cmdWidgets.editing = null;
      onsaved(saved, wasNew);
    } catch (e) {
      saveError = errorText(e);
    } finally {
      saving = false;
    }
  }

  async function del() {
    const id = form.id;
    try {
      await cmdWidgets.remove(id);
      cmdWidgets.editing = null;
      ondeleted(id);
    } catch (e) {
      saveError = errorText(e);
    }
  }
</script>

<Modal open={cmdWidgets.editing !== null} title={isNew ? "NEW COMMAND WIDGET" : "EDIT COMMAND WIDGET"} width={620} onclose={() => (cmdWidgets.editing = null)}>
  <form
    class="form"
    data-guide="cmd-editor"
    onsubmit={(e) => {
      e.preventDefault();
      void save();
    }}
  >
    {#if isNew}
      <div class="examples">
        <span class="muted small">START FROM</span>
        {#each EXAMPLES as ex, i (ex.label)}<button type="button" class="chip" onclick={() => example(i)}>{ex.label}</button>{/each}
      </div>
    {/if}
    <div class="two">
      <label class="field"><span class="label">NAME</span><input class="input" maxlength="40" bind:value={form.name} placeholder="Root used" /></label>
      <label class="field"
        ><span class="label">HOST</span>
        <select class="input" bind:value={form.hostId}>{#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}</select>
      </label>
    </div>
    {#if touched && errs.name}<p class="error-text small">{errs.name}</p>{/if}
    <label class="field"
      ><span class="label">COMMAND</span>
      <textarea class="input mono" rows="2" spellcheck="false" maxlength="1000" bind:value={form.command} placeholder="df -P / | tail -1"></textarea>
    </label>
    {#if errs.command && (touched || form.command.trim())}<p class={form.command.trim() ? "warn-text small" : "error-text small"}>{errs.command}</p>{/if}
    <p class="muted small hint">Runs as your SSH user on every refresh, with no terminal. It must only read: commands that delete, stop, prune or reboot, and sudo, are refused.</p>

    <div class="two">
      <label class="field"
        ><span class="label">READ THE VALUE AS</span>
        <select class="input" bind:value={form.parse}>{#each PARSES as p (p.id)}<option value={p.id}>{p.label}</option>{/each}</select>
        <span class="muted small">{PARSES.find((p) => p.id === form.parse)?.what}</span>
      </label>
      {#if form.parse === "regex"}
        <label class="field"><span class="label">PATTERN</span><input class="input mono" maxlength="300" spellcheck="false" bind:value={form.pattern} placeholder="(\d+)%" /></label>
      {:else}
        <label class="field"><span class="label">UNIT</span><input class="input" maxlength="12" bind:value={form.unit} placeholder="%, GB, users" /></label>
      {/if}
    </div>
    {#if errs.pattern && (touched || form.pattern)}<p class="error-text small">{errs.pattern}</p>{/if}
    {#if form.parse === "regex"}
      <label class="field narrow"><span class="label">UNIT</span><input class="input" maxlength="12" bind:value={form.unit} placeholder="%, GB, users" /></label>
    {/if}

    {#if form.parse !== "exit" && form.parse !== "text"}
      <div class="three">
        <label class="field"><span class="label">WARN AT</span><input class="input" inputmode="decimal" bind:value={warnText} placeholder="none" /></label>
        <label class="field"><span class="label">CRITICAL AT</span><input class="input" inputmode="decimal" bind:value={critText} placeholder="none" /></label>
        <label class="check"><input type="checkbox" bind:checked={form.below} /> Low is bad (alert at or below)</label>
      </div>
      {#if badThreshold}<p class="error-text small">Thresholds must be numbers.</p>{:else if errs.crit}<p class="error-text small">{errs.crit}</p>{/if}
    {/if}
    <label class="field narrow"
      ><span class="label">REFRESH</span>
      <select class="input" bind:value={form.intervalSecs}>{#each INTERVALS as [n, l] (n)}<option value={n}>every {l}</option>{/each}</select>
    </label>

    <div class="try">
      <button type="button" class="btn tiny" data-guide="cmd-try" disabled={trying || !form.command.trim() || !form.hostId} onclick={tryIt}>{trying ? "Running…" : "Try it"}</button>
      {#if tried}
        <span class="val {tried.level}">{display(tried, current.unit)}</span>
        <span class="muted small">exit {tried.exit}</span>
      {/if}
      {#if tryError}<span class="error-text small">{tryError}</span>{/if}
    </div>
    {#if tried}<pre class="out">{tried.output || "(no output)"}</pre>{/if}
    {#if saveError}<p class="error-text small" role="alert">{saveError}</p>{/if}
  </form>
  {#snippet footer()}
    {#if !isNew}
      {#if confirmDelete}
        <span class="muted small grow">Delete “{form.name}”? Its tiles go too.</span>
        <button type="button" class="btn ghost" onclick={() => (confirmDelete = false)}>Keep</button>
        <button type="button" class="btn danger" onclick={del}>Delete widget</button>
      {:else}
        <button type="button" class="btn ghost" onclick={() => (confirmDelete = true)}>Delete…</button>
        <span class="grow"></span>
      {/if}
    {/if}
    {#if !confirmDelete}
      <button type="button" class="btn ghost" onclick={() => (cmdWidgets.editing = null)}>Cancel</button>
      <button type="button" class="btn primary" data-guide="cmd-save" disabled={saving || (touched && !ok)} onclick={save}>{saving ? "Saving…" : isNew ? "Add widget" : "Save"}</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .narrow {
    max-width: 200px;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .three {
    display: grid;
    grid-template-columns: 1fr 1fr 1.3fr;
    gap: 10px;
    align-items: end;
  }
  .check {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: var(--fs-sm);
    color: var(--text-1);
    padding-bottom: 6px;
  }
  .mono {
    font-family: var(--font-mono);
  }
  textarea {
    resize: vertical;
  }
  .hint,
  .error-text,
  .warn-text {
    margin: 0;
  }
  .warn-text {
    color: var(--warn);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .examples {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    align-items: center;
  }
  .chip {
    background: none;
    border: 1px solid var(--line);
    color: var(--text-0);
    font: inherit;
    font-size: var(--fs-xs);
    padding: 2px 8px;
    cursor: pointer;
  }
  .chip:hover {
    border-color: var(--cyan);
    color: var(--cyan);
  }
  .try {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
  }
  .val {
    font-family: var(--font-mono);
    font-size: var(--fs-lg);
    color: var(--cyan);
  }
  .val.ok {
    color: var(--ok);
  }
  .val.warn {
    color: var(--warn);
  }
  .val.crit,
  .val.error {
    color: var(--danger);
  }
  .out {
    margin: 0;
    max-height: 140px;
    overflow: auto;
    font-size: var(--fs-xs);
    background: var(--bg-0);
    border: 1px solid var(--line);
    padding: 4px 6px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .grow {
    flex: 1;
  }
</style>
