<script lang="ts">
  /**
   * Area chart for stored history: one or two series over time, with gaps for
   * missing points. With `onzoom`, dragging across the chart picks a time
   * span to zoom into (M10t); double-click asks to zoom back out.
   */
  interface Series {
    label: string;
    color: string;
    values: (number | null)[];
  }
  interface Props {
    ts: number[];
    series: Series[];
    /** Fixed top (e.g. 100 for %), auto otherwise. */
    max?: number;
    format: (v: number) => string;
    from: number;
    to: number;
    /** Called with the dragged span (unix seconds); `null` on double-click. */
    onzoom?: (span: { from: number; to: number } | null) => void;
  }
  let { ts, series, max, format, from, to, onzoom }: Props = $props();

  /** Shortest span a drag can zoom into. */
  const MIN_ZOOM = 10 * 60;

  const W = 600;
  const H = 160;
  const top = $derived(max ?? Math.max(1e-9, ...series.flatMap((s) => s.values.filter((v): v is number => v != null))) * 1.1);
  const x = (t: number) => ((t - from) / Math.max(to - from, 1)) * W;
  const y = (v: number) => H - (Math.min(v, top) / top) * (H - 4) - 2;
  /** Breaks the line where points are missing or more than 3 steps apart. */
  const segments = (vals: (number | null)[]) => {
    const step = ts.length > 1 ? (ts[ts.length - 1] - ts[0]) / (ts.length - 1) : 60;
    const out: string[][] = [];
    let cur: string[] = [];
    vals.forEach((v, i) => {
      const gap = i > 0 && ts[i] - ts[i - 1] > step * 3;
      if (v == null || gap) {
        if (cur.length) out.push(cur);
        cur = [];
      }
      if (v != null) cur.push(`${x(ts[i]).toFixed(1)},${y(v).toFixed(1)}`);
    });
    if (cur.length) out.push(cur);
    return out;
  };
  const fmtTime = (t: number) => {
    const d = new Date(t * 1000);
    return to - from > 2 * 86400
      ? d.toLocaleDateString(undefined, { month: "short", day: "numeric" })
      : d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  };
  const ticks = $derived([0, 0.25, 0.5, 0.75, 1].map((f) => from + (to - from) * f));

  let hover = $state<number | null>(null);
  /** Drag start and current position, as times. */
  let drag = $state<{ a: number; b: number } | null>(null);
  const timeAt = (e: PointerEvent) => {
    const r = (e.currentTarget as SVGElement).getBoundingClientRect();
    return from + (Math.min(Math.max(e.clientX - r.left, 0), r.width) / r.width) * (to - from);
  };
  function onDown(e: PointerEvent) {
    if (!onzoom || e.button !== 0) return;
    (e.currentTarget as SVGElement).setPointerCapture(e.pointerId);
    const t = timeAt(e);
    drag = { a: t, b: t };
  }
  function onUp() {
    if (!drag) return;
    const lo = Math.min(drag.a, drag.b);
    const hi = Math.max(drag.a, drag.b);
    drag = null;
    if (hi - lo >= MIN_ZOOM) onzoom?.({ from: Math.floor(lo), to: Math.ceil(hi) });
  }
  function onMove(e: PointerEvent) {
    const t = timeAt(e);
    if (drag) drag = { ...drag, b: t };
    let best = -1;
    let bestD = Infinity;
    ts.forEach((p, i) => {
      const d = Math.abs(p - t);
      if (d < bestD) {
        bestD = d;
        best = i;
      }
    });
    hover = best >= 0 ? best : null;
  }
</script>

<div class="chart">
  <div class="legend">
    {#each series as s (s.label)}
      <span><i style:background={s.color}></i>{s.label}</span>
    {/each}
    <span class="spacer"></span>
    {#if hover != null && hover < ts.length}
      <span class="readout">
        {fmtTime(ts[hover])} ·
        {#each series as s, i (s.label)}{i ? " / " : ""}<span style:color={s.color}>{s.values[hover] != null ? format(s.values[hover]!) : "--"}</span>{/each}
      </span>
    {:else}
      <span class="muted">max {format(top / 1.1 > 0 && max == null ? top / 1.1 : top)}</span>
    {/if}
  </div>
  {#if !ts.length}
    <p class="muted empty">No history yet. Points are stored once a minute while Nightgrid runs.</p>
  {:else}
    <svg
      viewBox="0 0 {W} {H}"
      preserveAspectRatio="none"
      role="img"
      aria-label="History"
      onpointermove={onMove}
      onpointerdown={onDown}
      onpointerup={onUp}
      onpointercancel={() => (drag = null)}
      onpointerleave={() => (hover = null)}
      ondblclick={() => onzoom?.(null)}
    >
      {#each [0.25, 0.5, 0.75] as f (f)}
        <line x1="0" x2={W} y1={H * f} y2={H * f} class="grid" vector-effect="non-scaling-stroke" />
      {/each}
      {#each series as s (s.label)}
        {#each segments(s.values) as seg, i (i)}
          <polygon points="{seg[0].split(',')[0]},{H} {seg.join(' ')} {seg[seg.length - 1].split(',')[0]},{H}" fill={s.color} opacity="0.12" />
          <polyline points={seg.join(" ")} fill="none" stroke={s.color} stroke-width="1.3" vector-effect="non-scaling-stroke" />
        {/each}
      {/each}
      {#if drag}
        <rect x={x(Math.min(drag.a, drag.b))} width={Math.abs(x(drag.b) - x(drag.a))} y="0" height={H} class="sel" />
      {/if}
      {#if hover != null && hover < ts.length}
        <line x1={x(ts[hover])} x2={x(ts[hover])} y1="0" y2={H} class="cursor" vector-effect="non-scaling-stroke" />
      {/if}
    </svg>
    <div class="axis">
      {#each ticks as t (t)}<span>{fmtTime(t)}</span>{/each}
    </div>
  {/if}
</div>

<style>
  .chart {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .legend {
    display: flex;
    gap: 12px;
    font-size: var(--fs-xs);
    color: var(--text-1);
    align-items: center;
    min-height: 18px;
  }
  .legend i {
    display: inline-block;
    width: 10px;
    height: 2px;
    margin-right: 5px;
    vertical-align: middle;
  }
  .spacer {
    flex: 1;
  }
  .readout {
    font-family: var(--font-mono);
    color: var(--text-0);
  }
  svg {
    width: 100%;
    height: 160px;
    display: block;
    border: 1px solid var(--line);
    background: var(--bg-0);
    cursor: crosshair;
  }
  .grid {
    stroke: color-mix(in srgb, var(--line) 60%, transparent);
    stroke-dasharray: 2 4;
  }
  .cursor {
    stroke: var(--magenta);
  }
  .sel {
    fill: color-mix(in srgb, var(--cyan) 18%, transparent);
    stroke: var(--cyan);
    stroke-width: 1;
  }
  .axis {
    display: flex;
    justify-content: space-between;
    font-size: var(--fs-xs);
    color: var(--text-1);
    font-family: var(--font-mono);
  }
  .muted {
    color: var(--text-1);
  }
  .empty {
    margin: 0;
    padding: 24px;
    border: 1px dashed var(--line);
    text-align: center;
    font-size: var(--fs-sm);
  }
</style>
