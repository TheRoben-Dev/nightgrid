<script lang="ts">
  import { typedMatches } from "../confirm";
  /**
   * Proxmox (M10r): a Proxmox VE host's cluster through `pvesh` over SSH.
   * SUMMARY (nodes), GUESTS (power actions, screen, console), STORAGE and
   * TASKS (with their logs). Nothing needs an API token or the web UI.
   */
  import Modal from "../components/Modal.svelte";
  import SudoPrompt from "../components/system/SudoPrompt.svelte";
  import PveBackupsTab from "../components/proxmox/PveBackupsTab.svelte";
  import PveGuestPanel from "../components/proxmox/PveGuestPanel.svelte";
  import PveMakeDialogs from "../components/proxmox/PveMakeDialogs.svelte";
  import PveSandboxDialog from "../components/proxmox/PveSandboxDialog.svelte";
  import PveSandboxTab from "../components/proxmox/PveSandboxTab.svelte";
  import { settings } from "../stores/settings.svelte";
  import PveNetworkTab from "../components/proxmox/PveNetworkTab.svelte";
  import PveFirewall from "../components/proxmox/PveFirewall.svelte";
  import {
    ACTION_CONSEQUENCE,
    ACTION_WORD,
    counts,
    filterGuests,
    formatBytes,
    formatUptime,
    splitActions,
    guestTone,
    isDestructive,
    isLocal,
    pct,
    storageRows,
    taskLabel,
    taskTone,
    usageTone,
    type PveGuest,
  } from "../proxmox";
  import { ago } from "../gitstacks";
  import { desktops } from "../stores/desktops.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { proxmox, type PveTab } from "../stores/proxmox.svelte";
  import { terms } from "../stores/terms.svelte";

  const TABS: { id: PveTab; label: string }[] = [
    { id: "summary", label: "SUMMARY" },
    { id: "guests", label: "GUESTS" },
    { id: "sandbox", label: "SANDBOX" },
    { id: "backups", label: "BACKUPS" },
    { id: "storage", label: "STORAGE" },
    { id: "network", label: "NETWORK" },
    { id: "firewall", label: "FIREWALL" },
    { id: "tasks", label: "TASKS" },
  ];

  $effect(() => {
    if (proxmox.hostId != null && !hosts.hosts.some((h) => h.id === proxmox.hostId)) proxmox.pick(null);
  });
  $effect(() => {
    if (proxmox.hostId != null && !proxmox.overview && !proxmox.loading && !proxmox.error) void proxmox.load();
  });

  const o = $derived(proxmox.overview);
  const c = $derived(counts(o?.guests ?? []));
  const shown = $derived(filterGuests(o?.guests ?? [], proxmox.query, proxmox.node));
  const running = $derived((o?.tasks ?? []).filter((t) => t.end == null).length);
  const failed = $derived((o?.tasks ?? []).filter((t) => taskTone(t) === "bad").length);
  const storages = $derived(storageRows(o?.storages ?? []));
  let menuFor = $state<number | null>(null);
  $effect(() => {
    if (proxmox.tab === "storage" && proxmox.overview && !proxmox.createInfo && !proxmox.createInfoError) void proxmox.loadCreateInfo();
  });
  let typed = $state("");
  $effect(() => {
    if (proxmox.confirm) typed = "";
  });
  const when = (ts: number | null) => new Date((ts ?? 0) * 1000).toLocaleString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });

  function screen(g: PveGuest) {
    const h = proxmox.host;
    if (!h) return;
    void desktops.open({ kind: g.kind === "qemu" ? "qemu" : "lxc", id: String(g.vmid), name: g.name, state: g.status, vcpus: g.maxcpu, memoryMb: (g.maxmem ?? 0) / 1048576, autostart: null }, { hostId: h.id, hostName: h.name, elevate: true });
  }
  function consoleOf(g: PveGuest) {
    const h = proxmox.host;
    if (!h) return;
    terms.openVmConsole(h.id, { kind: g.kind === "qemu" ? "qemu" : "lxc", id: String(g.vmid), name: g.name, asRoot: true });
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && (menuFor = null)} />

<div class="pve" data-guide="proxmox">
  <div class="toolbar">
    <label class="hostpick"
      ><span class="muted small">HOST</span>
      <select class="input" data-guide="proxmox-host" value={proxmox.hostId ?? ""} onchange={(e) => proxmox.pick(Number(e.currentTarget.value) || null)}>
        <option value="">Pick a Proxmox host…</option>
        {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
      </select>
    </label>
    {#if o?.present}
      <span class="badge-line small">
        <span class="mono">PVE {o.version}</span>
        {#if o.cluster}<span>· cluster <strong>{o.cluster}</strong></span><span class={o.quorate ? "ok" : "bad"}>{o.quorate ? "· quorate" : "· NO QUORUM"}</span>{:else}<span class="muted">· single node</span>{/if}
      </span>
    {/if}
    <span class="spacer"></span>
    {#if o?.present}
      <div class="tabstrip" role="tablist" aria-label="Proxmox view">
        {#each TABS.filter((t) => t.id !== "sandbox" || settings.on("proxmox.sandbox")) as t (t.id)}
          <button type="button" role="tab" data-guide="proxmox-tab-{t.id}" class:on={proxmox.tab === t.id} aria-selected={proxmox.tab === t.id} onclick={() => (proxmox.tab = t.id)}>
            {t.label}{#if t.id === "guests"}<span class="badge">{c.running}/{c.vms + c.cts}</span>{/if}{#if t.id === "tasks" && (running || failed)}<span class="badge {failed ? 'bad' : 'warn'}">{running || failed}</span>{/if}
          </button>
        {/each}
      </div>
    {/if}
    <button type="button" class="btn tiny" disabled={proxmox.loading || proxmox.hostId == null} onclick={() => proxmox.load()}>{proxmox.loading ? "Reading…" : "Refresh"}</button>
  </div>

  {#if proxmox.notice}
    <p class="notice {proxmox.notice.tone}" role="status">{proxmox.notice.text} <button type="button" class="x" aria-label="Dismiss" onclick={() => (proxmox.notice = null)}>×</button></p>
  {/if}
  {#if proxmox.error}<p class="error-text small pre" role="alert">{proxmox.error}</p>{/if}
  {#if o?.notes.length}<p class="warn-text small pre">{o.notes.join("\n")}</p>{/if}

  <div class="pane">
    {#if proxmox.hostId == null}
      <div class="empty">
        <p class="label">PROXMOX VE</p>
        <p class="muted">Pick the host that runs Proxmox VE. Nightgrid reads the cluster with <code>pvesh</code> over SSH as root (the sudo prompt asks when needed), so no API token, open port or web UI is needed. Any node of a cluster shows the whole cluster.</p>
      </div>
    {:else if !o}
      <p class="muted small pad">{proxmox.loading ? "Reading the cluster…" : ""}</p>
    {:else if !o.present}
      <div class="empty">
        <p class="label">NO PROXMOX HERE</p>
        <p class="muted"><code>pvesh</code> isn't installed on {proxmox.host?.name}. Pick a Proxmox VE node instead; plain KVM guests are under System → VMS.</p>
      </div>
    {:else if proxmox.tab === "summary"}
      <div class="scroll">
        <div class="cards">
          {#each o.nodes as n (n.name)}
            {@const cpu = (n.cpu ?? 0) * 100}
            {@const mem = pct(n.mem, n.maxmem)}
            {@const disk = pct(n.disk, n.maxdisk)}
            <div class="card" class:off={!n.online} data-guide="proxmox-node">
              <p class="ttl"><span class="dot {n.online ? 'ok' : 'bad'}">●</span> <strong>{n.name}</strong>{#if n.local}<span class="tag">THIS HOST</span>{/if}<span class="muted small right mono">{n.ip ?? ""}</span></p>
              {#if n.online}
                <div class="bars">
                  <span class="k">CPU</span><span class="bar"><span class={usageTone(cpu)} style:width="{cpu}%"></span></span><span class="v">{cpu.toFixed(0)}% of {n.maxcpu}</span>
                  <span class="k">MEM</span><span class="bar"><span class={usageTone(mem)} style:width="{mem ?? 0}%"></span></span><span class="v">{formatBytes(n.mem)} / {formatBytes(n.maxmem)}</span>
                  <span class="k">ROOT</span><span class="bar"><span class={usageTone(disk)} style:width="{disk ?? 0}%"></span></span><span class="v">{formatBytes(n.disk)} / {formatBytes(n.maxdisk)}</span>
                </div>
                <p class="muted small">Up {formatUptime(n.uptime)} · {o.guests.filter((g) => g.node === n.name && g.status === "running").length} running</p>
              {:else}
                <p class="bad small">Offline</p>
              {/if}
              {#if n.local && o.local}
                <p class="muted small">{o.local.cpuModel} ({o.local.sockets}×{o.local.cores} cores, {o.local.threads} threads) · kernel {o.local.kernel} · load {o.local.loadavg.join(" ")} · {o.local.bootMode.toUpperCase()}</p>
              {/if}
            </div>
          {/each}
        </div>
        <p class="muted small pad">
          {c.vms} VM{c.vms === 1 ? "" : "s"} and {c.cts} container{c.cts === 1 ? "" : "s"}: {c.running} running, {c.stopped} stopped{#if c.templates}, plus {c.templates} template{c.templates === 1 ? "" : "s"}{/if}.
          <button type="button" class="link" onclick={() => (proxmox.tab = "guests")}>Guests</button>
        </p>
      </div>
    {:else if proxmox.tab === "guests"}
      <div class="filter">
        <input class="input" placeholder="Filter: name, id, #tag, running…" bind:value={proxmox.query} spellcheck="false" />
        {#if o.nodes.length > 1}
          <select class="input" value={proxmox.node ?? ""} onchange={(e) => (proxmox.node = e.currentTarget.value || null)}>
            <option value="">All nodes</option>
            {#each o.nodes as n (n.name)}<option value={n.name}>{n.name}</option>{/each}
          </select>
        {/if}
        <span class="sp"></span>
        <button type="button" class="btn tiny" data-guide="proxmox-new-vm" disabled={!proxmox.localNode} onclick={() => proxmox.open({ kind: "vm" })}>+ VM</button>
        <button type="button" class="btn tiny" data-guide="proxmox-new-ct" disabled={!proxmox.localNode} onclick={() => proxmox.open({ kind: "ct" })}>+ Container</button>
      </div>
      <div class="scroll">
        <div class="grow headrow" aria-hidden="true"><span>ID</span><span>NAME</span><span>NODE</span><span>STATE</span><span class="n">CPU</span><span class="n">MEMORY</span><span class="n">UP</span><span></span></div>
        {#each shown as g (g.vmid)}
          {@const tone = guestTone(g)}
          {@const local = isLocal(o, g.node)}
          {@const { main, more } = splitActions(g)}
          <div class="grow {tone}" data-guide="proxmox-guest">
            <span class="mono">{g.vmid}</span>
            <span class="name">
              <span class="kind">{g.kind === "qemu" ? "VM" : "CT"}</span>
              <button type="button" class="gname" title="Snapshots and backups" onclick={() => proxmox.openGuest(g)}>{g.name || "(no name)"}</button>
              {#each g.tags as t (t)}<span class="tag">{t}</span>{/each}
              {#if g.template}<span class="tag muted">TEMPLATE</span>{/if}
              {#if g.lock}<span class="tag warn" title="An operation holds this guest">LOCK: {g.lock}</span>{/if}
              {#if g.hastate}<span class="tag" title="Managed by HA">HA {g.hastate}</span>{/if}
            </span>
            <span class="muted">{g.node}</span>
            <span class="state {tone}">{g.status.toUpperCase()}</span>
            <span class="n data">{g.status === "running" ? `${((g.cpu ?? 0) * 100).toFixed(0)}% / ${g.maxcpu}` : `${g.maxcpu} cpu`}</span>
            <span class="n data">{g.status === "running" ? `${formatBytes(g.mem)} / ${formatBytes(g.maxmem)}` : formatBytes(g.maxmem)}</span>
            <span class="n data">{g.status === "running" ? formatUptime(g.uptime) : "--"}</span>
            <span class="acts">
              {#each main as a (a)}
                <button type="button" class="btn tiny ghost" disabled={proxmox.busy.includes(g.vmid)} onclick={() => proxmox.request(g, a)}>{ACTION_WORD[a]}</button>
              {/each}
              {#if g.status === "running" && !g.template}
                {#if local}
                  {#if g.kind === "qemu"}<button type="button" class="btn tiny ghost" title="Open the VM's screen in Desktops" onclick={() => screen(g)}>Screen</button>{/if}
                  <button type="button" class="btn tiny ghost" title={g.kind === "qemu" ? "Serial console (qm terminal) in a terminal tab" : "A shell in the container (pct enter)"} onclick={() => consoleOf(g)}>Console</button>
                {:else}
                  <span class="muted small" title="Screens and consoles run on the guest's node: add {g.node} as a host to open them">on {g.node}</span>
                {/if}
              {/if}
              {#if more.length || g.status === "stopped" || g.template || g.kind === "qemu"}
                <span class="pop">
                  <button type="button" class="btn tiny ghost" aria-label="More actions for {g.name}" aria-expanded={menuFor === g.vmid} disabled={proxmox.busy.includes(g.vmid)} onclick={() => (menuFor = menuFor === g.vmid ? null : g.vmid)}>⋯</button>
                  {#if menuFor === g.vmid}
                    <ul class="menu" role="menu">
                      {#each more as a (a)}
                        <li><button type="button" role="menuitem" class:danger={a === "stop"} onclick={() => { menuFor = null; proxmox.request(g, a); }}>{ACTION_WORD[a]}…</button></li>
                      {/each}
                      {#if !g.lock && (g.kind === "qemu" || g.status === "stopped")}
                        <li><button type="button" role="menuitem" onclick={() => { menuFor = null; proxmox.open({ kind: "clone", guest: g }); }}>Clone…</button></li>
                      {/if}
                      {#if !g.lock && settings.on("proxmox.sandbox") && !g.tags.includes("nightgrid-sandbox")}
                        <li><button type="button" role="menuitem" onclick={() => { menuFor = null; proxmox.open({ kind: "sandbox", guest: g }); }}>Sandbox copy…</button></li>
                      {/if}
                      {#if g.status === "stopped" && !g.template && !g.lock}
                        <li><button type="button" role="menuitem" onclick={() => { menuFor = null; proxmox.askTemplate(g); }}>Convert to template…</button></li>
                      {/if}
                      {#if g.status === "stopped" && !g.lock}
                        <li><button type="button" role="menuitem" class="danger" onclick={() => { menuFor = null; proxmox.askDestroy(g); }}>Destroy…</button></li>
                      {/if}
                    </ul>
                  {/if}
                </span>
              {/if}
            </span>
          </div>
        {:else}
          <p class="muted small pad">{o.guests.length ? "No guest matches the filter." : "No VMs or containers yet."}</p>
        {/each}
      </div>
    {:else if proxmox.tab === "sandbox"}
      <PveSandboxTab {screen} {consoleOf} />
    {:else if proxmox.tab === "backups"}
      <PveBackupsTab />
    {:else if proxmox.tab === "storage"}
      <div class="scroll">
        <div class="srow headrow" aria-hidden="true"><span>STORAGE</span><span>TYPE</span><span>CONTENT</span><span>NODES</span><span>USAGE</span><span class="n">USED</span></div>
        {#each storages as r (r.key)}
          {@const p = pct(r.s.used, r.s.total)}
          <div class="srow" class:off={r.s.status !== "available"}>
            <strong>{r.s.storage}{#if r.s.shared}<span class="tag">SHARED</span>{/if}</strong>
            <span class="muted">{r.s.plugin}</span>
            <span class="muted small">{r.s.content.join(", ")}</span>
            <span class="muted small">{r.nodes.join(", ")}</span>
            {#if r.s.status === "available"}
              <span class="bar"><span class={usageTone(p)} style:width="{p ?? 0}%"></span></span>
              <span class="n data">{formatBytes(r.s.used)} / {formatBytes(r.s.total)}</span>
            {:else}
              <span class="warn small">{r.s.status.toUpperCase()}</span><span></span>
            {/if}
          </div>
        {/each}
        <h3 class="hd" data-guide="proxmox-images">
          ISO IMAGES & CT TEMPLATES
          <button type="button" class="btn tiny ghost" disabled={!proxmox.localNode} onclick={() => proxmox.open({ kind: "iso" })}>Download ISO…</button>
          <button type="button" class="btn tiny ghost" disabled={!proxmox.localNode} onclick={() => proxmox.open({ kind: "appliance" })}>Get CT template…</button>
        </h3>
        {#if !proxmox.createInfo}
          <p class="muted small pad">{proxmox.createInfoError ?? "Reading…"}</p>
        {:else}
          {#each [...proxmox.createInfo.isos, ...proxmox.createInfo.templates] as v (v.volid)}
            <div class="vrow">
              <span class="kind">{v.content === "iso" ? "ISO" : "CT"}</span>
              <span class="mono ell" title={v.volid}>{v.name}</span>
              <span class="muted small">{v.volid.split(":")[0]}</span>
              <span class="n data">{formatBytes(v.size)}</span>
            </div>
          {:else}
            <p class="muted small pad">None on {proxmox.localNode}. Download an installer ISO for VMs, or a template for containers.</p>
          {/each}
        {/if}
      </div>
    {:else if proxmox.tab === "network"}
      <PveNetworkTab />
    {:else if proxmox.tab === "firewall"}
      <div class="scroll fwpad"><PveFirewall /></div>
    {:else if proxmox.tab === "tasks"}
      <div class="scroll">
        <div class="trow headrow" aria-hidden="true"><span>STARTED</span><span>TASK</span><span>NODE</span><span>USER</span><span>RESULT</span></div>
        {#each o.tasks as t (t.upid)}
          {@const tone = taskTone(t)}
          <button type="button" class="trow" data-guide="proxmox-task" onclick={() => proxmox.openListed(t, taskLabel(t))}>
            <span class="muted small nowrap" title={when(t.start)}>{ago(t.start)}</span>
            <span>{taskLabel(t)}</span>
            <span class="muted">{t.node}</span>
            <span class="muted small">{t.user}</span>
            <span class="{tone} small ell">{t.end == null ? "running…" : t.status}</span>
          </button>
        {:else}
          <p class="muted small pad">No recent tasks.</p>
        {/each}
      </div>
    {/if}
  </div>
</div>

<Modal
  open={proxmox.pending !== null}
  tone="danger"
  title="{proxmox.pending ? ACTION_WORD[proxmox.pending.action].toUpperCase() : ''} {proxmox.pending?.guest.kind === 'qemu' ? 'VM' : 'CONTAINER'}"
  width={480}
  onclose={() => (proxmox.pending = null)}
>
  {#if proxmox.pending}
    {@const p = proxmox.pending}
    <p>{ACTION_WORD[p.action]} <strong>{p.guest.name || p.guest.vmid}</strong> ({p.guest.vmid}) on node <strong>{p.guest.node}</strong>?</p>
    {#if ACTION_CONSEQUENCE[p.action]}<p class="muted small">{ACTION_CONSEQUENCE[p.action]}</p>{/if}
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (proxmox.pending = null)}>Cancel</button>
    {#if proxmox.pending}
      {@const p = proxmox.pending}
      <button type="button" class="btn {isDestructive(p.action) ? 'danger' : 'primary'}" onclick={() => proxmox.act(p.guest, p.action)}>{ACTION_WORD[p.action]} {p.guest.name || p.guest.vmid}</button>
    {/if}
  {/snippet}
</Modal>

<PveGuestPanel />
<PveMakeDialogs />
{#if settings.on("proxmox.sandbox")}<PveSandboxDialog />{/if}

<Modal open={proxmox.confirm !== null} tone="danger" title={proxmox.confirm?.title ?? ""} width={500} onclose={() => (proxmox.confirm = null)}>
  {#if proxmox.confirm}
    <p>{proxmox.confirm.body}</p>
    {#if proxmox.confirm.typed}
      <label class="typed small">Type <strong class="mono verbatim">{proxmox.confirm.typed}</strong> to confirm <input class="input mono" bind:value={typed} spellcheck="false" /></label>
    {/if}
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (proxmox.confirm = null)}>Cancel</button>
    <button type="button" class="btn danger" disabled={!!proxmox.confirm?.typed && !typedMatches(typed, proxmox.confirm.typed)} onclick={() => proxmox.runConfirm()}>{proxmox.confirm?.button ?? ""}</button>
  {/snippet}
</Modal>

<Modal open={!!proxmox.task && !proxmox.task.hidden} title="TASK · {proxmox.task?.title.toUpperCase() ?? ''}" width={720} onclose={() => proxmox.closeTask()}>
  {#if proxmox.task}
    {@const t = proxmox.task}
    <p class="small">
      {#if t.error}<span class="bad">{t.error}</span>
      {:else if !t.detail}<span class="muted">Reading…</span>
      {:else if t.detail.running}<span class="warn">Running…</span>
      {:else}<span class={t.detail.exitstatus === "OK" ? "ok" : "bad"}>Finished: {t.detail.exitstatus}</span>{/if}
      <span class="muted mono upid">{t.upid}</span>
    </p>
    <pre class="log" data-guide="proxmox-task-log">{t.detail?.log.join("\n") ?? ""}</pre>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => proxmox.closeTask()}>Close</button>
  {/snippet}
</Modal>

<SudoPrompt />

<style>
  .pve {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }
  .toolbar {
    display: flex;
    gap: 12px;
    align-items: center;
    flex-wrap: wrap;
  }
  .hostpick {
    display: inline-flex;
    gap: 6px;
    align-items: center;
  }
  .badge-line {
    display: inline-flex;
    gap: 6px;
    color: var(--text-1);
  }
  .spacer {
    flex: 1;
  }
  .pane {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line);
    background: var(--bg-1);
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  .empty {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 20px;
    max-width: 680px;
  }
  .empty p {
    margin: 0;
    line-height: 1.6;
  }
  .label {
    color: var(--text-0);
    font-size: var(--fs-lg);
  }
  .pad {
    padding: 10px;
    margin: 0;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .mono {
    font-family: var(--font-mono);
  }
  .pre {
    white-space: pre-wrap;
    margin: 0;
  }
  .ok {
    color: var(--ok);
  }
  .warn,
  .warn-text {
    color: var(--warn);
  }
  .bad {
    color: var(--danger);
  }
  .notice {
    margin: 0;
    font-size: var(--fs-sm);
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .notice.ok {
    color: var(--ok);
  }
  .notice.err {
    color: var(--danger);
  }
  .x {
    background: none;
    border: 0;
    color: var(--text-1);
    cursor: pointer;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    gap: 10px;
    padding: 10px;
  }
  .card {
    border: 1px solid var(--line);
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    background: var(--bg-0);
  }
  .card.off {
    opacity: 0.6;
  }
  .card p {
    margin: 0;
  }
  .ttl {
    display: flex;
    gap: 6px;
    align-items: baseline;
    color: var(--text-0);
  }
  .right {
    margin-left: auto;
  }
  .dot {
    font-size: var(--fs-xs);
  }
  .bars {
    display: grid;
    grid-template-columns: 42px 1fr auto;
    gap: 4px 8px;
    align-items: center;
    font-size: var(--fs-xs);
  }
  .k {
    color: var(--text-1);
    letter-spacing: 0.1em;
  }
  .v {
    color: var(--text-0);
    font-family: var(--font-mono);
  }
  .bar {
    height: 6px;
    background: color-mix(in srgb, var(--line) 70%, transparent);
    position: relative;
    overflow: hidden;
  }
  .bar > span {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--cyan);
  }
  .bar > span.warn {
    background: var(--warn);
  }
  .bar > span.bad {
    background: var(--danger);
  }
  .tag {
    font-size: var(--fs-xs);
    border: 1px solid var(--line);
    padding: 0 var(--tag-px);
    color: var(--cyan);
    margin-left: 4px;
    white-space: nowrap;
    display: inline-block;
    line-height: calc(var(--tag-h) - 2px);
  }
  .tag.warn {
    color: var(--warn);
    border-color: var(--warn);
  }
  .sp {
    margin-left: auto;
  }
  .fwpad {
    padding: 4px 8px;
  }
  .hd {
    margin: 0;
    padding: 12px 8px 4px;
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    font-weight: normal;
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .vrow {
    display: grid;
    grid-template-columns: 40px minmax(200px, 1fr) 140px 100px;
    gap: 8px;
    align-items: center;
    padding: 5px 8px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-sm);
  }
  .filter {
    display: flex;
    gap: 8px;
    padding: 8px;
    border-bottom: 1px solid var(--line);
  }
  .filter input {
    flex: 1;
    max-width: 420px;
  }
  .grow,
  .srow,
  .trow {
    display: grid;
    gap: 8px;
    align-items: center;
    padding: 6px 8px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-sm);
    color: var(--text-0);
  }
  .grow {
    grid-template-columns: 56px minmax(160px, 2fr) 60px 76px 76px 150px 64px minmax(230px, auto);
  }
  .grow.ok {
    border-left: 2px solid var(--ok);
  }
  .grow.warn {
    border-left: 2px solid var(--warn);
  }
  .grow.off {
    border-left: 2px solid transparent;
  }
  .srow {
    grid-template-columns: minmax(140px, 1.2fr) 80px minmax(120px, 1fr) minmax(90px, 0.8fr) minmax(120px, 1fr) 170px;
  }
  .srow.off {
    opacity: 0.7;
  }
  .trow {
    grid-template-columns: 110px minmax(160px, 1.2fr) 70px 90px minmax(160px, 2fr);
    width: 100%;
    background: none;
    border-top: 0;
    border-right: 0;
    border-left: 0;
    text-align: left;
    font: inherit;
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  button.trow:hover {
    background: color-mix(in srgb, var(--mint) 6%, transparent);
  }
  .headrow {
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    position: sticky;
    top: 0;
    background: var(--bg-1);
    z-index: 1;
    cursor: default;
  }
  .n {
    text-align: right;
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .gname {
    background: none;
    border: 0;
    padding: 0;
    color: var(--text-0);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .gname:hover {
    color: var(--cyan);
    text-decoration: underline;
  }
  .typed {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .kind {
    font-size: var(--fs-xs);
    color: var(--text-1);
    margin-right: 4px;
  }
  .state {
    font-size: var(--fs-xs);
    letter-spacing: 0.06em;
  }
  .state.off {
    color: var(--text-1);
  }
  .acts {
    display: flex;
    gap: 4px;
    justify-content: flex-end;
    align-items: center;
  }
  .nowrap {
    white-space: nowrap;
  }
  .acts .btn {
    white-space: nowrap;
  }
  .pop {
    position: relative;
  }
  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 2px);
    z-index: 5;
    list-style: none;
    margin: 0;
    padding: 4px;
    min-width: 140px;
    background: var(--bg-1);
    border: 1px solid var(--line);
  }
  .menu button {
    width: 100%;
    text-align: left;
    background: none;
    border: 0;
    color: var(--text-0);
    padding: 5px 8px;
    cursor: pointer;
    font: inherit;
    font-size: var(--fs-sm);
  }
  .menu button:hover {
    background: color-mix(in srgb, var(--mint) 10%, transparent);
  }
  .menu button.danger {
    color: var(--danger);
  }
  .ell {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--cyan);
    cursor: pointer;
    font: inherit;
  }
  .upid {
    margin-left: 8px;
    overflow-wrap: anywhere;
  }
  .log {
    margin: 0;
    max-height: 50vh;
    min-height: 120px;
    overflow: auto;
    background: var(--bg-0);
    border: 1px solid var(--line);
    padding: 6px 8px;
    font-size: var(--fs-xs);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  code {
    color: var(--cyan);
  }
</style>
