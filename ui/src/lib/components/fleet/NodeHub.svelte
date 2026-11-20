<script lang="ts">
  /**
   * Fleet → NODE → hub card (D-175): which server is the network's hub,
   * its members as the hub sees them, and making a hub, adding members and
   * removing it. Every root step goes through the sudo flow, per host.
   */
  import { api, errorText, type HubReport, type NodeSetup, type NodeView } from "../../api";
  import { bytesText, MEMBER_TEXT, memberTone, newer, roleOf, spanText } from "../../node";
  import { hosts } from "../../stores/hosts.svelte";
  import { system } from "../../stores/system.svelte";
  import Modal from "../Modal.svelte";

  interface Props {
    views: NodeView[];
    setups: Record<number, NodeSetup>;
    loading: boolean;
    onchange: (ids?: number[]) => Promise<void> | void;
    onopen: (hostId: number, tab: "config" | "logs", part: "hub") => void;
  }
  let { views, setups, loading, onchange, onopen }: Props = $props();

  const nameOf = (id: number) => hosts.hosts.find((h) => h.id === id)?.name ?? `host ${id}`;
  const isClient = (id: number) => hosts.hosts.find((h) => h.id === id)?.role === "client";

  const hubSetup = $derived(Object.values(setups).find((s) => s.hub));
  const hub = $derived(hubSetup?.hub?.overview ? { hostId: hubSetup.hostId, hubId: hubSetup.hub.overview.hubId } : null);
  const servers = $derived(views.filter((v) => v.state === "ready" && !isClient(v.hostId)));
  /** Hosts whose node can be a hub or a member (Node 0.26+): servers first, then client devices that already run the node. */
  const capable = $derived(
    views
      .filter((v) => v.state === "ready" && v.caps.includes("hub.member"))
      .sort((a, b) => Number(isClient(a.hostId)) - Number(isClient(b.hostId)) || nameOf(a.hostId).localeCompare(nameOf(b.hostId))),
  );
  /** Why "Make a hub" can't be pressed, said on the card (not only in a tooltip). */
  const cantMake = $derived.by(() => {
    if (loading || capable.length) return null;
    const old = views.filter((v) => v.state === "ready" && !v.caps.includes("hub.member"));
    if (old.length) return `A hub needs Nightgrid Node 0.26 or newer: press Update next to ${old.map((v) => nameOf(v.hostId)).join(", ")} below first.`;
    return "No host has Nightgrid Node running and connected yet: install it on the server that should be the hub (below), then come back here.";
  });
  const candidates = $derived(capable.filter((v) => roleOf(v, setups[v.hostId], hub) !== "member" && v.hostId !== hub?.hostId));
  const memberHosts = $derived(views.filter((v) => roleOf(v, setups[v.hostId], hub) === "member").map((v) => v.hostId));

  let busy = $state(false);
  let make = $state<{ hostId: number | null; fromFile: boolean } | null>(null);
  let add = $state<{ picked: number[] } | null>(null);
  let remove = $state<{ purge: boolean } | null>(null);
  let log = $state<{ title: string; lines: { text: string; tone: "ok" | "warn" | "err" | "muted" }[]; done: boolean } | null>(null);

  const say = (text: string, tone: "ok" | "warn" | "err" | "muted" = "muted") => {
    if (log) log.lines = [...log.lines, { text, tone }];
  };

  function report(host: string, r: HubReport) {
    for (const s of r.steps) say(`${host}: ${s}`, "ok");
    for (const w of r.warnings) say(`${host}: ${w}`, "warn");
  }

  /** The best first hub: a server that is up the most is unknowable here, so prefer one already linked. */
  function suggest(): number | null {
    const linked = capable.find((v) => !isClient(v.hostId) && setups[v.hostId]?.link?.access === "ok");
    return (linked ?? capable[0])?.hostId ?? null;
  }

  async function runMake() {
    const m = make;
    if (!m?.hostId) return;
    make = null;
    const id = m.hostId;
    busy = true;
    log = { title: `HUB · ${nameOf(id).toUpperCase()}`, lines: [], done: false };
    try {
      say(`Installing the hub on ${nameOf(id)}…`);
      const r = await system.withSudo("install the Nightgrid hub", (sudo) => api.hubInstall(id, m.fromFile, sudo), nameOf(id));
      if (!r) {
        say("Cancelled.");
        return;
      }
      report(nameOf(id), r.report);
      await onchange([id]);
      const joined = await join(id, [id]);
      if (joined) say(`${nameOf(id)} is the hub. Add the other servers as members next.`, "ok");
    } catch (e) {
      say(errorText(e), "err");
    } finally {
      busy = false;
      if (log) log.done = true;
    }
  }

  /** Joins each host to the hub on `hubHost`, then lets the hub reach the ones it can't yet. */
  async function join(hubHost: number, ids: number[]): Promise<boolean> {
    const nets: string[] = [];
    const listen: string[] = [];
    let ok = true;
    for (const id of ids) {
      say(`${nameOf(id)}: joining…`);
      try {
        const r = await system.withSudo("join the Nightgrid hub", (sudo) => api.hubJoin(id, hubHost, sudo), nameOf(id));
        for (const s of r.steps) say(`${nameOf(id)}: ${s}`, "ok");
        for (const w of r.warnings) say(`${nameOf(id)}: ${w}`, "warn");
        nets.push(...r.hubNetworks);
        listen.push(...r.hubListen.filter((a) => !listen.includes(a)));
      } catch (e) {
        ok = false;
        say(`${nameOf(id)}: ${errorText(e)}`, "err");
      }
    }
    if (nets.length || listen.length) {
      say(`${nameOf(hubHost)}: letting the collector reach ${[...nets, ...listen].join(", ")}…`);
      try {
        const r = await system.withSudo("let the hub reach its members", (sudo) => api.hubAllowNetworks(hubHost, nets, listen, sudo), nameOf(hubHost));
        for (const s of r.steps) say(`${nameOf(hubHost)}: ${s}`, r.ok ? "ok" : "err");
        for (const w of r.warnings) say(`${nameOf(hubHost)}: ${w}`, "warn");
        if (r.failure) say(r.failure, "err");
      } catch (e) {
        ok = false;
        say(`${nameOf(hubHost)}: ${errorText(e)}`, "err");
      }
    }
    await onchange([hubHost, ...ids]);
    return ok;
  }

  async function runAdd() {
    const a = add;
    if (!a || !hub || !a.picked.length) return;
    add = null;
    busy = true;
    log = { title: "ADD MEMBERS", lines: [], done: false };
    try {
      if (await join(hub.hostId, a.picked)) say("Done. The hub pulls each member within about a minute.", "ok");
    } finally {
      busy = false;
      if (log) log.done = true;
    }
  }

  async function leave(id: number) {
    if (!hub) return;
    const h = hub.hostId;
    busy = true;
    log = { title: `LEAVE · ${nameOf(id).toUpperCase()}`, lines: [], done: false };
    try {
      const r = await system.withSudo("leave the Nightgrid hub", (sudo) => api.hubLeave(id, h, sudo), nameOf(id));
      for (const s of r.steps) say(`${nameOf(id)}: ${s}`, "ok");
      for (const w of r.warnings) say(`${nameOf(id)}: ${w}`, "warn");
      say(`${nameOf(id)} is synced directly again.`, "ok");
    } catch (e) {
      say(`${nameOf(id)}: ${errorText(e)}`, "err");
    } finally {
      busy = false;
      if (log) log.done = true;
      await onchange([h, id]);
    }
  }

  async function runRemove() {
    const r = remove;
    if (!r || !hub) return;
    remove = null;
    const h = hub.hostId;
    busy = true;
    log = { title: `REMOVE HUB · ${nameOf(h).toUpperCase()}`, lines: [], done: false };
    try {
      for (const id of memberHosts) {
        try {
          const x = await system.withSudo("leave the Nightgrid hub", (sudo) => api.hubLeave(id, h, sudo), nameOf(id));
          for (const s of x.steps) say(`${nameOf(id)}: ${s}`, "ok");
        } catch (e) {
          say(`${nameOf(id)}: ${errorText(e)} (it keeps working on its own)`, "warn");
        }
      }
      const x = await system.withSudo("remove the Nightgrid hub", (sudo) => api.hubUninstall(h, r.purge, sudo), nameOf(h));
      report(nameOf(h), x.report);
      say("The hub is gone; every node keeps working and syncing directly.", "ok");
    } catch (e) {
      say(errorText(e), "err");
    } finally {
      busy = false;
      if (log) log.done = true;
      await onchange([h, ...memberHosts]);
    }
  }

  /** The node next to the hub is newer than the hub: they come from the same release and should match. */
  const hubBehind = $derived.by(() => {
    if (!hubSetup?.hub?.probe.version) return null;
    const node = views.find((v) => v.hostId === hubSetup.hostId)?.version;
    return node && newer(node, hubSetup.hub.probe.version) ? node : null;
  });

  async function updateHub() {
    if (!hubSetup) return;
    const id = hubSetup.hostId;
    busy = true;
    log = { title: `UPDATE HUB · ${nameOf(id).toUpperCase()}`, lines: [], done: false };
    try {
      say(`Updating the hub on ${nameOf(id)} (its config and store stay)…`);
      const r = await system.withSudo("update the Nightgrid hub", (sudo) => api.hubInstall(id, false, sudo), nameOf(id));
      if (r) {
        report(nameOf(id), r.report);
        say(`The hub is on ${r.report.version || "the new version"}.`, "ok");
      }
    } catch (e) {
      say(errorText(e), "err");
    } finally {
      busy = false;
      if (log) log.done = true;
      await onchange([id]);
    }
  }

  const ago = (t: number | null | undefined) => (t ? `${spanText(Date.now() / 1000 - t)} ago` : "never");
  const hostOfMember = (nodeId: string) => Object.values(setups).find((s) => s.nodeId === nodeId)?.hostId;
</script>

<section class="hub" data-guide="node-hub">
  {#if hubSetup?.hub}
    {@const h = hubSetup.hub}
    {@const o = h.overview}
    <div class="row">
      <h3 class="hd">HUB · {nameOf(hubSetup.hostId).toUpperCase()}</h3>
      <span class="small {o?.collector === 'ok' ? 'ok-text' : 'warn-text'}">● collector {o?.collector ?? h.probe.collect ?? "unknown"}</span>
      {#if o}<span class="muted small">{o.members.length} member{o.members.length === 1 ? "" : "s"} · {bytesText(o.storeBytes)} stored · Hub {h.probe.version || "unknown"}</span>{/if}
      {#if hubBehind}<span class="warn-text small" title="The hub is a separate program and isn't updated with the node">node is on {hubBehind}</span>{/if}
      <span class="grow"></span>
      {#if hubBehind}<button type="button" class="btn tiny primary" disabled={busy} onclick={updateHub}>Update hub</button>{/if}
      <button type="button" class="btn tiny" disabled={busy || !candidates.length} title={candidates.length ? "" : "Every server with Node 0.26+ is already a member"} onclick={() => (add = { picked: candidates.map((v) => v.hostId) })}>Add members…</button>
      <button type="button" class="btn tiny ghost" onclick={() => onopen(hubSetup.hostId, "config", "hub")}>Config</button>
      <button type="button" class="btn tiny ghost" onclick={() => onopen(hubSetup.hostId, "logs", "hub")}>Logs</button>
      <button type="button" class="btn tiny ghost danger" disabled={busy} onclick={() => (remove = { purge: false })}>Remove…</button>
    </div>
    {#if h.error}<p class="warn-text small">{h.error}</p>{/if}
    {#if o?.members.length}
      <div class="members">
        {#each o.members as m (m.id)}
          {@const hid = hostOfMember(m.id)}
          <div class="mrow">
            <span class="name">{hid !== undefined ? nameOf(hid) : m.name}</span>
            <span class="small state {memberTone(m.state)}">● {MEMBER_TEXT[m.state] ?? m.state}</span>
            <span class="muted small">{ago(m.lastContact)}{m.alerts ? ` · ${m.alerts} alert${m.alerts === 1 ? "" : "s"}` : ""}</span>
            <span class="muted small err">{m.error ?? ""}</span>
            {#if hid !== undefined && hid !== hubSetup.hostId}<button type="button" class="btn tiny ghost" disabled={busy} onclick={() => leave(hid)}>Leave</button>{:else}<span></span>{/if}
          </div>
        {/each}
      </div>
    {:else if o}
      <p class="muted small">No members yet. Add the servers this hub should watch.</p>
    {/if}
    {#if h.events.length}
      <details class="small">
        <summary class="muted">Hub events ({h.events.length})</summary>
        {#each h.events as e, i (i)}<p class="small {e.level === 'critical' ? 'error-text' : e.level === 'warn' ? 'warn-text' : 'muted'}">{new Date((e.at ?? 0) * 1000).toLocaleString()} · {e.title}</p>{/each}
      </details>
    {/if}
  {:else}
    <div class="row">
      <h3 class="hd">HUB</h3>
      <p class="muted small grow">
        {#if loading}Looking for a hub…{:else}No hub yet. A hub is one always-on server per network that watches the others while Nightgrid is closed: it pulls their history read-only and notices a host going offline. Each node keeps working on its own if the hub is down.{/if}
      </p>
      <button type="button" class="btn tiny" disabled={busy || loading || !capable.length} title={cantMake ?? ""} onclick={() => (make = { hostId: suggest(), fromFile: false })}>Make a hub…</button>
    </div>
    {#if cantMake}<p class="warn-text small">{cantMake}</p>{/if}
  {/if}
</section>

<Modal open={!!make} title="MAKE A HUB" width={520} onclose={() => (make = null)}>
  {#if make}
    <label class="small field">Hub host
      <select bind:value={make.hostId}>
        {#each capable as v (v.hostId)}<option value={v.hostId}>{nameOf(v.hostId)}{isClient(v.hostId) ? " (marked as a client device)" : ""}</option>{/each}
      </select>
    </label>
    {#if make.hostId != null && isClient(make.hostId)}<p class="warn-text small">{nameOf(make.hostId)} is marked as a client device (Hosts → edit → role). A hub should be always on; that's fine if this one is.</p>{/if}
    <p class="muted small">No Tailscale on this network? That works: hosts without a tailnet address use their LAN address for the link and the hub.</p>
    <p class="muted small">Pick the server that is always on (ideally on the UPS). Other servers report to it; ones only it can reach on its LAN are set up to use their LAN address.</p>
    <ol class="small steps">
      <li>Download the hub from the same Node release (checked against SHA256SUMS) and install it as root: its own account and sandboxed services.</li>
      <li>Turn on this host's node-to-node link and the hub's collector (mutual TLS, pinned certificates; it can only read, never run anything on members).</li>
      <li>Make this host the hub's first member. Then add the others.</li>
    </ol>
    <label class="small check"><input type="checkbox" bind:checked={make.fromFile} /> Use a nightgrid-hub binary file I pick instead</label>
  {/if}
  {#snippet footer()}
    <span class="sp"></span>
    <button type="button" class="btn ghost" onclick={() => (make = null)}>Cancel</button>
    <button type="button" class="btn primary" disabled={!make?.hostId} onclick={runMake}>Make hub</button>
  {/snippet}
</Modal>

<Modal open={!!add} title="ADD MEMBERS" width={520} onclose={() => (add = null)}>
  {#if add}
    <p class="small">Each picked server, as root on it: turns on its link, allows the hub's address, pairs with the hub and lets it read (<code>serve add sync</code>). You may be asked for each server's sudo password.</p>
    {#each candidates as v (v.hostId)}
      <label class="small check"><input type="checkbox" checked={add.picked.includes(v.hostId)} onchange={(e) => add && (add.picked = e.currentTarget.checked ? [...add.picked, v.hostId] : add.picked.filter((x) => x !== v.hostId))} /> {nameOf(v.hostId)}</label>
    {/each}
    {#if servers.length > capable.length}<p class="muted small">Servers on Node older than 0.26 aren't listed: update them first.</p>{/if}
  {/if}
  {#snippet footer()}
    <span class="sp"></span>
    <button type="button" class="btn ghost" onclick={() => (add = null)}>Cancel</button>
    <button type="button" class="btn primary" disabled={!add?.picked.length} onclick={runAdd}>Add {add?.picked.length ?? 0}</button>
  {/snippet}
</Modal>

<Modal open={!!remove} title="REMOVE THE HUB" width={500} onclose={() => (remove = null)}>
  {#if remove && hub}
    <p class="small">Takes every member out first ({memberHosts.length}), then removes the hub from <strong>{nameOf(hub.hostId)}</strong>. Nodes keep working and Nightgrid syncs each one directly, as before.</p>
    <label class="small check"><input type="checkbox" bind:checked={remove.purge} /> Also delete the hub's store, config and account</label>
  {/if}
  {#snippet footer()}
    <span class="sp"></span>
    <button type="button" class="btn ghost" onclick={() => (remove = null)}>Cancel</button>
    <button type="button" class="btn primary danger" onclick={runRemove}>Remove hub</button>
  {/snippet}
</Modal>

<Modal open={!!log} title={log?.title ?? ""} width={560} onclose={() => log?.done && (log = null)}>
  {#if log}
    <div class="log">
      {#each log.lines as l, i (i)}<p class="small {l.tone === 'ok' ? 'ok-text' : l.tone === 'warn' ? 'warn-text' : l.tone === 'err' ? 'error-text' : 'muted'}">{l.text}</p>{/each}
      {#if !log.done}<p class="muted small">Working…</p>{/if}
    </div>
  {/if}
  {#snippet footer()}
    <span class="sp"></span>
    <button type="button" class="btn primary" disabled={!log?.done} onclick={() => (log = null)}>Done</button>
  {/snippet}
</Modal>

<style>
  .hub {
    border: 1px solid var(--line);
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
  }
  .grow,
  .sp {
    flex: 1;
  }
  .hd {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    color: var(--text-2);
    letter-spacing: 0.1em;
    margin: 0;
  }
  .members {
    display: flex;
    flex-direction: column;
  }
  .mrow {
    display: grid;
    grid-template-columns: minmax(100px, 1fr) minmax(130px, 1fr) minmax(90px, auto) minmax(0, 2fr) auto;
    gap: 10px;
    align-items: center;
    padding: 3px 0;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-xs);
  }
  .err {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .state.ok,
  .ok-text {
    color: var(--ok);
  }
  .state.warn,
  .warn-text {
    color: var(--warn);
  }
  .state.bad {
    color: var(--bad);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .check,
  .field {
    display: flex;
    gap: 6px;
    align-items: center;
    margin: 4px 0;
  }
  .steps {
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .log {
    max-height: 50vh;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .log p {
    margin: 0;
  }
</style>
