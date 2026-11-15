<script lang="ts">
  /** Palette picker for the CUSTOM scheme: each slot picks from curated swatches (M10i). */
  import { settings } from "../stores/settings.svelte";
  import {
    COLOURS,
    resolveCustom,
    SURFACES,
    surfaceOf,
    TEXTS,
    textOf,
    THEMES,
    type ColourKey,
    type ThemeId,
    type ThemeTokens,
  } from "../theme/themes";

  const tokens = $derived(resolveCustom(settings.value.customTheme));
  const surface = $derived(surfaceOf(tokens));
  const text = $derived(textOf(tokens));
  const slots = Object.entries(COLOURS) as [ColourKey, (typeof COLOURS)[ColourKey]][];
  const STARTS: ThemeId[] = ["nightgrid", "ice", "bloodline"];

  function set(patch: Partial<ThemeTokens>) {
    settings.update({ theme: "custom", customTheme: { ...tokens, ...patch } });
  }
</script>

<div class="editor">
  <div class="row head">
    <span class="label">START FROM</span>
    <div class="starts">
      {#each STARTS as id (id)}
        <button type="button" class="btn ghost" onclick={() => set({ ...THEMES[id].tokens })}>{THEMES[id].label}</button>
      {/each}
      <button type="button" class="btn ghost" onclick={() => settings.update({ theme: "custom", customTheme: {} })} title="Back to the default custom palette">RESET</button>
    </div>
  </div>

  <div class="row">
    <span class="label">SURFACES<small>Backgrounds and lines</small></span>
    <div class="opts" role="radiogroup" aria-label="Surfaces">
      {#each SURFACES as s (s.label)}
        {@const on = surface === s.label}
        <button type="button" role="radio" aria-checked={on} class="chip" class:on title={s.label} onclick={() => set(s.values)}>
          <span class="stack" aria-hidden="true">
            {#each ["bg-0", "bg-1", "bg-2", "line"] as const as k (k)}<i style:background={s.values[k]}></i>{/each}
          </span>
          <span>{s.label}</span>
        </button>
      {/each}
    </div>
  </div>

  <div class="row">
    <span class="label">TEXT<small>Body and secondary</small></span>
    <div class="opts" role="radiogroup" aria-label="Text">
      {#each TEXTS as t (t.label)}
        {@const on = text === t.label}
        <button type="button" role="radio" aria-checked={on} class="chip" class:on title={t.label} onclick={() => set(t.values)}>
          <span class="aa" style:background={tokens["bg-1"]} aria-hidden="true">
            <b style:color={t.values["text-0"]}>A</b><b style:color={t.values["text-1"]}>a</b>
          </span>
          <span>{t.label}</span>
        </button>
      {/each}
    </div>
  </div>

  {#each slots as [key, slot] (key)}
    <div class="row">
      <span class="label">{slot.label.toUpperCase()}<small>{slot.hint}</small></span>
      <div class="opts" role="radiogroup" aria-label={slot.label}>
        {#each slot.values as c (c)}
          {@const on = tokens[key] === c}
          <button
            type="button"
            role="radio"
            aria-checked={on}
            aria-label={`${slot.label} ${c}`}
            title={c}
            class="dot"
            class:on
            style:background={c}
            onclick={() => set({ [key]: c })}
          ></button>
        {/each}
      </div>
    </div>
  {/each}
  <p class="note muted">Every choice is pre-checked for contrast, so any mix stays readable. Saved with your settings and included in profile exports.</p>
</div>

<style>
  .editor {
    margin-top: 14px;
    border-top: 1px solid var(--line);
    padding-top: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .row {
    display: grid;
    grid-template-columns: 170px 1fr;
    gap: 12px;
    align-items: center;
  }
  .label {
    font-family: var(--font-display);
    font-weight: 600;
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    display: flex;
    flex-direction: column;
  }
  .label small {
    font-family: var(--font-body, inherit);
    letter-spacing: 0;
    font-weight: 400;
    opacity: 0.8;
  }
  .starts,
  .opts {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px 4px 4px;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-0);
    font-size: var(--fs-xs);
    cursor: pointer;
  }
  .chip:hover,
  .dot:hover {
    border-color: var(--mint);
  }
  .chip.on {
    border-color: var(--magenta);
    background: color-mix(in srgb, var(--magenta) 12%, transparent);
  }
  .stack {
    display: flex;
    border: 1px solid var(--line);
  }
  .stack i {
    width: 7px;
    height: 18px;
  }
  .aa {
    display: inline-flex;
    padding: 1px 5px;
    border: 1px solid var(--line);
    font-size: var(--fs-sm, 13px);
  }
  .dot {
    width: 24px;
    height: 24px;
    border: 2px solid var(--line);
    cursor: pointer;
    padding: 0;
  }
  .dot.on {
    border-color: var(--text-0);
    box-shadow: 0 0 0 2px var(--magenta);
  }
  .note {
    margin: 4px 0 0;
    font-size: var(--fs-xs);
  }
  @media (max-width: 800px) {
    .row {
      grid-template-columns: 1fr;
    }
  }
</style>
