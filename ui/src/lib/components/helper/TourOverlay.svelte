<script lang="ts">
  /**
   * The running tour (M10q): brackets around the step's control and a
   * callout with the step text. The overlay lets every click through; only
   * the callout's own buttons take clicks. It sits in the top layer (as a
   * popover) so it stays visible above open dialogs.
   */
  import { tour } from "../../stores/tour.svelte";
  import Drift from "./Drift.svelte";

  let root = $state<HTMLDivElement | null>(null);
  let note = $state<HTMLDivElement | null>(null);
  let box = $state<{ x: number; y: number; w: number; h: number } | null>(null);
  let vw = $state(1200);
  let vh = $state(800);

  // Keep the overlay above dialogs opened after it.
  $effect(() => {
    const el = root;
    if (!el || !tour.tour) return;
    const raise = () => {
      try {
        if (el.matches(":popover-open")) el.hidePopover();
        el.showPopover();
      } catch {
        /* no popover support: stays a fixed layer */
      }
    };
    raise();
    let dialogs = document.querySelectorAll("dialog[open]").length;
    const t = setInterval(() => {
      const n = document.querySelectorAll("dialog[open]").length;
      if (n > dialogs) raise();
      dialogs = n;
    }, 150);
    return () => clearInterval(t);
  });

  // Follow the target as the page moves.
  $effect(() => {
    const id = tour.targetId;
    if (!id) {
      box = null;
      return;
    }
    let raf = 0;
    let el: HTMLElement | null = null;
    const track = () => {
      vw = innerWidth;
      vh = innerHeight;
      const found = document.querySelector<HTMLElement>(`[data-guide="${id}"]`);
      if (found !== el) {
        el?.classList.remove("guide-target");
        el = found;
        el?.classList.add("guide-target");
        el?.scrollIntoView({ block: "nearest", inline: "nearest" });
      }
      const r = el?.getBoundingClientRect();
      if (r && r.width + r.height > 0) {
        if (!box || box.x !== r.left || box.y !== r.top || box.w !== r.width || box.h !== r.height) box = { x: r.left, y: r.top, w: r.width, h: r.height };
      } else box = null;
      raf = requestAnimationFrame(track);
    };
    track();
    return () => {
      cancelAnimationFrame(raf);
      el?.classList.remove("guide-target");
    };
  });

  $effect(() => {
    if (!tour.tour) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !document.querySelector("dialog[open]") && !document.querySelector('[aria-label="Command palette"]')) tour.stop();
    };
    // Capture: runs before the palette or a dialog closes itself on the same Esc.
    addEventListener("keydown", onKey, true);
    return () => removeEventListener("keydown", onKey, true);
  });

  const PAD = 5;
  const W = 330;
  // Below the target, else above it; centered when there is no target.
  $effect(() => {
    if (!note) return;
    if (!box) {
      note.style.setProperty("--x", `${Math.max(8, vw / 2 - W / 2)}px`);
      note.style.setProperty("--y", `${Math.max(8, vh / 2 - 90)}px`);
      note.classList.remove("above");
      return;
    }
    const below = box.y + box.h + 170 < vh;
    note.style.setProperty("--x", `${Math.max(8, Math.min(box.x - PAD, vw - W - 8))}px`);
    note.style.setProperty("--y", `${below ? box.y + box.h + PAD + 10 : box.y - PAD - 10}px`);
    note.classList.toggle("above", !below);
  });

  const waitHint = $derived.by(() => {
    const w = tour.step?.wait;
    if (!w) return "";
    if (w.kind === "click") return "Waiting for your click…";
    if (w.kind === "view") return "Waiting for you to open it…";
    if (w.kind === "appear") return "Waiting for it to open…";
    if (w.kind === "gone") return "Waiting for you to close it…";
    return "";
  });
</script>

{#if tour.tour && tour.step}
  <div class="layer" popover="manual" bind:this={root} aria-live="polite">
    {#if box}
      <svg class="frame" width="100%" height="100%" aria-hidden="true">
        {#each [[0, 0, 1, 1], [1, 0, -1, 1], [0, 1, 1, -1], [1, 1, -1, -1]] as [cx, cy, dx, dy], i (i)}
          {@const x = box.x - PAD + cx * (box.w + PAD * 2)}
          {@const y = box.y - PAD + cy * (box.h + PAD * 2)}
          <path d="M{x} {y + dy * 10} V{y} H{x + dx * 10}" />
        {/each}
      </svg>
    {/if}
    <div class="note" bind:this={note} role="dialog" aria-label="Tour: {tour.tour.title}">
      <div class="head">
        <Drift mood="point" scale={2} />
        <div>
          <p class="t micro">TOUR · {tour.tour.title.toUpperCase()}</p>
          <p class="n micro">STEP {tour.index + 1} / {tour.tour.steps.length}</p>
        </div>
      </div>
      <p class="txt">{tour.step.text}</p>
      {#if tour.missing && tour.step.wait.kind !== "view"}
        <p class="muted small">I can't see that control on this screen. It may be in another view or tab; skip ahead if you like.</p>
      {/if}
      {#if waitHint}<p class="wait micro">{waitHint}</p>{/if}
      <div class="btns">
        <button type="button" class="btn ghost tiny" onclick={() => tour.stop()}>End tour</button>
        <span class="spacer"></span>
        {#if tour.index > 0}<button type="button" class="btn ghost tiny" onclick={() => tour.back()}>Back</button>{/if}
        {#if tour.step.wait.kind === "next"}
          <button type="button" class="btn primary tiny" onclick={() => tour.next()}>{tour.index + 1 === tour.tour.steps.length ? "Finish" : "Next"}</button>
        {:else}
          <button type="button" class="btn tiny" onclick={() => tour.next()}>Skip step</button>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .layer {
    position: fixed;
    inset: 0;
    width: 100vw;
    height: 100vh;
    margin: 0;
    padding: 0;
    border: 0;
    background: transparent;
    overflow: visible;
    pointer-events: none;
    z-index: 950;
  }
  .layer:not(:popover-open) {
    display: block;
  }
  .frame {
    position: absolute;
    inset: 0;
  }
  .frame path {
    fill: none;
    stroke: var(--magenta);
    stroke-width: 2;
    filter: drop-shadow(0 0 3px var(--magenta));
  }
  .note {
    position: absolute;
    left: var(--x);
    top: var(--y);
    width: 330px;
    pointer-events: auto;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px;
    background: var(--bg-1);
    border: 1px solid var(--magenta);
    box-shadow: 0 8px 28px rgb(0 0 0 / 0.45);
    color: var(--text-0);
    font-size: var(--fs-sm);
  }
  .note:global(.above) {
    transform: translateY(-100%);
  }
  .head {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .head p {
    margin: 0;
  }
  .t {
    color: var(--magenta);
    letter-spacing: 0.12em;
  }
  .n {
    color: var(--text-1);
    letter-spacing: 0.1em;
  }
  .txt {
    margin: 0;
    line-height: 1.5;
  }
  .small {
    font-size: var(--fs-xs);
    margin: 0;
  }
  .wait {
    margin: 0;
    color: var(--cyan);
    letter-spacing: 0.08em;
  }
  .btns {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .spacer {
    flex: 1;
  }
  :global(:root[data-effects="off"]) .frame path {
    filter: none;
  }
</style>
