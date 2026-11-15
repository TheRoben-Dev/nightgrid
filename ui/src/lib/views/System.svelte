<script lang="ts">
  import { scramble } from "../scramble";
  import Modal from "../components/Modal.svelte";
  import AuthPrompt from "../components/terminal/AuthPrompt.svelte";
  import HostKeyPrompt from "../components/terminal/HostKeyPrompt.svelte";
  import DisksPanel from "../components/system/DisksPanel.svelte";
  import JournalPanel from "../components/system/JournalPanel.svelte";
  import SchedulesPanel from "../components/system/SchedulesPanel.svelte";
  import SecurityPanel from "../components/system/SecurityPanel.svelte";
  import SudoPrompt from "../components/system/SudoPrompt.svelte";
  import UnitsPanel from "../components/system/UnitsPanel.svelte";
  import UpdatesPanel from "../components/system/UpdatesPanel.svelte";
  import BackupsPanel from "../components/system/BackupsPanel.svelte";
  import VmsPanel from "../components/system/VmsPanel.svelte";
  import { hosts } from "../stores/hosts.svelte";
  import { actionVerb, system, UNIT_CONSEQUENCE, type SystemTab } from "../stores/system.svelte";
  import { ui } from "../stores/ui.svelte";

  // Pick a host once the host list has loaded.
  $effect(() => {
    if (system.hostId === null && hosts.hosts.length) system.select(hosts.hosts[0].id);
  });

  const TABS: { id: SystemTab; label: string }[] = [
    { id: "units", label: "UNITS" },
    { id: "journal", label: "JOURNAL" },
    { id: "schedules", label: "SCHEDULES" },
    { id: "security", label: "SECURITY" },
    { id: "updates", label: "UPDATES" },
    { id: "disks", label: "DISKS" },
    { id: "vms", label: "VMS" },
    { id: "backups", label: "BACKUPS" },
  ];

  const SUDO_WORD = { root: "ROOT", noPassword: "SUDO · NOPASSWD", password: "SUDO · PASSWORD", missing: "NO SUDO" } as const;

  const hostName = $derived(system.host?.name ?? "");
  const pending = $derived(system.pendingUnit);
  const upCount = $derived(system.updates?.updates.length ?? 0);
  let typed = $state("");
  $effect(() => {
    if (!system.confirmPower) typed = "";
  });
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
    <div class="tabs" role="tablist" aria-label="System view">
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
      <span class="sudo" class:warn={system.info.sudo === "missing"} title="How Nightgrid gets root on this host">{SUDO_WORD[system.info.sudo]}</span>
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
      </span>
    {/if}
  </div>

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
    {:else if system.tab === "backups"}
      <BackupsPanel />
    {:else if system.prompt}
      <div class="overlay">
        {#if system.prompt.kind === "hostKeyUnknown" || system.prompt.kind === "hostKeyChanged"}
          <HostKeyPrompt prompt={system.prompt} onaccept={() => system.acceptHostKey()} oncancel={() => system.cancelPrompt()} />
        {:else if system.prompt.kind === "passwordRequired" || system.prompt.kind === "passphraseRequired"}
          <AuthPrompt prompt={system.prompt} canRemember={ui.keychain} onsubmit={(s, r) => system.provideSecret(s, r)} oncancel={() => system.cancelPrompt()} />
        {/if}
      </div>
    {:else if system.tab === "units"}
      <UnitsPanel />
    {:else if system.tab === "journal"}
      <JournalPanel />
    {:else if system.tab === "schedules"}
      <SchedulesPanel />
    {:else if system.tab === "security"}
      <SecurityPanel />
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
    {#each (system.updates?.updates ?? []).slice(0, 12) as p (p.name)}
      <li><code>{p.name}</code> <span class="muted">{p.current ?? ""} → {p.available}</span></li>
    {/each}
    {#if upCount > 12}<li class="muted">…and {upCount - 12} more</li>{/if}
  </ul>
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
    <span>Type <strong>{hostName}</strong> to confirm</span>
    <input class="input" bind:value={typed} autocomplete="off" spellcheck="false" />
  </label>
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (system.confirmPower = null)}>Cancel</button>
    <button
      type="button"
      class="btn danger"
      disabled={typed.trim() !== hostName}
      onclick={() => system.confirmPower && system.power(system.confirmPower)}>{system.confirmPower ? POWER_LABEL[system.confirmPower] : ""} {hostName}</button
    >
  {/snippet}
</Modal>

<style>
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
  .tabs {
    display: flex;
    border: 1px solid var(--line);
  }
  .tabs button {
    border: 0;
    background: transparent;
    color: var(--text-1);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    padding: 3px 10px;
    cursor: pointer;
  }
  .tabs button + button {
    border-left: 1px solid var(--line);
  }
  .tabs button.on {
    color: var(--magenta);
    box-shadow: inset 0 -2px 0 var(--magenta);
  }
  .badge {
    margin-left: 6px;
    padding: 0 4px;
    color: var(--bg-0);
    background: var(--cyan);
  }
  .badge.warn {
    background: var(--warn);
  }
  .badge.bad {
    background: var(--danger);
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
  .tiny {
    min-height: 22px;
    padding: 0 8px;
    font-size: var(--fs-xs);
  }
</style>
