<script lang="ts">
  /**
   * The network map as a topology graph (D-141): pan with the background,
   * zoom with the wheel, drag a node to place it (remembered), drop it on a
   * router, access point, switch or host to say it connects through that.
   */
  import { onMount } from "svelte";
  import { bounds, canAttach, canParent, NH, NW, type TopoLink, type TopoNode } from "../netmap/layout";
  import { icon } from "../netmap/icons";
  import { netmap } from "../stores/netmap.svelte";

  let { onpick }: { onpick: (id: string) => void } = $props();

  const topo = $derived(netmap.topo);
  let svg = $state<SVGSVGElement | null>(null);
  let view = $state({ x: 20, y: 20, k: 1 });
  let fitted = $state(false);

  // Drag state: the node, where it started, how far it has moved.
  let drag = $state<{ id: string; dx: number; dy: number; moved: boolean; follow: Set<string>; target: string | null } | null>(null);
  let start = { cx: 0, cy: 0, vx: 0, vy: 0 };
  let panning = $state(false);

  const pos = (t: TopoNode) => (drag && (drag.id === t.id || drag.follow.has(t.id)) ? { x: t.x + drag.dx, y: t.y + drag.dy } : { x: t.x, y: t.y });
  const byId = $derived(new Map(topo.nodes.map((t) => [t.id, t])));

  function fit() {
    if (!svg || !topo.nodes.length) return;
    const r = svg.getBoundingClientRect();
    if (!r.width || !r.height) return;
    const b = bounds(topo.nodes, 40);
    const k = Math.min(1.2, Math.max(0.25, Math.min(r.width / b.w, r.height / b.h)));
    view = { k, x: (r.width - b.w * k) / 2 - b.x * k, y: Math.max(10, (r.height - b.h * k) / 2) - b.y * k };
    fitted = true;
  }
  $effect(() => {
    if (!fitted && topo.nodes.length && svg) fit();
  });
  // A new scan with a different set of nodes: fit again.
  let lastCount = 0;
  $effect(() => {
    const n = topo.nodes.length;
    if (Math.abs(n - lastCount) > 3) fitted = false;
    lastCount = n;
  });
  onMount(() => {
    const ro = new ResizeObserver(() => {
      if (!fitted) fit();
    });
    if (svg) ro.observe(svg);
    return () => ro.disconnect();
  });

  function toMap(e: { clientX: number; clientY: number }) {
    const r = svg!.getBoundingClientRect();
    return { x: (e.clientX - r.left - view.x) / view.k, y: (e.clientY - r.top - view.y) / view.k };
  }

  function wheel(e: WheelEvent) {
    e.preventDefault();
    const r = svg!.getBoundingClientRect();
    const k = Math.min(2.5, Math.max(0.15, view.k * Math.exp(-e.deltaY * 0.0015)));
    const px = e.clientX - r.left;
    const py = e.clientY - r.top;
    view = { k, x: px - ((px - view.x) * k) / view.k, y: py - ((py - view.y) * k) / view.k };
  }
  function zoom(f: number) {
    const r = svg!.getBoundingClientRect();
    const k = Math.min(2.5, Math.max(0.15, view.k * f));
    const px = r.width / 2;
    const py = r.height / 2;
    view = { k, x: px - ((px - view.x) * k) / view.k, y: py - ((py - view.y) * k) / view.k };
  }

  function descendants(id: string): Set<string> {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- built once per drag, never mutated after
    const out = new Set<string>();
    const walk = (p: string) => {
      for (const t of topo.nodes) {
        if (t.parent !== p || t.pinned || out.has(t.id)) continue;
        out.add(t.id);
        walk(t.id);
      }
    };
    walk(id);
    return out;
  }

  function down(e: PointerEvent, t?: TopoNode) {
    if (e.button !== 0) return;
    start = { cx: e.clientX, cy: e.clientY, vx: view.x, vy: view.y };
    svg?.setPointerCapture(e.pointerId);
    if (t) {
      e.stopPropagation();
      drag = { id: t.id, dx: 0, dy: 0, moved: false, follow: t.ghost ? new Set() : descendants(t.id), target: null };
    } else panning = true;
  }
  function move(e: PointerEvent) {
    if (drag) {
      const dx = (e.clientX - start.cx) / view.k;
      const dy = (e.clientY - start.cy) / view.k;
      const moved = drag.moved || Math.hypot(e.clientX - start.cx, e.clientY - start.cy) > 4;
      let target: string | null = null;
      if (moved) {
        const p = toMap(e);
        const me = drag;
        const hit = topo.nodes.find(
          (t) => t.id !== me.id && !me.follow.has(t.id) && !t.ghost && p.x >= t.x && p.x <= t.x + NW && p.y >= t.y && p.y <= t.y + NH,
        );
        if (hit && canParent(hit) && canAttach(topo, me.id, hit.id)) target = hit.id;
      }
      drag = { ...drag, dx, dy, moved, target };
    } else if (panning) {
      view = { ...view, x: start.vx + e.clientX - start.cx, y: start.vy + e.clientY - start.cy };
    }
  }
  async function up(e: PointerEvent) {
    svg?.releasePointerCapture(e.pointerId);
    panning = false;
    const d = drag;
    if (!d) return;
    const t = byId.get(d.id);
    if (!t || !d.moved) {
      drag = null;
      if (t) onpick(t.id);
      return;
    }
    if (t.ghost) {
      drag = null;
      return;
    }
    if (d.target) {
      await netmap.setParent(t.id, d.target, true);
    } else {
      await netmap.move(t.id, t.x + d.dx, t.y + d.dy);
    }
    drag = null;
  }
  function key(e: KeyboardEvent, t: TopoNode) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onpick(t.id);
    }
  }

  function path(l: TopoLink): string {
    const a = byId.get(l.from);
    const b = byId.get(l.to);
    if (!a || !b) return "";
    const pa = pos(a);
    const pb = pos(b);
    const below = pb.y >= pa.y + NH;
    const x1 = pa.x + NW / 2;
    const y1 = below ? pa.y + NH : pa.y + NH / 2;
    const x2 = pb.x + NW / 2;
    const y2 = below ? pb.y : pb.y + NH / 2;
    if (!below) return `M${x1},${y1} L${x2},${y2}`;
    const m = (y1 + y2) / 2;
    return `M${x1},${y1} C${x1},${m} ${x2},${m} ${x2},${y2}`;
  }

  function zoneBox(members: string[]) {
    const ns = members.map((m) => byId.get(m)).filter((t): t is TopoNode => !!t).map(pos);
    return bounds(ns, 16);
  }

  const clip = (s: string, n: number) => (s.length > n ? s.slice(0, n - 1) + "…" : s);
  function subText(t: TopoNode): string {
    const n = t.node;
    if (n.kind === "host") {
      if (n.unseen && !t.ghost) return "not seen in this scan";
      if (n.ports) return `LAN ${n.ports.lan} · tailnet ${n.ports.tailnet} · local ${n.ports.local}`;
    }
    return n.sub;
  }
  function badge(t: TopoNode): { text: string; cls: string; title: string } | null {
    const n = t.node;
    if (t.ghost) return { text: "MISSING", cls: "miss", title: "In the expected map, not seen in this scan" };
    const risk = netmap.svcRisk(t.key);
    if (risk) return { text: "⚠", cls: risk === "bad" ? "" : "miss", title: risk === "bad" ? "A risky service is open (see Services)" : "A service worth checking is open" };
    if (n.exposures.length && n.kind !== "internet") return { text: `▲ ${n.exposures.length}`, cls: "", title: "Reachable from the internet" };
    if (netmap.canScan(n.id)) return { text: "◉", cls: "scan", title: "Can run a scan" };
    if (n.device?.online === false) return { text: "offline", cls: "off", title: "Offline" };
    if (n.device?.isNew && !n.device.trusted) return { text: "NEW", cls: "new", title: "First seen in this scan" };
    if (t.unexpected) return { text: "+", cls: "new", title: "Not in the expected map" };
    if (n.device?.routed) return { text: "⇢", cls: "routed", title: "In a routed range" };
    if (n.device?.trusted) return { text: "✓", cls: "trusted", title: "Trusted" };
    return null;
  }
</script>

<div class="topo">
  <div class="tools" role="toolbar" aria-label="Map view">
    <button type="button" class="btn ghost tiny" onclick={() => zoom(1.25)} aria-label="Zoom in">+</button>
    <button type="button" class="btn ghost tiny" onclick={() => zoom(0.8)} aria-label="Zoom out">−</button>
    <button type="button" class="btn ghost tiny" onclick={fit} title="Fit the whole map">FIT</button>
  </div>
  <svg
    bind:this={svg}
    class:panning
    class:dragging={drag?.moved}
    role="application"
    aria-label="Network topology: drag the background to pan, the wheel to zoom, a node to move it"
    onwheel={wheel}
    onpointerdown={(e) => down(e)}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
  >
    <g transform={`translate(${view.x},${view.y}) scale(${view.k})`}>
      {#each topo.zones as z (z.id)}
        {@const b = zoneBox(z.members)}
        <g class={`zone ${z.kind}`}>
          <rect x={b.x} y={b.y - 12} width={b.w} height={b.h + 12} rx="6" />
          <text x={b.x + 10} y={b.y + 1}>{z.label}</text>
        </g>
      {/each}
      {#each topo.links as l (l.from + ">" + l.to)}
        <path class={`link ${l.kind}`} class:extra={!l.tree} class:manual={l.manual} d={path(l)} />
      {/each}
      {#each topo.nodes as t (t.id)}
        {@const p = pos(t)}
        {@const b = badge(t)}
        <g
          class={`node ${t.node.kind} st-${t.status}`}
          class:sel={netmap.selected === t.id}
          class:target={drag?.target === t.id}
          class:lifted={drag?.id === t.id && drag.moved}
          class:pinned={t.pinned}
          transform={`translate(${p.x},${p.y})`}
          role="button"
          tabindex="0"
          aria-pressed={netmap.selected === t.id}
          aria-label={`${t.node.label}, ${t.node.sub}`}
          data-node={t.id}
          onpointerdown={(e) => down(e, t)}
          onkeydown={(e) => key(e, t)}
        >
          <rect class="box" width={NW} height={NH} rx="3" />
          <rect class="tag" width="3" height={NH} />
          <g class="ico" transform="translate(12,18)"><path d={icon(t.icon)} /></g>
          <text class="label" x="38" y="22">{clip(t.node.label, 18)}</text>
          <text class="sub" x="38" y="39">{clip(subText(t), 22)}</text>
          {#if b}<text class={`badge ${b.cls}`} x={NW - 8} y="16" text-anchor="end"><title>{b.title}</title>{b.text}</text>{/if}
        </g>
      {/each}
    </g>
  </svg>
</div>

<style>
  .topo {
    position: relative;
    height: 100%;
    min-height: 420px;
  }
  .tools {
    position: absolute;
    top: 6px;
    right: 6px;
    display: flex;
    gap: 4px;
    z-index: 1;
  }
  svg {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 420px;
    cursor: grab;
    touch-action: none;
    user-select: none;
  }
  svg.panning {
    cursor: grabbing;
  }
  svg.dragging .node {
    cursor: grabbing;
  }
  .zone rect {
    fill: color-mix(in srgb, var(--cyan) 4%, transparent);
    stroke: color-mix(in srgb, var(--cyan) 35%, transparent);
    stroke-dasharray: 6 4;
  }
  .zone.routed rect {
    fill: color-mix(in srgb, var(--warn) 4%, transparent);
    stroke: color-mix(in srgb, var(--warn) 40%, transparent);
  }
  .zone.tailnet rect {
    fill: color-mix(in srgb, var(--mint) 4%, transparent);
    stroke: color-mix(in srgb, var(--mint) 40%, transparent);
  }
  .zone text {
    font-family: var(--font-display);
    font-size: 10px;
    letter-spacing: var(--tracking-label);
    fill: var(--text-1);
  }
  .link {
    fill: none;
    stroke-width: 1.5;
  }
  .link.lan {
    stroke: color-mix(in srgb, var(--cyan) 60%, transparent);
  }
  .link.wan {
    stroke: var(--text-1);
  }
  .link.routed {
    stroke: color-mix(in srgb, var(--warn) 70%, transparent);
    stroke-dasharray: 2 4;
  }
  .link.tailnet {
    stroke: color-mix(in srgb, var(--mint) 65%, transparent);
    stroke-dasharray: 5 4;
  }
  .link.internet {
    stroke: var(--danger);
    stroke-width: 2;
  }
  .link.extra {
    opacity: 0.5;
  }
  .link.manual {
    stroke-width: 2;
  }
  .node {
    cursor: pointer;
    outline: none;
  }
  .box {
    fill: var(--bg-2);
    stroke: var(--line);
  }
  .tag {
    fill: var(--line);
  }
  .node.host .tag {
    fill: var(--cyan);
  }
  .node.gateway .tag,
  .node.tailnet .tag {
    fill: var(--mint);
  }
  .node.internet .tag,
  .st-exposed .tag {
    fill: var(--danger);
  }
  .st-new .tag {
    fill: var(--cyan);
  }
  .st-offline text,
  .st-unseen text {
    opacity: 0.55;
  }
  .st-unseen .box {
    stroke-dasharray: 4 3;
  }
  .st-ghost .box {
    fill: transparent;
    stroke: var(--warn);
    stroke-dasharray: 5 3;
  }
  .st-ghost text,
  .st-ghost .ico {
    opacity: 0.6;
  }
  .ico path {
    fill: none;
    stroke: var(--text-1);
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .node.gateway .ico path,
  .node.tailnet .ico path {
    stroke: var(--mint);
  }
  .node.host .ico path {
    stroke: var(--cyan);
  }
  .node.internet .ico path,
  .st-exposed .ico path {
    stroke: var(--danger);
  }
  .node:hover .box,
  .node:focus-visible .box {
    stroke: var(--mint);
  }
  .node.sel .box {
    stroke: var(--magenta);
    fill: color-mix(in srgb, var(--magenta) 10%, var(--bg-2));
  }
  .node.target .box {
    stroke: var(--mint);
    stroke-width: 2.5;
    fill: color-mix(in srgb, var(--mint) 12%, var(--bg-2));
  }
  .node.lifted {
    opacity: 0.85;
  }
  .label {
    fill: var(--text-0);
    font-size: 13px;
    font-weight: 600;
  }
  .sub {
    fill: var(--text-1);
    font-size: 10.5px;
    font-family: var(--font-mono);
  }
  .badge {
    fill: var(--danger);
    font-size: 10px;
    font-weight: 700;
  }
  .badge.scan,
  .badge.new {
    fill: var(--cyan);
  }
  .badge.scan {
    font-weight: 400;
  }
  .badge.off,
  .badge.routed {
    fill: var(--text-1);
    font-weight: 400;
  }
  .badge.trusted {
    fill: var(--mint);
  }
  .badge.miss {
    fill: var(--warn);
  }
</style>
