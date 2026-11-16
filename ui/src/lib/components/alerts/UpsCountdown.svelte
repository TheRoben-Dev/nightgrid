<script lang="ts">
  /**
   * UPS shutdown countdown (M10s): shown above every view while a UPS on
   * battery is about to shut its group down. The countdown itself runs in
   * the backend (it works with the window closed); this only shows it and
   * can cancel it.
   */
  import { api, errorText } from "../../api";
  import { alerts } from "../../stores/alerts.svelte";
  import { fleet } from "../../stores/fleet.svelte";
  import { ui } from "../../stores/ui.svelte";
  import { clock, remaining } from "../../ups";

  const c = $derived(alerts.snap?.upsCountdown ?? null);
  let now = $state(Date.now());
  let error = $state<string | null>(null);
  let busy = $state(false);
  $effect(() => {
    if (!c) return;
    now = Date.now();
    const t = setInterval(() => (now = Date.now()), 500);
    return () => clearInterval(t);
  });
  const left = $derived(c ? remaining(c, now) : 0);

  async function cancel() {
    busy = true;
    error = null;
    try {
      await api.upsCancel();
    } catch (e) {
      error = errorText(e);
    }
    busy = false;
  }
</script>

{#if c}
  <div class="ups-cd" role="alert" aria-live="assertive">
    <span class="tag">UPS · {c.label}</span>
    <span class="clock" aria-label="{left} seconds left">{left > 0 ? clock(left) : "NOW"}</span>
    <span class="what">
      {c.reason}. {left > 0 ? "Shutting down" : "Shutting down now:"} <strong>{c.targets.join(", ")}</strong>{left > 0 ? " unless you cancel." : ""}
      {#if error}<span class="err">{error}</span>{/if}
    </span>
    <button
      type="button"
      class="btn ghost tiny"
      onclick={() => {
        fleet.tab = "ups";
        ui.go("fleet");
      }}>Details</button
    >
    <button type="button" class="btn danger tiny" data-guide="ups-cancel" disabled={busy || left <= 0} onclick={cancel}>Cancel shutdown</button>
  </div>
{/if}

<style>
  .ups-cd {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    padding: 6px 12px;
    border: 1px solid var(--danger);
    background: color-mix(in srgb, var(--danger) 12%, var(--bg-0));
    font-size: var(--fs-xs);
  }
  .tag {
    color: var(--danger);
    letter-spacing: var(--tracking-label);
    font-family: var(--font-display);
  }
  .clock {
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: var(--fs-lg);
    font-variant-numeric: tabular-nums;
    min-width: 4ch;
  }
  .what {
    flex: 1;
    min-width: 200px;
    color: var(--text-0);
  }
  .err {
    color: var(--danger);
    margin-left: 8px;
  }
</style>
