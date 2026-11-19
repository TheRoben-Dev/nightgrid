<script lang="ts">
  /** Sandbox clones (M12c): every Nightgrid sandbox in the cluster with its expiry. */
  import { onDestroy } from "svelte";
  import { formatBytes, type PveGuest } from "../../proxmox";
  import { EXTEND, expiryText, expiryTone, ownTags, sandboxes } from "../../sandbox";
  import { proxmox } from "../../stores/proxmox.svelte";

  let { screen, consoleOf }: { screen: (g: PveGuest) => void; consoleOf: (g: PveGuest) => void } = $props();

  let now = $state(Date.now() / 1000);
  const tick = setInterval(() => (now = Date.now() / 1000), 30_000);
  onDestroy(() => clearInterval(tick));

  const list = $derived(sandboxes(proxmox.overview?.guests ?? []));
  const nameOf = (vmid: number | null) => (vmid == null ? "?" : (proxmox.overview?.guests.find((g) => g.vmid === vmid)?.name ?? `${vmid} (gone)`));
  let extendFor = $state<number | null>(null);
</script>

<div class="sbx" data-guide="proxmox-sandbox">
  <div class="bar">
    <p class="muted small">Copies to try something on, cut off from your network and destroyed when they expire. Make one here or from a guest's ⋯ menu.</p>
    <span class="sp"></span>
    <button type="button" class="btn tiny" data-guide="proxmox-sandbox-new" disabled={!proxmox.localNode} onclick={() => proxmox.open({ kind: "sandbox", guest: null })}>+ Sandbox</button>
  </div>
  {#if proxmox.sandboxNotes}
    <div class="notes" role="status">
      <strong class="small">Sandbox {proxmox.sandboxNotes.vmid}:</strong>
      <ul>{#each proxmox.sandboxNotes.notes as n (n)}<li class="small">{n}</li>{/each}</ul>
      <button type="button" class="x" aria-label="Dismiss" onclick={() => (proxmox.sandboxNotes = null)}>×</button>
    </div>
  {/if}
  <div class="scroll">
    {#each list as s (s.guest.vmid)}
      {@const g = s.guest}
      {@const tone = expiryTone(s.until, now)}
      {@const local = proxmox.overview?.nodes.find((n) => n.name === g.node)?.local}
      <div class="row" data-guide="proxmox-sandbox-row">
        <span class="mono">{g.vmid}</span>
        <span class="name">
          <span class="kind">{g.kind === "qemu" ? "VM" : "CT"}</span>
          <button type="button" class="gname" onclick={() => proxmox.openGuest(g, "hardware")} title="Hardware: reconnect its network when you keep it">{g.name || "(no name)"}</button>
          {#each ownTags(g.tags) as t (t)}<span class="tag">{t}</span>{/each}
          <span class="muted small">copy of {nameOf(s.source)} on {g.node}</span>
        </span>
        <span class="state {g.status === 'running' ? 'ok' : 'muted'}">{g.status.toUpperCase()}</span>
        <span class="small {tone}" title={s.until ? new Date(s.until * 1000).toLocaleString() : ""}>{expiryText(s.until, now)}</span>
        <span class="muted small data">{formatBytes(g.maxdisk)}</span>
        <span class="acts">
          {#if g.status === "stopped"}
            <button type="button" class="btn tiny ghost" disabled={proxmox.busy.includes(g.vmid)} onclick={() => proxmox.request(g, "start")}>Start</button>
          {:else if g.status === "running" && local}
            {#if g.kind === "qemu"}<button type="button" class="btn tiny ghost" onclick={() => screen(g)}>Screen</button>{/if}
            <button type="button" class="btn tiny ghost" onclick={() => consoleOf(g)}>Console</button>
          {/if}
          <span class="pop">
            <button type="button" class="btn tiny ghost" aria-expanded={extendFor === g.vmid} disabled={proxmox.busy.includes(g.vmid)} onclick={() => (extendFor = extendFor === g.vmid ? null : g.vmid)}>Expiry ▾</button>
            {#if extendFor === g.vmid}
              <ul class="menu" role="menu">
                {#each EXTEND as e (e.hours)}
                  <li><button type="button" role="menuitem" onclick={() => { extendFor = null; void proxmox.extendSandbox(g, e.hours); }}>{e.label}</button></li>
                {/each}
              </ul>
            {/if}
          </span>
          <button type="button" class="btn tiny ghost" title="Keep it as an ordinary guest" onclick={() => proxmox.askKeepSandbox(g)}>Keep…</button>
          <button type="button" class="btn tiny ghost danger" onclick={() => proxmox.askDestroySandbox(g)}>Destroy…</button>
        </span>
      </div>
    {:else}
      <p class="muted small pad">No sandboxes. Make one to try an upgrade on a copy first.</p>
    {/each}
    {#if list.some((s) => s.until != null && s.until < now)}
      <p class="warn small pad">An overdue sandbox lost its timer, usually because the node restarted. Destroy it, or set a new expiry to arm it again.</p>
    {/if}
  </div>
</div>

<style>
  .sbx {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
    flex: 1;
  }
  .bar {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .bar p {
    margin: 0;
  }
  .sp {
    flex: 1;
  }
  .notes {
    position: relative;
    border: 1px solid var(--line);
    padding: 6px 28px 6px 10px;
  }
  .notes ul {
    margin: 4px 0 0;
    padding-left: 18px;
  }
  .x {
    position: absolute;
    top: 4px;
    right: 6px;
    background: none;
    border: 0;
    color: var(--text-2);
    cursor: pointer;
  }
  .scroll {
    overflow: auto;
    min-height: 0;
  }
  .row {
    display: grid;
    grid-template-columns: 70px minmax(200px, 1fr) 90px 170px 80px auto;
    gap: 10px;
    align-items: center;
    padding: 6px 4px;
    border-bottom: 1px solid var(--line);
  }
  .name {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
    min-width: 0;
  }
  .kind {
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .gname {
    background: none;
    border: 0;
    padding: 0;
    color: var(--text-0);
    font: inherit;
    cursor: pointer;
  }
  .gname:hover {
    color: var(--cyan);
  }
  .tag {
    font-size: var(--fs-xs);
    border: 1px solid var(--line);
    padding: 0 var(--tag-px);
    color: var(--text-2);
    display: inline-block;
    line-height: calc(var(--tag-h) - 2px);
    white-space: nowrap;
  }
  .acts {
    display: flex;
    gap: 4px;
    justify-content: flex-end;
    align-items: center;
  }
  .pop {
    position: relative;
  }
  .menu {
    position: absolute;
    right: 0;
    top: 100%;
    z-index: 5;
    list-style: none;
    margin: 2px 0 0;
    padding: 4px 0;
    background: var(--bg-1);
    border: 1px solid var(--line);
    min-width: 160px;
  }
  .menu button {
    display: block;
    width: 100%;
    text-align: left;
    background: none;
    border: 0;
    padding: 4px 10px;
    color: var(--text-1);
    font: inherit;
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--surface-2, var(--bg-1));
  }
  .mono {
    font-family: var(--font-mono);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .ok {
    color: var(--ok);
  }
  .warn {
    color: var(--warn);
  }
  .err,
  .danger {
    color: var(--danger);
  }
  .pad {
    padding: 10px 4px;
  }
</style>
