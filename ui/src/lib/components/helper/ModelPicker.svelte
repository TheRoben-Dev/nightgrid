<script lang="ts">
  /**
   * DRIFT's model choice (Settings → DRIFT): three sizes, how each fits this
   * computer, download on demand, switch and delete. The model in use keeps
   * answering while another downloads.
   */
  import { onMount } from "svelte";
  import type { ModelOption } from "../../api";
  import { helper } from "../../stores/helper.svelte";

  onMount(() => void helper.loadModels());

  const gb = (n: number | null | undefined) => `${((n ?? 0) / 1e9).toFixed(1)} GB`;
  const m = $derived(helper.models);
  const hw = $derived(m?.hardware);
  let confirmDelete = $state<string | null>(null);

  const BADGE: Record<string, string> = { good: "FITS", tight: "TIGHT", no: "WON'T FIT", unknown: "UNKNOWN" };

  const pct = $derived(helper.progress && helper.progress.total ? Math.floor((helper.progress.done / helper.progress.total) * 100) : 0);

  function memLine(): string {
    if (!hw) return "";
    const mem = hw.memory ? `${Math.round(hw.memory / 1024 ** 3)} GB ${hw.unified ? "unified memory" : "RAM"}` : "memory unknown";
    const bits = [hw.cpu || hw.arch, hw.cores ? `${hw.cores} cores` : "", mem].filter(Boolean);
    return bits.join(" · ");
  }

  function gpuLine(): string {
    if (!hw) return "";
    if (hw.gpuBuild) return hw.unified ? "Runs on the GPU (Metal), sharing memory with everything else." : "Runs on the GPU.";
    const g = hw.gpus.map((x) => (x.vram ? `${x.name} (${Math.round(x.vram / 1024 ** 3)} GB)` : x.name)).join(", ");
    return `Runs on the CPU in this build${g ? `; your GPU (${g}) isn't used yet` : ""}, so memory is what counts and bigger models answer slower.`;
  }

  const inUse = (o: ModelOption) => m?.active === o.id;
</script>

{#if m && hw}
  <div class="picker" data-guide="settings-models">
    <p class="micro">MODEL</p>
    <p class="hw"><span class="data">{memLine()}</span></p>
    <p class="muted small">{gpuLine()}</p>
    <div class="list" role="list">
      {#each m.models as o (o.id)}
        {@const rec = m.recommended === o.id}
        {@const dl = helper.downloadingId === o.id}
        <div class="opt chamfer-sm" class:active={inUse(o)} class:nofit={o.fit === "no"} role="listitem">
          <div class="row">
            <span class="tier">{o.tier.toUpperCase()}</span>
            <span class="name">{o.name}</span>
            <span class="badges">
              {#if rec}<span class="badge rec">RECOMMENDED</span>{/if}
              <span class="badge {o.fit}">{BADGE[o.fit] ?? o.fit}</span>
            </span>
          </div>
          <p class="muted small">{o.params} · {gb(o.bytes)} download · {o.blurb}</p>
          <p class="small reason" class:warn={o.fit === "no" || o.fit === "tight"}>{o.reason[0]?.toUpperCase()}{o.reason.slice(1)}.</p>
          {#if dl && helper.progress}
            <div class="row">
              <span class="data">{pct}% · {gb(helper.progress.done)} of {gb(helper.progress.total)}</span>
              <button type="button" class="btn ghost tiny" onclick={() => helper.cancelDownload()}>Pause</button>
            </div>
          {:else}
            <div class="row actions">
              {#if inUse(o) && o.state === "ready"}
                <span class="ok">IN USE{helper.status?.loaded ? " · LOADED" : ""}</span>
              {:else if o.state === "ready"}
                <button type="button" class="btn tiny" disabled={helper.busy} onclick={() => void helper.useModel(o.id)}>Use this model</button>
              {:else}
                <button
                  type="button"
                  class="btn tiny"
                  class:primary={rec}
                  disabled={!!helper.downloadingId}
                  title={o.fit === "no" ? "This computer probably doesn't have the memory for it" : undefined}
                  onclick={() => void helper.download(o.id, true)}
                >
                  {o.state === "partial" ? `Resume (${gb(o.have)} of ${gb(o.bytes)})` : o.fit === "no" ? "Download anyway" : inUse(o) ? `Download (${gb(o.bytes)})` : `Download and use (${gb(o.bytes)})`}
                </button>
              {/if}
              {#if o.state !== "missing"}
                {#if confirmDelete === o.id}
                  <button type="button" class="btn ghost tiny" onclick={() => (confirmDelete = null)}>Keep</button>
                  <button type="button" class="btn danger tiny" onclick={() => ((confirmDelete = null), void helper.deleteModel(o.id))}>Delete {gb(o.state === "ready" ? o.bytes : o.have)}</button>
                {:else}
                  <button type="button" class="btn ghost tiny" disabled={dl} onclick={() => (confirmDelete = o.id)}>Delete…</button>
                {/if}
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
    <p class="muted small">
      Each model is downloaded once from Hugging Face (a fixed file, checked against its SHA-256), then runs on this computer. The model in use keeps answering while
      another downloads; DRIFT switches when it's ready. Delete the ones you don't use to get the space back.
    </p>
    {#if helper.downloadError}<p class="error" role="alert">{helper.downloadError}</p>{/if}
  </div>
{:else}
  <p class="muted small">Reading this computer's memory…</p>
{/if}

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  p {
    margin: 0;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .opt {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 10px;
    border: 1px solid var(--line);
  }
  .opt.active {
    border-color: var(--cyan);
  }
  .opt.nofit .name {
    color: var(--text-2);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .actions {
    justify-content: flex-end;
  }
  .tier {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--magenta);
    min-width: 72px;
  }
  .name {
    color: var(--text-0);
  }
  .badges {
    margin-left: auto;
    display: flex;
    gap: 4px;
  }
  .badge {
    font-size: var(--fs-xs);
    letter-spacing: 0.12em;
    padding: 0 6px;
    border: 1px solid var(--line);
    color: var(--text-1);
  }
  .badge.good {
    color: var(--mint);
    border-color: var(--mint);
  }
  .badge.tight {
    color: var(--warn);
    border-color: var(--warn);
  }
  .badge.no {
    color: var(--danger);
    border-color: var(--danger);
  }
  .badge.rec {
    color: var(--cyan);
    border-color: var(--cyan);
  }
  .reason.warn {
    color: var(--text-1);
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
  .error {
    color: var(--danger);
  }
</style>
