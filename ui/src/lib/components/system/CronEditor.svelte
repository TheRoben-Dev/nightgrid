<script lang="ts">
  /**
   * Crontab editor (M10v): the login user's crontab, or root's / another
   * user's through sudo. Every change is checked and written back whole; the
   * host keeps a copy of the previous crontab.
   */
  import { api, type CrontabJob } from "../../api";
  import { redact } from "../../helper/prompt";
  import { jobs } from "../../stores/jobs.svelte";
  import { CRON_PRESETS } from "../../jobs";
  import Modal from "../Modal.svelte";

  interface Props {
    open: boolean;
    onclose: () => void;
  }
  let { open, onclose }: Props = $props();

  let who = $state<"me" | "root" | "other">("me");
  let other = $state("");
  let editing = $state<CrontabJob | null>(null);
  let schedule = $state("0 3 * * *");
  let command = $state("");
  let every = $state<string | null>(null);
  let confirmRemove = $state<number | null>(null);
  let revealed = $state<number[]>([]);

  const user = $derived(who === "me" ? null : who === "root" ? "root" : other.trim() || null);

  $effect(() => {
    if (open && !jobs.tab && !jobs.tabBusy) void jobs.openTab(null);
  });
  $effect(() => {
    const s = schedule;
    const t = setTimeout(async () => {
      every = await api.cronDescribe(s).catch(() => null);
    }, 150);
    return () => clearTimeout(t);
  });

  function load() {
    reset();
    void jobs.openTab(user);
  }
  function reset() {
    editing = null;
    schedule = "0 3 * * *";
    command = "";
    confirmRemove = null;
  }
  function startEdit(j: CrontabJob) {
    editing = j;
    schedule = j.schedule;
    command = j.command;
    confirmRemove = null;
  }
  async function save() {
    const ok = await jobs.edit(editing ? { op: "update", line: editing.line, schedule, command } : { op: "add", schedule, command });
    if (ok) reset();
  }
  const shown = (j: CrontabJob) => (revealed.includes(j.line) ? j.command : redact(j.command));
  const tabName = $derived(jobs.tab ? (jobs.tab.user ? `${jobs.tab.user}'s crontab` : "your crontab") : "crontab");
</script>

<Modal {open} title="CRONTAB" width={820} {onclose}>
  <div class="who" data-guide="cron-editor">
    <div class="seg tabstrip sm" role="group" aria-label="Whose crontab">
      <button type="button" class:on={who === "me"} onclick={() => ((who = "me"), load())}>MINE</button>
      <button type="button" class:on={who === "root"} onclick={() => ((who = "root"), load())}>ROOT</button>
      <button type="button" class:on={who === "other"} onclick={() => (who = "other")}>OTHER USER</button>
    </div>
    {#if who === "other"}
      <input class="input user" bind:value={other} placeholder="user name" aria-label="User name" spellcheck="false" />
      <button type="button" class="btn ghost tiny" disabled={!other.trim()} onclick={load}>Open</button>
    {/if}
    <span class="spacer"></span>
    <button type="button" class="btn ghost tiny" disabled={jobs.tabBusy} onclick={() => void jobs.openTab(jobs.tabUser)}>Reload</button>
  </div>

  {#if jobs.tabError}<p class="bad small" role="alert">{jobs.tabError}</p>{/if}
  {#if jobs.tabBusy && !jobs.tab}<p class="muted small">Reading {user ? `${user}'s` : "your"} crontab…</p>{/if}

  {#if jobs.tab}
    {#if jobs.tab.env.length}<p class="env mono small">{jobs.tab.env.join("  ·  ")}</p>{/if}
    <div class="list" aria-label={tabName}>
      {#each jobs.tab.jobs as j (j.line)}
        <div class="job" class:off={!j.enabled} class:sel={editing?.line === j.line}>
          <input type="checkbox" checked={j.enabled} title={j.enabled ? "Turn off (kept as a comment)" : "Turn on"} disabled={jobs.tabBusy} onchange={() => void jobs.edit({ op: "setEnabled", line: j.line, enabled: !j.enabled })} />
          <span class="when"><span>{j.every}</span>{#if j.every !== j.schedule}<code class="muted">{j.schedule}</code>{/if}</span>
          <code class="cmd">{shown(j)}</code>
          <span class="acts">
            {#if shown(j) !== j.command}<button type="button" class="btn ghost tiny" onclick={() => (revealed = [...revealed, j.line])}>Reveal</button>{/if}
            {#if confirmRemove === j.line}
              <button type="button" class="btn danger tiny" disabled={jobs.tabBusy} onclick={async () => (await jobs.edit({ op: "remove", line: j.line })) && reset()}>Delete this job</button>
              <button type="button" class="btn ghost tiny" onclick={() => (confirmRemove = null)}>Keep</button>
            {:else}
              <button type="button" class="btn ghost tiny" onclick={() => startEdit(j)}>Edit</button>
              <button type="button" class="btn ghost tiny" onclick={() => (confirmRemove = j.line)}>Delete…</button>
            {/if}
          </span>
        </div>
      {:else}
        <p class="muted small">No jobs in {tabName}.</p>
      {/each}
    </div>

    <form class="form" onsubmit={(e) => (e.preventDefault(), void save())}>
      <h4 class="label">{editing ? `EDIT LINE ${editing.line}` : "ADD A JOB"}</h4>
      <div class="row">
        <label class="field sched">
          <span>Schedule</span>
          <input class="input mono" bind:value={schedule} spellcheck="false" aria-label="Cron schedule" />
        </label>
        <label class="field">
          <span>Common</span>
          <select class="input" onchange={(e) => { const v = (e.currentTarget as HTMLSelectElement).value; if (v) schedule = v; }}>
            <option value="">choose…</option>
            {#each CRON_PRESETS as p (p.value)}<option value={p.value}>{p.label}</option>{/each}
          </select>
        </label>
        <span class="every small {every ? 'ok' : 'bad'}">{every ?? "not a cron schedule"}</span>
      </div>
      <label class="field">
        <span>Command</span>
        <input class="input mono" bind:value={command} spellcheck="false" placeholder="/usr/local/bin/backup.sh >> /var/log/backup.log 2>&1" aria-label="Command" />
      </label>
      <div class="row">
        <span class="muted small">Write % as \% (cron reads % as a line break). The previous crontab is kept in ~/.nightgrid/cron-backups on the host.</span>
        <span class="spacer"></span>
        {#if editing}<button type="button" class="btn ghost tiny" onclick={reset}>Cancel</button>{/if}
        <button type="submit" class="btn primary tiny" data-guide="cron-save" disabled={jobs.tabBusy || !every || !command.trim()}>{editing ? "Save" : "Add"}</button>
      </div>
    </form>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={onclose}>Close</button>
  {/snippet}
</Modal>

<style>
  .who,
  .row {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
  }
  .spacer {
    flex: 1;
  }
  .user {
    width: 160px;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .mono {
    font-family: var(--font-mono, monospace);
  }
  .env {
    color: var(--text-1);
    margin: 8px 0 0;
  }
  .list {
    margin: 8px 0;
    border: 1px solid var(--line);
    max-height: 300px;
    overflow: auto;
  }
  .job {
    display: grid;
    grid-template-columns: 20px minmax(130px, 200px) 1fr auto;
    gap: 10px;
    align-items: center;
    padding: 4px 8px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-xs);
  }
  .job.off {
    opacity: 0.55;
  }
  .job.sel {
    box-shadow: inset 2px 0 0 var(--magenta);
  }
  .when {
    display: flex;
    flex-direction: column;
  }
  .cmd {
    overflow-wrap: anywhere;
  }
  .acts {
    display: flex;
    gap: 4px;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 6px;
    border: 1px solid var(--line);
    padding: 8px;
  }
  h4 {
    margin: 0;
    color: var(--cyan);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    font-weight: 500;
  }
  .sched {
    flex: 1 1 200px;
  }
  .every {
    min-width: 160px;
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
