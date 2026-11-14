<script lang="ts">
  import { barcode } from "../hex";

  interface Props {
    seed: string;
    bars?: number;
    tone?: "magenta" | "mint" | "muted";
  }
  let { seed, bars = 40, tone = "muted" }: Props = $props();
  const widths = $derived(barcode(seed, bars));
</script>

<!-- Decorative only. -->
<span class="barcode {tone}" aria-hidden="true">
  {#each widths as w, i (i)}<i class:wide={w > 2} class:gap={w === 1}></i>{/each}
</span>

<style>
  .barcode {
    display: inline-flex;
    align-items: stretch;
    gap: 1px;
    height: 10px;
    opacity: 0.8;
  }
  i {
    display: block;
    width: 2px;
    background: currentColor;
  }
  i.wide {
    width: 4px;
  }
  i.gap {
    background: transparent;
  }
  .magenta {
    color: var(--magenta);
  }
  .mint {
    color: var(--mint);
  }
  .muted {
    color: var(--line);
  }
</style>
