<script lang="ts">
  /** Sandbox clones (M12c): copy a guest, cut it off from the network, give it an expiry. */
  import Modal from "../Modal.svelte";
  import { api, errorText, type PveIsolation } from "../../api";
  import { validGuestName, vmidFree, type PveGuest, type PveSnapshot } from "../../proxmox";
  import { bridgeChoices, LIFETIMES, sandboxName, sandboxOf } from "../../sandbox";
  import { proxmox } from "../../stores/proxmox.svelte";
  import { system } from "../../stores/system.svelte";

  const m = $derived.by(() => {
    const x = proxmox.make;
    return x && x.kind === "sandbox" ? x : null;
  });
  const o = $derived(proxmox.overview);
  const info = $derived(proxmox.createInfo);
  const sources = $derived((o?.guests ?? []).filter((g) => !sandboxOf(g)));

  let sourceId = $state(0);
  let snapshot = $state("");
  let snaps = $state<PveSnapshot[] | null>(null);
  let snapErr = $state<string | null>(null);
  let vmid = $state(0);
  let idTouched = false;
  let name = $state("");
  let nameTouched = false;
  let full = $state(true);
  let storage = $state("");
  let isolation = $state<PveIsolation>("cut");
  let bridge = $state("");
  let hours = $state(24);
  let start = $state(false);
  let busy = $state(false);
  let err = $state<string | null>(null);

  const src = $derived<PveGuest | null>(sources.find((g) => g.vmid === sourceId) ?? null);

  let openedFor: unknown = null;
  $effect(() => {
    if (m && m !== openedFor) {
      openedFor = m;
      err = null;
      busy = false;
      sourceId = m.guest?.vmid ?? 0;
      snapshot = "";
      idTouched = false;
      nameTouched = false;
      isolation = "cut";
      hours = 24;
      start = false;
      storage = "";
    } else if (!m) openedFor = null;
  });

  // A new source: fresh name, clone type and snapshot list.
  let snapsFor = 0;
  $effect(() => {
    const g = src;
    if (!g || g.vmid === snapsFor) return;
    snapsFor = g.vmid;
    if (!nameTouched) name = sandboxName(g);
    full = !g.template;
    snapshot = "";
    snaps = null;
    snapErr = null;
    const id = proxmox.hostId;
    if (id === null) return;
    system
      .withSudo("list snapshots", (s) => api.pveSnapshots(id, g.node, g.kind, g.vmid, s), proxmox.host?.name)
      .then((r) => {
        if (snapsFor === g.vmid) snaps = r.filter((x) => x.name !== "current");
      })
      .catch((e) => {
        if (snapsFor === g.vmid) snapErr = errorText(e);
      });
  });
  $effect(() => {
    if (!m) snapsFor = 0;
  });

  $effect(() => {
    if (info && !idTouched) vmid = Math.max(info.nextId, 100);
    const bs = bridgeChoices(info?.bridges ?? []);
    if (bs.length && !bs.some((b) => b.name === bridge)) bridge = bs[0].name;
  });
  const stores = $derived(src ? proxmox.storagesFor(src.kind === "lxc" ? "rootdir" : "images") : []);
  const bridges = $derived(bridgeChoices(info?.bridges ?? []));
  const pickedBridge = $derived(bridges.find((b) => b.name === bridge) ?? null);

  // A snapshot copy is always full; a linked clone needs a template.
  $effect(() => {
    if (snapshot || (src && !src.template)) full = true;
  });

  const idOk = $derived(vmidFree(o, vmid));
  const ok = $derived(!!src && idOk && validGuestName(name) && (isolation === "cut" || !!bridge));
  const ctRunning = $derived(src?.kind === "lxc" && src.status === "running" && !snapshot);

  function submit() {
    if (!m || !src || !ok || busy) return;
    busy = true;
    err = null;
    proxmox
      .makeSandbox({
        node: src.node,
        kind: src.kind,
        vmid: src.vmid,
        newid: vmid,
        name,
        snapshot: snapshot || null,
        full,
        storage: full && storage ? storage : null,
        isolation,
        bridge: isolation === "bridge" ? bridge : null,
        hours,
        start,
      })
      .catch((e) => (err = errorText(e)))
      .finally(() => (busy = false));
  }
</script>

<Modal open={!!m} title={src ? `SANDBOX OF ${(src.name || String(src.vmid)).toUpperCase()}` : "NEW SANDBOX"} width={620} onclose={() => (proxmox.make = null)}>
  {#if m}
    <form class="form" data-guide="proxmox-sandbox-make" onsubmit={(e) => (e.preventDefault(), submit())}>
      <p class="muted small">A copy to try an upgrade or a risky change on. It starts cut off from your network, so it can't clash with the original, and it's destroyed when it expires.</p>
      {#if proxmox.createInfoError}<p class="error-text small pre">{proxmox.createInfoError}</p>{/if}
      <div class="row3">
        <label class="field span2"
          ><span class="label">COPY OF</span>
          <select class="input" bind:value={sourceId}>
            <option value={0} disabled>Pick a guest…</option>
            {#each sources as g (g.vmid)}<option value={g.vmid}>{g.vmid} · {g.name || "(no name)"}{g.template ? " (template)" : ""} on {g.node}</option>{/each}
          </select>
        </label>
        <label class="field"
          ><span class="label">STATE</span>
          <select class="input" bind:value={snapshot} disabled={!src}>
            <option value="">As it is now</option>
            {#each snaps ?? [] as s (s.name)}<option value={s.name}>Snapshot {s.name}</option>{/each}
          </select>
        </label>
      </div>
      {#if snapErr}<p class="error-text small">Snapshots: {snapErr}</p>{/if}
      <div class="row3">
        <label class="field span2"><span class="label">NAME</span><input class="input mono" bind:value={name} oninput={() => (nameTouched = true)} spellcheck="false" /></label>
        <label class="field"><span class="label">NEW ID</span><input class="input mono" type="number" min="100" bind:value={vmid} oninput={() => (idTouched = true)} /></label>
      </div>
      {#if name && !validGuestName(name)}<p class="error-text small">Letters, digits, - and . only.</p>{/if}
      {#if vmid && !idOk}<p class="error-text small">That id is taken or out of range.</p>{/if}

      {#if src}
        <div class="checks">
          <label class="check"><input type="radio" name="sbfull" checked={full} onchange={() => (full = true)} /> Full copy</label>
          <label class="check" title={!src.template ? "Only templates can be linked-cloned" : snapshot ? "A snapshot copy is always full" : ""}
            ><input type="radio" name="sbfull" checked={!full} disabled={!src.template || !!snapshot} onchange={() => (full = false)} /> Linked (templates only: fast, shares the base disk)</label
          >
        </div>
        {#if full && stores.length}
          <label class="field narrow"><span class="label">DISKS ON</span><select class="input" bind:value={storage}><option value="">Same as the original</option>{#each stores as s (s)}<option value={s}>{s}</option>{/each}</select></label>
        {/if}
      {/if}

      <fieldset class="iso">
        <legend class="label">NETWORK</legend>
        <label class="check"><input type="radio" name="sbiso" checked={isolation === "cut"} onchange={() => (isolation = "cut")} /> Cut off: every network card's cable pulled</label>
        <label class="check"><input type="radio" name="sbiso" checked={isolation === "bridge"} disabled={!bridges.length} onchange={() => (isolation = "bridge")} /> On a separate bridge, so sandboxes can reach each other</label>
        {#if isolation === "bridge"}
          <select class="input" bind:value={bridge} aria-label="Bridge">{#each bridges as b (b.name)}<option value={b.name}>{b.label}</option>{/each}</select>
          {#if pickedBridge && !pickedBridge.isolated}<p class="warn small">{bridge} has physical ports: the sandbox can reach that network. Make a bridge with no ports under NETWORK for a true lab.</p>{/if}
          {#if src && proxmox.localNode && src.node !== proxmox.localNode}<p class="muted small">Bridges are listed as on {proxmox.localNode}; the sandbox lands on {src.node}.</p>{/if}
        {/if}
      </fieldset>

      <div class="row3">
        <label class="field span2"><span class="label">DESTROY AFTER</span><select class="input" bind:value={hours}>{#each LIFETIMES as l (l.hours)}<option value={l.hours}>{l.label}</option>{/each}</select></label>
        <label class="check startck"><input type="checkbox" bind:checked={start} /> Start it when ready</label>
      </div>
      <p class="muted small">PCI and USB passthrough is removed from the copy, and so are a container's bind mounts of the node's folders, so it never touches the original's devices or data. The node destroys the sandbox on time even while Nightgrid is closed.</p>
      {#if ctRunning}<p class="warn small">Proxmox copies a running container only from a snapshot. Pick one, or stop {src?.name} first.</p>{/if}
      {#if err}<p class="error-text small pre">{err}</p>{/if}
      <button type="submit" hidden aria-hidden="true" tabindex="-1"></button>
    </form>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (proxmox.make = null)}>Cancel</button>
    <button type="button" class="btn primary" data-guide="proxmox-sandbox-go" disabled={busy || !ok || ctRunning} onclick={submit}>{busy ? "Starting…" : "Make sandbox"}</button>
  {/snippet}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .narrow {
    max-width: 260px;
  }
  .row3 {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
    align-items: end;
  }
  .span2 {
    grid-column: span 2;
  }
  .checks {
    display: flex;
    gap: 14px;
    flex-wrap: wrap;
  }
  .check {
    display: inline-flex;
    gap: 6px;
    align-items: center;
    font-size: var(--fs-sm);
    color: var(--text-1);
  }
  .startck {
    padding-bottom: 8px;
  }
  .iso {
    display: flex;
    flex-direction: column;
    gap: 6px;
    border: 1px solid var(--line);
    padding: 8px 10px;
    margin: 0;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .pre {
    white-space: pre-wrap;
  }
  .warn {
    color: var(--warn);
  }
  p {
    margin: 0;
  }
</style>
