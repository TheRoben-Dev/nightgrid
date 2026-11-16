<script lang="ts">
  import { onMount } from "svelte";
  import { DEFAULT_KEYMAP, formatChord } from "../../keys";
  import { helper } from "../../stores/helper.svelte";
  import Panel from "../Panel.svelte";
  import Toggle from "../Toggle.svelte";
  import ModelPicker from "./ModelPicker.svelte";
  import { TOURS } from "../../helper/tours";
  import { tour } from "../../stores/tour.svelte";

  onMount(() => void helper.refresh());
</script>

<Panel title="HELPER" guide="settings-helper">
  <Toggle
    label="DRIFT helper"
    hint={`A local assistant (${formatChord(DEFAULT_KEYMAP["helper.toggle"])}): answers questions about Nightgrid, explains selected logs and errors, and points at controls. It runs on this computer; nothing you ask is sent anywhere.`}
    checked={helper.enabled}
    onchange={(v) => void helper.setEnabled(v)}
  />
  {#if helper.enabled}
    <ModelPicker />
    {#if helper.status?.loaded}
      <button type="button" class="btn ghost" disabled={helper.busy} onclick={() => void helper.unload()}>
        Free memory (unload model)
      </button>
    {/if}
  {/if}
  <div class="tours" data-guide="settings-tours">
    <p class="micro">GUIDED TOURS</p>
    <p class="muted small">DRIFT points at one control at a time and waits for you. They work without the model. Esc ends a tour.</p>
    <ul>
      {#each TOURS as t (t.id)}
        <li>
          <button type="button" class="btn ghost tiny" onclick={() => tour.start(t.id)}>▶ {t.title}</button>
          <span class="muted small">{t.what}</span>
        </li>
      {/each}
    </ul>
  </div>
</Panel>

<style>
  .tours {
    margin-top: 10px;
  }
  .tours p {
    margin: 0 0 4px;
  }
  .tours ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .tours li {
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .small {
    font-size: var(--fs-xs);
  }
</style>
