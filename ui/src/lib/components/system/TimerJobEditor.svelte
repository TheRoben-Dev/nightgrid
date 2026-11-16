<script lang="ts">
  /**
   * Nightgrid timer editor (M10v): a command on a systemd calendar, as a
   * `nightgrid-job-*` service + timer on the host (needs root).
   */
  import { api, errorText, type CalendarPreview, type InstalledJob, type TimerJob } from "../../api";
  import { blankJob, TIMER_PRESETS, unitFor } from "../../jobs";
  import { jobs } from "../../stores/jobs.svelte";
  import { system } from "../../stores/system.svelte";
  import Modal from "../Modal.svelte";

  interface Props {
    open: boolean;
    /** The job being edited; null for a new one. */
    editing: InstalledJob | null;
    onclose: () => void;
  }
  let { open, editing, onclose }: Props = $props();

  let job = $state<TimerJob>(blankJob());
  let preview = $state<CalendarPreview | null>(null);
  let previewing = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let confirmRemove = $state(false);

  $effect(() => {
    if (!open) return;
    job = editing ? { ...editing.job } : blankJob();
    preview = null;
    error = null;
    confirmRemove = false;
  });

  const unit = $derived(unitFor(job.name));
  const clash = $derived(!!unit && unit !== editing?.unit && jobs.timers.some((t) => t.unit === unit));
  const valid = $derived(!!unit && !!job.command.trim() && !!job.calendar.trim() && !clash);

  async function check() {
    const id = system.hostId;
    if (id === null) return;
    previewing = true;
    try {
      preview = await api.timerCalendarPreview(id, job.calendar);
    } catch (e) {
      preview = { ok: false, normalized: errorText(e), next: [] };
    }
    previewing = false;
  }
  async function save() {
    busy = true;
    error = await jobs.saveTimer(job, editing?.unit ?? null);
    busy = false;
    if (!error) onclose();
  }
  async function remove() {
    if (!editing) return;
    busy = true;
    error = await jobs.removeTimer(editing.unit);
    busy = false;
    if (!error) onclose();
  }
</script>

<Modal {open} title={editing ? "EDIT TIMER" : "NEW TIMER"} width={640} {onclose}>
  <div class="form" data-guide="timer-editor">
    <label class="field">
      <span>Name</span>
      <input class="input" bind:value={job.name} placeholder="Nightly sync" maxlength="60" />
    </label>
    {#if unit}<p class="muted small">Unit: <code>{unit}.timer</code>{#if clash}<span class="bad"> already exists: pick another name</span>{/if}</p>{/if}
    <label class="field">
      <span>Command</span>
      <input class="input mono" bind:value={job.command} spellcheck="false" placeholder="rsync -a /srv/data/ /mnt/backup/data/" />
    </label>
    <div class="row">
      <label class="field grow">
        <span>When (systemd OnCalendar)</span>
        <input class="input mono" bind:value={job.calendar} spellcheck="false" oninput={() => (preview = null)} />
      </label>
      <label class="field">
        <span>Common</span>
        <select class="input" onchange={(e) => { const v = (e.currentTarget as HTMLSelectElement).value; if (v) { job.calendar = v; preview = null; } }}>
          <option value="">choose…</option>
          {#each TIMER_PRESETS as p (p.value)}<option value={p.value}>{p.label}</option>{/each}
        </select>
      </label>
      <button type="button" class="btn ghost tiny" disabled={previewing || !job.calendar.trim()} onclick={check}>{previewing ? "…" : "Check"}</button>
    </div>
    {#if preview}
      <p class="small {preview.ok ? 'ok' : 'bad'}">{preview.ok ? `systemd reads it as ${preview.normalized}` : preview.normalized}</p>
      {#if preview.next.length}<p class="muted small">Next: {preview.next.join(" · ")}</p>{/if}
    {/if}
    <div class="row">
      <label class="field grow">
        <span>Run as</span>
        <input class="input" value={job.runAs ?? ""} oninput={(e) => (job.runAs = (e.currentTarget as HTMLInputElement).value || null)} placeholder="root" spellcheck="false" />
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={job.persistent} />
        <span>Catch up on a run missed while the host was off</span>
      </label>
    </div>
    <p class="muted small">Output goes to the journal: Journal on the timer's row shows it. Use Run now on the row to try it.</p>
    {#if error}<p class="bad small" role="alert">{error}</p>{/if}
  </div>
  {#snippet footer()}
    {#if editing}
      {#if confirmRemove}
        <button type="button" class="btn danger" disabled={busy} onclick={remove}>Remove {editing.unit}</button>
        <button type="button" class="btn ghost" onclick={() => (confirmRemove = false)}>Keep</button>
      {:else}
        <button type="button" class="btn ghost" onclick={() => (confirmRemove = true)}>Remove…</button>
      {/if}
    {/if}
    <span class="spacer"></span>
    <button type="button" class="btn ghost" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" data-guide="timer-save" disabled={busy || !valid} onclick={save}>{busy ? "Saving…" : "Save"}</button>
  {/snippet}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .row {
    display: flex;
    gap: 10px;
    align-items: flex-end;
    flex-wrap: wrap;
  }
  .grow {
    flex: 1 1 220px;
  }
  .check {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: var(--fs-xs);
    padding-bottom: 6px;
  }
  .spacer {
    flex: 1;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .mono {
    font-family: var(--font-mono, monospace);
  }
  p {
    margin: 0;
  }
  .ok {
    color: var(--ok);
  }
  .bad {
    color: var(--danger);
  }
  .muted {
    color: var(--text-1);
  }
</style>
