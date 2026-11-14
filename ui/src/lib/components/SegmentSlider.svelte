<script lang="ts">
  /**
   * Segmented tick slider with an N/M readout. Keyboard: ←/→ (or ↓/↑),
   * Home/End. Each segment is a ≥28px click target.
   */
  interface Props {
    label: string;
    steps: readonly number[];
    value: number;
    format?: (v: number) => string;
    onchange: (v: number) => void;
  }
  let { label, steps, value, format = (v) => String(v), onchange }: Props = $props();

  const index = $derived(Math.max(0, steps.indexOf(value)));

  function set(i: number) {
    const clamped = Math.min(steps.length - 1, Math.max(0, i));
    if (steps[clamped] !== value) onchange(steps[clamped]);
  }

  function onkeydown(e: KeyboardEvent) {
    const map: Record<string, number> = {
      ArrowRight: index + 1,
      ArrowUp: index + 1,
      ArrowLeft: index - 1,
      ArrowDown: index - 1,
      Home: 0,
      End: steps.length - 1,
    };
    if (e.key in map) {
      e.preventDefault();
      set(map[e.key]);
    }
  }
</script>

<div class="row">
  <span class="label" id="seg-{label}">{label}</span>
  <div
    class="track"
    role="slider"
    tabindex="0"
    aria-labelledby="seg-{label}"
    aria-valuemin={steps[0]}
    aria-valuemax={steps[steps.length - 1]}
    aria-valuenow={value}
    aria-valuetext={format(value)}
    {onkeydown}
  >
    {#each steps as s, i (s)}
      <button
        type="button"
        tabindex="-1"
        class="seg"
        class:filled={i < index}
        class:current={i === index}
        aria-label={format(s)}
        onclick={() => set(i)}
      ></button>
    {/each}
  </div>
  <span class="readout">{format(value)} <span class="muted">{index + 1}/{steps.length}</span></span>
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 140px 1fr auto;
    align-items: center;
    gap: 12px;
  }
  .track {
    display: flex;
    gap: 3px;
    padding: 2px 0;
  }
  .seg {
    flex: 1;
    min-height: var(--hit);
    padding: 0;
    border: 0;
    background: transparent;
    cursor: pointer;
    position: relative;
  }
  .seg::after {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    height: 3px;
    transform: translateY(-50%);
    background: var(--line);
    transition: background var(--dur-fast) var(--ease);
  }
  .seg.filled::after {
    background: color-mix(in srgb, var(--mint) 55%, var(--line));
  }
  .seg.current::after {
    height: 7px;
    background: var(--magenta);
    box-shadow: var(--glow-active);
  }
  .seg:hover::after {
    background: var(--text-1);
  }
  .readout {
    min-width: 92px;
    text-align: right;
    color: var(--text-0);
  }
</style>
