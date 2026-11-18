<script lang="ts">
  /**
   * Fleet → DRILLS (1.1, D-171): prove backups restore. A drill is a backup
   * job's test restore, or a Proxmox guest's newest backup restored into an
   * isolated sandbox, booted, checked and destroyed; by hand or on a schedule.
   */
  import { onMount } from "svelte";
  import type { Drill } from "../../api";
  import { EVERY, blankDrill, drillTone, duration, everyLabel } from "../../fleet11";
  import { backups } from "../../stores/backups.svelte";
  import { drills } from "../../stores/fleet11.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { library } from "../../stores/library.svelte";

  onMount(() => {
    void drills.load();
    if (!backups.loaded) void backups.load();
  });

  const now = $derived.by(() => {
    void drills.board;
    return Math.floor(Date.now() / 1000);
  });
  const name = (id: number) => hosts.byId(id)?.name ?? `host ${id}`;
  const open = $derived(drills.board.drills.find((v) => v.drill.id === drills.open) ?? null);
  const servers = $derived(hosts.hosts.filter((h) => h.role !== "client"));
  const testable = $derived(backups.jobs);

  function add() {
    drills.editing = blankDrill(servers[0]?.id ?? hosts.hosts[0]?.id ?? 0);
  }
  function pickJob(d: Drill, id: number) {
    const j = backups.jobs.find((x) => x.id === id);
    d.jobId = id;
    if (j) {
      d.hostId = j.hostId;
      if (!d.name) d.name = `${j.name} restore`;
    }
  }
  const when = (t: number | null) => (t ? new Date(t * 1000).toLocaleString() : "--");
  const ok = (d: Drill) => d.name.trim() && (d.kind === "job" ? d.jobId != null : d.node && d.guestKind && d.vmid && d.storage && d.targetStorage);
</script>

<div class="drills" data-guide="fleet-drills">
  <div class="row">
    <button type="button" class="btn primary" onclick={add} data-guide="drills-add">+ Drill</button>
    <span class="muted small">A backup only counts once it has been restored. Scheduled drills run on connected hosts, without asking for a password.</span>
    <span class="sp"></span>
    <button type="button" class="link small" onclick={() => library.open("howto/restore-drills")}>Restore drills ▤</button>
  </div>
  {#if drills.error}<p class="error-text small">{drills.error}</p>{/if}

  <div class="split">
    <div class="list">
      {#each drills.board.drills as v (v.drill.id)}
        {@const t = drillTone(v, now)}
        <button type="button" class="drow" class:on={drills.open === v.drill.id} onclick={() => drills.select(v.drill.id)}>
          <span class="st {t}">{drills.isRunning(v.drill.id!) ? "RUNNING" : !v.last ? "NEVER RUN" : v.last.ok ? "PASSED" : "FAILED"}</span>
          <span class="dn">
            <span class="name">{v.drill.name}</span>
            <span class="muted small">{v.drill.kind === "proxmox" ? `Proxmox ${v.drill.guestKind} ${v.drill.vmid} → sandbox` : "test restore"} · {name(v.drill.hostId)} · {everyLabel(v.drill.everyDays)}{v.drill.enabled ? "" : " (paused)"}</span>
          </span>
          <span class="muted small">{v.runs ? `${v.passed}/${v.runs} passed` : ""}</span>
        </button>
      {:else}
        <p class="muted small pad">No drills yet. <strong>+ Drill</strong> picks a backup job to test-restore, or a Proxmox guest whose newest backup is restored into an isolated sandbox and destroyed again.</p>
      {/each}
    </div>

    <div class="detail">
      {#if drills.editing}
        {@const d = drills.editing}
        <h3 class="hd">{d.id == null ? "NEW DRILL" : "EDIT DRILL"}</h3>
        <div class="seg" role="group" aria-label="Kind">
          <button type="button" class:on={d.kind === "job"} onclick={() => (d.kind = "job")}>Backup job</button>
          <button type="button" class:on={d.kind === "proxmox"} onclick={() => (d.kind = "proxmox")}>Proxmox guest</button>
        </div>
        {#if d.kind === "job"}
          <label class="fld"
            ><span class="label">BACKUP JOB</span>
            <select class="input" value={d.jobId ?? ""} onchange={(e) => pickJob(d, Number(e.currentTarget.value))}>
              <option value="" disabled>pick one</option>
              {#each testable as j (j.id)}<option value={j.id}>{j.name} · {name(j.hostId)} ({j.tool})</option>{/each}
            </select>
          </label>
          {#if !testable.length}<p class="muted small">Add a backup job in Fleet → BACKUPS first.</p>{/if}
          <p class="muted small">Restores one random file from the latest snapshot into a temporary folder, checks it and deletes it, as root with the job's own environment.</p>
        {:else}
          <label class="fld"
            ><span class="label">PROXMOX HOST</span>
            <select class="input" bind:value={d.hostId}>{#each servers as h (h.id)}<option value={h.id}>{h.name}</option>{/each}</select>
          </label>
          <div class="two">
            <label class="fld"><span class="label">NODE</span><input class="input" bind:value={d.node} placeholder="pve" /></label>
            <label class="fld"
              ><span class="label">GUEST</span>
              <select class="input" bind:value={d.guestKind}><option value="qemu">VM (qemu)</option><option value="lxc">container (lxc)</option></select>
            </label>
            <label class="fld"><span class="label">GUEST ID</span><input class="input" type="number" min="100" bind:value={d.vmid} placeholder="101" /></label>
            <label class="fld"><span class="label">BACKUPS ON STORAGE</span><input class="input" bind:value={d.storage} placeholder="backup-nfs" /></label>
            <label class="fld"><span class="label">RESTORE TO STORAGE</span><input class="input" bind:value={d.targetStorage} placeholder="local-lvm" /></label>
          </div>
          <p class="muted small">Restores the newest backup as a new guest, pulls its network cables, removes passthrough and bind mounts, starts it, waits for the guest agent (or a command in the container), then destroys it. A 2 h expiry timer on the node removes it even if Nightgrid goes away. The restore storage needs room for the guest's disks.</p>
        {/if}
        <label class="fld"><span class="label">NAME</span><input class="input" bind:value={d.name} placeholder="Home Assistant restore" /></label>
        <div class="two">
          <label class="fld"
            ><span class="label">RUN</span>
            <select class="input" bind:value={d.everyDays}>{#each EVERY as e (e)}<option value={e}>{everyLabel(e)}</option>{/each}</select>
          </label>
          <label class="fld chk"><input type="checkbox" bind:checked={d.enabled} /> <span>scheduled runs on</span></label>
        </div>
        <div class="row">
          <button type="button" class="btn primary" disabled={!ok(d)} onclick={() => drills.save({ ...d, vmid: d.vmid == null ? null : Number(d.vmid) })}>Save</button>
          <button type="button" class="btn ghost" onclick={() => (drills.editing = null)}>Cancel</button>
        </div>
      {:else if open}
        {@const d = open.drill}
        <div class="row">
          <span class="dn"><span class="name">{d.name}</span><span class="muted small">{name(d.hostId)} · {everyLabel(d.everyDays)} · next {open.nextAt ? when(open.nextAt) : "by hand"}</span></span>
          <span class="sp"></span>
          <button type="button" class="btn primary" disabled={drills.isRunning(d.id!)} onclick={() => drills.run(d.id!)} data-guide="drills-run">{drills.isRunning(d.id!) ? "Running…" : "Run now"}</button>
          <button type="button" class="btn ghost" onclick={() => (drills.editing = { ...d })}>Edit</button>
          <button type="button" class="btn ghost" disabled={!drills.runs.length} onclick={() => drills.export(d.id!, d.name)}>Report</button>
          <button type="button" class="btn ghost danger" onclick={() => drills.remove(d.id!)}>Remove</button>
        </div>
        {#if drills.isRunning(d.id!)}<p class="muted small">{d.kind === "proxmox" ? "Restoring a whole guest takes a few minutes; you can leave this tab." : "Restoring one file…"}</p>{/if}
        {#each drills.runs as r (r.id)}
          <div class="run">
            <div class="row tight"><span class={r.ok ? "ok" : "bad"}>{r.ok ? "PASSED" : "FAILED"}</span><span class="muted small">{when(r.at)} · {duration(r.seconds)}</span></div>
            {#if r.detail}<span class="small data">{r.detail}</span>{/if}
            <ol class="small">{#each r.steps as s, i (i)}<li>{s}</li>{/each}</ol>
            {#if r.error}<p class="error-text small">{r.error}</p>{/if}
          </div>
        {:else}
          <p class="muted small">Not run yet.</p>
        {/each}
      {:else}
        <p class="muted pad">Pick a drill to see its runs, or add one.</p>
      {/if}
    </div>
  </div>
</div>

<style>
  .drills {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .row.tight {
    gap: 6px;
  }
  .sp {
    flex: 1;
  }
  .split {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(260px, 2fr) 3fr;
    gap: 10px;
  }
  .list,
  .detail {
    overflow: auto;
    min-height: 0;
    border: 1px solid var(--line);
  }
  .detail {
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .drow {
    all: unset;
    box-sizing: border-box;
    width: 100%;
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 7px 10px;
    border-bottom: 1px solid var(--line);
    cursor: pointer;
  }
  .drow:hover,
  .drow:focus-visible,
  .drow.on {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }
  .drow.on {
    box-shadow: inset 2px 0 0 var(--magenta);
  }
  .st {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: 0.06em;
    width: 76px;
    flex: none;
  }
  .dn {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .name {
    color: var(--text-0);
  }
  .hd {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    color: var(--text-2);
    letter-spacing: 0.1em;
    margin: 0;
  }
  .seg {
    display: flex;
    border: 1px solid var(--line);
    width: fit-content;
  }
  .seg button {
    border: 0;
    background: transparent;
    color: var(--text-1);
    padding: 3px 10px;
    cursor: pointer;
  }
  .seg button.on {
    color: var(--magenta);
    box-shadow: inset 0 -2px 0 var(--magenta);
  }
  .fld {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .fld.chk {
    flex-direction: row;
    align-items: center;
    gap: 6px;
    align-self: end;
  }
  .two {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 8px;
  }
  .run {
    border-top: 1px solid var(--line);
    padding-top: 6px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  ol {
    margin: 0;
    padding-left: 18px;
    color: var(--text-1);
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--cyan);
    font: inherit;
    cursor: pointer;
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
  .muted {
    color: var(--text-2);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .pad {
    padding: 10px;
    margin: 0;
  }
  p {
    margin: 0;
  }
  @media (max-width: 900px) {
    .split {
      grid-template-columns: 1fr;
    }
  }
</style>
