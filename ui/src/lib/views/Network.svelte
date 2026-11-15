<script lang="ts">
  import type { Exposure, ExposureKind, Listener, ListenScope } from "../api";
  import Toggle from "../components/Toggle.svelte";
  import { exposureIp, seenText, serviceLabel, type MapLink, type MapNode } from "../netmap/graph";
  import { hosts } from "../stores/hosts.svelte";
  import { netmap } from "../stores/netmap.svelte";
  import { system } from "../stores/system.svelte";

  const W = 196;
  const H = 54;
  const COL = 236;
  const ROW = 66;
  const TOP = 34;
  const HEADS = ["INTERNET", "NETWORKS", "HOSTS", "OTHER DEVICES"];

  const g = $derived(netmap.graph);
  const rows = $derived(Math.max(1, ...g.nodes.map((n) => n.row + 1)));
  const width = COL * 3 + W + 24;
  const height = $derived(TOP + rows * ROW + 12);
  const byId = $derived(new Map(g.nodes.map((n) => [n.id, n])));

  const x = (n: MapNode) => 12 + n.col * COL;
  const y = (n: MapNode) => TOP + n.row * ROW;

  function path(l: MapLink): string {
    const a = byId.get(l.from);
    const b = byId.get(l.to);
    if (!a || !b) return "";
    const x1 = x(a) + W;
    const y1 = y(a) + H / 2;
    const x2 = x(b);
    const y2 = y(b) + H / 2;
    // Same or backwards column (router → host two columns over): still left to right.
    const mid = (x1 + x2) / 2;
    return `M${x1},${y1} C${mid},${y1} ${mid},${y2} ${x2},${y2}`;
  }

  const EXPOSURE: Record<ExposureKind, string> = {
    publicAddress: "Public address",
    upnp: "Router port forward (UPnP)",
    funnel: "Tailscale Funnel",
    tunnel: "Cloudflare tunnel",
  };
  const SCOPE: Record<ListenScope, string> = { all: "every interface", address: "one address", vpn: "tailnet", local: "this host only" };

  function pick(n: MapNode) {
    netmap.selected = netmap.selected === n.id ? null : n.id;
  }
  function key(e: KeyboardEvent, n: MapNode) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      pick(n);
    }
  }

  const exposedPorts = (n: MapNode) => new Set(n.exposures.map((e) => e.port).filter((p) => p !== null));
  const listeners = (id: number): Listener[] | null => {
    const a = system.portMap[id];
    if (!a || typeof a === "string") return null;
    // eslint-disable-next-line svelte/prefer-svelte-reactivity -- local, rebuilt on each read
    const seen = new Set<string>();
    return a.listeners
      .filter((l) => {
        const k = `${l.proto}/${l.port}/${l.scope}`;
        if (seen.has(k)) return false;
        seen.add(k);
        return true;
      })
      .sort((p, q) => p.port - q.port);
  };
  const whereTo = (e: Exposure) => exposureIp(e) ?? "this host";
  const ago = (t: number | null) => (t ? new Date(t).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : "");
  const tsPeers = $derived(netmap.result?.devices.filter((d) => d.seen.includes("tailscale")) ?? []);

  function addAsHost(n: MapNode) {
    const name = n.device?.names[0]?.split(".")[0] ?? "";
    hosts.addFrom({ name, hostname: n.ip ?? "" });
  }
</script>

<div class="net">
  <header class="bar">
    <label class="from">
      <span class="lbl">SCAN FROM</span>
      <select class="input" value={String(netmap.from)} onchange={(e) => (netmap.scanHost = e.currentTarget.value === "local" ? "local" : Number(e.currentTarget.value))} aria-label="Scan from">
        {#each hosts.hosts as h (h.id)}<option value={String(h.id)}>{h.name}</option>{/each}
        <option value="local">This computer</option>
      </select>
    </label>
    <div class="sweep">
      <Toggle
        label="Ping sweep"
        hint="Pings each address in the subnet (/24 at most)."
        checked={netmap.sweep}
        onchange={(v) => (netmap.sweep = v)}
      />
    </div>
    <button type="button" class="btn primary" data-guide="netmap-scan" onclick={() => netmap.scan()} disabled={netmap.loading}>
      {netmap.loading ? "SCANNING…" : "SCAN"}
    </button>
    <button type="button" class="btn ghost" data-guide="netmap-ports" onclick={() => system.scanAllPorts()} disabled={system.portMapLoading || !hosts.hosts.length} title="Runs the System → Security port check on every host">
      {system.portMapLoading ? "AUDITING…" : "AUDIT PORTS ON ALL HOSTS"}
    </button>
    <span class="legend" aria-label="Legend">
      <i class="sw internet"></i>internet-facing <i class="sw lan"></i>LAN <i class="sw tailnet"></i>tailnet
    </span>
  </header>

  {#if netmap.error}<p class="err" role="alert">{netmap.error}</p>{/if}
  {#if netmap.result}
    <p class="meta muted">
      Scanned from <strong>{netmap.resultHost === "local" ? "this computer" : (hosts.byId(netmap.resultHost ?? -1)?.name ?? "?")}</strong> at {ago(netmap.scannedAt)}
      · {netmap.result.devices.length} devices{#if netmap.result.swept} · swept {netmap.result.swept}{/if}
      {#each netmap.result.notes as n (n)}<span class="note"> · {n}</span>{/each}
    </p>
  {:else}
    <p class="meta muted">Nothing is scanned until you click <strong>Scan</strong>. Discovery reads the chosen host's neighbour table, mDNS, Tailscale peers and router port forwards; it doesn't probe anything outside your network.</p>
  {/if}

  <div class="body">
    <div class="canvas" data-guide="netmap-graph">
      {#if !hosts.hosts.length && !netmap.result}
        <p class="empty muted">Scan from this computer, or add hosts to see them on the map and scan from them.</p>
      {:else}
        <svg {width} {height} viewBox={`0 0 ${width} ${height}`} role="group" aria-label="Network map">
          {#each HEADS as h, i (h)}
            <text class="head" x={12 + i * COL} y="16">{h}</text>
          {/each}
          {#each g.links as l (l.from + l.to)}
            <path class={`link ${l.kind}`} d={path(l)} />
          {/each}
          {#each g.nodes as n (n.id)}
            {@const exposed = n.exposures.length > 0 && n.kind !== "internet"}
            <g
              class={`node ${n.kind}`}
              class:sel={netmap.selected === n.id}
              class:exposed
              class:unseen={n.unseen}
              transform={`translate(${x(n)},${y(n)})`}
              role="button"
              tabindex="0"
              aria-pressed={netmap.selected === n.id}
              aria-label={`${n.label}, ${n.sub}`}
              onclick={() => pick(n)}
              onkeydown={(e) => key(e, n)}
            >
              <rect width={W} height={H} />
              <rect class="tag" width="3" height={H} />
              <text class="label" x="12" y="21">{n.label.length > 21 ? n.label.slice(0, 20) + "…" : n.label}</text>
              <text class="sub" x="12" y="39">
                {#if n.kind === "host"}
                  {#if n.unseen}not seen in this scan
                  {:else if n.ports}LAN {n.ports.lan} · tailnet {n.ports.tailnet} · local {n.ports.local}
                  {:else}{n.sub.length > 26 ? n.sub.slice(0, 25) + "…" : n.sub}{/if}
                {:else}{n.sub.length > 27 ? n.sub.slice(0, 26) + "…" : n.sub}{/if}
              </text>
              {#if exposed}
                <text class="badge" x={W - 10} y="21" text-anchor="end">▲ {n.exposures.length}</text>
              {/if}
              {#if n.kind === "device" && n.device?.online === false}
                <text class="badge off" x={W - 10} y="21" text-anchor="end">offline</text>
              {/if}
            </g>
          {/each}
        </svg>
      {/if}
    </div>

    <aside class="detail" aria-label="Details">
      {#if !netmap.current}
        <p class="muted">Select a node for details. Hosts show their listening ports once audited; unknown devices can be added as hosts.</p>
      {:else}
        {@const n = netmap.current}
        <h3>{n.label}</h3>
        <p class="muted mono">{n.sub}</p>

        {#if n.kind === "internet"}
          <p>Ways in found by the last scan. Nothing outside your network was contacted to find these.</p>
          <ul class="exp">
            {#each n.exposures as e, i (i)}
              <li><strong>{EXPOSURE[e.kind]}</strong><span class="mono">{e.public}</span><span class="muted">→ {e.target} ({whereTo(e)})</span>{#if e.detail}<span class="muted">{e.detail}</span>{/if}</li>
            {/each}
          </ul>
        {:else if n.kind === "gateway"}
          {#if n.device?.mac}<p class="mono muted">MAC {n.device.mac}</p>{/if}
          <p>The default route of the scanned host. Router port forwards (UPnP) are listed under <strong>Internet</strong> and on the host they land on.</p>
        {:else if n.kind === "tailnet"}
          <ul class="plain">
            {#each tsPeers as d (d.ip)}
              <li><span class:ok={d.online} class:off={d.online === false}>●</span> {d.names[0] ?? d.ip} <span class="muted mono">{d.ip}{d.os ? ` · ${d.os}` : ""}</span></li>
            {:else}
              <li class="muted">No peers in the last scan.</li>
            {/each}
          </ul>
        {:else if n.id === "local" && netmap.result}
          <p>The computer Nightgrid runs on. The scan ran here with its own tools.</p>
          {#if n.exposures.length}
            <h4 class="danger-h">INTERNET-FACING</h4>
            <ul class="exp">
              {#each n.exposures as e, i (i)}
                <li><strong>{EXPOSURE[e.kind]}</strong><span class="mono">{e.public}</span><span class="muted">→ {e.target}</span></li>
              {/each}
            </ul>
          {/if}
          <h4>INTERFACES</h4>
          <ul class="plain mono">
            {#each netmap.result.ifaces as f (f.name + f.addr)}<li>{f.name} {f.addr}/{f.prefix} <span class="muted">{f.kind}</span></li>{/each}
          </ul>
        {:else if n.kind === "host" && n.hostId !== undefined}
          {@const hid = n.hostId}
          {@const ls = listeners(hid)}
          {@const ex = exposedPorts(n)}
          {#if n.exposures.length}
            <h4 class="danger-h">INTERNET-FACING</h4>
            <ul class="exp">
              {#each n.exposures as e, i (i)}
                <li><strong>{EXPOSURE[e.kind]}</strong><span class="mono">{e.public}</span><span class="muted">→ {e.target}</span></li>
              {/each}
            </ul>
          {/if}
          {#if netmap.resultHost === hid && netmap.result}
            <h4>INTERFACES</h4>
            <ul class="plain mono">
              {#each netmap.result.ifaces as f (f.name + f.addr)}<li>{f.name} {f.addr}/{f.prefix} <span class="muted">{f.kind}</span></li>{/each}
            </ul>
          {/if}
          <h4>LISTENING PORTS</h4>
          {#if ls}
            <table>
              <tbody>
                {#each ls as l (l.proto + l.port + l.scope)}
                  <tr class:internet={ex.has(l.port)}>
                    <td class="mono">{l.port}/{l.proto}</td>
                    <td>{l.container ?? l.process ?? l.service ?? ""}</td>
                    <td class="r">{ex.has(l.port) ? "INTERNET" : SCOPE[l.scope]}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          {:else if typeof system.portMap[hid] === "string"}
            <p class="err">{system.portMap[hid]}</p>
          {:else}
            <p class="muted">Not audited yet.</p>
          {/if}
          <div class="actions">
            <button type="button" class="btn" onclick={() => netmap.audit(hid)} disabled={netmap.auditing.includes(hid)}>{netmap.auditing.includes(hid) ? "AUDITING…" : "AUDIT PORTS"}</button>
            <button type="button" class="btn ghost" onclick={() => { netmap.scanHost = hid; void netmap.scan(); }} disabled={netmap.loading}>SCAN FROM HERE</button>
          </div>
        {:else if n.kind === "device" && n.device}
          {@const d = n.device}
          <dl>
            <dt>ADDRESS</dt><dd class="mono">{d.ip}</dd>
            {#if d.mac}<dt>MAC</dt><dd class="mono">{d.mac}</dd>{/if}
            {#if d.names.length}<dt>NAMES</dt><dd>{d.names.join(", ")}</dd>{/if}
            {#if d.services.length}<dt>SERVICES</dt><dd>{[...new Set(d.services.map(serviceLabel))].join(", ")}</dd>{/if}
            {#if d.os}<dt>OS</dt><dd>{d.os}{d.online === false ? " (offline)" : ""}</dd>{/if}
            <dt>SEEN BY</dt><dd>{seenText(d)}</dd>
          </dl>
          <p class="muted">Not one of your hosts. Add it to manage it over SSH; nothing is sent to it until you connect.</p>
          <div class="actions">
            <button type="button" class="btn primary" data-guide="netmap-add-host" onclick={() => addAsHost(n)}>ADD AS HOST</button>
          </div>
        {/if}
      {/if}
    </aside>
  </div>
</div>

<style>
  .net {
    display: flex;
    flex-direction: column;
    gap: 10px;
    height: 100%;
    min-height: 0;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
  }
  .from {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .lbl,
  .head,
  h4 {
    font-family: var(--font-display);
    font-weight: 600;
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
  }
  .sweep :global(.toggle) {
    padding: 0;
  }
  .sweep {
    max-width: 320px;
  }
  .legend {
    margin-left: auto;
    font-size: var(--fs-xs);
    color: var(--text-1);
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .sw {
    display: inline-block;
    width: 18px;
    height: 0;
    border-top: 2px solid;
  }
  .sw.internet {
    border-color: var(--danger);
  }
  .sw.lan {
    border-color: var(--cyan);
  }
  .sw.tailnet {
    border-color: var(--mint);
    border-top-style: dashed;
  }
  .meta {
    margin: 0;
    font-size: var(--fs-xs);
  }
  .err {
    color: var(--danger);
    margin: 0;
  }
  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 320px;
    gap: 12px;
  }
  .canvas {
    overflow: auto;
    border: 1px solid var(--line);
    background:
      linear-gradient(var(--bg-0), var(--bg-0)) padding-box,
      repeating-linear-gradient(0deg, transparent 0 31px, color-mix(in srgb, var(--line) 50%, transparent) 31px 32px);
    padding: 8px;
  }
  .canvas svg {
    display: block;
    width: 100%;
    min-width: 720px;
    height: auto;
  }
  .empty {
    padding: 20px;
  }
  .head {
    fill: var(--text-1);
  }
  .link {
    fill: none;
    stroke-width: 1.5;
  }
  .link.lan {
    stroke: color-mix(in srgb, var(--cyan) 55%, transparent);
  }
  .link.tailnet {
    stroke: color-mix(in srgb, var(--mint) 60%, transparent);
    stroke-dasharray: 5 4;
  }
  .link.internet {
    stroke: var(--danger);
    stroke-width: 2;
  }
  .node {
    cursor: pointer;
    outline: none;
  }
  .node rect {
    fill: var(--bg-2);
    stroke: var(--line);
  }
  .node .tag {
    stroke: none;
    fill: var(--line);
  }
  .node.host .tag {
    fill: var(--cyan);
  }
  .node.gateway .tag,
  .node.tailnet .tag {
    fill: var(--mint);
  }
  .node.internet .tag,
  .node.exposed .tag {
    fill: var(--danger);
  }
  .node.internet rect:first-child {
    stroke: color-mix(in srgb, var(--danger) 60%, var(--line));
  }
  .node:hover rect:first-child,
  .node:focus-visible rect:first-child {
    stroke: var(--mint);
  }
  .node.sel rect:first-child {
    stroke: var(--magenta);
    fill: color-mix(in srgb, var(--magenta) 10%, var(--bg-2));
  }
  .node.unseen text {
    opacity: 0.55;
  }
  .node.unseen rect:first-child {
    stroke-dasharray: 4 3;
  }
  .label {
    fill: var(--text-0);
    font-size: 13px;
    font-weight: 600;
  }
  .sub {
    fill: var(--text-1);
    font-size: 11px;
    font-family: var(--font-mono);
  }
  .badge {
    fill: var(--danger);
    font-size: 11px;
    font-weight: 700;
  }
  .badge.off {
    fill: var(--text-1);
    font-weight: 400;
  }
  .detail {
    border: 1px solid var(--line);
    background: var(--bg-1);
    padding: 12px;
    overflow: auto;
  }
  .detail h3 {
    margin: 0;
    font-family: var(--font-display);
    letter-spacing: 0.06em;
  }
  .detail h4 {
    margin: 14px 0 6px;
  }
  .detail .danger-h {
    color: var(--danger);
  }
  .mono {
    font-family: var(--font-mono);
  }
  .exp,
  .plain {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .exp li {
    display: flex;
    flex-direction: column;
    gap: 2px;
    border-left: 2px solid var(--danger);
    padding-left: 8px;
    font-size: var(--fs-xs);
    overflow-wrap: anywhere;
  }
  .ok {
    color: var(--ok);
  }
  .off {
    color: var(--text-1);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--fs-xs);
  }
  td {
    padding: 4px 0;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  }
  tr.internet td {
    color: var(--danger);
    font-weight: 600;
  }
  .r {
    text-align: right;
  }
  dl {
    display: grid;
    grid-template-columns: 80px 1fr;
    gap: 6px 10px;
    margin: 10px 0;
    font-size: var(--fs-xs);
  }
  dt {
    color: var(--text-1);
    letter-spacing: var(--tracking-label);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 12px;
    flex-wrap: wrap;
  }
  @media (max-width: 1100px) {
    .body {
      grid-template-columns: 1fr;
    }
  }
</style>
