<script lang="ts">
  /** A host's ASCII icon, drawn in mono with the host colour (cyan when unset). */
  interface Props {
    art: string | null;
    color?: string | null;
    size?: "sm" | "md";
  }
  let { art, color = null, size = "sm" }: Props = $props();
</script>

{#if art}
  <pre class="ascii {size}" style:--c={color ?? "var(--cyan)"} aria-hidden="true">{art}</pre>
{/if}

<style>
  .ascii {
    margin: 0;
    font-family: var(--font-mono);
    line-height: 1.12;
    letter-spacing: 0;
    white-space: pre;
    overflow: hidden;
    max-width: 100%;
    /* Host colour fading toward magenta, like the app's active glow. */
    background: linear-gradient(180deg, var(--c) 30%, color-mix(in srgb, var(--c) 45%, var(--magenta)));
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    filter: drop-shadow(0 0 4px color-mix(in srgb, var(--c) 45%, transparent));
    user-select: none;
  }
  .sm {
    font-size: 10px;
  }
  .md {
    font-size: 13px;
  }
  :global(:root[data-effects="off"]) .ascii {
    filter: none;
  }
</style>
