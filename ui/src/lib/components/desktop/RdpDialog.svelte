<script lang="ts">
  /** RDP to a guest (M10n): pick its address, then tunnel and open the system RDP client. */
  import { desktops } from "../../stores/desktops.svelte";
  import Modal from "../Modal.svelte";

  const a = $derived(desktops.rdpAsk);
  function set(p: { addr?: string; port?: number }) {
    if (desktops.rdpAsk) desktops.rdpAsk = { ...desktops.rdpAsk, ...p };
  }
</script>

<Modal open={!!a} title="REMOTE DESKTOP (RDP)" width={540} onclose={() => (desktops.rdpAsk = null)}>
  {#if a}
    <p>
      Connect to <code>{a.vm.name}</code> on <strong>{a.hostName}</strong> with this computer's RDP client. Nightgrid opens a tunnel from a free local port through
      {a.hostName}'s SSH connection to the guest, so nothing new is exposed on the network.
    </p>
    {#if a.loading}
      <p class="muted">Looking up the guest's address…</p>
    {:else}
      {#if a.found?.addrs.length}
        <div class="addrs">
          {#each a.found.addrs as ip (ip)}
            <button type="button" class="chip" class:on={a.addr === ip} onclick={() => set({ addr: ip })}>{ip}</button>
          {/each}
        </div>
      {/if}
      {#if a.found?.note}<p class="muted small">{a.found.note}</p>{/if}
      <div class="row">
        <label class="field grow">
          <span>Guest address</span>
          <input class="input" value={a.addr} oninput={(e) => set({ addr: e.currentTarget.value })} placeholder="192.168.122.45" autocomplete="off" spellcheck="false" />
        </label>
        <label class="field port">
          <span>Port</span>
          <input class="input" type="number" min="1" max="65535" value={a.port} oninput={(e) => set({ port: Number(e.currentTarget.value) || 3389 })} />
        </label>
      </div>
      <p class="muted small">
        Remote Desktop has to be enabled in the guest. Sign in with the guest's own account in the RDP client; Nightgrid doesn't see or store it. The tunnel stays up until you stop it in the Desktops view.
      </p>
    {/if}
    {#if a.error}<p class="error-text">{a.error}</p>{/if}
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (desktops.rdpAsk = null)}>Cancel</button>
    <button type="button" class="btn primary" data-guide="desktop-rdp-start" disabled={!a || a.loading || a.busy || !a.addr.trim()} onclick={() => desktops.startRdp()}
      >{a?.busy ? "Opening…" : "Open RDP"}</button
    >
  {/snippet}
</Modal>

<style>
  .addrs {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    margin: 6px 0;
  }
  .chip {
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    padding: 0 var(--chip-px);
    font-family: var(--font-mono);
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: var(--sp-2);
    min-height: var(--chip-h);
    font-size: var(--fs-xs);
    white-space: nowrap;
  }
  .chip.on {
    color: var(--magenta);
    border-color: var(--magenta);
  }
  .row {
    display: flex;
    gap: 10px;
  }
  .grow {
    flex: 1;
  }
  .port {
    width: 100px;
  }
  .small {
    font-size: var(--fs-xs);
  }
</style>
