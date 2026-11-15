<script lang="ts">
  import { onMount } from "svelte";
  import { DEFAULT_KEYMAP, formatChord } from "../../keys";
  import { helper } from "../../stores/helper.svelte";
  import Panel from "../Panel.svelte";
  import Toggle from "../Toggle.svelte";
  import ModelCard from "./ModelCard.svelte";

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
    <ModelCard />
    {#if helper.status?.loaded}
      <button type="button" class="btn ghost" disabled={helper.busy} onclick={() => void helper.unload()}>
        Free memory (unload model)
      </button>
    {/if}
  {/if}
</Panel>
