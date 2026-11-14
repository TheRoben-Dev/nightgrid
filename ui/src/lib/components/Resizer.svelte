<script lang="ts">
  /**
   * Draggable splitter for a size in the layout store. Place it on the edge
   * of the sized element. `invert` when the sized element is after the
   * handle (e.g. a right-hand panel or a bottom console), so dragging
   * towards it makes it smaller. Keyboard: arrows resize, Home/End go to
   * min/max, Enter or double-click resets.
   */
  import { layout, SIZES, type SizeKey } from "../stores/layout.svelte";

  interface Props {
    key: SizeKey;
    axis: "x" | "y";
    invert?: boolean;
    label: string;
    /** Absolutely positioned on the parent's edge instead of taking space in the flow. */
    overlay?: boolean;
  }
  let { key, axis, invert = false, label, overlay = false }: Props = $props();

  const spec = $derived(SIZES[key]);
  const value = $derived(layout.get(key));

  function drag(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    const parent = el.parentElement;
    if (!parent) return;
    const total = axis === "x" ? parent.getBoundingClientRect().width : parent.getBoundingClientRect().height;
    const start = axis === "x" ? e.clientX : e.clientY;
    const startValue = value;
    el.setPointerCapture(e.pointerId);
    layout.dragging = true;
    const move = (ev: PointerEvent) => {
      let d = (axis === "x" ? ev.clientX : ev.clientY) - start;
      if (invert) d = -d;
      const delta = spec.unit === "%" ? (total > 0 ? (d / total) * 100 : 0) : d;
      layout.set(key, startValue + delta);
    };
    const up = () => {
      layout.dragging = false;
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
  }

  function onkeydown(e: KeyboardEvent) {
    const step = spec.unit === "px" ? 16 : 2;
    const grow = axis === "x" ? (invert ? "ArrowLeft" : "ArrowRight") : invert ? "ArrowUp" : "ArrowDown";
    const shrink = axis === "x" ? (invert ? "ArrowRight" : "ArrowLeft") : invert ? "ArrowDown" : "ArrowUp";
    let next: number | null = null;
    if (e.key === grow) next = value + step;
    else if (e.key === shrink) next = value - step;
    else if (e.key === "Home") next = spec.min;
    else if (e.key === "End") next = spec.max;
    else if (e.key === "Enter") {
      layout.reset(key);
      e.preventDefault();
      return;
    }
    if (next !== null) {
      e.preventDefault();
      layout.set(key, next);
    }
  }
</script>

<!-- A focusable separator is an ARIA widget (resizable splitter). -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="resizer {axis}"
  class:overlay
  role="separator"
  aria-label={label}
  aria-orientation={axis === "x" ? "vertical" : "horizontal"}
  aria-valuenow={Math.round(value)}
  aria-valuemin={spec.min}
  aria-valuemax={spec.max}
  tabindex="0"
  title="Drag to resize · double-click to reset"
  onpointerdown={drag}
  ondblclick={() => layout.reset(key)}
  {onkeydown}
></div>

<style>
  .resizer {
    flex: none;
    position: relative;
    z-index: 3;
    touch-action: none;
  }
  .resizer.x {
    width: 8px;
    margin: 0 -1px;
    cursor: col-resize;
  }
  .resizer.y {
    height: 8px;
    margin: -1px 0;
    cursor: row-resize;
  }
  .resizer.overlay {
    position: absolute;
    margin: 0;
  }
  .resizer.overlay.x {
    top: 0;
    bottom: 0;
    right: -4px;
  }
  .resizer::after {
    content: "";
    position: absolute;
    background: transparent;
    transition: background var(--dur-fast) var(--ease);
  }
  .resizer.x::after {
    left: 3px;
    top: 0;
    bottom: 0;
    width: 2px;
  }
  .resizer.y::after {
    top: 3px;
    left: 0;
    right: 0;
    height: 2px;
  }
  .resizer:hover::after,
  .resizer:active::after {
    background: var(--mint);
  }
  .resizer:focus-visible {
    outline: none;
  }
  .resizer:focus-visible::after {
    background: var(--accent);
  }
</style>
