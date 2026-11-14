<script lang="ts">
  import { updates } from "../../stores/updates.svelte";
  import Modal from "../Modal.svelte";

  const u = $derived(updates.prompt);
  const pct = $derived(updates.progress == null ? null : Math.round(updates.progress * 100));
  const date = $derived(u?.date ? new Date(u.date).toLocaleDateString() : null);
</script>

<Modal
  open={!!u}
  title={u ? `UPDATE AVAILABLE :: v${u.version}` : "UPDATE"}
  width={620}
  onclose={() => !updates.installing && updates.later()}
>
  {#if u}
    <p class="lead">
      Nightgrid <strong class="data">v{u.version}</strong> is out{date ? ` (${date})` : ""}. You're on v{u.current}.
    </p>
    {#if u.notes}
      <!-- Release notes are shown as plain text, never rendered as HTML. -->
      <pre class="notes">{u.notes}</pre>
    {/if}
    {#if updates.phase !== "idle"}
      <div class="progress" role="status">
        <span class="label">{updates.phase === "installing" ? "INSTALLING · RESTARTING" : "DOWNLOADING"}</span>
        <span class="bar" aria-hidden="true"><span style:width="{pct ?? 30}%" class:indeterminate={pct == null}></span></span>
        <span class="data">{pct == null ? "…" : `${pct}%`}</span>
      </div>
    {/if}
    {#if updates.error}<p class="error-text" role="alert">{updates.error}</p>{/if}
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" disabled={updates.installing} onclick={() => updates.skip()}>Skip this version</button>
    <button type="button" class="btn" disabled={updates.installing} onclick={() => updates.later()}>Later</button>
    <button type="button" class="btn primary chamfer-sm" disabled={updates.installing} onclick={() => void updates.install()}>
      {updates.installing ? "Updating…" : "Install & restart"}
    </button>
  {/snippet}
</Modal>

<style>
  .lead {
    margin: 0 0 12px;
    line-height: 1.6;
  }
  .data {
    color: var(--cyan);
  }
  .notes {
    margin: 0;
    max-height: 280px;
    overflow: auto;
    padding: 10px 12px;
    border: 1px solid var(--line);
    background: var(--bg-0);
    color: var(--text-1);
    font-size: var(--fs-sm);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .progress {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 10px;
    align-items: center;
    margin-top: 12px;
  }
  .bar {
    height: 6px;
    border: 1px solid var(--line);
    position: relative;
    overflow: hidden;
  }
  .bar span {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--magenta);
    box-shadow: var(--glow-active);
    transition: width var(--dur-fast) linear;
  }
  .bar span.indeterminate {
    animation: slide 1.1s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      left: -30%;
    }
    to {
      left: 100%;
    }
  }
  .error-text {
    margin: 10px 0 0;
  }
</style>
