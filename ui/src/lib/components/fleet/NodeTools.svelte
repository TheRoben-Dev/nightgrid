<script lang="ts">
  /**
   * Fleet → NODE → one host's setup (D-175): its link and hub role, the
   * node's (and hub's) config with a checked save that rolls back, status
   * from `doctor` and the units, and the journal.
   */
  import { api, errorText, type ConfigSaveReport, type NodePart, type NodeSetup } from "../../api";
  import { ROLE_TEXT, type NodeRole } from "../../node";
  import { system } from "../../stores/system.svelte";

  interface Props {
    hostId: number;
    name: string;
    setup: NodeSetup | undefined;
    role: NodeRole;
    hubName: string | null;
    /** The hub's host, to fix a member's link to it. */
    hubHost?: number | null;
    tab: "status" | "link" | "config" | "logs";
    part: NodePart;
    onjoin: () => void;
    onleave: () => void;
    onchange: () => Promise<void> | void;
  }
  let { hostId, name, setup, role, hubName, hubHost = null, tab, part = $bindable(), onjoin, onleave, onchange }: Props = $props();

  let text = $state("");
  let original = $state("");
  let path = $state("");
  let loadedFor = $state("");
  let busy = $state(false);
  let saved = $state<ConfigSaveReport | null>(null);
  let error = $state<string | null>(null);
  let logs = $state("");
  let lines = $state(200);

  const hasHub = $derived(!!setup?.hub);
  const key = $derived(`${hostId}:${part}`);

  async function loadConfig() {
    error = null;
    saved = null;
    try {
      const f = await api.nodeConfigRead(hostId, part);
      text = f.text;
      original = f.text;
      path = f.path;
      loadedFor = key;
      if (!f.exists) error = `${f.path} doesn't exist yet: the defaults apply. Saving creates it.`;
    } catch (e) {
      error = errorText(e);
    }
  }

  async function save() {
    busy = true;
    error = null;
    saved = null;
    try {
      saved = await system.withSudo(`save the ${part === "hub" ? "hub" : "node"} config`, (sudo) => api.nodeConfigSave(hostId, part, text, sudo), name);
      if (saved.ok) original = text;
      await onchange();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function loadLogs() {
    busy = true;
    error = null;
    try {
      logs = await system.withSudo("read the node's logs", (sudo) => api.nodeLogs(hostId, part, lines, sudo), name);
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    if (tab === "config" && loadedFor !== key) void loadConfig();
  });
  $effect(() => {
    void key;
    if (tab === "logs") logs = "";
  });

  const link = $derived(setup?.link);
  let repaired = $state<ConfigSaveReport | null>(null);
  let linkOn = $state<ConfigSaveReport | null>(null);

  /** Turns the link on as root; without a tailnet address it listens on the LAN. */
  async function enableLink() {
    busy = true;
    error = null;
    linkOn = null;
    try {
      linkOn = await system.withSudo("turn on the node-to-node link", (sudo) => api.nodeLinkEnable(hostId, sudo), name);
      await onchange();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  /** A member can't reach its hub: Nightgrid fixes it on the hub host (update the hub, listen where the member pings). */
  async function repair() {
    if (hubHost == null) return;
    const h = hubHost;
    busy = true;
    error = null;
    repaired = null;
    try {
      repaired = await system.withSudo(`let ${name} reach the hub`, (sudo) => api.hubRepairLink(hostId, h, sudo), hubName ?? undefined);
      await onchange();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }
  const peerName = (id: string) => link?.peers.find((p) => p.id === id)?.name ?? id.slice(0, 8);
</script>

{#if (tab === "config" || tab === "logs") && hasHub}
  <div class="seg tabstrip sm" role="group" aria-label="Which program">
    <button type="button" class:on={part === "node"} onclick={() => (part = "node")}>NODE</button>
    <button type="button" class:on={part === "hub"} onclick={() => (part = "hub")}>HUB</button>
  </div>
{/if}

{#if error}<p class="warn-text small">{error}</p>{/if}

{#if tab === "status"}
  {#if setup?.units.length}
    <h4 class="sub">SERVICES</h4>
    {#each setup.units as u (u.unit)}
      <div class="kv"><span class="data">{u.unit}</span><span class={u.state === "active" ? "ok-text" : u.state === "failed" ? "error-text" : "muted"}>{u.state}</span></div>
    {/each}
  {/if}
  {#if setup?.doctor.length}
    <h4 class="sub">DOCTOR</h4>
    {#each setup.doctor as f, i (i)}
      <div class="kv"><span>{f.check}</span><span class="detail {f.level === 'ok' ? 'ok-text' : f.level === 'fail' ? 'error-text' : f.level === 'warn' ? 'warn-text' : 'muted'}" title={f.detail}>{f.detail}</span></div>
    {/each}
  {/if}
  {#each setup?.notes ?? [] as n, i (i)}<p class="muted small">{n}</p>{/each}
{:else if tab === "link"}
  <h4 class="sub">ROLE</h4>
  <p class="small">{ROLE_TEXT[role]}{role === "member" && hubName ? ` ${hubName}` : ""}</p>
  {#if role === "hub"}
    <p class="muted small">This host runs the hub, and is its own first member.</p>
  {:else if role === "member"}
    <p class="muted small">The hub pulls this node's history read-only. Nightgrid still syncs and samples this host directly.</p>
    <button type="button" class="btn tiny ghost" onclick={onleave}>Leave the hub…</button>
  {:else if role === "otherHub"}
    <p class="muted small">This node names a hub Nightgrid doesn't know (another network, or a hub that was removed).</p>
    <button type="button" class="btn tiny ghost" onclick={onleave}>Forget that hub…</button>
  {:else if !link}
    <p class="muted small">Needs Nightgrid Node 0.20 or newer, connected, for linking.</p>
  {:else if !link.canJoin}
    <p class="muted small">Joining a hub needs Nightgrid Node 0.26 or newer: update this host first.</p>
  {:else if hubName}
    <button type="button" class="btn tiny" onclick={onjoin}>Report to the hub on {hubName}…</button>
  {:else}
    <p class="muted small">No hub yet: make one above, then add this host.</p>
  {/if}
  {#if link}
    <h4 class="sub">NODE-TO-NODE LINK</h4>
    <div class="kv"><span>State</span><span class={link.access === "ok" ? "ok-text" : "muted"}>{link.access === "off" ? "off (not allowed by root yet)" : link.access}</span></div>
    <div class="kv"><span>Listens on</span><span class="data">{link.listening ?? (link.access === "ok" ? "nowhere (no tailnet address)" : "nowhere yet")}</span></div>
    <div class="kv"><span>Serves peers</span><span class="data">{link.serve.join(", ") || "nothing"}</span></div>
    {#if link.access !== "ok" || !link.listening}
      <p class="muted small">
        {#if link.access !== "ok"}The link is off until root allows it (<code>sudo nightgrid-node allow peers</code>).{:else}The link has no tailnet address to listen on.{/if}
        Making a hub or joining one turns it on for you{#if !link.listening}; without Tailscale it uses this host's LAN address (<code>[peers] listen = "lan"</code>){/if}. Or do it now:
      </p>
      <div class="row"><button type="button" class="btn tiny" disabled={busy} onclick={enableLink}>{busy ? "Turning on…" : "Turn on the link"}</button></div>
    {/if}
    {#if linkOn}
      {#each linkOn.steps as s, i (i)}<p class="small {linkOn.ok ? 'ok-text' : 'muted'}">{s}</p>{/each}
      {#each linkOn.warnings as w, i (i)}<p class="warn-text small">{w}</p>{/each}
      {#if linkOn.failure}<pre class="error-text small fail">{linkOn.failure}</pre>{/if}
    {/if}
    {#if link.readings.length}
      <h4 class="sub">LINKED</h4>
      {#each link.readings as r (r.id)}
        <div class="kv"><span>{peerName(r.id)}</span><span class={r.ok ? "ok-text" : "warn-text"} title={r.error ?? ""}>{r.ok ? `reachable${r.rttMs != null ? ` · ${r.rttMs} ms` : ""}` : (r.error ?? "unreachable")}</span></div>
        {#if !r.ok && r.id === link.hubId && hubHost != null}
          <p class="muted small">The hub still pulls this host (it connects out), but this node can't reach the hub back for its pings. Usually the hub is older than its node, or its collector doesn't answer on the address this host uses.</p>
          <div class="row"><button type="button" class="btn tiny primary" disabled={busy} onclick={repair}>{busy ? "Fixing…" : `Fix on ${hubName ?? "the hub"}`}</button></div>
        {/if}
      {/each}
      {#if repaired}
        {#each repaired.steps as s, i (i)}<p class="small {repaired.ok ? 'ok-text' : 'muted'}">{s}</p>{/each}
        {#each repaired.warnings as w, i (i)}<p class="warn-text small">{w}</p>{/each}
        {#if repaired.failure}<pre class="error-text small fail">{repaired.failure}</pre>{/if}
        {#if repaired.ok}<p class="muted small">The link checks again within a minute (or press Check again).</p>{/if}
      {/if}
    {/if}
  {/if}
{:else if tab === "config"}
  <p class="muted small">{path} · root-owned. Saving checks it first{part === "node" ? " with the node's own doctor" : ""}, keeps the old file as <code>.bak</code>, restarts the services, re-applies changed <code>networks</code>, and puts the old file back if the service doesn't stay up.</p>
  <textarea class="conf data" spellcheck="false" bind:value={text} aria-label="Config file"></textarea>
  <div class="row">
    <button type="button" class="btn tiny primary" disabled={busy || text === original} onclick={save}>{busy ? "Saving…" : "Save"}</button>
    <button type="button" class="btn tiny ghost" disabled={busy || text === original} onclick={() => (text = original)}>Revert</button>
    <button type="button" class="btn tiny ghost" disabled={busy} onclick={loadConfig}>Reload</button>
  </div>
  {#if saved}
    {#each saved.steps as s, i (i)}<p class="small {saved.ok ? 'ok-text' : 'muted'}">{s}</p>{/each}
    {#each saved.warnings as w, i (i)}<p class="warn-text small">{w}</p>{/each}
    {#if saved.failure}<pre class="error-text small fail">{saved.failure}</pre>{/if}
  {/if}
{:else}
  <div class="row">
    <label class="small">Lines
      <select bind:value={lines}>
        {#each [100, 200, 500, 1000] as n (n)}<option value={n}>{n}</option>{/each}
      </select>
    </label>
    <button type="button" class="btn tiny" disabled={busy} onclick={loadLogs}>{busy ? "Loading…" : logs ? "Reload" : "Load logs"}</button>
  </div>
  {#if logs}<pre class="logs data">{logs}</pre>{:else}<p class="muted small">The journal of the {part === "hub" ? "hub's" : "node's"} services (as root).</p>{/if}
{/if}

<style>
  .seg {
    display: inline-flex;
    border: 1px solid var(--line);
    margin: 6px 0;
  }
  .sub {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    color: var(--text-2);
    letter-spacing: 0.1em;
    margin: 12px 0 4px;
  }
  .kv {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: var(--fs-xs);
    padding: 2px 0;
  }
  .detail {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 65%;
  }
  .row {
    display: flex;
    gap: 6px;
    align-items: center;
    margin: 6px 0;
  }
  .conf {
    width: 100%;
    min-height: 320px;
    resize: vertical;
    font-size: var(--fs-xs);
    background: var(--bg-2, transparent);
    color: var(--text);
    border: 1px solid var(--line);
    padding: 6px;
  }
  .logs,
  .fail {
    white-space: pre-wrap;
    word-break: break-all;
    font-size: var(--fs-xs);
    max-height: 420px;
    overflow: auto;
    border: 1px solid var(--line);
    padding: 6px;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .ok-text {
    color: var(--ok);
  }
  .warn-text {
    color: var(--warn);
  }
</style>
