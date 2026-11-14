<script lang="ts">
  import { terms } from "../../stores/terms.svelte";
  import type { LayoutNode } from "../../term/layout";
  import SplitNode from "./SplitNode.svelte";
  import TermPane from "./TermPane.svelte";

  interface Props {
    node: LayoutNode;
    tabId: number;
    active: number;
    multi: boolean;
    path?: string;
  }
  let { node, tabId, active, multi, path = "" }: Props = $props();

  let box: HTMLDivElement | undefined = $state();

  function drag(e: PointerEvent) {
    if (node.kind !== "split" || !box) return;
    e.preventDefault();
    const dir = node.dir;
    const rect = box.getBoundingClientRect();
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      const r = dir === "row" ? (ev.clientX - rect.left) / rect.width : (ev.clientY - rect.top) / rect.height;
      terms.resize(tabId, path, r);
    };
    const up = () => {
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", up);
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", up);
  }

  function keyResize(e: KeyboardEvent) {
    if (node.kind !== "split") return;
    const back = node.dir === "row" ? "ArrowLeft" : "ArrowUp";
    const fwd = node.dir === "row" ? "ArrowRight" : "ArrowDown";
    if (e.key === back || e.key === fwd) {
      e.preventDefault();
      terms.resize(tabId, path, node.ratio + (e.key === fwd ? 0.05 : -0.05));
    }
  }
</script>

{#if node.kind === "pane"}
  {@const s = terms.session(node.pane)}
  {#if s}
    {#key node.pane}
      <TermPane session={s} {tabId} active={active === node.pane} {multi} />
    {/key}
  {/if}
{:else}
  <div class="split {node.dir}" bind:this={box}>
    <div class="cell" style:flex-basis="{node.ratio * 100}%">
      <SplitNode node={node.a} {tabId} {active} {multi} path={path + "a"} />
    </div>
    <!-- A focusable separator is an ARIA widget (resizable splitter). -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      class="divider"
      role="separator"
      aria-orientation={node.dir === "row" ? "vertical" : "horizontal"}
      aria-valuenow={Math.round(node.ratio * 100)}
      aria-valuemin={15}
      aria-valuemax={85}
      tabindex="0"
      onpointerdown={drag}
      onkeydown={keyResize}
    ></div>
    <div class="cell" style:flex-basis="{(1 - node.ratio) * 100}%">
      <SplitNode node={node.b} {tabId} {active} {multi} path={path + "b"} />
    </div>
  </div>
{/if}

<style>
  .split {
    display: flex;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
  }
  .split.col {
    flex-direction: column;
  }
  .cell {
    flex-grow: 0;
    flex-shrink: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
  }
  .divider {
    flex: none;
    background: transparent;
    position: relative;
  }
  .row > .divider {
    width: 5px;
    cursor: col-resize;
  }
  .col > .divider {
    height: 5px;
    cursor: row-resize;
  }
  .divider::after {
    content: "";
    position: absolute;
    background: var(--line);
  }
  .row > .divider::after {
    left: 2px;
    top: 0;
    bottom: 0;
    width: 1px;
  }
  .col > .divider::after {
    top: 2px;
    left: 0;
    right: 0;
    height: 1px;
  }
  .divider:hover::after,
  .divider:focus-visible::after {
    background: var(--mint);
  }
</style>
