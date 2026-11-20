<script lang="ts">
  import { typedMatches } from "../../confirm";
  import Modal from "../Modal.svelte";
  import RdpDialog from "../desktop/RdpDialog.svelte";
  import { desktops } from "../../stores/desktops.svelte";
  import { system } from "../../stores/system.svelte";
  import { vmActions, vmCounts, vmKey, vmTone, vms, VM_ACTION_WORD, VM_CONSEQUENCE, VM_KIND_LABEL } from "../../stores/vms.svelte";

  const r = $derived(vms.report);
  const counts = $derived(vmCounts(r));
  const hostName = $derived(system.host?.name ?? "");
  const pending = $derived(vms.pending);
  let typed = $state("");
  $effect(() => {
    if (!vms.pending) typed = "";
  });
  const needsTyping = $derived(pending?.action === "stop");
  const mem = (mb: number | null) => (mb == null ? "—" : mb >= 1024 ? `${(mb / 1024).toFixed(mb % 1024 ? 1 : 0)} GB` : `${mb} MB`);
</script>

<div class="vms" data-guide="system-vms">
  <div class="bar">
    {#if r}
      <span class="sum"><span class="mint">{counts.running}</span> / {counts.total} RUNNING{#if r.proxmox}<span class="tag">PROXMOX</span>{/if}{#if r.libvirt}<span class="tag">LIBVIRT</span>{/if}</span>
    {/if}
    <span class="spacer"></span>
    {#if r?.needsRoot && !vms.elevated}
      <button type="button" class="btn ghost tiny" data-guide="vms-root" onclick={() => vms.load(true)} disabled={vms.loading}>Read as root</button>
    {/if}
    {#if r || vms.loading}
      <button type="button" class="btn ghost tiny" onclick={() => vms.load()} disabled={vms.loading}>{vms.loading ? "Reading…" : "Refresh"}</button>
    {/if}
  </div>
  {#if vms.notice}
    <p class="notice {vms.notice.tone}" role="status">
      {vms.notice.text}<button type="button" class="x" aria-label="Dismiss" onclick={() => (vms.notice = null)}>×</button>
    </p>
  {/if}
  {#if vms.error}<p class="error-text">{vms.error}</p>{/if}
  {#each r?.notes ?? [] as n (n)}<p class="muted note">{n}</p>{/each}

  <div class="grid">
    {#if !r && vms.loading}
      <p class="muted">Looking for virsh, qm and pct…</p>
    {:else if r && !r.libvirt && !r.proxmox}
      <div class="empty">
        <p class="label">NO HYPERVISOR</p>
        <p class="muted">
          Nothing here runs libvirt (<code>virsh</code>) or Proxmox VE (<code>qm</code> / <code>pct</code>). Containers live in the Docker view.
        </p>
      </div>
    {:else if r && !r.vms.length && !r.needsRoot}
      <p class="muted">No guests defined.</p>
    {/if}
    {#each r?.vms ?? [] as v (vmKey(v))}
      {@const busy = vms.busy.includes(vmKey(v))}
      <article class="card {vmTone(v)}">
        <header>
          <span class="kind">{VM_KIND_LABEL[v.kind]}</span>
          <span class="name" title={v.name}>{v.name}</span>
          {#if v.kind !== "libvirt"}<span class="muted">#{v.id}</span>{/if}
          <span class="state"><span class="dot" aria-hidden="true"></span>{v.state.toUpperCase()}</span>
        </header>
        <dl>
          <div><dt>VCPU</dt><dd>{v.vcpus ?? "—"}</dd></div>
          <div><dt>MEMORY</dt><dd>{mem(v.memoryMb)}</dd></div>
          <div><dt>AUTOSTART</dt><dd>{v.autostart == null ? "—" : v.autostart ? "yes" : "no"}</dd></div>
        </dl>
        <footer>
          {#each vmActions(v) as a (a)}
            <button type="button" class="btn tiny {a === 'start' ? 'primary' : a === 'stop' ? 'danger' : 'ghost'}" disabled={busy} onclick={() => vms.request(v, a)}>{VM_ACTION_WORD[a]}</button>
          {/each}
          <span class="spacer"></span>
          {#if v.kind !== "lxc"}
            <button
              type="button"
              class="btn ghost tiny"
              data-guide="vms-screen"
              disabled={v.state !== "running" && v.state !== "paused"}
              title="The VM's screen in the Desktops view (VNC over SSH)"
              onclick={() => desktops.open(v)}>Screen</button
            >
            <button
              type="button"
              class="btn ghost tiny"
              data-guide="vms-rdp"
              disabled={v.state !== "running"}
              title="Remote Desktop through an SSH tunnel, in this computer's RDP client"
              onclick={() => desktops.askRdp(v)}>RDP</button
            >
          {/if}
          <button
            type="button"
            class="btn ghost tiny"
            disabled={v.state !== "running"}
            title={v.kind === "lxc" ? "Shell inside the container (pct enter)" : "Serial console (needs a console on the guest)"}
            onclick={() => vms.console(v)}>{v.kind === "lxc" ? "Shell" : "Console"}</button
          >
        </footer>
      </article>
    {/each}
  </div>
</div>

<RdpDialog />

<Modal open={!!pending} title={pending ? `${VM_ACTION_WORD[pending.action].toUpperCase()} GUEST` : ""} tone="danger" width={520} onclose={() => (vms.pending = null)}>
  {#if pending}
    <p>{VM_ACTION_WORD[pending.action]} <code>{pending.vm.name}</code>{#if pending.vm.kind !== "libvirt"} (#{pending.vm.id}){/if} on <strong>{hostName}</strong>?</p>
    <p class="muted">{VM_CONSEQUENCE[pending.action] ?? ""}</p>
    {#if needsTyping}
      <label class="field">
        <span>Type <strong class="verbatim">{pending.vm.name}</strong> to confirm</span>
        <input class="input" bind:value={typed} autocomplete="off" spellcheck="false" />
      </label>
    {/if}
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (vms.pending = null)}>Cancel</button>
    <button
      type="button"
      class="btn danger"
      disabled={!pending || (needsTyping && !typedMatches(typed, pending.vm.name))}
      onclick={() => pending && vms.act(pending.vm, pending.action)}>{pending ? `${VM_ACTION_WORD[pending.action]} ${pending.vm.name}` : ""}</button
    >
  {/snippet}
</Modal>

<style>
  .vms {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  .bar {
    display: flex;
    gap: 10px;
    align-items: center;
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
  }
  .spacer {
    flex: 1;
  }
  .mint {
    color: var(--mint);
  }
  .tag {
    margin-left: 8px;
    padding: 0 var(--tag-px);
    border: 1px solid var(--line);
    color: var(--cyan);
    display: inline-block;
    line-height: calc(var(--tag-h) - 2px);
    white-space: nowrap;
  }
  .note {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .notice {
    margin: 0;
    padding: 4px 8px;
    font-size: var(--fs-xs);
    border: 1px solid var(--line);
    display: flex;
    justify-content: space-between;
  }
  .notice.ok {
    color: var(--mint);
  }
  .notice.err {
    color: var(--danger);
  }
  .x {
    border: 0;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
  .grid {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 8px;
    align-content: start;
  }
  .empty {
    padding: 16px;
    display: grid;
    gap: 8px;
  }
  .empty p {
    margin: 0;
  }
  code {
    color: var(--cyan);
  }
  .card {
    border: 1px solid var(--line);
    background: var(--bg-1);
    padding: 8px 10px;
    display: grid;
    gap: 8px;
    border-left: 3px solid var(--text-2);
  }
  .card.ok {
    border-left-color: var(--ok);
  }
  .card.warn {
    border-left-color: var(--warn);
  }
  header {
    display: flex;
    gap: 8px;
    align-items: baseline;
    min-width: 0;
  }
  .kind {
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    color: var(--cyan);
  }
  .name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }
  .state {
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    display: flex;
    gap: 5px;
    align-items: center;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text-2);
  }
  .ok .dot {
    background: var(--ok);
    box-shadow: 0 0 6px var(--ok);
  }
  .ok .state {
    color: var(--ok);
  }
  .warn .dot {
    background: var(--warn);
  }
  .warn .state {
    color: var(--warn);
  }
  dl {
    margin: 0;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 4px;
  }
  dt {
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    color: var(--text-2);
  }
  dd {
    margin: 0;
    font-variant-numeric: tabular-nums;
  }
  footer {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .field {
    display: grid;
    gap: 6px;
    margin-top: 10px;
  }
</style>
