<script lang="ts">
  /**
   * "Show me": magenta corner brackets around the target control plus a
   * callout with DRIFT's note. Pure decoration: the overlay never takes
   * clicks, and it clears on any click, Esc, or after 8 s.
   */
  import { helper } from "../../stores/helper.svelte";
  import Drift from "./Drift.svelte";

  let box = $state<{ x: number; y: number; w: number; h: number } | null>(null);
  let target: HTMLElement | null = null;

  $effect(() => {
    const spot = helper.spotlight;
    if (!spot) {
      box = null;
      return;
    }
    target = document.querySelector<HTMLElement>(`[data-guide="${spot.id}"]`);
    if (!target) return;
    target.classList.add("guide-target");
    target.setAttribute("aria-describedby", "drift-spot-note");
    let raf = 0;
    const track = () => {
      const r = target?.getBoundingClientRect();
      if (r && r.width + r.height > 0) {
        if (!box || box.x !== r.left || box.y !== r.top || box.w !== r.width || box.h !== r.height) box = { x: r.left, y: r.top, w: r.width, h: r.height };
      } else box = null;
      raf = requestAnimationFrame(track);
    };
    track();
    const clear = () => helper.clearSpotlight();
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && clear();
    // Capture so the click still reaches the control underneath.
    addEventListener("pointerdown", clear, true);
    addEventListener("keydown", onKey, true);
    return () => {
      cancelAnimationFrame(raf);
      removeEventListener("pointerdown", clear, true);
      removeEventListener("keydown", onKey, true);
      target?.classList.remove("guide-target");
      target?.removeAttribute("aria-describedby");
      target = null;
    };
  });

  const PAD = 5;
  /** Callout below the target, or above it near the bottom of the window. */
  const below = $derived(box ? box.y + box.h + 90 < innerHeight : true);
  const left = $derived(box ? Math.max(8, Math.min(box.x - PAD, innerWidth - 300)) : 0);
  let noteEl: HTMLDivElement | undefined = $state();
  // Position through CSS variables (the CSP forbids inline style attributes).
  $effect(() => {
    if (!noteEl || !box) return;
    noteEl.style.setProperty("--x", `${left}px`);
    noteEl.style.setProperty("--y", `${below ? box.y + box.h + PAD + 8 : box.y - PAD - 8}px`);
  });
</script>

{#if helper.spotlight && box}
  <div class="spot" aria-hidden="true">
    <svg class="frame" width="100%" height="100%">
      {#each [[0, 0, 1, 1], [1, 0, -1, 1], [0, 1, 1, -1], [1, 1, -1, -1]] as [cx, cy, dx, dy], i (i)}
        {@const x = box.x - PAD + cx * (box.w + PAD * 2)}
        {@const y = box.y - PAD + cy * (box.h + PAD * 2)}
        <path d="M{x} {y + dy * 9} V{y} H{x + dx * 9}" />
      {/each}
    </svg>
  </div>
  <div class="note" class:above={!below} role="status" id="drift-spot-note" bind:this={noteEl}>
    <Drift mood="point" scale={2} />
    <span>{helper.spotlight.note}</span>
  </div>
{/if}

<style>
  .spot {
    position: fixed;
    inset: 0;
    z-index: 900;
    pointer-events: none;
  }
  .frame path {
    fill: none;
    stroke: var(--magenta);
    stroke-width: 2;
    filter: drop-shadow(0 0 3px var(--magenta));
  }
  .note {
    position: fixed;
    left: var(--x);
    top: var(--y);
    z-index: 901;
    pointer-events: none;
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 300px;
    padding: 6px 10px 6px 6px;
    background: var(--bg-1);
    border: 1px solid var(--magenta);
    color: var(--text-0);
    font-size: var(--fs-sm);
  }
  .note.above {
    transform: translateY(-100%);
  }
  :global(.guide-target) {
    outline: 1px solid var(--magenta) !important;
    outline-offset: 1px;
    box-shadow: var(--glow-active, 0 0 8px var(--magenta));
  }
  :global(:root[data-effects="off"]) .frame path {
    filter: none;
  }
</style>
