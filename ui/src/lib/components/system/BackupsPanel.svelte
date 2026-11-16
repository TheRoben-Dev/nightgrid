<script lang="ts">
  /**
   * Backup status board (M10g): restic / borg / rsync / rclone jobs on every
   * host with last run, size, result and a stale flag. Jobs point at a
   * systemd unit or a path. Nothing here runs a backup; a test restore (M10m)
   * opens the repository only when started, as root on the host.
   */
  import type { BackupJob, BackupStatus } from "../../api";
  import { backups, bytes, jobState, staleWords, testUnsupported, TOOL_LABEL, type JobState } from "../../stores/backups.svelte";
  import { ui } from "../../stores/ui.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { system } from "../../stores/system.svelte";
  import Modal from "../Modal.svelte";
  import ExplainButton from "../helper/ExplainButton.svelte";

  const STATE_WORD: Record<JobState, string> = {
    ok: "OK",
    failed: "FAILED",
    stale: "STALE",
    running: "RUNNING",
    unknown: "NO DATA",
    error: "CAN'T READ",
  };
  const STALE_STEPS = [6, 12, 26, 50, 7 * 24 + 2, 31 * 24];

  let now = $state(Date.now() / 1000);
  $effect(() => {
    const t = setInterval(() => (now = Date.now() / 1000), 30_000);
    return () => clearInterval(t);
  });

  const hostName = (id: number) => hosts.hosts.find((h) => h.id === id)?.name ?? `host ${id}`;
  const groups = $derived(
    [...new Set(backups.jobs.map((j) => j.hostId))].map((id) => ({ id, name: hostName(id), jobs: backups.jobs.filter((j) => j.hostId === id) })),
  );
  const counts = $derived.by(() => {
    const c = { ok: 0, bad: 0, stale: 0 };
    for (const j of backups.jobs) {
      const s = jobState(backups.status[j.id]);
      if (s === "ok") c.ok++;
      else if (s === "failed" || s === "error") c.bad++;
      else if (s === "stale") c.stale++;
    }
    return c;
  });

  function rel(ts: number | null): string {
    if (ts == null) return "never";
    const m = Math.round(Math.max(0, now - ts) / 60);
    if (m < 1) return "just now";
    if (m < 60) return `${m}m ago`;
    const h = Math.floor(m / 60);
    if (h < 48) return `${h}h ago`;
    return `${Math.floor(h / 24)}d ago`;
  }
  const stamp = (ts: number | null) => (ts == null ? "" : new Date(ts * 1000).toLocaleString());

  function sizeText(s: BackupStatus | undefined): string {
    if (!s?.size && !s?.added) return "";
    const parts = [];
    if (s.size) parts.push(bytes(s.size));
    if (s.added) parts.push(`+${bytes(s.added)}`);
    return parts.join(" · ");
  }

  function explainText(): string {
    const lines = ["Backup jobs (Nightgrid board):"];
    for (const g of groups)
      for (const j of g.jobs) {
        const s = backups.status[j.id];
        lines.push(
          `- ${g.name} / ${j.name} [${j.tool}, ${j.source} ${j.target}, stale after ${staleWords(j.staleHours)}]: ${STATE_WORD[jobState(s)]}; last run ${rel(s?.lastRun ?? null)}; last good ${rel(s?.lastOk ?? null)}${s?.summary ? `; ${s.summary}` : ""}${s?.error ? `; ${s.error}` : ""}`,
        );
      }
    return lines.join("\n");
  }

  const e = $derived(backups.editing);
  let detectPick = $state<number | null>(null);
  const detectId = $derived(detectPick ?? system.hostId ?? hosts.preferred("system")?.id ?? null);
  const tr = $derived(backups.testResult);
  const trJob = $derived(tr ? backups.jobs.find((j) => j.id === tr.jobId) : undefined);
  const detectFor = $derived(backups.detectHost === null ? "" : hostName(backups.detectHost));
</script>

<div class="board">
  <div class="bar">
    <span class="counts" data-guide="backups-summary">
      <span class="mint">{backups.jobs.length}</span> JOBS{#if counts.bad}<span class="bad">&nbsp;· {counts.bad} FAILING</span>{/if}{#if counts.stale}<span class="warn"
          >&nbsp;· {counts.stale} STALE</span
        >{/if}
    </span>
    <span class="spacer"></span>
    {#if backups.jobs.length}<ExplainButton source="backup board" text={explainText} question="Are my backups healthy? What's failing or stale, and what should I check first?" />{/if}
    <select class="input tiny-select" aria-label="Host to search for backup jobs" value={detectId ?? ""} onchange={(ev) => (detectPick = Number(ev.currentTarget.value))}>
      {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
    </select>
    <button type="button" class="btn ghost tiny" data-guide="backups-detect" disabled={backups.detecting || detectId === null} onclick={() => detectId !== null && backups.detect(detectId)}
      >{backups.detecting ? "Looking…" : "Find jobs"}</button
    >
    <button type="button" class="btn ghost tiny" data-guide="backups-add" onclick={() => backups.edit(null, detectId ?? undefined)}>+ Add job</button>
    <button type="button" class="btn ghost tiny" disabled={backups.checking || !backups.jobs.length} onclick={() => backups.checkAll()}>{backups.checking ? "Checking…" : "Check all"}</button>
  </div>

  <div class="scroll" data-guide="backups-board">
    {#if backups.error}<p class="error-text pad">{backups.error}</p>{/if}
    {#if backups.suggestions}
      <section class="found">
        <h3 class="label">// FOUND ON {detectFor.toUpperCase()}</h3>
        {#if !backups.suggestions.length}
          <p class="muted pad">No restic, borg, rsync or rclone jobs in the systemd timers or cron tables that aren't on the board already.</p>
        {:else}
          <ul class="sugg">
            {#each backups.suggestions as s, i (i)}
              <li>
                <span class="tool">{TOOL_LABEL[s.tool]}</span>
                <span class="sd"><strong>{s.name}</strong> <span class="muted small">{s.detail}</span></span>
                <button type="button" class="btn ghost tiny" onclick={() => backups.adopt(s)}>Add…</button>
              </li>
            {/each}
          </ul>
        {/if}
        <button type="button" class="btn ghost tiny closer" onclick={() => (backups.suggestions = null)}>Hide</button>
      </section>
    {/if}

    {#if backups.loaded && !backups.jobs.length}
      <div class="empty">
        <p class="label">NO BACKUP JOBS YET</p>
        <p class="muted">
          Point Nightgrid at a backup that already runs on a host: a <strong>systemd timer</strong> (it reads the last run, exit status and the totals restic, borg, rsync or rclone print) or a
          <strong>path</strong> such as a restic or borg repository or an rsync target (it reads the newest file and the size). <strong>Find jobs</strong> looks through the host's timers and cron tables.
        </p>
      </div>
    {/if}

    {#each groups as g (g.id)}
      <section>
        <h3 class="label">// {g.name.toUpperCase()}{#if backups.hostErrors[g.id]}<span class="herr"> · {backups.hostErrors[g.id]}</span>{/if}</h3>
        <div class="grid" role="table" aria-label="Backup jobs on {g.name}">
          <div class="head" role="row">
            <span role="columnheader">JOB</span><span role="columnheader">STATE</span><span role="columnheader">LAST RUN</span><span role="columnheader">SIZE</span><span></span>
          </div>
          {#each g.jobs as j (j.id)}
            {@const s = backups.status[j.id]}
            {@const st = jobState(s)}
            {@const lt = backups.tests[j.id]}
            {@const why = testUnsupported(j)}
            <div class="row {st}" role="row">
              <span class="name">
                <span><span class="tool">{TOOL_LABEL[j.tool]}</span> {j.name}</span>
                <code class="muted small" title={j.target}>{j.source === "unit" ? "unit" : "path"} {j.target}</code>
                {#if lt}<span class="tested small" class:bad={!lt.ok} title={lt.note}>{lt.ok ? "✓ restore tested" : "✗ restore test failed"} {rel(lt.at)}</span>{/if}
              </span>
              <span class="state">
                <span class="badge {st}" title={st === "stale" ? `No good run in the last ${staleWords(j.staleHours)}` : (s?.error ?? "")}>{STATE_WORD[st]}</span>
                {#if s?.summary || s?.error}<span class="muted small sum" title={s.error ?? s.summary}>{s.error ?? s.summary}</span>{/if}
              </span>
              <span class="when" title={stamp(s?.lastRun ?? null)}>
                {s ? rel(s.lastRun) : "…"}
                {#if s && s.lastOk !== s.lastRun && s.lastOk}<span class="muted small">good {rel(s.lastOk)}</span>{/if}
                {#if s?.exitCode}<span class="muted small">exit {s.exitCode}</span>{/if}
              </span>
              <span class="size">{sizeText(s)}</span>
              <span class="acts">
                <button
                  type="button"
                  class="btn ghost tiny"
                  data-guide="backups-test"
                  title={why ?? "Restore one file from the latest backup into a temporary folder and check it"}
                  disabled={!!why || backups.testing.includes(j.id)}
                  onclick={() => (backups.confirmTest = j)}>{backups.testing.includes(j.id) ? "Testing…" : "Test restore"}</button
                >
                {#if j.source === "unit"}<button type="button" class="btn ghost tiny" title="Journal of this unit" onclick={() => {
                      system.select(j.hostId);
                      ui.go("system");
                      void system.showJournal(j.target.endsWith(".timer") ? j.target.replace(/\.timer$/, ".service") : j.target);
                    }}>Journal</button
                  >{/if}
                <button type="button" class="btn ghost tiny" onclick={() => backups.edit(j)}>Edit</button>
              </span>
            </div>
          {/each}
        </div>
      </section>
    {/each}
    {#if backups.jobs.length}
      <p class="muted small note">
        The board is read-only: Nightgrid never runs a backup, and opens a repository only for a <strong>Test restore</strong> you start (as root, with the unit's own environment, into a temporary folder it deletes). Sizes come from the last run's output (restic, borg <code>--stats</code>, rsync <code>--stats</code> or <code>-v</code>, rclone) or from
        <code>du</code> for paths. Journal lines may need the login to be in the <code>systemd-journal</code> or <code>adm</code> group.
      </p>
    {/if}
  </div>
</div>

<Modal open={!!e} title={e?.id === null ? "ADD BACKUP JOB" : "EDIT BACKUP JOB"} width={560} onclose={() => (backups.editing = null)}>
  {#if e}
    <form
      class="form"
      onsubmit={(ev) => {
        ev.preventDefault();
        void backups.save();
      }}
    >
      <label class="fld"><span class="label">HOST</span>
        <select class="input" bind:value={e.input.hostId}>
          {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
        </select>
      </label>
      <label class="fld"><span class="label">NAME</span><input class="input" bind:value={e.input.name} placeholder="nightly restic" maxlength="80" /></label>
      <label class="fld"><span class="label">TOOL</span>
        <select class="input" bind:value={e.input.tool}>
          {#each Object.entries(TOOL_LABEL) as [v, l] (v)}<option value={v}>{l}</option>{/each}
        </select>
      </label>
      <div class="fld">
        <span class="label">READ FROM</span>
        <div class="seg" role="radiogroup" aria-label="Read from">
          <label><input type="radio" bind:group={e.input.source} value="unit" /> systemd unit</label>
          <label><input type="radio" bind:group={e.input.source} value="path" /> path</label>
        </div>
      </div>
      <label class="fld"
        ><span class="label">{e.input.source === "unit" ? "TIMER OR SERVICE" : "REPOSITORY, TARGET OR MARKER FILE"}</span>
        <input class="input mono" bind:value={e.input.target} placeholder={e.input.source === "unit" ? "restic-backup.timer" : "/mnt/backup/restic"} />
      </label>
      <label class="fld"><span class="label">STALE AFTER</span>
        <select class="input" bind:value={e.input.staleHours}>
          {#each [...new Set([...STALE_STEPS, e.input.staleHours])].sort((a, b) => a - b) as h (h)}<option value={h}>{staleWords(h)}</option>{/each}
        </select>
      </label>
      <p class="muted small">
        {#if e.input.source === "unit"}
          The last run's result and exit status come from systemd; size and snapshot id from what the tool printed in the journal.
        {:else}
          The newest file under the path (three levels deep) counts as the last run, and <code>du</code> gives the size. A restic or borg repository updates its files on every backup.
        {/if}
      </p>
      {#if backups.saveError}<p class="error-text">{backups.saveError}</p>{/if}
      <button type="submit" hidden aria-hidden="true" tabindex="-1"></button>
    </form>
  {/if}
  {#snippet footer()}
    {#if e && e.id !== null}
      <button
        type="button"
        class="btn ghost danger-text"
        onclick={() => {
          const j = backups.jobs.find((x) => x.id === e.id);
          backups.editing = null;
          if (j) backups.pendingDelete = j;
        }}>Remove…</button
      >
    {/if}
    <span class="spacer"></span>
    <button type="button" class="btn ghost" onclick={() => (backups.editing = null)}>Cancel</button>
    <button type="button" class="btn primary" disabled={!e?.input.name.trim() || !e?.input.target.trim()} onclick={() => void backups.save()}>Save</button>
  {/snippet}
</Modal>

<Modal open={!!backups.confirmTest} title="TEST RESTORE" width={520} onclose={() => (backups.confirmTest = null)}>
  {#if backups.confirmTest}
    {@const j = backups.confirmTest}
    <p>Restore one file from the latest backup of <strong>{j.name}</strong> on <strong>{hostName(j.hostId)}</strong> and check it?</p>
    <p class="muted">
      {#if j.tool === "restic" || j.tool === "borg"}
        Nightgrid picks a random file under 20 MB from the latest {j.tool === "restic" ? "snapshot" : "archive"}, restores it into a new folder under <code>/tmp</code>, compares its size with
        the backup's listing and deletes the folder. It runs as root{j.source === "unit" ? " with the unit's own environment and user, so the repository password stays on the host" : ""}.
        Nothing in the repository or the live files changes.
      {:else}
        Nightgrid copies a random file under 20 MB from <code>{j.target}</code> into a new folder under <code>/tmp</code>, compares checksums and deletes the folder. It runs as root.
      {/if}
    </p>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (backups.confirmTest = null)}>Cancel</button>
    <button type="button" class="btn primary" onclick={() => backups.confirmTest && backups.testRestore(backups.confirmTest)}>Run test restore</button>
  {/snippet}
</Modal>

<Modal open={!!tr} title={tr?.ok ? "RESTORE TEST PASSED" : "RESTORE TEST FAILED"} tone={tr?.ok ? "default" : "danger"} width={560} onclose={() => (backups.testResult = null)}>
  {#if tr}
    <p>
      <strong>{trJob?.name ?? "Backup job"}</strong> on {trJob ? hostName(trJob.hostId) : "its host"}:
      {#if tr.ok}
        restored <code>{tr.file}</code> ({bytes(tr.size)}){tr.snapshot ? ` from ${tr.snapshot}` : ""}{tr.snapshotTime ? ` (${tr.snapshotTime})` : ""}. Size matched; the temporary copy was deleted.
      {:else}
        <span class="bad">{tr.error}</span>
      {/if}
    </p>
    {#if tr.sha256}<p class="muted small">SHA-256 <code class="hash">{tr.sha256}</code></p>{/if}
    {#if tr.steps.length}
      <ol class="steps muted small">
        {#each tr.steps as s, i (i)}<li>{s}</li>{/each}
      </ol>
    {/if}
  {/if}
  {#snippet footer()}
    <button type="button" class="btn primary" onclick={() => (backups.testResult = null)}>Close</button>
  {/snippet}
</Modal>

<Modal open={!!backups.pendingDelete} title="REMOVE BACKUP JOB" tone="danger" width={480} onclose={() => (backups.pendingDelete = null)}>
  {#if backups.pendingDelete}
    <p>
      Remove <strong>{backups.pendingDelete.name}</strong> on <strong>{hostName(backups.pendingDelete.hostId)}</strong> from the board? The backup itself and its repository aren't touched.
    </p>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (backups.pendingDelete = null)}>Cancel</button>
    <button type="button" class="btn danger" onclick={() => backups.pendingDelete && backups.remove(backups.pendingDelete as BackupJob)}>Remove {backups.pendingDelete?.name ?? ""}</button>
  {/snippet}
</Modal>

<style>
  .board {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  .bar {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .counts {
    color: var(--text-1);
    letter-spacing: 0.1em;
  }
  .mint {
    color: var(--mint);
  }
  .bad,
  .herr {
    color: var(--danger);
  }
  .warn {
    color: var(--warn, var(--yellow));
  }
  .spacer {
    flex: 1;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border: 1px solid var(--line);
    background: var(--bg-0);
  }
  section + section {
    border-top: 1px solid var(--line);
  }
  h3 {
    margin: 0;
    padding: 8px 10px 4px;
    color: var(--cyan);
  }
  .herr {
    font-family: var(--font-body, inherit);
    letter-spacing: 0;
    text-transform: none;
  }
  .pad {
    padding: 8px 10px 12px;
    margin: 0;
  }
  .empty {
    padding: 24px 16px;
    max-width: 640px;
  }
  .grid .head,
  .grid .row {
    display: grid;
    grid-template-columns: minmax(200px, 1.4fr) minmax(160px, 1.2fr) 110px 100px 270px;
    gap: 10px;
    align-items: center;
    padding: 5px 10px;
  }
  .head {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-2);
    border-bottom: 1px solid var(--line);
  }
  .row {
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
    font-size: var(--fs-sm);
    border-left: 2px solid transparent;
  }
  .row:hover {
    background: var(--bg-1);
  }
  .row.failed,
  .row.error {
    border-left-color: var(--danger);
  }
  .row.stale {
    border-left-color: var(--warn, var(--yellow));
  }
  .name,
  .state,
  .when {
    display: flex;
    flex-direction: column;
    min-width: 0;
    gap: 1px;
  }
  .name > *,
  .sum {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tool {
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
    color: var(--cyan);
    border: 1px solid color-mix(in srgb, var(--cyan) 50%, transparent);
    padding: 0 4px;
    margin-right: 4px;
  }
  .badge {
    align-self: flex-start;
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    padding: 1px 6px;
    border: 1px solid var(--line);
    color: var(--text-2);
  }
  .badge.ok {
    color: var(--mint);
    border-color: var(--mint);
  }
  .badge.failed,
  .badge.error {
    color: var(--bg-0);
    background: var(--danger);
    border-color: var(--danger);
  }
  .badge.stale {
    color: var(--bg-0);
    background: var(--warn, var(--yellow));
    border-color: var(--warn, var(--yellow));
  }
  .badge.running {
    color: var(--cyan);
    border-color: var(--cyan);
  }
  .size {
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .acts {
    display: flex;
    gap: 4px;
    justify-content: flex-end;
  }
  .small {
    font-size: var(--fs-xs);
  }
  code {
    font-family: var(--font-mono);
  }
  .note {
    padding: 10px;
    margin: 0;
  }
  .found {
    border-bottom: 1px solid var(--line);
    background: color-mix(in srgb, var(--cyan) 5%, var(--bg-0));
    padding-bottom: 8px;
  }
  .sugg {
    list-style: none;
    margin: 0;
    padding: 0 10px;
  }
  .sugg li {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 4px 0;
  }
  .sd {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .closer {
    margin: 4px 10px 0;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .fld {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .seg {
    display: flex;
    gap: 14px;
    font-size: var(--fs-sm);
  }
  .mono {
    font-family: var(--font-mono);
  }
  .acts {
    white-space: nowrap;
  }
  .tested {
    color: var(--mint);
  }
  .tested.bad {
    color: var(--danger);
  }
  .tiny-select {
    font-size: var(--fs-xs);
    padding: 1px 4px;
    max-width: 140px;
  }
  .steps {
    margin: 6px 0 0;
    padding-left: 18px;
  }
  .hash {
    overflow-wrap: anywhere;
  }
</style>
