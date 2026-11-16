<script lang="ts">
  /** Proxmox → NETWORK (M10r part 5): the node's bridges, bonds and VLANs, staged then applied with a rollback. */
  import { onDestroy } from "svelte";
  import { BOND_MODES, ifaceProblem, ifaceSpec, newIface, type PveIface, type PveIfaceSpec } from "../../proxmox";
  import { proxmox } from "../../stores/proxmox.svelte";

  $effect(() => {
    if (proxmox.overview && !proxmox.network && !proxmox.networkError) void proxmox.loadNetwork();
  });
  const n = $derived(proxmox.network);
  const node = $derived(proxmox.localNode);

  let edit = $state<{ spec: PveIfaceSpec; create: boolean } | null>(null);
  let saving = $state(false);
  const problem = $derived(edit ? ifaceProblem(edit.spec) : null);
  const physical = $derived((n?.ifaces ?? []).filter((i) => i.kind === "eth" || i.kind === "bond").map((i) => i.iface));

  function open(i: PveIface) {
    edit = { spec: ifaceSpec(i), create: false };
  }
  function add(kind: "bridge" | "bond" | "vlan") {
    edit = { spec: newIface(kind, n?.ifaces ?? []), create: true };
  }
  async function save() {
    if (!edit || problem) return;
    saving = true;
    try {
      await proxmox.saveIface(edit.spec, edit.create);
      edit = null;
    } catch {
      /* shown as a notice */
    } finally {
      saving = false;
    }
  }
  /** The bridge or bond that already uses a port, other than the one being edited. */
  function usedBy(port: string, self: string): string | null {
    return n?.ifaces.find((i) => i.iface !== self && (i.kind === "bridge" || i.kind === "bond") && i.ports.split(/\s+/).includes(port))?.iface ?? null;
  }
  function togglePort(p: string) {
    if (!edit) return;
    const ports = edit.spec.ports.split(/\s+/).filter(Boolean);
    edit.spec.ports = (ports.includes(p) ? ports.filter((x) => x !== p) : [...ports, p]).join(" ");
  }

  // Countdown for an applied change.
  let now = $state(Date.now());
  const tick = setInterval(() => (now = Date.now()), 1000);
  onDestroy(() => clearInterval(tick));
  const left = $derived(proxmox.netApply ? Math.max(0, Math.round((proxmox.netApply.until - now) / 1000)) : 0);

  const diffLines = $derived((n?.diff ?? "").split("\n"));
  const lineClass = (l: string) => (l.startsWith("+++") || l.startsWith("---") ? "meta" : l.startsWith("+") ? "add" : l.startsWith("-") ? "del" : l.startsWith("@@") ? "meta" : "");
</script>

<div class="scroll" data-guide="proxmox-network">
  {#if proxmox.networkError}<p class="error-text small pre pad">{proxmox.networkError}</p>{/if}
  {#if proxmox.netApply}
    {@const a = proxmox.netApply}
    <div class="armed" role="alert">
      <strong>KEEP THE NEW NETWORK?</strong>
      <span class="small">{a.node} rolls back by itself in <span class="data">{left}</span> s unless you keep it. If this view still answers, the node is reachable.</span>
      <span class="sp"></span>
      <button type="button" class="btn tiny ghost" onclick={() => proxmox.settleNetwork(false)}>Roll back now</button>
      <button type="button" class="btn tiny primary" data-guide="proxmox-network-keep" disabled={left === 0} onclick={() => proxmox.settleNetwork(true)}>Keep</button>
    </div>
  {:else if n?.rollbackArmed}
    <p class="warn small pad">A network rollback is armed on {node}: it restores the previous config shortly, unless it's kept from the window that applied it.</p>
  {/if}
  {#if !n}
    <p class="muted small pad">{proxmox.networkError ? "" : "Reading…"}</p>
  {:else}
    {#if n.diff}
      <div class="staged">
        <div class="hd">
          STAGED CHANGES <span class="muted small">(not live yet)</span>
          <span class="sp"></span>
          <button type="button" class="btn tiny ghost" onclick={() => proxmox.revertNetwork()}>Revert</button>
          <button type="button" class="btn tiny primary" data-guide="proxmox-network-apply" disabled={!!proxmox.netApply} onclick={() => proxmox.askApplyNetwork()}>Apply…</button>
        </div>
        <pre class="diff">{#each diffLines as l, i (i)}<span class="ln {lineClass(l)}">{l}</span>{/each}</pre>
      </div>
    {/if}
    <div class="hd">
      INTERFACES ON {node?.toUpperCase()}
      <span class="sp"></span>
      <button type="button" class="btn tiny" onclick={() => add("bridge")}>+ Bridge</button>
      <button type="button" class="btn tiny" onclick={() => add("bond")}>+ Bond</button>
      <button type="button" class="btn tiny" onclick={() => add("vlan")}>+ VLAN</button>
    </div>
    <div class="row headrow" aria-hidden="true"><span>NAME</span><span>TYPE</span><span>STATE</span><span>PORTS / SLAVES</span><span>ADDRESS</span><span>GATEWAY</span><span>COMMENT</span><span></span></div>
    {#each n.ifaces as i (i.iface)}
      <div class="row" class:off={!i.active}>
        <strong class="mono">{i.iface}</strong>
        <span class="muted">{i.kind}{#if i.vlanAware}<span class="tag">VLAN-AWARE</span>{/if}{#if i.vlanId}<span class="muted small">&nbsp;· tag {i.vlanId}</span>{/if}</span>
        <span class="small {i.active ? 'ok' : 'muted'}">{i.active ? "UP" : i.exists || i.kind !== "eth" ? "DOWN" : "MISSING"}{#if !i.autostart && i.kind !== "eth"}<span class="muted"> · manual</span>{/if}</span>
        <span class="mono small">{i.ports || (i.bondMode ? "" : "--")}{#if i.bondMode}<span class="muted"> · {i.bondMode}</span>{/if}</span>
        <span class="mono small">{i.cidr || "--"}{#if i.cidr6}<br />{i.cidr6}{/if}</span>
        <span class="mono small">{i.gateway || ""}</span>
        <span class="muted small ell" title={i.comments}>{i.comments}</span>
        <span class="acts">
          <button type="button" class="btn tiny ghost" onclick={() => open(i)}>Edit…</button>
          {#if i.kind !== "eth"}<button type="button" class="btn tiny ghost" onclick={() => proxmox.askDeleteIface(i.iface, !!(i.cidr || i.gateway))}>Remove…</button>{/if}
        </span>
      </div>
    {/each}

    {#if edit}
      {@const s = edit.spec}
      <form class="box" data-guide="proxmox-iface-edit" onsubmit={(e) => (e.preventDefault(), void save())}>
        <div class="line">
          <strong class="small">{edit.create ? `NEW ${s.kind.toUpperCase()}` : `EDIT ${s.iface}`}</strong>
          {#if edit.create}<input class="input mono name" bind:value={s.iface} aria-label="Name" spellcheck="false" />{/if}
          <label class="check"><input type="checkbox" bind:checked={s.autostart} /> start at boot</label>
          {#if s.kind === "bridge"}<label class="check" title="Guests can use any VLAN tag on this bridge"><input type="checkbox" bind:checked={s.vlanAware} /> VLAN aware</label>{/if}
        </div>
        {#if s.kind === "bridge" || s.kind === "bond"}
          <div class="line">
            <span class="label">{s.kind === "bond" ? "SLAVES" : "PORTS"}</span>
            {#each physical.filter((p) => p !== s.iface) as p (p)}
              {@const owner = usedBy(p, s.iface)}
              <label class="check" title={owner ? `Already a port of ${owner}` : ""}><input type="checkbox" checked={s.ports.split(/\s+/).includes(p)} onchange={() => togglePort(p)} /> <span class="mono">{p}</span>{#if owner}<span class="warn small">&nbsp;in {owner}</span>{/if}</label>
            {/each}
            {#if s.kind === "bridge"}<span class="muted small">none = an internal bridge for guests only</span>{/if}
          </div>
        {/if}
        {#if s.kind === "bond"}
          <div class="line">
            <span class="label">MODE</span>
            <select class="input" bind:value={s.bondMode}>{#each BOND_MODES as m (m)}<option value={m}>{m}</option>{/each}</select>
            <span class="muted small">{s.bondMode === "802.3ad" ? "LACP: the switch ports must be a LAG" : s.bondMode === "active-backup" ? "failover, works with any switch" : ""}</span>
          </div>
        {/if}
        <div class="line">
          <label class="field"><span class="label">IPv4 / PREFIX</span><input class="input mono" bind:value={s.cidr} placeholder="none" spellcheck="false" /></label>
          <label class="field"><span class="label">GATEWAY</span><input class="input mono" bind:value={s.gateway} placeholder="none" spellcheck="false" disabled={!s.cidr} /></label>
          <label class="field"><span class="label">MTU</span><input class="input" inputmode="numeric" placeholder="1500" value={s.mtu ?? ""} oninput={(e) => (s.mtu = e.currentTarget.value.trim() ? Number(e.currentTarget.value) : null)} /></label>
          <label class="field grow"><span class="label">COMMENT</span><input class="input" maxlength="200" bind:value={s.comments} /></label>
        </div>
        {#if !edit.create && (s.cidr !== n.ifaces.find((x) => x.iface === s.iface)?.cidr || s.gateway !== n.ifaces.find((x) => x.iface === s.iface)?.gateway) && n.ifaces.find((x) => x.iface === s.iface)?.gateway}
          <p class="warn small">This interface has the node's default gateway. If Nightgrid reaches {node} through it, applying a wrong address cuts the connection; the armed rollback then restores it after 90 s.</p>
        {/if}
        {#if problem}<p class="error-text small">{problem}</p>{/if}
        <div class="line">
          <span class="muted small">Staged only: nothing changes on {node} until you apply.</span>
          <span class="sp"></span>
          <button type="button" class="btn tiny ghost" onclick={() => (edit = null)}>Cancel</button>
          <button type="submit" class="btn tiny primary" disabled={!!problem || saving}>{saving ? "Staging…" : "Stage"}</button>
        </div>
      </form>
    {/if}
    <p class="muted small pad">Guests attach to bridges (vmbrN). A VLAN-aware bridge lets each guest NIC pick its own tag; a VLAN interface (vmbr0.20) gives the node itself an address on that VLAN.</p>
  {/if}
</div>

<style>
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  .pad {
    padding: 6px 10px;
    margin: 0;
  }
  .pre {
    white-space: pre-wrap;
  }
  .hd {
    padding: 10px 8px 4px;
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .sp {
    margin-left: auto;
  }
  .armed {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    margin: 6px 8px;
    padding: 8px 10px;
    border: 1px solid var(--magenta);
    color: var(--text-0);
  }
  .armed strong {
    color: var(--magenta);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
  }
  .staged {
    margin: 4px 8px;
    border: 1px solid var(--warn);
  }
  .diff {
    margin: 0;
    padding: 6px 8px;
    max-height: 220px;
    overflow: auto;
    font-size: var(--fs-xs);
    background: var(--bg-0);
  }
  .diff .ln {
    display: block;
    min-height: 1.2em;
  }
  .diff .add {
    color: var(--ok);
  }
  .diff .del {
    color: var(--danger);
  }
  .diff .meta {
    color: var(--text-1);
  }
  .row {
    display: grid;
    grid-template-columns: 110px 130px 90px minmax(120px, 1fr) 150px 110px minmax(80px, 1fr) 150px;
    gap: 8px;
    align-items: center;
    padding: 5px 8px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-sm);
    color: var(--text-0);
  }
  .row.off {
    opacity: 0.65;
  }
  .headrow {
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
  }
  .acts {
    display: flex;
    gap: 4px;
    justify-content: flex-end;
  }
  .box {
    margin: 8px;
    padding: 8px;
    border: 1px solid var(--cyan);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .line {
    display: flex;
    gap: 10px;
    align-items: end;
    flex-wrap: wrap;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .grow {
    flex: 1;
    min-width: 160px;
  }
  .name {
    width: 120px;
  }
  .check {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    font-size: var(--fs-sm);
    color: var(--text-1);
  }
  .tag {
    font-size: var(--fs-xs);
    border: 1px solid var(--line);
    padding: 0 4px;
    margin-left: 4px;
    color: var(--cyan);
  }
  .mono {
    font-family: var(--font-mono);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .ell {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ok {
    color: var(--ok);
  }
  .warn {
    color: var(--warn);
    margin: 0;
  }
  .error-text {
    margin: 0;
  }
</style>
