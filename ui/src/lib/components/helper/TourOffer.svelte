<script lang="ts">
  /**
   * The one-time first-run offer (M10q): a small card asking whether DRIFT
   * should show the user around. Either answer is remembered; the tours stay
   * in Settings → DRIFT and in DRIFT's "What can I do here?".
   */
  import { settings } from "../../stores/settings.svelte";
  import { tour } from "../../stores/tour.svelte";
  import Drift from "./Drift.svelte";

  const show = $derived(settings.loaded && !settings.value.tourOffered && !tour.tour);

  const start = () => tour.start("first-run");
  const later = () => void settings.update({ tourOffered: true });
</script>

{#if show}
  <div class="offer" role="dialog" aria-label="Tour offer">
    <Drift mood="success" scale={2} />
    <div class="body">
      <p>New here? I can show you around in a minute. I point at things and wait for you; I never click anything.</p>
      <div class="btns">
        <button type="button" class="btn ghost tiny" onclick={later}>No thanks</button>
        <button type="button" class="btn primary tiny" onclick={start}>Show me around</button>
      </div>
      <p class="muted small">The tours stay in Settings → DRIFT.</p>
    </div>
  </div>
{/if}

<style>
  .offer {
    position: fixed;
    right: 16px;
    bottom: 16px;
    z-index: 900;
    width: 340px;
    max-width: calc(100vw - 32px);
    display: flex;
    gap: 10px;
    padding: 10px 12px;
    background: var(--bg-1);
    border: 1px solid var(--magenta);
    box-shadow: 0 8px 28px rgb(0 0 0 / 0.45);
    font-size: var(--fs-sm);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .body p {
    margin: 0;
    line-height: 1.45;
  }
  .btns {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
  }
  .small {
    font-size: var(--fs-xs);
  }
</style>
