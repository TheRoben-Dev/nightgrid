<script lang="ts">
  /** A blueprint plan as a list of steps to tick (Fleet → BLUEPRINTS, file or starter). */
  import type { BpStep, BpStepResult } from "../../api";
  import { groups, pickable, STATE_WORD, toggleStep } from "../../blueprint";

  let { plan, picked = $bindable(), results, busy }: { plan: BpStep[]; picked: Set<string>; results: BpStepResult[] | null; busy: boolean } = $props();

  const resultOf = (id: string) => results?.find((r) => r.id === id);
</script>

<div class="plan" role="list">
  {#each groups(plan) as g (g.label)}
    <p class="glabel label">{g.label}</p>
    {#each g.steps as s (s.id)}
      {@const r = resultOf(s.id)}
      <label class="step st-{s.state}" class:off={!pickable(s)} role="listitem">
        <input type="checkbox" checked={picked.has(s.id)} disabled={!pickable(s) || busy} onchange={() => (picked = toggleStep(picked, plan, s.id))} />
        <span class="tag">{STATE_WORD[s.state]}</span>
        <span class="body">
          <span class="title">{s.title}</span>
          <span class="detail">{s.detail}</span>
          {#if r}<span class="res" class:bad={!r.ok} class:ok={r.ok}>{r.ok ? "✓" : "✗"} {r.message}</span>{/if}
        </span>
      </label>
    {/each}
  {:else}
    <p class="muted small">The blueprint is empty.</p>
  {/each}
</div>

<style>
  .label {
    margin: 0;
    color: var(--cyan);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
  }
  .small {
    font-size: var(--fs-xs);
    margin: 0;
  }
  .muted {
    color: var(--text-1);
  }
  .plan {
    border-top: 1px solid var(--line);
    display: flex;
    flex-direction: column;
  }
  .glabel {
    padding: 8px 0 2px;
  }
  .step {
    display: grid;
    grid-template-columns: 18px 64px 1fr;
    gap: 8px;
    align-items: start;
    padding: 4px 0;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
    cursor: pointer;
  }
  .step.off {
    cursor: default;
    opacity: 0.75;
  }
  .tag {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    padding-top: 2px;
  }
  .st-new .tag {
    color: var(--cyan);
  }
  .st-differs .tag {
    color: var(--warn);
  }
  .st-manual .tag {
    color: var(--magenta);
  }
  .body {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .title {
    font-size: var(--fs-sm);
    color: var(--text-0);
    overflow-wrap: anywhere;
  }
  .detail,
  .res {
    font-size: var(--fs-xs);
    color: var(--text-1);
    overflow-wrap: anywhere;
  }
  .res.ok {
    color: var(--ok, #4ade80);
  }
  .res.bad {
    color: var(--danger);
  }
</style>
