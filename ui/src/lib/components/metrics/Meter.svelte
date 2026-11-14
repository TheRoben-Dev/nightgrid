<script lang="ts">
  /** Thin horizontal gauge; turns yellow at `warn` and red at `crit`. */
  interface Props {
    value: number | null;
    max?: number;
    warn?: number;
    crit?: number;
    label?: string;
  }
  let { value, max = 100, warn = 75, crit = 90, label }: Props = $props();
  const pct = $derived(value == null ? 0 : Math.max(0, Math.min(100, (value / max) * 100)));
  const tone = $derived(value == null ? "none" : pct >= crit ? "crit" : pct >= warn ? "warn" : "ok");
</script>

<span
  class="meter {tone}"
  role="meter"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={Math.round(pct)}
>
  <span class="fill" style:width="{pct}%"></span>
</span>

<style>
  .meter {
    position: relative;
    display: block;
    height: 4px;
    background: color-mix(in srgb, var(--line) 70%, transparent);
    overflow: hidden;
  }
  .fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--cyan);
    box-shadow: 0 0 6px color-mix(in srgb, var(--cyan) 60%, transparent);
    transition: width var(--dur) ease-out;
  }
  .warn .fill {
    background: var(--accent);
    box-shadow: 0 0 6px color-mix(in srgb, var(--accent) 60%, transparent);
  }
  .crit .fill {
    background: var(--danger);
    box-shadow: 0 0 6px color-mix(in srgb, var(--danger) 60%, transparent);
  }
  :global(:root[data-effects="off"]) .fill {
    box-shadow: none;
  }
</style>
