<script lang="ts">
  /** Model status with download (resumable), cancel and delete. */
  import { helper } from "../../stores/helper.svelte";

  const gb = (n: number) => `${(n / 1e9).toFixed(2)} GB`;
  const s = $derived(helper.status);
  const pct = $derived(helper.progress && helper.progress.total ? Math.floor((helper.progress.done / helper.progress.total) * 100) : 0);
  let confirmDelete = $state(false);
</script>

{#if s}
  <div class="card">
    <div class="row">
      <span class="name">{s.model}</span>
      <span class="muted small">{s.license} · {gb(s.bytes ?? 0)} · {s.gpu ? "GPU (Metal)" : "CPU"}</span>
    </div>
    {#if helper.progress && (!helper.downloadingId || helper.downloadingId === s.id)}
      <div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={pct}>
        <span class="fill">{"█".repeat(Math.round(pct / 4))}</span><span class="rest">{"░".repeat(25 - Math.round(pct / 4))}</span>
      </div>
      <div class="row">
        <span class="data">{pct}% · {gb(helper.progress.done)} of {gb(helper.progress.total)}</span>
        <button type="button" class="btn ghost" onclick={() => helper.cancelDownload()}>Pause</button>
      </div>
    {:else if s.state === "ready"}
      <div class="row">
        <span class="ok">READY{s.loaded ? " · LOADED" : ""}</span>
        {#if confirmDelete}
          <span class="actions">
            <button type="button" class="btn ghost" onclick={() => (confirmDelete = false)}>Keep</button>
            <button type="button" class="btn danger" onclick={() => ((confirmDelete = false), void helper.deleteModel())}>Delete {gb(s.bytes ?? 0)}</button>
          </span>
        {:else}
          <button type="button" class="btn ghost" onclick={() => (confirmDelete = true)}>Delete model…</button>
        {/if}
      </div>
    {:else}
      <p class="muted small">
        Downloaded once from Hugging Face (a fixed file, checked against its SHA-256), then everything runs on this computer.
        {#if s.state === "partial"}{gb(s.have ?? 0)} already downloaded.{/if}
      </p>
      <button type="button" class="btn primary" onclick={() => void helper.download()}>
        {s.state === "partial" ? "Resume download" : `Download (${gb(s.bytes ?? 0)})`}
      </button>
    {/if}
    {#if helper.downloadError}<p class="error" role="alert">{helper.downloadError}</p>{/if}
  </div>
{/if}

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px;
    border: 1px solid var(--line);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .name {
    color: var(--text-0);
  }
  .small {
    font-size: var(--fs-xs);
    margin: 0;
  }
  .bar {
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    letter-spacing: -1px;
    white-space: nowrap;
    overflow: hidden;
  }
  .fill {
    color: var(--cyan);
  }
  .rest {
    color: var(--line);
  }
  .data {
    color: var(--cyan);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
  }
  .ok {
    color: var(--mint);
    letter-spacing: 0.14em;
    font-size: var(--fs-xs);
  }
  .actions {
    display: flex;
    gap: 6px;
  }
  .error {
    color: var(--danger);
    margin: 0;
  }
</style>
