<script lang="ts">
  /** Proxmox → BACKUPS (M10r part 2): vzdump jobs, guests no job covers, and each guest's latest backup. */
  import { ago } from "../../gitstacks";
  import { backupsByGuest, formatBytes, scheduleText } from "../../proxmox";
  import { proxmox } from "../../stores/proxmox.svelte";

  $effect(() => {
    if (!proxmox.backups && !proxmox.backupsLoading && !proxmox.backupsError) void proxmox.loadBackups();
  });
  const b = $derived(proxmox.backups);
  const rows = $derived(backupsByGuest(proxmox.overview, b));
  function open(vmid: number) {
    const g = proxmox.overview?.guests.find((x) => x.vmid === vmid);
    if (g) proxmox.openGuest(g, "backups");
  }
</script>

<div class="scroll" data-guide="proxmox-backups">
  {#if proxmox.backupsError}<p class="error-text small pre pad">{proxmox.backupsError}</p>{/if}
  {#if b?.notes.length}<p class="warn small pre pad">{b.notes.join("\n")}</p>{/if}
  {#if !b}
    <p class="muted small pad">{proxmox.backupsLoading ? "Reading backup jobs and storages…" : ""}</p>
  {:else}
    <h3 class="hd">JOBS</h3>
    {#each b.jobs as j (j.id)}
      <div class="job" class:off={!j.enabled}>
        <strong>{scheduleText(j.schedule)}</strong>
        <span>{j.guests === "all" ? "all guests" : `guests ${j.guests}`}{#if j.exclude}<span class="muted">&nbsp;except {j.exclude}</span>{/if}</span>
        <span class="muted">→ {j.storage} · {j.mode}</span>
        <span class="muted small">{j.comment}</span>
        <span class="small {j.enabled ? 'ok' : 'warn'}">{j.enabled ? (j.nextRun ? `next ${new Date(j.nextRun * 1000).toLocaleString(undefined, { weekday: "short", hour: "2-digit", minute: "2-digit" })}` : "enabled") : "DISABLED"}</span>
      </div>
    {:else}
      <p class="warn small pad">No backup jobs. Create one in Proxmox (Datacenter → Backup), or back up guests one at a time from their BACKUPS tab.</p>
    {/each}
    {#if b.uncovered.length}
      <p class="warn small pad">Not in any backup job: {b.uncovered.map((u) => `${u.name || u.vmid} (${u.vmid})`).join(", ")}.</p>
    {/if}

    <h3 class="hd">BY GUEST <button type="button" class="btn tiny ghost" disabled={proxmox.backupsLoading} onclick={() => proxmox.loadBackups()}>{proxmox.backupsLoading ? "Reading…" : "Reload"}</button></h3>
    <div class="row headrow" aria-hidden="true"><span>ID</span><span>GUEST</span><span>LATEST</span><span class="n">COUNT</span><span class="n">SIZE</span><span>JOB</span><span></span></div>
    {#each rows as r (r.vmid)}
      <div class="row tone-{r.tone}">
        <span class="mono">{r.vmid}</span>
        <span><span class="kind">{r.kind === "qemu" ? "VM" : "CT"}</span> {r.name}</span>
        <span class={r.tone}>{r.latest ? ago(r.latest.time) : "never"}</span>
        <span class="n data">{r.files.length}</span>
        <span class="n data">{r.size ? formatBytes(r.size) : "--"}</span>
        <span class="small {r.covered ? 'muted' : 'warn'}">{r.covered ? "in a job" : "no job"}</span>
        <span class="acts"><button type="button" class="btn tiny ghost" disabled={!proxmox.overview?.guests.some((g) => g.vmid === r.vmid)} onclick={() => open(r.vmid)}>Open…</button></span>
      </div>
    {/each}
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
    margin: 0;
    padding: 10px 8px 4px;
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    font-weight: normal;
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .job {
    display: grid;
    grid-template-columns: 120px minmax(160px, 1.2fr) minmax(160px, 1fr) minmax(100px, 1fr) 150px;
    gap: 8px;
    align-items: center;
    padding: 6px 8px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-sm);
    color: var(--text-0);
  }
  .job.off {
    opacity: 0.7;
  }
  .row {
    display: grid;
    grid-template-columns: 56px minmax(160px, 2fr) 120px 60px 100px 80px 80px;
    gap: 8px;
    align-items: center;
    padding: 5px 8px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-sm);
    color: var(--text-0);
  }
  .row.tone-bad {
    border-left: 2px solid var(--danger);
  }
  .row.tone-warn {
    border-left: 2px solid var(--warn);
  }
  .row.tone-ok {
    border-left: 2px solid var(--ok);
  }
  .headrow {
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
  }
  .n {
    text-align: right;
  }
  .mono {
    font-family: var(--font-mono);
  }
  .kind {
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .acts {
    display: flex;
    justify-content: flex-end;
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
  .bad {
    color: var(--danger);
  }
</style>
