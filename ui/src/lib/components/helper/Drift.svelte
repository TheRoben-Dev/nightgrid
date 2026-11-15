<script lang="ts">
  /**
   * DRIFT, drawn from docs/mascot/drift-sprites.json: one rect per pixel run,
   * coloured by theme tokens through classes (no inline styles, CSP-safe).
   * Integer scales only; motion is off under reduced effects.
   */
  import sprites from "../../../../../docs/mascot/drift-sprites.json";
  import type { DriftState } from "../../stores/helper.svelte";

  interface Props {
    mood: DriftState;
    /** Integer pixel scale (2× inline, 3× in the panel). */
    scale?: number;
    /** 16×16 icon instead of the panel sprite. */
    icon?: boolean;
  }
  let { mood, scale = 3, icon = false }: Props = $props();

  type Frames = Record<string, string[]>;
  const frames = sprites.frames as Frames;
  const icons = sprites.icon as Frames;
  const glow = new Set(sprites.glow_keys);

  let blink = $state(false);
  $effect(() => {
    if (mood !== "idle" || icon) return;
    let t: ReturnType<typeof setTimeout>;
    const next = () => {
      t = setTimeout(
        () => {
          blink = true;
          t = setTimeout(() => {
            blink = false;
            next();
          }, 120);
        },
        4000 + Math.random() * 3000,
      );
    };
    next();
    return () => {
      clearTimeout(t);
      blink = false;
    };
  });

  const rows = $derived.by(() => {
    if (icon) return icons[mood === "think" || mood === "talk" ? "think" : mood === "error" ? "error" : "idle"];
    return frames[blink && mood === "idle" ? "blink" : mood] ?? frames.idle;
  });
  const w = $derived(rows[0].length);
  const h = $derived(rows.length);

  /** Horizontal runs of one colour key, so a frame is ~100 rects, not ~400. */
  const runs = $derived.by(() => {
    const out: { x: number; y: number; n: number; k: string }[] = [];
    rows.forEach((row, y) => {
      let x = 0;
      while (x < row.length) {
        const k = row[x];
        let n = 1;
        while (x + n < row.length && row[x + n] === k) n++;
        if (k !== ".") out.push({ x, y, n, k });
        x += n;
      }
    });
    return out;
  });
  const cls = (k: string) => `k-${k === k.toUpperCase() ? k : `${k}l`}${glow.has(k) && !icon && scale >= 2 ? " glow" : ""}`;
</script>

<svg
  class="drift s-{mood}"
  class:icon
  viewBox="0 0 {w} {h}"
  width={w * scale}
  height={h * scale}
  shape-rendering="crispEdges"
  role="img"
  aria-label="DRIFT ({mood})"
>
  <g class="body">
    {#each runs as r (`${r.x},${r.y}`)}<rect x={r.x} y={r.y} width={r.n} height="1" class={cls(r.k)} />{/each}
  </g>
</svg>

<style>
  .drift {
    display: block;
    image-rendering: pixelated;
    overflow: visible;
  }
  .k-M {
    fill: var(--mint);
  }
  .k-ml {
    fill: color-mix(in srgb, var(--mint) 35%, var(--bg-2));
  }
  .k-D {
    fill: var(--bg-2);
  }
  .k-B {
    fill: var(--bg-0);
  }
  .k-L {
    fill: var(--line);
  }
  .k-C {
    fill: var(--cyan);
  }
  .k-cl {
    fill: color-mix(in srgb, var(--cyan) 35%, var(--bg-0));
  }
  .k-W {
    fill: var(--text-0);
  }
  .k-K {
    fill: var(--magenta);
  }
  .k-kl {
    fill: color-mix(in srgb, var(--magenta) 45%, var(--bg-0));
  }
  .k-Y {
    fill: var(--accent);
  }
  .k-G {
    fill: var(--ok);
  }
  .k-R {
    fill: var(--danger);
  }
  .k-O {
    fill: var(--warn, var(--accent));
  }
  .glow {
    filter: drop-shadow(0 0 1px currentColor);
  }
  /* Hover bob (2 px over 2.4 s) on a step clock; thinking pulses the LED. */
  .drift:not(.icon) .body {
    animation: bob 2.4s steps(2, jump-none) infinite;
  }
  .s-think .k-Y {
    animation: pulse 0.48s steps(2, jump-none) infinite;
  }
  .s-sleep .body,
  .s-error .body {
    animation: none;
  }
  @keyframes bob {
    50% {
      transform: translateY(1px);
    }
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  :global(:root[data-effects="off"]) .drift .body,
  :global(:root[data-effects="off"]) .drift .k-Y {
    animation: none;
  }
  :global(:root[data-effects="off"]) .glow {
    filter: none;
  }
  @media (prefers-reduced-motion: reduce) {
    .drift .body,
    .drift .k-Y {
      animation: none;
    }
  }
</style>
