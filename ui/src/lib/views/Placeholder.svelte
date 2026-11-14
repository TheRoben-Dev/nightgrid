<script lang="ts">
  import Barcode from "../components/Barcode.svelte";
  import Panel from "../components/Panel.svelte";
  import type { Section } from "../sections";

  interface Props {
    section: Section;
  }
  let { section }: Props = $props();
</script>

<Panel title={section.label.toUpperCase()} reveal>
  <div class="offline">
    <span class="glyph" aria-hidden="true">◇</span>
    <p class="label">MODULE OFFLINE</p>
    <p class="muted">{section.label} arrives in {section.milestone}.</p>
    <Barcode seed={section.id} bars={30} tone="magenta" />
  </div>
</Panel>

<style>
  .offline {
    min-height: 280px;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 8px;
    text-align: center;
  }
  .glyph {
    font-size: 28px;
    color: var(--magenta);
  }
  p {
    margin: 0;
  }
  .label {
    color: var(--text-0);
    font-size: var(--fs-lg);
  }
</style>
