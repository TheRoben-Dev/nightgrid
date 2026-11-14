<script lang="ts">
  import { api, type ConflictPolicy, type JobView } from "../../api";
  import { formatSize, formatSpeed, formatTime } from "../../files/util";
  import { files, sideLabel } from "../../stores/files.svelte";

  let applyAll = $state(false);

  const pct = (j: JobView) => (j.bytesTotal ? Math.min(100, (j.bytesDone / j.bytesTotal) * 100) : j.state === "done" ? 100 : 0);
  const eta = (j: JobView) => {
    if (!j.speed || j.state !== "running") return "";
    const s = Math.max(0, Math.round((j.bytesTotal - j.bytesDone) / j.speed));
    return s >= 3600 ? `${Math.floor(s / 3600)}h${Math.floor((s % 3600) / 60)}m` : s >= 60 ? `${Math.floor(s / 60)}m${s % 60}s` : `${s}s`;
  };
  // Jobs that need the user first, then active ones, then history.
  const rank: Record<string, number> = { conflict: 0, failed: 1, running: 2, paused: 3, queued: 4, done: 5, skipped: 5, cancelled: 5 };
  const ordered = $derived([...files.jobs].sort((a, b) => rank[a.state] - rank[b.state] || a.id - b.id));
  const finished = $derived(files.jobs.filter((j) => ["done", "skipped", "cancelled"].includes(j.state)).length);
  const totalSpeed = $derived(files.jobs.reduce((a, j) => a + (j.state === "running" ? j.speed : 0), 0));

  function resolve(j: JobView, p: ConflictPolicy) {
    void files.resolve(j.id, p, applyAll);
  }

  const stateLabel: Record<string, string> = {
    queued: "QUEUED",
    running: "XFER",
    paused: "PAUSED",
    conflict: "CONFLICT",
    done: "DONE",
    skipped: "SKIPPED",
    failed: "FAILED",
    cancelled: "CANCELLED",
  };
</script>

<section class="queue" class:open={files.queueOpen} aria-label="Transfer queue">
  <header>
    <button type="button" class="toggle" aria-expanded={files.queueOpen} onclick={() => (files.queueOpen = !files.queueOpen)}>
      <span class="label"><span class="cyan">//</span> TRANSFERS</span>
      <span class="count">{files.activeTransfers} ACTIVE</span>
      {#if files.conflicts}<span class="count warn">{files.conflicts} NEED YOU</span>{/if}
      {#if totalSpeed}<span class="count mint">{formatSpeed(totalSpeed)}</span>{/if}
      <span class="chev" aria-hidden="true">{files.queueOpen ? "▾" : "▸"}</span>
    </button>
    <span class="spacer"></span>
    {#if finished}
      <button type="button" class="btn ghost tiny" onclick={() => api.transfersClear()}>Clear finished ({finished})</button>
    {/if}
  </header>
  {#if files.queueOpen}
    <div class="jobs">
      {#if !files.jobs.length}
        <p class="muted none">No transfers. Select files and press F5, use COPY →, or drag them onto the other pane.</p>
      {/if}
      {#each ordered as j (j.id)}
        <div class="job {j.state}">
          <div class="line">
            <span class="st">{stateLabel[j.state]}</span>
            <span class="name" title={j.srcPath}>{j.isDir ? "▣ " : ""}{j.name}</span>
            <span class="route muted">{sideLabel(j.src)} → {sideLabel(j.dst)}:{j.dstDir}</span>
            <span class="spacer"></span>
            {#if j.bytesTotal}
              <span class="nums">{formatSize(j.bytesDone)} / {formatSize(j.bytesTotal)}</span>
            {/if}
            {#if j.filesTotal > 1}<span class="nums">{j.filesDone}/{j.filesTotal} files</span>{/if}
            {#if j.state === "running"}<span class="nums mint">{formatSpeed(j.speed)} {eta(j)}</span>{/if}
            <span class="acts">
              {#if j.state === "running" || j.state === "queued"}
                <button type="button" class="act" title="Pause" aria-label="Pause {j.name}" onclick={() => api.transferPause(j.id)}>❚❚</button>
              {/if}
              {#if j.state === "paused"}
                <button type="button" class="act" title="Resume" aria-label="Resume {j.name}" onclick={() => api.transferResume(j.id)}>▶</button>
              {/if}
              {#if j.state === "failed"}
                <button type="button" class="act" title="Retry (continues partial files)" aria-label="Retry {j.name}" onclick={() => api.transferResume(j.id)}>↻</button>
              {/if}
              {#if !["done", "skipped", "cancelled"].includes(j.state)}
                <button type="button" class="act" title="Cancel" aria-label="Cancel {j.name}" onclick={() => api.transferCancel(j.id)}>×</button>
              {/if}
            </span>
          </div>
          <div class="bar" aria-hidden="true"><span style:width="{pct(j)}%"></span></div>
          {#if j.error}<p class="error-text err">{j.error}</p>{/if}
          {#if j.state === "conflict" && j.conflict}
            <div class="conflict" role="group" aria-label="Conflict for {j.name}">
              <p class="q">
                <strong>{j.name}</strong> already exists in <code>{sideLabel(j.dst)}:{j.dstDir}</code>.
              </p>
              <div class="compare">
                <span class="k">SOURCE</span><span>{formatSize(j.conflict.src.size)}</span><span>{formatTime(j.conflict.src.mtime)}</span>
                <span class="k">EXISTING</span><span>{formatSize(j.conflict.dst.size)}</span><span>{formatTime(j.conflict.dst.mtime)}</span>
              </div>
              <div class="choices">
                <button type="button" class="btn danger tiny" onclick={() => resolve(j, "overwrite")}
                  >{j.conflict.dst.kind === "dir" ? "Merge into" : "Overwrite"} {j.name}</button
                >
                <button type="button" class="btn ghost tiny" onclick={() => resolve(j, "rename")}>Keep both</button>
                <button type="button" class="btn ghost tiny" onclick={() => resolve(j, "skip")}>Skip</button>
                <label class="all"><input type="checkbox" bind:checked={applyAll} /> apply to all conflicts</label>
              </div>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .queue {
    border: 1px solid var(--line);
    background: var(--bg-0);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .queue.open {
    max-height: 40%;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
  }
  .queue.open header {
    border-bottom: 1px solid var(--line);
  }
  .toggle {
    display: flex;
    gap: 12px;
    align-items: center;
    border: 0;
    background: transparent;
    color: var(--text-0);
    cursor: pointer;
    padding: 2px 0;
  }
  .cyan {
    color: var(--cyan);
  }
  .count {
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    color: var(--text-1);
  }
  .warn {
    color: var(--warn);
  }
  .mint {
    color: var(--mint);
  }
  .chev {
    color: var(--text-1);
  }
  .spacer {
    flex: 1;
  }
  .jobs {
    overflow: auto;
    min-height: 0;
  }
  .none {
    margin: 0;
    padding: 10px 12px;
    font-size: var(--fs-sm);
  }
  .job {
    padding: 6px 10px 4px;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
    font-size: var(--fs-sm);
  }
  .line {
    display: flex;
    align-items: center;
    gap: 10px;
    white-space: nowrap;
    min-width: 0;
  }
  .st {
    font-family: var(--font-display);
    font-size: 10px;
    letter-spacing: 0.12em;
    border: 1px solid currentColor;
    padding: 0 5px;
    color: var(--text-1);
    width: 74px;
    text-align: center;
    flex: none;
  }
  .running .st {
    color: var(--magenta);
    box-shadow: var(--glow-active);
  }
  .done .st {
    color: var(--mint);
  }
  .conflict .st,
  .job.conflict .st,
  .paused .st {
    color: var(--warn);
  }
  .failed .st {
    color: var(--danger);
  }
  .name {
    color: var(--text-0);
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 60px;
  }
  .route {
    font-size: var(--fs-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .nums {
    font-size: var(--fs-xs);
    color: var(--text-1);
    font-family: var(--font-mono);
  }
  .acts {
    display: flex;
    gap: 2px;
  }
  .act {
    width: 22px;
    height: 20px;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
    font-size: 10px;
  }
  .act:hover {
    color: var(--text-0);
    border-color: var(--text-1);
  }
  .bar {
    height: 3px;
    margin-top: 5px;
    background: color-mix(in srgb, var(--line) 60%, transparent);
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--magenta);
    transition: width 0.25s linear;
  }
  .done .bar span {
    background: var(--mint);
  }
  .failed .bar span {
    background: var(--danger);
  }
  .err {
    margin: 4px 0 0;
    font-size: var(--fs-xs);
  }
  .conflict {
    margin-top: 6px;
    padding: 8px;
    border: 1px solid color-mix(in srgb, var(--warn) 60%, var(--line));
  }
  .q {
    margin: 0 0 6px;
  }
  code {
    color: var(--cyan);
  }
  .compare {
    display: grid;
    grid-template-columns: 80px 100px 1fr;
    gap: 2px 10px;
    font-size: var(--fs-xs);
    font-family: var(--font-mono);
    margin-bottom: 8px;
  }
  .k {
    color: var(--text-1);
    font-family: var(--font-display);
    letter-spacing: 0.1em;
  }
  .choices {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .all {
    font-size: var(--fs-xs);
    color: var(--text-1);
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .tiny {
    padding: 2px 10px;
    font-size: var(--fs-xs);
  }
</style>
