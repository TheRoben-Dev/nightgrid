<script lang="ts">
  import { onMount } from "svelte";
  import { errorText, type TunnelView } from "../api";
  import { openExternal, writeClipboard } from "../clipboard";
  import Panel from "../components/Panel.svelte";
  import Sparkline from "../components/metrics/Sparkline.svelte";
  import { formatSize } from "../files/util";
  import { hosts } from "../stores/hosts.svelte";
  import { isActive, KIND_LABEL, localEndpoint, tunnels } from "../stores/tunnels.svelte";

  onMount(() => tunnels.start());

  // Quick "open web UI": a port (and optional host) as the host sees it.
  let quickHost = $state<number | null>(null);
  let quickTarget = $state("localhost:8080");
  let quickBusy = $state(false);
  let quickMsg = $state<{ text: string; tone: "ok" | "err" } | null>(null);
  $effect(() => {
    if (quickHost === null && hosts.hosts.length) quickHost = hosts.hosts[0].id;
  });

  const groups = $derived.by(() => {
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- local, rebuilt on each read
    const by = new Map<number, TunnelView[]>();
    for (const t of tunnels.tunnels) by.set(t.hostId, [...(by.get(t.hostId) ?? []), t]);
    return [...by.entries()].map(([hostId, list]) => ({ hostId, name: hosts.byId(hostId)?.name ?? `host ${hostId}`, list }));
  });

  async function quickOpen(e: Event) {
    e.preventDefault();
    if (quickHost === null) return;
    const raw = quickTarget.trim();
    const url = /^https?:\/\//i.test(raw) ? raw : `http://${/^\d+$/.test(raw) ? `localhost:${raw}` : raw}`;
    quickBusy = true;
    quickMsg = null;
    try {
      const local = await tunnels.openThrough(quickHost, url);
      quickMsg = { text: `Opened ${local}`, tone: "ok" };
    } catch (err) {
      quickMsg = { text: errorText(err), tone: "err" };
    } finally {
      quickBusy = false;
    }
  }

  const stateWord = (t: TunnelView) => {
    const s = tunnels.stateOf(t.id);
    switch (s.kind) {
      case "running":
        return "UP";
      case "starting":
        return "STARTING";
      case "retrying":
        return `RETRY ${s.retrySecs}s`;
      case "failed":
        return "FAILED";
      default:
        return "OFF";
    }
  };
  const stateMsg = (t: TunnelView) => {
    const s = tunnels.stateOf(t.id);
    return s.kind === "failed" || s.kind === "retrying" ? s.message : "";
  };
  const rate = (id: number) => {
    const r = tunnels.rates[id];
    return r?.length ? `${formatSize(r.at(-1) ?? 0)}/s` : "";
  };
</script>

<div class="tunnels">
  <form class="toolbar" onsubmit={quickOpen} aria-label="Open a web UI through a tunnel">
    <span class="tlabel">OPEN WEB UI VIA</span>
    <select class="input" value={quickHost ?? ""} onchange={(e) => (quickHost = Number(e.currentTarget.value))} aria-label="Host" disabled={!hosts.hosts.length}>
      {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
    </select>
    <input class="input target" bind:value={quickTarget} placeholder="localhost:8080 or a URL" aria-label="Address as the host sees it" />
    <button type="submit" class="btn" disabled={quickBusy || quickHost === null || !quickTarget.trim()} data-guide="tunnels-open-web">
      {quickBusy ? "Opening…" : "Open"}
    </button>
    {#if quickMsg}<span class="qmsg {quickMsg.tone}" role="status">{quickMsg.text}</span>{/if}
    <span class="spacer"></span>
    <button type="button" class="btn primary" disabled={!hosts.hosts.length} onclick={() => tunnels.openNew(quickHost ?? hosts.hosts[0].id)} data-guide="tunnels-add">
      Add tunnel
    </button>
  </form>

  {#if tunnels.error}<p class="error-text" role="alert">{tunnels.error}</p>{/if}

  {#if tunnels.loaded && !tunnels.tunnels.length}
    <Panel title="NO TUNNELS" prefix=">">
      <div class="empty">
        <p class="label">NOTHING FORWARDED</p>
        <p class="muted">
          Save port forwards per host: <strong>local</strong> (reach a port that only listens on the host),
          <strong>remote</strong> (expose something on this computer to the host) or a <strong>SOCKS</strong> proxy that
          browses from the host's network. Auto-start keeps them up across reconnects. For a one-off, type a port above and
          press Open.
        </p>
        {#if hosts.hosts.length}
          <div class="actions">
            <button type="button" class="btn primary chamfer-sm" onclick={() => tunnels.openNew(hosts.hosts[0].id)}>Add tunnel</button>
            <button type="button" class="btn chamfer-sm" onclick={() => tunnels.openNew(hosts.hosts[0].id, "dynamic")}>SOCKS proxy</button>
          </div>
        {:else}
          <p class="muted">Add a host first.</p>
        {/if}
      </div>
    </Panel>
  {/if}

  {#each groups as g (g.hostId)}
    <Panel title={g.name.toUpperCase()} prefix=">" count={g.list.length}>
      <div class="table" role="table" aria-label="Tunnels on {g.name}">
        {#each g.list as t (t.id)}
          {@const s = tunnels.stateOf(t.id)}
          {@const endpoint = localEndpoint(t, s)}
          {@const tr = tunnels.traffic[t.id]}
          <div class="row {s.kind}" role="row">
            <span class="st" role="cell" title={stateMsg(t)}><span class="dot" aria-hidden="true"></span>{stateWord(t)}</span>
            <span class="name" role="cell">
              <span class="n">{t.name || KIND_LABEL[t.kind]}</span>
              <code class="spec">{t.spec}</code>
            </span>
            <span class="chips" role="cell">
              <span class="chip kind">{KIND_LABEL[t.kind]}</span>
              {#if t.autoStart}<span class="chip auto" title="Starts with Nightgrid and on reconnect">AUTO</span>{/if}
              {#if t.temporary}<span class="chip temp" title="Opened from Open web UI / a service; not saved">TEMP</span>{/if}
              {#if t.public}<span class="chip pub" title="Listens beyond loopback: reachable from other machines">PUBLIC</span>{/if}
            </span>
            <span class="ep" role="cell">
              {#if endpoint}
                <button type="button" class="link" title="Copy {endpoint}" onclick={() => writeClipboard(endpoint)}>{endpoint}</button>
              {:else if s.kind === "running" && t.kind === "remote"}
                <span class="muted">{g.name}:{s.boundPort}</span>
              {/if}
            </span>
            <span class="traffic" role="cell">
              {#if tr && isActive(s)}
                <span title="Received / sent">↓{formatSize(tr.rx)} ↑{formatSize(tr.tx)}</span>
                <span class="muted">{tr.active} open · {rate(t.id)}</span>
              {/if}
            </span>
            <span class="spark" role="cell">
              {#if (tunnels.rates[t.id]?.length ?? 0) > 1}
                <Sparkline values={tunnels.rates[t.id]} capacity={60} height={18} label="Traffic, last minute" />
              {/if}
            </span>
            <span class="acts" role="cell">
              {#if endpoint && t.kind === "local"}
                <button type="button" class="btn ghost tiny" onclick={() => openExternal(`http://${endpoint}/`)}>Browse</button>
              {/if}
              <button type="button" class="btn tiny" class:primary={!isActive(s)} disabled={tunnels.busy.includes(t.id)} onclick={() => tunnels.toggle(t.id)}>
                {isActive(s) ? (t.temporary ? "Close" : "Stop") : "Start"}
              </button>
              {#if !t.temporary}
                <button type="button" class="btn ghost tiny" onclick={() => (tunnels.editing = t.id)}>Edit</button>
              {/if}
            </span>
          </div>
          {#if stateMsg(t)}<p class="msg {s.kind}" role="status">{stateMsg(t)}</p>{/if}
        {/each}
      </div>
    </Panel>
  {/each}
</div>

<style>
  .tunnels {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .tlabel {
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
  }
  .target {
    width: 240px;
  }
  .qmsg.ok {
    color: var(--mint);
  }
  .qmsg.err {
    color: var(--danger);
  }
  .spacer {
    flex: 1;
  }
  .table {
    display: grid;
    gap: 2px;
  }
  .row {
    --tone: var(--text-1);
    display: grid;
    grid-template-columns: 100px minmax(160px, 1.4fr) minmax(120px, 1fr) 150px 170px 90px auto;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border: 1px solid var(--line);
    border-left: 2px solid var(--tone);
    background: var(--bg-1);
    font-size: var(--fs-sm);
  }
  .row.running {
    --tone: var(--ok);
  }
  .row.starting,
  .row.retrying {
    --tone: var(--warn);
  }
  .row.failed {
    --tone: var(--danger);
  }
  .st {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: 0.12em;
    color: var(--tone);
  }
  .dot {
    width: 6px;
    height: 6px;
    background: var(--tone);
  }
  .running .dot {
    box-shadow: 0 0 6px var(--ok);
  }
  .name {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .n {
    color: var(--text-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .spec {
    font-size: var(--fs-xs);
    color: var(--cyan);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .chip {
    padding: 0 5px;
    border: 1px solid var(--line);
    font-size: var(--fs-xs);
    letter-spacing: 0.06em;
    color: var(--text-1);
  }
  .chip.auto {
    color: var(--mint);
  }
  .chip.temp {
    color: var(--magenta);
  }
  .chip.pub {
    color: var(--warn);
    border-color: color-mix(in srgb, var(--warn) 50%, var(--line));
  }
  .link {
    all: unset;
    cursor: pointer;
    color: var(--cyan);
  }
  .link:hover,
  .link:focus-visible {
    text-decoration: underline;
  }
  .traffic {
    display: flex;
    flex-direction: column;
    font-size: var(--fs-xs);
    color: var(--cyan);
  }
  .spark {
    height: 18px;
  }
  .acts {
    display: flex;
    gap: 4px;
    justify-content: flex-end;
  }
  .tiny {
    min-height: 22px;
    padding: 0 10px;
    font-size: var(--fs-xs);
  }
  .msg {
    margin: -2px 0 2px;
    padding: 2px 10px;
    font-size: var(--fs-xs);
    color: var(--warn);
  }
  .msg.failed {
    color: var(--danger);
  }
  .empty p {
    margin: 0 0 10px;
    line-height: 1.6;
    max-width: 72ch;
  }
  .empty .label {
    color: var(--text-0);
    font-size: var(--fs-lg);
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 14px;
  }
  .error-text {
    margin: 0;
  }
  @media (max-width: 1200px) {
    .row {
      grid-template-columns: 90px minmax(140px, 1fr) minmax(100px, 1fr) auto;
    }
    .traffic,
    .spark,
    .ep {
      display: none;
    }
  }
</style>
