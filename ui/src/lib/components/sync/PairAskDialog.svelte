<script lang="ts">
  /**
   * A device on the LAN asked this one for a pairing code (D-208): show it
   * big, wherever the user is. Typing it there pairs the two.
   */
  import { onMount } from "svelte";
  import { scramble } from "../../scramble";
  import { sync } from "../../stores/sync.svelte";
  import Modal from "../Modal.svelte";

  let nowMs = $state(Date.now());
  onMount(() => {
    const t = setInterval(() => {
      nowMs = Date.now();
      // Paired (the code is used up) or expired: close.
      if (sync.asked) void sync.refresh().then(() => {
        if (sync.asked && !sync.info?.pairing) sync.asked = null;
      });
    }, 1500);
    return () => clearInterval(t);
  });
  const left = $derived(sync.asked ? Math.max(0, 300 - Math.round((nowMs - sync.asked.at) / 1000)) : 0);
  async function cancel() {
    await sync.pairCancel();
    sync.asked = null;
  }
</script>

<Modal open={!!sync.asked} title="PAIRING REQUEST" width={460} onclose={() => (sync.asked = null)}>
  {#if sync.asked}
    <p><b>{sync.asked.from}</b> on your network wants to pair with this device. If that's you, type this code there:</p>
    <p class="code mono" use:scramble={sync.asked.code}></p>
    <p class="muted small">Valid {Math.floor(left / 60)}:{String(left % 60).padStart(2, "0")}. Not you? Cancel, and nothing happens.</p>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => void cancel()}>Cancel</button>
    <button type="button" class="btn" onclick={() => (sync.asked = null)}>Hide</button>
  {/snippet}
</Modal>

<style>
  .code {
    font-size: 2em;
    letter-spacing: 0.2em;
    color: var(--cyan);
    text-align: center;
    margin: 12px 0;
  }
  .muted {
    color: var(--text-1);
  }
  .small {
    font-size: var(--fs-xs);
  }
</style>
