<script lang="ts">
  /**
   * "Explain" next to logs, output and files: hands the text to DRIFT with a
   * label saying what it is. Hidden while the helper is off. The text is
   * read only when clicked (selected lines win over the whole thing).
   */
  import { currentSelection, helper } from "../../stores/helper.svelte";

  interface Props {
    /** What the text is, shown on the context chip ("Docker logs: web"). */
    source: string;
    /** Returns the text to explain, optionally with a more exact label. */
    text: () => string | { text: string; source: string };
    question?: string;
    host?: string | null;
    label?: string;
    /** Prefer text the user selected inside this element. */
    within?: HTMLElement | null;
  }
  let { source, text, question = "What does this show? Is anything wrong, and how do I fix it?", host = null, label = "Explain", within = null }: Props = $props();

  function selectionInside(): string {
    if (!within) return "";
    const sel = window.getSelection();
    if (!sel || sel.isCollapsed || !sel.anchorNode || !within.contains(sel.anchorNode)) return "";
    return currentSelection();
  }

  function go() {
    const picked = selectionInside();
    const got = picked ? { text: picked, source: `${source}, selected lines` } : text();
    const o = typeof got === "string" ? { text: got, source } : got;
    void helper.explain({ ...o, question, host });
  }
</script>

{#if helper.enabled}
  <button type="button" class="btn ghost tiny explain" title="Ask DRIFT about this (stays on this computer)" disabled={helper.busy} onclick={go}>
    <span class="spark" aria-hidden="true">✦</span>{label}
  </button>
{/if}

<style>
  .explain {
    color: var(--cyan);
  }
  .spark {
    margin-right: 4px;
    color: var(--magenta);
  }
</style>
