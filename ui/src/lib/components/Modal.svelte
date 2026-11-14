<script lang="ts">
  import type { Snippet } from "svelte";
  import { hexId } from "../hex";

  interface Props {
    open: boolean;
    title: string;
    /** `danger` paints the frame red (destructive or security-critical dialogs). */
    tone?: "default" | "danger";
    width?: number;
    onclose: () => void;
    children: Snippet;
    footer?: Snippet;
  }
  let { open, title, tone = "default", width = 560, onclose, children, footer }: Props = $props();

  let dialog: HTMLDialogElement | undefined = $state();

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  });
</script>

<dialog
  bind:this={dialog}
  class:danger={tone === "danger"}
  style:width="min({width}px, calc(100vw - 48px))"
  aria-label={title}
  oncancel={(e) => {
    e.preventDefault();
    onclose();
  }}
>
  {#if open}
    <header>
      <h2 class="label"><span class="prefix" aria-hidden="true">//</span> {title}</h2>
      <span class="hex" aria-hidden="true">{hexId(title)}</span>
      <button type="button" class="x" aria-label="Close" onclick={onclose}>×</button>
    </header>
    <div class="body">{@render children()}</div>
    {#if footer}<footer>{@render footer()}</footer>{/if}
  {/if}
</dialog>

<style>
  dialog {
    padding: 0;
    border: 1px solid color-mix(in srgb, var(--mint) 40%, var(--line));
    background: var(--bg-1);
    color: var(--text-0);
    max-height: calc(100vh - 64px);
    overflow: hidden;
    flex-direction: column;
  }
  dialog[open] {
    display: flex;
  }
  dialog::backdrop {
    background: color-mix(in srgb, var(--bg-0) 78%, transparent);
  }
  dialog.danger {
    border-color: var(--danger);
    box-shadow: 0 0 24px color-mix(in srgb, var(--danger) 30%, transparent);
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 8px 8px 14px;
    border-bottom: 1px solid var(--line);
    background-image: linear-gradient(90deg, color-mix(in srgb, var(--mint) 7%, transparent), transparent 60%);
  }
  .danger header {
    background-image: linear-gradient(90deg, color-mix(in srgb, var(--danger) 20%, transparent), transparent 70%);
  }
  h2 {
    margin: 0;
    flex: 1;
    color: var(--text-0);
  }
  .prefix {
    color: var(--cyan);
  }
  .danger .prefix {
    color: var(--danger);
  }
  .hex {
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .x {
    width: var(--hit);
    height: var(--hit);
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-1);
    font-size: 18px;
    cursor: pointer;
  }
  .x:hover {
    color: var(--text-0);
    border-color: var(--line);
  }
  .body {
    padding: 14px;
    overflow: auto;
    min-height: 0;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 10px 14px;
    border-top: 1px solid var(--line);
  }
</style>
