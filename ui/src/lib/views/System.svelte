<script lang="ts">
  import { typedMatches } from "../confirm";
  import { scramble } from "../scramble";
  import Modal from "../components/Modal.svelte";
  import AuthPrompt from "../components/terminal/AuthPrompt.svelte";
  import HostKeyPrompt from "../components/terminal/HostKeyPrompt.svelte";
  import DisksPanel from "../components/system/DisksPanel.svelte";
  import JournalPanel from "../components/system/JournalPanel.svelte";
  import FirewallPanel from "../components/system/FirewallPanel.svelte";
  import SchedulesPanel from "../components/system/SchedulesPanel.svelte";
  import TimelinePanel from "../components/system/TimelinePanel.svelte";
  import ConfigPanel from "../components/system/ConfigPanel.svelte";
  import SecurityPanel from "../components/system/SecurityPanel.svelte";
  import SudoPrompt from "../components/system/SudoPrompt.svelte";
  import UnitsPanel from "../components/system/UnitsPanel.svelte";
  import UpdatesPanel from "../components/system/UpdatesPanel.svelte";
  import VmsPanel from "../components/system/VmsPanel.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { actionVerb, system, UNIT_CONSEQUENCE, type SystemTab } from "../stores/system.svelte";
  import { ui } from "../stores/ui.svelte";
  import { platform } from "../stores/platform.svelte";
  import PlatformNote from "../components/PlatformNote.svelte";
  import type { PowerAction } from "../api";

  // Pick a host once the host list has loaded.
  $effect(() => {
    const first = system.hostId === null ? hosts.preferred("system") : undefined;
    if (first) system.select(first.id);
  });

  const TABS: { id: SystemTab; label: string }[] = [
    { id: "units", label: "UNITS" },
    { id: "journal", label: "JOURNAL" },
    { id: "schedules", label: "SCHEDULES" },
    { id: "timeline", label: "TIMELINE" },
    { id: "config", label: "CONFIG" },
    { id: "security", label: "SECURITY" },
    { id: "firewall", label: "FIREWALL" },
    { id: "updates", label: "UPDATES" },
    { id: "disks", label: "DISKS" },
    { id: "vms", label: "VMS" },
  ];

  const SUDO_WORD = { root: "ROOT", noPassword: "SUDO · NOPASSWD", password: "SUDO · PASSWORD", missing: "NO SUDO" } as const;
  // Windows hosts: administrators act directly over SSH; others can't change system settings.
  const ADMIN_WORD = { root: "ADMINISTRATOR", noPassword: "ADMINISTRATOR", password: "ADMINISTRATOR", missing: "NOT AN ADMINISTRATOR" } as const;
  const sysHost = $derived(system.hostId === null ? undefined : hosts.byId(system.hostId));
  const onWindows = $derived(platform.isWindows(sysHost));
  // The tab's Windows note when it can't run there (planned or impossible).
  const tabBlock = $derived(platform.blocked(sysHost, `system.${system.tab}`));
  // Works with differences on Windows: a strip above the tab says how.
  const tabPartial = $derived(tabBlock ? undefined : platform.note(sysHost, `system.${system.tab}`));

  const hostName = $derived(system.host?.name ?? "");
  const pending = $derived(system.pendingUnit);
  const upCount = $derived(system.updates?.updates.filter((u) => !u.held).length ?? 0);
  let typed = $state("");
  $effect(() => {
    if (!system.confirmPower) typed = "";
  });
  let groupTyped = $state("");
  const GR_MARK = { pending: "○", running: "◐", done: "✓", failed: "✕" } as const;
  const groupMembers = $derived(system.confirmGroupPower ? hosts.hosts.filter((h) => h.groupId === system.confirmGroupPower?.groupId) : []);
  const groupName = $derived(hosts.groups.find((g) => g.id === system.confirmGroupPower?.groupId)?.name ?? "");
  function openGroup() {
    const g = hosts.byId(system.hostId ?? -1)?.groupId ?? hosts.groups[0]?.id;
    if (g == null) return;
    groupTyped = "";
    system.confirmGroupPower = { groupId: g, action: "shutdown" };
  }
  const POWER_TITLE = { reboot: "REBOOT HOST", shutdown: "SHUT DOWN HOST", suspend: "SUSPEND HOST" } as const;
  const POWER_LABEL = { reboot: "Reboot", shutdown: "Shut down", suspend: "Suspend" } as const;
  const POWER_TEXT = {
    reboot: "Everything on it stops: containers, services, terminals and tunnels through it drop until it's back.",
    shutdown: "It stays off until someone powers it on (or Wake-on-LAN wakes it). Everything on it stops.",
    suspend: "It goes to sleep: services stop answering until it wakes (Wake-on-LAN, a key press, or its own timer).",
  } as const;
</script>

<div class="system">
  <div class="toolbar">
    <label class="hostpick">
      <span class="label">HOST</span>
      <select class="input" value={system.hostId ?? ""} onchange={(e) => system.select(Number(e.currentTarget.value))} aria-label="Host">
        {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
      </select>
    </label>
    <div class="tabstrip" role="tablist" aria-label="System view">
      {#each TABS as t (t.id)}
        <button type="button" role="tab" data-guide="system-tab-{t.id}" class:on={system.tab === t.id} aria-selected={system.tab === t.id} onclick={() => system.openTab(t.id)}>
          {t.label}{#if t.id === "updates" && upCount}<span class="badge">{upCount}</span>{/if}{#if t.id === "security" && system.security && system.security.score < 75}<span class="badge {system.security.score < 60 ? 'bad' : 'warn'}"
              >{system.security.grade}</span
            >{/if}{#if t.id === "disks" && system.disks && (system.disks.health === "warn" || system.disks.health === "bad")}<span
              class="badge {system.disks.health}">!</span
            >{/if}
        </button>
      {/each}
    </div>
    {#if system.info}
      <span class="sudo" class:warn={system.info.sudo === "missing"} title={onWindows ? "Whether the login user is a Windows administrator" : "How Nightgrid gets root on this host"}
        >{onWindows ? ADMIN_WORD[system.info.sudo] : SUDO_WORD[system.info.sudo]}</span
      >
      {#if system.info.remembered}
        <button type="button" class="btn ghost tiny" title="Delete the saved sudo password from the OS keychain" onclick={() => system.forgetSudo()}>
          Forget sudo password
        </button>
      {/if}
    {/if}
    {#if system.hostId !== null}
      <span class="power" data-guide="system-power">
        {#if hosts.byId(system.hostId)?.macAddress}
          <button type="button" class="btn ghost tiny" title="Send a Wake-on-LAN packet" onclick={() => system.wake()}>WAKE</button>
        {/if}
        <button type="button" class="btn ghost tiny" onclick={() => (system.confirmPower = "suspend")}>SUSPEND…</button>
        <button type="button" class="btn ghost tiny" onclick={() => (system.confirmPower = "shutdown")}>SHUT DOWN…</button>
        <button type="button" class="btn ghost tiny danger-text" onclick={() => (system.confirmPower = "reboot")}>REBOOT…</button>
        {#if hosts.groups.length}
          <button type="button" class="btn ghost tiny" data-guide="system-group-power" title="Suspend, shut down or reboot every host in a group" onclick={openGroup}>GROUP…</button>
        {/if}
      </span>
    {/if}
  </div>

  {#if system.groupRun}
    <div class="grouprun" role="status">
      {#each system.groupRun as r (r.hostId)}
        <span class="gr {r.state}" title={r.error ?? ""}>{GR_MARK[r.state]} {r.name}{r.error ? `: ${r.error}` : ""}</span>
      {/each}
      {#if !system.groupRun.some((r) => r.state === "pending" || r.state === "running")}
        <button type="button" class="x" aria-label="Dismiss" onclick={() => (system.groupRun = null)}>×</button>
      {/if}
    </div>
  {/if}

  {#if system.notice}
    <p class="notice {system.notice.tone}" role="status">
      <span use:scramble={system.notice.text}></span>
      <button type="button" class="x" aria-label="Dismiss" onclick={() => (system.notice = null)}>×</button>
    </p>
  {/if}

  <div class="pane">
    {#if !hosts.hosts.length}
      <div class="empty">
        <p class="label">NO HOSTS</p>
        <p class="muted">Add a host first; its services, journal, timers, cron jobs, updates and disks show up here.</p>
        <button type="button" class="btn primary" onclick={() => (hosts.editing = 0)}>Add host</button>
      </div>
    {:else if system.prompt}
      <div class="overlay">
        {#if system.prompt.kind === "hostKeyUnknown" || system.prompt.kind === "hostKeyChanged"}
          <HostKeyPrompt prompt={system.prompt} onaccept={() => system.acceptHostKey()} oncancel={() => system.cancelPrompt()} />
        {:else if system.prompt.kind === "passwordRequired" || system.prompt.kind === "passphraseRequired"}
          <AuthPrompt prompt={system.prompt} canRemember={ui.keychain} onsubmit={(s, r) => system.provideSecret(s, r)} oncancel={() => system.cancelPrompt()} />
        {/if}
      </div>
    {:else if tabBlock}
      <PlatformNote note={tabBlock} host={hostName} />
    {:else}
      {#if tabPartial}<PlatformNote note={tabPartial} host={hostName} />{/if}
    {/if}
    {#if !hosts.hosts.length || system.prompt || tabBlock}
      <!-- shown above -->
    {:else if system.tab === "units"}
      <UnitsPanel />
    {:else if system.tab === "journal"}
      <JournalPanel />
    {:else if system.tab === "schedules"}
      <SchedulesPanel />
    {:else if system.tab === "timeline"}
      <TimelinePanel />
    {:else if system.tab === "config"}
      <ConfigPanel />
    {:else if system.tab === "security"}
      <SecurityPanel />
    {:else if system.tab === "firewall"}
      <FirewallPanel />
    {:else if system.tab === "updates"}
      <UpdatesPanel />
    {:else if system.tab === "vms"}
      <VmsPanel />
    {:else}
      <DisksPanel />
    {/if}
  </div>
</div>

<SudoPrompt />

<Modal open={!!pending} title={pending ? `${pending.action.toUpperCase()} UNIT` : ""} tone="danger" width={520} onclose={() => (system.pendingUnit = null)}>
  {#if pending}
    <p>{actionVerb(pending.action)} <code>{pending.unit}</code> on <strong>{hostName}</strong>?</p>
    <p class="muted">{UNIT_CONSEQUENCE[pending.action] ?? ""}</p>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (system.pendingUnit = null)}>Cancel</button>
    <button type="button" class="btn danger" onclick={() => pending && system.act(pending.unit, pending.action)}>
      {pending ? `${actionVerb(pending.action)} ${pending.unit}` : ""}
    </button>
  {/snippet}
</Modal>

<Modal open={system.confirmUpgrade} title="UPGRADE PACKAGES" tone="danger" width={560} onclose={() => (system.confirmUpgrade = false)}>
  <p>Install <strong>{upCount}</strong> package upgrade{upCount === 1 ? "" : "s"} on <strong>{hostName}</strong>?</p>
  <ul class="targets">
    {#each (system.updates?.updates ?? []).filter((u) => !u.held).slice(0, 12) as p (p.name)}
      <li><code>{p.name}</code> <span class="muted">{p.current ?? ""} → {p.available}</span></li>
    {/each}
    {#if upCount > 12}<li class="muted">…and {upCount - 12} more</li>{/if}
  </ul>
  {#if system.updates?.held.length}<p class="muted">Held packages are left out: {system.updates.held.join(", ")}.</p>{/if}
  <p class="muted">
    Runs non-interactively with sudo; existing config files are kept. Services may restart. Keep Nightgrid connected until
    it finishes: a dropped connection can interrupt the package manager.
  </p>
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (system.confirmUpgrade = false)}>Cancel</button>
    <button type="button" class="btn danger" onclick={() => system.runPackages("upgrade")}>Upgrade {upCount} on {hostName}</button>
  {/snippet}
</Modal>

<Modal
  open={system.confirmPower !== null}
  title={system.confirmPower ? POWER_TITLE[system.confirmPower] : ""}
  tone="danger"
  width={500}
  onclose={() => (system.confirmPower = null)}
>
  <p>{system.confirmPower ? POWER_LABEL[system.confirmPower] : ""} <strong>{hostName}</strong> now?</p>
  <p class="muted">{system.confirmPower ? POWER_TEXT[system.confirmPower] : ""} Nightgrid shows it as {system.confirmPower === "suspend" ? "suspended" : system.confirmPower === "shutdown" ? "shut down" : "rebooting"}, not offline, and raises no offline alert.</p>
  <label class="field">
    <span>Type <strong class="verbatim">{hostName}</strong> to confirm</span>
    <input class="input" bind:value={typed} autocomplete="off" spellcheck="false" />
  </label>
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (system.confirmPower = null)}>Cancel</button>
    <button
      type="button"
      class="btn danger"
      disabled={!typedMatches(typed, hostName)}
      onclick={() => system.confirmPower && system.power(system.confirmPower)}>{system.confirmPower ? POWER_LABEL[system.confirmPower] : ""} {hostName}</button
    >
  {/snippet}
</Modal>

<Modal
  open={system.confirmGroupPower !== null}
  title="GROUP POWER"
  tone="danger"
  width={520}
  onclose={() => (system.confirmGroupPower = null)}
>
  {#if system.confirmGroupPower}
    {@const g = system.confirmGroupPower}
    <div class="gform">
      <label class="field"
        ><span>Group</span>
        <select class="input" value={g.groupId} onchange={(e) => (system.confirmGroupPower = { ...g, groupId: Number(e.currentTarget.value) })}>
          {#each hosts.groups as gr (gr.id)}<option value={gr.id}>{gr.name}</option>{/each}
        </select>
      </label>
      <label class="field"
        ><span>Action</span>
        <select class="input" value={g.action} onchange={(e) => (system.confirmGroupPower = { ...g, action: e.currentTarget.value as PowerAction })}>
          <option value="suspend">Suspend</option>
          <option value="shutdown">Shut down</option>
          <option value="reboot">Reboot</option>
        </select>
      </label>
    </div>
    <p>{POWER_LABEL[g.action]} these hosts, one after another:</p>
    <ul class="targets">
      {#each groupMembers as h (h.id)}<li>{h.name}</li>{:else}<li class="muted">No hosts in this group.</li>{/each}
    </ul>
    <p class="muted">{POWER_TEXT[g.action]} Hosts whose sudo needs a password that Nightgrid doesn't remember are skipped and listed.</p>
    <label class="field">
      <span>Type <strong class="verbatim">{groupName}</strong> to confirm</span>
      <input class="input" bind:value={groupTyped} autocomplete="off" spellcheck="false" />
    </label>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (system.confirmGroupPower = null)}>Cancel</button>
    <button
      type="button"
      class="btn danger"
      disabled={!groupMembers.length || !typedMatches(groupTyped, groupName)}
      onclick={() => system.confirmGroupPower && system.groupPower(system.confirmGroupPower.groupId, system.confirmGroupPower.action)}
      >{system.confirmGroupPower ? POWER_LABEL[system.confirmGroupPower.action] : ""} {groupMembers.length} host{groupMembers.length === 1 ? "" : "s"}</button
    >
  {/snippet}
</Modal>

<style>
  .grouprun {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
    align-items: center;
    font-size: var(--fs-xs);
  }
  .gr.done {
    color: var(--ok);
  }
  .gr.running {
    color: var(--warn);
  }
  .gr.failed {
    color: var(--danger);
  }
  .gform {
    display: flex;
    gap: 10px;
    margin-bottom: 8px;
  }
  .power {
    display: flex;
    gap: 4px;
    margin-left: auto;
  }
  .danger-text {
    color: var(--danger);
  }
  .system {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .hostpick {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .sudo {
    color: var(--text-1);
    letter-spacing: 0.1em;
  }
  .sudo.warn {
    color: var(--warn);
  }
  .notice {
    margin: 0;
    padding: 4px 8px;
    font-size: var(--fs-xs);
    border: 1px solid var(--line);
    display: flex;
    gap: 8px;
    align-items: center;
    justify-content: space-between;
  }
  .notice.ok {
    color: var(--mint);
    border-color: color-mix(in srgb, var(--mint) 50%, var(--line));
  }
  .notice.err {
    color: var(--danger);
    border-color: color-mix(in srgb, var(--danger) 50%, var(--line));
  }
  .x {
    border: 0;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
  .pane {
    flex: 1;
    min-height: 0;
  }
  .empty {
    padding: 24px;
    display: grid;
    gap: 10px;
    justify-items: start;
  }
  .empty p {
    margin: 0;
  }
  .overlay {
    padding: 24px;
    display: grid;
    place-items: center;
  }
  code {
    color: var(--cyan);
  }
  .targets {
    margin: 8px 0;
    padding-left: 18px;
    max-height: 220px;
    overflow: auto;
    font-size: var(--fs-sm);
  }
  .field {
    display: grid;
    gap: 6px;
    margin-top: 10px;
  }
</style>
