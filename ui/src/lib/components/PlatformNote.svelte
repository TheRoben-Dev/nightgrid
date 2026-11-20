<script lang="ts">
  import { scramble } from "../scramble";
  import type { PlatformSupport } from "../api";

  interface Props {
    note: PlatformSupport;
    /** The host's name, for the heading. */
    host?: string;
  }
  let { note, host = "" }: Props = $props();

  const TITLE = {
    none: "NOT ON WINDOWS HOSTS",
    planned: "NOT ON WINDOWS HOSTS YET",
    partial: "ON WINDOWS HOSTS",
    full: "ON WINDOWS HOSTS",
  } as const;
  const blocking = $derived(note.level === "none" || note.level === "planned");
</script>

{#if blocking}
  <div class="block" role="note" data-guide="platform-note">
    <p class="label" use:scramble={TITLE[note.level]}></p>
    <h3>{note.label}{host ? ` · ${host}` : ""}</h3>
    <p>{note.why}</p>
    <p class="muted small">
      {host || "This host"} runs Windows (OpenSSH for Windows), so its shell is cmd or PowerShell rather than a Linux shell.
    </p>
  </div>
{:else}
  <p class="strip" role="note"><span class="tag">WINDOWS</span> {note.why}</p>
{/if}

<style>
  .block {
    max-width: 560px;
    margin: 32px auto;
    padding: 18px 20px;
    border: 1px solid color-mix(in srgb, var(--cyan, var(--mint)) 40%, var(--line));
    background: var(--bg-1);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  h3 {
    margin: 0;
    font-size: 15px;
  }
  p {
    margin: 0;
    line-height: 1.5;
  }
  .small {
    font-size: 12px;
  }
  .strip {
    margin: 0 0 8px;
    padding: 6px 10px;
    border-left: 2px solid var(--cyan, var(--mint));
    background: color-mix(in srgb, var(--cyan, var(--mint)) 8%, transparent);
    font-size: 12px;
    line-height: 1.45;
  }
  .tag {
    font-weight: 600;
    letter-spacing: 0.08em;
    margin-right: 6px;
  }
</style>
