<script lang="ts">
  /** One Proxmox guest (M10r part 2): its snapshots and backups. */
  import Modal from "../Modal.svelte";
  import PveHardware from "./PveHardware.svelte";
  import PveFirewall from "./PveFirewall.svelte";
  import { ago } from "../../gitstacks";
  import { scopeOf, backupStorages, formatBytes, nextVmid, restoreStorages, snapName, snapTree, validSnapname, type PveBackupFile } from "../../proxmox";
  import { proxmox } from "../../stores/proxmox.svelte";

  const d = $derived(proxmox.detail);
  const g = $derived(d?.guest);
  const o = $derived(proxmox.overview);

  // Take a snapshot.
  let name = $state("");
  let desc = $state("");
  let vmstate = $state(false);
  let taking = $state(false);
  let lastFor = -1;
  $effect(() => {
    if (g && g.vmid !== lastFor) {
      lastFor = g.vmid;
      name = snapName();
      desc = "";
      vmstate = false;
      restoreOf = null;
    }
  });
  const nameOk = $derived(validSnapname(name) && !(d?.snaps ?? []).some((s) => s.name === name));
  async function take() {
    if (!g || !nameOk) return;
    taking = true;
    try {
      await proxmox.takeSnapshot(g, name, desc, vmstate);
      name = snapName(new Date(Date.now() + 60_000));
      desc = "";
    } catch {
      /* shown as a notice */
    } finally {
      taking = false;
    }
  }

  // Back up now.
  const stores = $derived(g ? backupStorages(o, g.node) : []);
  let storage = $state("");
  let mode = $state("snapshot");
  let notes = $state("{{guestname}} manual");
  $effect(() => {
    if (stores.length && !stores.includes(storage)) storage = stores[0];
  });
  const files = $derived(g ? (proxmox.backups?.files ?? []).filter((f) => f.vmid === g.vmid) : []);

  // Restore.
  let restoreOf = $state<PveBackupFile | null>(null);
  let rVmid = $state(0);
  let rStorage = $state("");
  let rStart = $state(false);
  const rStores = $derived(g ? restoreStorages(o, g.node, g.kind) : []);
  function openRestore(f: PveBackupFile, asNew: boolean) {
    restoreOf = f;
    rVmid = asNew ? nextVmid(o) : f.vmid;
    rStorage = rStores[0] ?? "";
    rStart = false;
  }
</script>

<Modal open={!!d} title="{g?.kind === 'qemu' ? 'VM' : 'CT'} {g?.vmid ?? ''} · {g?.name.toUpperCase() ?? ''}" width={d?.tab === "hardware" || d?.tab === "firewall" ? 860 : 760} onclose={() => (proxmox.detail = null)}>
  {#if d && g}
    <div class="tabs" role="tablist" aria-label="Guest">
      <button type="button" role="tab" class:on={d.tab === "snapshots"} aria-selected={d.tab === "snapshots"} onclick={() => proxmox.guestTab("snapshots")}>SNAPSHOTS</button>
      <button type="button" role="tab" data-guide="proxmox-guest-backups" class:on={d.tab === "backups"} aria-selected={d.tab === "backups"} onclick={() => proxmox.guestTab("backups")}>BACKUPS</button>
      <button type="button" role="tab" data-guide="proxmox-hardware" class:on={d.tab === "hardware"} aria-selected={d.tab === "hardware"} onclick={() => proxmox.guestTab("hardware")}>HARDWARE</button>
      <button type="button" role="tab" data-guide="proxmox-guest-firewall" class:on={d.tab === "firewall"} aria-selected={d.tab === "firewall"} onclick={() => proxmox.guestTab("firewall")}>FIREWALL</button>
      <span class="muted small sp">{g.node} · {g.status}</span>
    </div>

    {#if d.tab === "snapshots"}
      <form class="take" data-guide="proxmox-snapshot" onsubmit={(e) => (e.preventDefault(), void take())}>
        <input class="input mono" maxlength="40" bind:value={name} aria-label="Snapshot name" spellcheck="false" />
        <input class="input grow" maxlength="200" bind:value={desc} placeholder="What's about to change (optional)" />
        {#if g.kind === "qemu" && g.status === "running"}<label class="check" title="Save the RAM too, so a rollback resumes the running VM"><input type="checkbox" bind:checked={vmstate} /> with RAM</label>{/if}
        <button type="submit" class="btn tiny primary" disabled={!nameOk || taking}>{taking ? "Starting…" : "Take snapshot"}</button>
      </form>
      {#if name && !validSnapname(name)}<p class="error-text small">A letter first, then letters, digits, _ or -; 2 to 40 characters.</p>{/if}
      {#if d.error}<p class="error-text small pre">{d.error}</p>{/if}
      {#if !d.snaps}
        <p class="muted small">Reading…</p>
      {:else if !d.snaps.length}
        <p class="muted small">No snapshots yet. Take one before an upgrade or a risky change; rolling back takes seconds.</p>
      {:else}
        <ul class="snaps">
          {#each snapTree(d.snaps) as { s, depth } (s.name)}
            <li style:padding-left="{depth * 18}px" class:here={s.current}>
              <span class="branch" aria-hidden="true">{depth ? "└" : "●"}</span>
              <strong class="mono">{s.name}</strong>
              {#if s.current}<span class="tag">YOU ARE HERE</span>{/if}
              {#if s.vmstate}<span class="tag muted" title="Includes the VM's RAM">RAM</span>{/if}
              <span class="muted small desc" title={s.description}>{s.description}</span>
              <span class="muted small nowrap">{ago(s.time)}</span>
              <button type="button" class="btn tiny ghost" onclick={() => proxmox.askRollback(g, s, false)}>Roll back…</button>
              <button type="button" class="btn tiny ghost" onclick={() => proxmox.askDeleteSnapshot(g, s)}>Delete…</button>
            </li>
          {/each}
        </ul>
      {/if}
    {:else if d.tab === "hardware"}
      <PveHardware />
    {:else if d.tab === "firewall"}
      <div class="fwbox"><PveFirewall fixed={scopeOf("guest", { node: g.node, kind: g.kind, vmid: g.vmid })} /></div>
    {:else}
      <div class="take">
        {#if stores.length}
          <select class="input" bind:value={storage} aria-label="Backup storage">{#each stores as s (s)}<option value={s}>{s}</option>{/each}</select>
          <select class="input" bind:value={mode} aria-label="Mode" title="snapshot: no downtime; suspend: short pause; stop: guest stops for the backup (most consistent)">
            <option value="snapshot">snapshot (no downtime)</option>
            <option value="suspend">suspend</option>
            <option value="stop">stop (most consistent)</option>
          </select>
          <input class="input grow" maxlength="200" bind:value={notes} aria-label="Notes" />
          <button type="button" class="btn tiny primary" data-guide="proxmox-backup-now" onclick={() => void proxmox.backupNow(g, storage, mode, notes).catch(() => {})}>Back up now</button>
        {:else}
          <p class="muted small">No backup storage is available on {g.node}. Add one in Proxmox (Datacenter → Storage, content "VZDump backup file").</p>
        {/if}
      </div>
      {#if proxmox.backupsError}<p class="error-text small pre">{proxmox.backupsError}</p>{/if}
      {#if !proxmox.backups}
        <p class="muted small">{proxmox.backupsLoading ? "Reading backups…" : ""}</p>
      {:else if !files.length}
        <p class="warn small">No backups of {g.name} on any backup storage.</p>
      {:else}
        <ul class="files">
          {#each files as f (f.volid)}
            <li>
              <span class="nowrap" title={new Date((f.time ?? 0) * 1000).toLocaleString()}>{ago(f.time)}</span>
              <span class="muted small">{f.storage}</span>
              <span class="data small">{formatBytes(f.size)}</span>
              <span class="muted small desc" title={f.notes}>{f.notes}</span>
              {#if f.verified}<span class="tag {f.verified === 'ok' ? '' : 'bad'}">VERIFY {f.verified.toUpperCase()}</span>{/if}
              <button type="button" class="btn tiny ghost" title={f.protected ? "Protected: pruning and deleting skip it" : "Keep it from pruning and deletion"} onclick={() => proxmox.protect(f, !f.protected)}>{f.protected ? "🔒 Protected" : "Protect"}</button>
              <button type="button" class="btn tiny ghost" onclick={() => openRestore(f, true)}>Restore as new…</button>
              <button type="button" class="btn tiny ghost" onclick={() => openRestore(f, false)}>Restore over…</button>
              <button type="button" class="btn tiny ghost" disabled={f.protected} title={f.protected ? "Unprotect it first" : ""} onclick={() => proxmox.askDeleteBackup(f)}>Delete…</button>
            </li>
          {/each}
        </ul>
      {/if}
      {#if restoreOf}
        {@const f = restoreOf}
        <div class="restore" data-guide="proxmox-restore">
          <p class="small">Restore the backup from <strong>{new Date((f.time ?? 0) * 1000).toLocaleString()}</strong> to guest id</p>
          <input class="input mono id" type="number" min="100" bind:value={rVmid} aria-label="Guest id" />
          <span class="small">disks on</span>
          <select class="input" bind:value={rStorage} aria-label="Target storage">{#each rStores as s (s)}<option value={s}>{s}</option>{/each}</select>
          <label class="check"><input type="checkbox" bind:checked={rStart} /> start after</label>
          {#if o?.guests.some((x) => x.vmid === rVmid)}<span class="warn small">{rVmid} exists: it will be overwritten (you'll type its id to confirm).</span>{/if}
          <span class="sp"></span>
          <button type="button" class="btn tiny ghost" onclick={() => (restoreOf = null)}>Cancel</button>
          <button
            type="button"
            class="btn tiny primary"
            disabled={!rStorage || !(rVmid >= 100)}
            onclick={() => {
              proxmox.askRestore(f, g.node, rVmid, rStorage, rStart);
              restoreOf = null;
            }}>Restore</button
          >
        </div>
      {/if}
    {/if}
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (proxmox.detail = null)}>Close</button>
  {/snippet}
</Modal>

<style>
  .fwbox {
    max-height: 62vh;
    overflow: auto;
  }
  .tabs {
    display: flex;
    gap: 0;
    align-items: center;
    border-bottom: 1px solid var(--line);
    margin-bottom: 8px;
  }
  .tabs button {
    border: 0;
    background: transparent;
    color: var(--text-1);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    padding: 4px 10px;
    cursor: pointer;
  }
  .tabs button.on {
    color: var(--magenta);
    box-shadow: inset 0 -2px 0 var(--magenta);
  }
  .sp {
    margin-left: auto;
  }
  .take {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
    margin-bottom: 8px;
  }
  .grow {
    flex: 1;
    min-width: 160px;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .check {
    display: inline-flex;
    gap: 4px;
    align-items: center;
    font-size: var(--fs-sm);
    color: var(--text-1);
  }
  .snaps,
  .files {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 46vh;
    overflow: auto;
  }
  .snaps li,
  .files li {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 5px 4px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-sm);
  }
  .snaps li.here {
    background: color-mix(in srgb, var(--cyan) 6%, transparent);
  }
  .branch {
    color: var(--text-1);
    width: 1ch;
  }
  .desc {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag {
    font-size: var(--fs-xs);
    border: 1px solid var(--line);
    padding: 0 4px;
    color: var(--cyan);
    white-space: nowrap;
  }
  .tag.bad {
    color: var(--danger);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .nowrap {
    white-space: nowrap;
  }
  .warn {
    color: var(--warn);
  }
  .pre {
    white-space: pre-wrap;
  }
  .error-text {
    margin: 0 0 6px;
  }
  .restore {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
    margin-top: 8px;
    padding: 8px;
    border: 1px solid var(--cyan);
  }
  .restore p {
    margin: 0;
  }
  .id {
    width: 90px;
  }
  .btn {
    white-space: nowrap;
  }
</style>
