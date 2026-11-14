<script lang="ts">
  /** Tiny line + area chart. Draws left-to-right once when it first gets data. */
  interface Props {
    values: number[];
    /** Fixed top of the scale (e.g. 100 for %); auto when omitted. */
    max?: number;
    /** Most points the width represents; shorter series fill at least a fifth of it and stretch until full. */
    capacity?: number;
    color?: string;
    height?: number;
    label?: string;
  }
  let { values, max, capacity = 150, color = "var(--cyan)", height = 28, label }: Props = $props();

  const W = 100;
  const top = $derived(max ?? Math.max(1, ...values) * 1.15);
  const pts = $derived.by(() => {
    const n = values.length;
    if (n < 2) return "";
    const span = Math.min(capacity, Math.max(n, Math.ceil(capacity / 5)));
    const step = W / Math.max(span - 1, 1);
    const x0 = W - (n - 1) * step;
    return values
      .map((v, i) => `${(x0 + i * step).toFixed(2)},${(height - (Math.min(v, top) / top) * (height - 2) - 1).toFixed(2)}`)
      .join(" ");
  });
  const area = $derived(pts ? `${pts.split(" ")[0].split(",")[0]},${height} ${pts} ${W},${height}` : "");
  let drawn = $state(false);
  $effect(() => {
    if (pts && !drawn) queueMicrotask(() => (drawn = true));
  });
  const gid = `sg${Math.random().toString(36).slice(2, 8)}`;
</script>

<svg
  class="spark"
  class:drawn
  viewBox="0 0 {W} {height}"
  preserveAspectRatio="none"
  style:height="{height}px"
  style:--c={color}
  role="img"
  aria-label={label ?? "trend"}
>
  <defs>
    <linearGradient id={gid} x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color={color} stop-opacity="0.28" />
      <stop offset="1" stop-color={color} stop-opacity="0" />
    </linearGradient>
  </defs>
  {#if pts}
    <polygon points={area} fill="url(#{gid})" />
    <polyline points={pts} fill="none" stroke={color} stroke-width="1.2" vector-effect="non-scaling-stroke" />
  {/if}
</svg>

<style>
  .spark {
    display: block;
    width: 100%;
    overflow: visible;
  }
  /* Reveal with a clip, not stroke dashes: WebKit breaks dashes on
     non-scaling strokes and leaves gaps in the line. */
  .spark {
    clip-path: inset(-4px 100% -4px 0);
  }
  .spark.drawn {
    clip-path: inset(-4px 0 -4px 0);
    transition: clip-path var(--dur-slow) ease-out;
  }
  polyline {
    filter: drop-shadow(0 0 2px color-mix(in srgb, var(--c) 60%, transparent));
  }
  :global(:root[data-effects="off"]) polyline {
    filter: none;
  }
</style>
