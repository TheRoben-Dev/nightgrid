<script lang="ts">
  /**
   * Fleet → NODE (M12f): where the optional Nightgrid Node agent runs, what
   * it has handed over, and installing, updating or removing it.
   */
  import { api, errorText, type InstallReport, type NodeView, type Release } from "../../api";
  import { ACCESS_TEXT, bytesText, capsText, installAction, RES_TEXT, spanText, STATE_TEXT, stateTone, syncText } from "../../node";
  import { hosts } from "../../stores/hosts.svelte";
  import { library } from "../../stores/library.svelte";
  import { system } from "../../stores/system.svelte";
  import { ui } from "../../stores/ui.svelte";
  import Modal from "../Modal.svelte";

  let views = $state<NodeView[]>([]);
  let release = $state<Release | null>(null);
  let releaseErr = $state<string | null>(null);
  let error = $state<string | null>(null);
  let selected = $state<number | null>(null);
  let detail = $state<NodeView | null>(null);
  let busy = $state<string | null>(null);
  let notice = $state<{ text: string; tone: "ok" | "err" } | null>(null);
  let confirm = $state<{ kind: "install" | "uninstall"; v: NodeView; label: string; fromFile: boolean; purge: boolean } | null>(null);
  let report = $state<{ title: string; r: InstallReport } | null>(null);

  const nameOf = (id: number) => hosts.hosts.find((h) => h.id === id)?.name ?? `host ${id}`;
  const rows = $derived(
    views
      .filter((v) => hosts.hosts.some((h) => h.id === v.hostId))
      .sort((a, b) => Number(b.state === "ready") - Number(a.state === "ready") || nameOf(a.hostId).localeCompare(nameOf(b.hostId))),
  );
  const ready = $derived(rows.filter((v) => v.state === "ready").length);

  async function load() {
    try {
      views = await api.nodeList();
      error = null;
    } catch (e) {
      error = errorText(e);
    }
  }

  $effect(() => {
    void load();
    api
      .nodeRelease()
      .then((r) => (release = r))
      .catch((e) => (releaseErr = errorText(e)));
    const t = setInterval(load, 15_000);
    return () => clearInterval(t);
  });

  function put(v: NodeView) {
    views = [...views.filter((x) => x.hostId !== v.hostId), v];
    if (detail?.hostId === v.hostId) detail = { ...v, info: v.info ?? detail.info };
  }

  async function check(id: number) {
    selected = id;
    busy = `check-${id}`;
    try {
      const v = await api.nodeCheck(id);
      detail = v;
      put(v);
    } catch (e) {
      notice = { text: `${nameOf(id)}: ${errorText(e)}`, tone: "err" };
    } finally {
      busy = null;
    }
  }

  async function syncNow(id: number) {
    busy = `sync-${id}`;
    try {
      const r = await api.nodeSyncNow(id);
      notice = { text: `${nameOf(id)}: ${syncText(r)}.`, tone: "ok" };
      await load();
    } catch (e) {
      notice = { text: `${nameOf(id)}: ${errorText(e)}`, tone: "err" };
    } finally {
      busy = null;
    }
  }

  async function setUse(v: NodeView, on: boolean) {
    try {
      put(await api.nodeSetUse(v.hostId, on));
      if (on) void check(v.hostId);
    } catch (e) {
      notice = { text: errorText(e), tone: "err" };
    }
  }

  function askInstall(v: NodeView) {
    const label = installAction(v, release) ?? "Update";
    confirm = { kind: "install", v, label, fromFile: false, purge: false };
  }

  async function run() {
    const c = confirm;
    if (!c) return;
    confirm = null;
    const id = c.v.hostId;
    busy = `${c.kind}-${id}`;
    try {
      if (c.kind === "install") {
        const r = await system.withSudo("install Nightgrid Node", (sudo) => api.nodeInstall(id, c.fromFile, sudo), nameOf(id));
        if (!r) return;
        put(r.view);
        report = { title: `${c.label.toUpperCase()} · ${nameOf(id).toUpperCase()}`, r: r.report };
      } else {
        const r = await system.withSudo("remove Nightgrid Node", (sudo) => api.nodeUninstall(id, c.purge, sudo), nameOf(id));
        put(r.view);
        detail = null;
        report = { title: `REMOVED · ${nameOf(id).toUpperCase()}`, r: r.report };
      }
    } catch (e) {
      notice = { text: `${nameOf(id)}: ${errorText(e)}`, tone: "err" };
    } finally {
      busy = null;
    }
  }

  const when = (t: number | null | undefined) => (t ? new Date(t * 1000).toLocaleString() : "–");
  const ago = (t: number | null | undefined) => {
    if (!t) return "never";
    const s = Date.now() / 1000 - t;
    return s < 90 ? "just now" : `${spanText(s)} ago`;
  };
</script>

<div class="node">
  <div class="row">
    <p class="muted small grow">
      Nightgrid Node is an optional agent that keeps watching a server while this app is closed and hands everything over when it reconnects: graphs and uptime without gaps, alerts that fired meanwhile, and live samples without polling over SSH. No open ports, no outbound connections.
      <button type="button" class="link" onclick={() => (library.open("basics/nightgrid-node"), ui.go("library"))}>What it is ▤</button>
    </p>
    <span class="muted small">{ready} of {rows.length} connected · latest {release ? release.tag : releaseErr ? "unknown" : "…"}</span>
    <button type="button" class="btn tiny" onclick={load}>Refresh</button>
  </div>
  {#if error}<p class="error-text small">{error}</p>{/if}
  {#if notice}<p class="small {notice.tone === 'err' ? 'error-text' : 'ok-text'}">{notice.text}</p>{/if}

  <div class="body">
    <div class="list">
      <div class="trow head" aria-hidden="true"><span>HOST</span><span>NODE</span><span>VERSION</span><span>LAST SYNC</span><span></span></div>
      {#each rows as v (v.hostId)}
        {@const act = installAction(v, release)}
        <div class="trow" class:sel={selected === v.hostId} data-guide="node-host">
          <button type="button" class="name link" onclick={() => check(v.hostId)} title="Look for the node and show its details">{nameOf(v.hostId)}</button>
          <span class="state {stateTone(v.state)}">● {STATE_TEXT[v.state]}{#if v.state === "ready" && v.metricsLive}<span class="muted">&nbsp;· live</span>{/if}</span>
          <span class="data">{v.version ?? "–"}</span>
          <span class="muted small">{v.syncing ? "syncing…" : ago(v.syncedAt)}</span>
          <span class="acts">
            {#if act}<button type="button" class="btn tiny" disabled={!!busy} onclick={() => askInstall(v)}>{act}…</button>{/if}
            {#if v.state === "unknown" || v.state === "error"}<button type="button" class="btn tiny ghost" disabled={!!busy} onclick={() => check(v.hostId)}>{busy === `check-${v.hostId}` ? "Looking…" : "Check"}</button>{/if}
          </span>
        </div>
      {:else}
        <p class="muted small pad">No hosts yet.</p>
      {/each}
      <p class="muted small pad">Hosts are checked when Nightgrid connects to them; nothing is installed without you. Turn off the whole thing in Settings → Features → Fleet → Nightgrid Node.</p>
    </div>

    {#if detail}
      {@const v = detail}
      <aside class="detail" data-guide="node-detail">
        <h3 class="hd">{nameOf(v.hostId).toUpperCase()}</h3>
        <p class="state {stateTone(v.state)}">● {STATE_TEXT[v.state]}</p>
        {#if v.message}<p class="muted small">{v.message}</p>{/if}
        {#if v.state === "ready"}
          <dl>
            <dt>Version</dt><dd class="data">{v.version} · protocol {v.proto} · id {v.nodeId}</dd>
            <dt>Uses</dt><dd>{capsText(v.caps)}</dd>
            <dt>Live</dt><dd>{v.eventsLive ? "events as they happen" : "events every 15 minutes"}{v.metricsLive ? ", samples every 2 s" : ""}</dd>
            <dt>Last sync</dt><dd>{v.syncing ? "running…" : `${ago(v.syncedAt)}: ${syncText(v.lastSync)}`}{v.lastSync?.pruned ? "; some older events were pruned on the node" : ""}</dd>
            {#if v.info}
              <dt>Running</dt><dd>since {when(v.info.started)} · samples every {v.info.intervalSecs} s · {bytesText(v.info.rssBytes)} RAM · {bytesText(v.info.dbBytes)} history</dd>
            {/if}
          </dl>
          {#if v.rulesNote}<p class="warn-text small">{v.rulesNote}</p>{/if}
          {#if v.info?.coverage.length}
            <h4 class="sub">HISTORY ON THE NODE</h4>
            {#each v.info.coverage as c (c.res)}
              <div class="kv"><span>{RES_TEXT[c.res] ?? `${c.res} s`}</span><span class="data">{(c.points ?? 0).toLocaleString("en-US")} points{c.oldest ? ` · ${spanText(Date.now() / 1000 - c.oldest)}` : ""}</span></div>
            {/each}
          {/if}
          {#if v.info?.access.length}
            <h4 class="sub">GRANTED BY ROOT</h4>
            {#each v.info.access as a (a.name)}
              <div class="kv"><span>{a.name} <span class="muted">({ACCESS_TEXT[a.name] ?? "optional"})</span></span><span class={a.state === "ok" ? "ok-text" : "muted"}>{a.state}</span></div>
            {/each}
            <p class="muted small">Grant more on the host with <code>sudo nightgrid-node allow &lt;name&gt;</code>, or run <code>sudo nightgrid-node allow</code> for a menu.</p>
            <p class="muted small">Fix things on the host: <code>nightgrid-node fix</code></p>
          {/if}
          {#if v.info}
            <h4 class="sub">ALERTS ON THE NODE</h4>
            {#each v.info.alerts as a (a.key)}<p class="small {a.severity === 'critical' ? 'error-text' : 'warn-text'}">{a.title}: {a.detail}</p>{:else}<p class="muted small">None active.</p>{/each}
          {/if}
        {/if}
        <div class="row foot">
          {#if v.state === "ready"}<button type="button" class="btn tiny" disabled={!!busy} onclick={() => syncNow(v.hostId)}>{busy === `sync-${v.hostId}` ? "Syncing…" : "Sync now"}</button>{/if}
          <button type="button" class="btn tiny ghost" disabled={!!busy} onclick={() => check(v.hostId)}>{busy === `check-${v.hostId}` ? "Looking…" : "Check again"}</button>
          <span class="grow"></span>
          <label class="small check"><input type="checkbox" checked={v.useNode} onchange={(e) => setUse(v, e.currentTarget.checked)} /> Use on this host</label>
          {#if v.state !== "notInstalled" && v.state !== "unknown"}<button type="button" class="btn tiny ghost danger" disabled={!!busy} onclick={() => (confirm = { kind: "uninstall", v, label: "Remove", fromFile: false, purge: false })}>Remove…</button>{/if}
        </div>
      </aside>
    {/if}
  </div>
</div>

<Modal open={!!confirm} title={confirm ? `${confirm.label.toUpperCase()} NIGHTGRID NODE · ${nameOf(confirm.v.hostId).toUpperCase()}` : ""} width={500} onclose={() => (confirm = null)}>
  {#if confirm?.kind === "install"}
    <p class="small">On <strong>{nameOf(confirm.v.hostId)}</strong>, as root:</p>
    <ol class="small steps">
      <li>{confirm.fromFile ? "Upload the file you pick" : `Download ${release?.tag ?? "the latest release"} for ${confirm.v.arch || "its architecture"} from GitHub, check it against the release's SHA256SUMS, and upload it`} to a private temp folder.</li>
      <li>Run its <code>install</code>: its own system account, a sandboxed systemd service, no open ports and no outbound network.</li>
      <li>Let <strong>{confirm.v.user || "your login user"}</strong> talk to it (the <code>nightgrid-node</code> group). History and settings are kept on an update.</li>
    </ol>
    <label class="small check"><input type="checkbox" bind:checked={confirm.fromFile} /> Use a binary file I pick instead</label>
  {:else if confirm}
    <p class="small">Stops and removes Nightgrid Node from <strong>{nameOf(confirm.v.hostId)}</strong>, as root. What Nightgrid already synced stays here.</p>
    <label class="small check"><input type="checkbox" bind:checked={confirm.purge} /> Also delete its history, settings and account on the server</label>
  {/if}
  {#snippet footer()}
    <span class="sp"></span>
    <button type="button" class="btn ghost" onclick={() => (confirm = null)}>Cancel</button>
    <button type="button" class="btn primary" class:danger={confirm?.kind === "uninstall"} onclick={run}>{confirm?.label ?? "OK"}</button>
  {/snippet}
</Modal>

<Modal open={!!report} title={report?.title ?? ""} width={500} onclose={() => (report = null)}>
  {#if report}
    {#if report.r.version}<p class="small">Nightgrid Node {report.r.version}</p>{/if}
    <ul class="small steps">{#each report.r.steps as s, i (i)}<li>{s}</li>{/each}</ul>
    {#each report.r.warnings as w, i (i)}<p class="warn-text small">{w}</p>{/each}
    {#if report.r.users.length}<p class="muted small">Added {report.r.users.join(", ")} to the nightgrid-node group. Nightgrid uses it right away; a terminal needs a new login.</p>{/if}
  {/if}
  {#snippet footer()}
    <span class="sp"></span>
    <button type="button" class="btn primary" onclick={() => (report = null)}>Done</button>
  {/snippet}
</Modal>

<style>
  .node {
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
  .grow,
  .sp {
    flex: 1;
  }
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(280px, 380px);
    gap: 12px;
  }
  .list,
  .detail {
    overflow: auto;
    min-height: 0;
  }
  .detail {
    border: 1px solid var(--line);
    padding: 8px 12px;
  }
  .trow {
    display: grid;
    grid-template-columns: minmax(110px, 1fr) minmax(150px, 1.2fr) 80px 100px minmax(120px, auto);
    gap: 10px;
    align-items: center;
    padding: 5px 4px;
    border-bottom: 1px solid var(--line);
  }
  .trow.sel {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }
  .trow.head {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    color: var(--text-2);
  }
  .acts {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
  }
  .name {
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--cyan);
    font: inherit;
    cursor: pointer;
  }
  .state.ok {
    color: var(--ok);
  }
  .state.warn {
    color: var(--warn);
  }
  .state.bad {
    color: var(--bad);
  }
  .state.muted {
    color: var(--text-2);
  }
  .hd,
  .sub {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    color: var(--text-2);
    letter-spacing: 0.1em;
    margin: 4px 0;
  }
  .sub {
    margin-top: 12px;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 10px;
    font-size: var(--fs-xs);
    margin: 8px 0;
  }
  dt {
    color: var(--text-2);
  }
  dd {
    margin: 0;
  }
  .kv {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: var(--fs-xs);
    padding: 2px 0;
  }
  .foot {
    margin-top: 12px;
    border-top: 1px solid var(--line);
    padding-top: 8px;
  }
  .check {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .steps {
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .pad {
    padding: 8px 4px;
  }
  .ok-text {
    color: var(--ok);
  }
  .warn-text {
    color: var(--warn);
  }
  @media (max-width: 900px) {
    .body {
      grid-template-columns: 1fr;
    }
  }
</style>
