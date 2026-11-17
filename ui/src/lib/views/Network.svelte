<script lang="ts">
  import type { Exposure, ExposureKind, Listener, ListenScope } from "../api";
  import Toggle from "../components/Toggle.svelte";
  import TopoMap from "../components/TopoMap.svelte";
  import { canAttach, canParent } from "../netmap/layout";
  import { onMount } from "svelte";
  import { exposureIp, KIND_LABEL, labelKey, seenText, serviceLabel, type MapLink, type MapNode } from "../netmap/graph";
  import { hosts } from "../stores/hosts.svelte";
  import { netmap } from "../stores/netmap.svelte";
  import { nettools } from "../stores/nettools.svelte";
  import { services } from "../stores/services.svelte";
  import { settings } from "../stores/settings.svelte";
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
  const scannerName = $derived(netmap.from === "local" ? "this computer" : netmap.from === null ? "" : (hosts.byId(netmap.from)?.name ?? "?"));

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

  function pick(n: MapNode | string) {
    const id = typeof n === "string" ? n : n.id;
    netmap.selected = netmap.selected === id ? null : id;
  }
  const topoSel = $derived(netmap.topo.nodes.find((t) => t.id === netmap.selected) ?? null);
  const parents = $derived(
    topoSel && topoSel.node.kind !== "internet"
      ? netmap.topo.nodes.filter((t) => !t.ghost && canParent(t) && canAttach(netmap.topo, topoSel.id, t.id))
      : [],
  );
  const diff = $derived(netmap.topo.diff);
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

  // Saved networks (D-154).
  const curNet = $derived(netmap.result?.network ?? null);
  let editNet = $state<number | null>(null);
  let netName = $state("");
  let mergeInto = $state("");
  let forgetArmed = $state(false);
  function startEdit(id: number) {
    const n = netmap.networks.find((x) => x.id === id);
    editNet = editNet === id ? null : id;
    netName = n?.name ?? "";
    mergeInto = "";
    forgetArmed = false;
  }
  async function saveNet() {
    if (editNet !== null && (await netmap.renameNetwork(editNet, netName))) editNet = null;
  }
  const subnetOf = (n: { subnet: string | null; gateway: string | null }) => n.subnet ?? n.gateway ?? "no LAN";

  onMount(() => {
    void netmap.loadNetworks();
    if (!netmap.labelsLoaded) void netmap.loadLabels();
    if (!netmap.layoutLoaded) void netmap.loadLayout();
    void netmap.loadInventory();
    void netmap.loadWatch();
  });

  const watch = $derived(settings.value.netWatch);
  const watchFrom = $derived(watch.scanner === null ? "this computer" : (hosts.byId(watch.scanner)?.name ?? "a removed host"));
  let secOpen = $state(false);
  const worst = $derived(netmap.findings.some((f) => f.level === "bad") ? "bad" : netmap.findings.some((f) => f.level === "warn") ? "warn" : "ok");
  function setWatch(patch: Partial<typeof watch>) {
    void settings.update({ netWatch: { ...watch, ...patch } });
  }

  const devKey = (d: { mac: string | null; ip: string }) => labelKey(d.mac, d.ip);
  const kindText = (k: string | null | undefined) => (k && k !== "other" ? (KIND_LABEL[k] ?? k) : "");
  const since = (t: number | null | undefined) => {
    if (!t) return "";
    const s = Math.max(0, Date.now() / 1000 - t);
    if (s < 3600) return "just now";
    if (s < 86400) return `${Math.round(s / 3600)} h ago`;
    return `${Math.round(s / 86400)} days ago`;
  };
  let showInventory = $state(false);

  // Label form for the selected device (or router).
  let fName = $state("");
  let fKind = $state("other");
  let fTags = $state("");
  let fNote = $state("");
  let fKey = $state<string | null>(null);
  let saved = $state(false);
  const formKey = $derived(netmap.current?.device ? labelKey(netmap.current.device.mac, netmap.current.device.ip) : null);
  $effect(() => {
    const k = formKey;
    if (k === fKey) return;
    fKey = k;
    const l = netmap.current?.named;
    fName = l?.name ?? "";
    fKind = l?.kind ?? netmap.current?.device?.kind ?? "other";
    fTags = l?.tags.map((t) => "#" + t).join(" ") ?? "";
    fNote = l?.note ?? "";
    saved = false;
    netmap.labelError = null;
  });
  async function saveLabel() {
    if (!fKey) return;
    const tags = fTags.split(/[\s,]+/).filter(Boolean);
    saved = await netmap.saveLabel({ key: fKey, name: fName, kind: fKind, tags, note: fNote });
  }
  const labelled = $derived(
    netmap.labels
      .map((l) => {
        const d = netmap.result?.devices.find((x) => labelKey(x.mac, x.ip) === l.key);
        return { l, ip: d?.ip ?? (l.key.startsWith("ip:") ? l.key.slice(3) : null), node: d ? `d${d.ip}` : null };
      })
      .sort((a, b) => (a.l.name || a.l.key).localeCompare(b.l.name || b.l.key)),
  );

  function addAsHost(n: MapNode) {
    const name = n.device?.names[0]?.split(".")[0] ?? "";
    hosts.addFrom({ name, hostname: n.ip ?? "" });
  }
</script>

{#snippet toolRow(ip: string, mac: string | null | undefined)}
  <div class="toolrow" data-guide="netmap-tools">
    <span class="lbl">TOOLS</span>
    <button type="button" class="btn ghost tiny" onclick={() => nettools.open("ping", ip, netmap.resultHost)}>PING</button>
    <button type="button" class="btn ghost tiny" onclick={() => nettools.open("ports", ip, netmap.resultHost)}>PORTS</button>
    <button type="button" class="btn ghost tiny" onclick={() => nettools.open("http", ip, netmap.resultHost)}>WEB</button>
    <button type="button" class="btn ghost tiny" onclick={() => nettools.open("path", ip, netmap.resultHost)}>PATH</button>
    <button type="button" class="btn ghost tiny" onclick={() => nettools.open("dns", ip, netmap.resultHost)}>DNS</button>
    {#if mac}<button type="button" class="btn ghost tiny" title="Wake-on-LAN from the machine that scanned" onclick={() => nettools.open("wol", mac, netmap.resultHost)}>WAKE</button>{/if}
  </div>
{/snippet}

{#snippet svcScan(key: string, ip: string, name: string, hostId: number | null)}
  {@const s = netmap.svc[key]}
  <h4 data-guide="netmap-services">SERVICES</h4>
  {#if !s}
    <p class="muted small">Connects to 46 common ports from {netmap.resultHost === null ? "this computer" : "the machine that scanned"} and reads what answers. Nothing is logged in to.</p>
    <button type="button" class="btn ghost" onclick={() => netmap.scanServices(key, ip)}>SCAN SERVICES</button>
  {:else if s.running}
    <p class="muted">Scanning from {s.from}…</p>
  {:else if s.error}
    <p class="err">{s.error}</p>
    <button type="button" class="btn ghost tiny" onclick={() => netmap.scanServices(key, ip)}>TRY AGAIN</button>
  {:else if s.result}
    {#if s.result.note}<p class="warnline small">{s.result.note}</p>{/if}
    <ul class="exp risks">
      {#each s.result.facts.filter((f) => f.label.startsWith("Port ")) as f (f.label)}<li class={f.tone}><strong>{f.label}</strong><span>{f.value}</span></li>{/each}
    </ul>
    {#if s.result.table?.rows.length}
      <table>
        <tbody>
          {#each s.result.table.rows as r (r[0])}
            {@const web = r[1].startsWith("Web")}
            <tr>
              <td class="mono">{r[0]}</td>
              <td title={r[2]}>{r[1]}</td>
              <td class="r">
                {#if web}<button type="button" class="btn ghost tiny" title="Watch it under Services" onclick={() => services.addFrom({ name: `${name} :${r[0]}`, url: `${r[3] === "TLS" ? "https" : "http"}://${ip}${(r[3] === "TLS" && r[0] === "443") || (r[3] !== "TLS" && r[0] === "80") ? "" : `:${r[0]}`}/`, hostId, allowUntrusted: r[3] === "TLS" })}>ADD AS SERVICE</button>{/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}<p class="muted">No open ports among the common ones.</p>{/if}
    <p class="muted small">From {s.from}. <button type="button" class="linkish" onclick={() => netmap.scanServices(key, ip)}>Scan again</button> · <button type="button" class="linkish" onclick={() => nettools.open("services", ip, netmap.resultHost)}>other ports…</button></p>
  {/if}
{/snippet}

{#snippet labelForm()}
  <form class="labelform" data-guide="netmap-label" onsubmit={(e) => { e.preventDefault(); void saveLabel(); }}>
    <h4>LABEL</h4>
    <label><span class="lbl">NAME</span><input class="input" maxlength="80" placeholder="e.g. Living room TV" bind:value={fName} oninput={() => (saved = false)} /></label>
    <label
      ><span class="lbl">TYPE</span>
      <select class="input" bind:value={fKind} onchange={() => (saved = false)}>
        {#each Object.entries(KIND_LABEL) as [k, v] (k)}<option value={k}>{v}</option>{/each}
      </select>
    </label>
    <label><span class="lbl">TAGS</span><input class="input" placeholder="#media #upstairs" bind:value={fTags} oninput={() => (saved = false)} /></label>
    <label><span class="lbl">NOTE</span><textarea class="input" rows="2" maxlength="2000" bind:value={fNote} oninput={() => (saved = false)}></textarea></label>
    {#if netmap.labelError}<p class="err">{netmap.labelError}</p>{/if}
    <div class="actions">
      <button type="submit" class="btn">{saved ? "SAVED" : "SAVE LABEL"}</button>
      {#if netmap.current?.named}
        <button type="button" class="btn ghost" onclick={() => { if (fKey) void netmap.deleteLabel(fKey); fName = ""; fKind = "other"; fTags = ""; fNote = ""; }}>CLEAR</button>
      {/if}
    </div>
    <p class="muted small">Kept on this computer and in profile exports, keyed by {fKey?.startsWith("ip:") ? "address (no MAC seen)" : "MAC address"}. DRIFT can read it.</p>
  </form>
{/snippet}

<div class="net">
  <header class="bar">
    <div class="sweep">
      <Toggle
        label="Ping sweep"
        hint="Pings each address in the subnet (/24 at most), and tries a few TCP ports (SSH, SMB, web, NFS) on each, for devices that ignore ping. Takes about half a minute."
        checked={netmap.sweep}
        onchange={(v) => (netmap.sweep = v)}
      />
    </div>
    <label class="ranges" data-guide="netmap-ranges" title="Subnets behind a router, VPN or Tailscale subnet route that the scanner can reach: ping and port answers, reverse DNS and UPnP only (no MAC or local names across a router). /24 to /30, up to 4.">
      <span class="lbl">ROUTED RANGES</span>
      <input class="input mono" placeholder="10.20.0.0/24" bind:value={netmap.ranges} spellcheck="false" />
    </label>
    <button
      type="button"
      class="btn primary"
      data-guide="netmap-scan"
      onclick={() => netmap.scan()}
      disabled={netmap.loading || netmap.from === null}
      title={netmap.from === null ? "Select a node marked ◉ on the map: the scan runs from there" : undefined}
    >
      {netmap.loading ? "SCANNING…" : netmap.from === null ? "SELECT A SCANNER ON THE MAP" : `SCAN FROM ${scannerName.toUpperCase()}`}
    </button>
    <button type="button" class="btn ghost" data-guide="netmap-ports" onclick={() => system.scanAllPorts()} disabled={system.portMapLoading || !hosts.hosts.length} title="Runs the System → Security port check on every host">
      {system.portMapLoading ? "AUDITING…" : "AUDIT PORTS ON ALL HOSTS"}
    </button>
    <span class="legend" aria-label="Legend">
      <i class="sw internet"></i>internet-facing <i class="sw lan"></i>LAN <i class="sw tailnet"></i>tailnet <i class="sw routed"></i>routed <b class="scanmark">◉</b>can scan
    </span>
  </header>

  {#if netmap.error}<p class="err" role="alert">{netmap.error}</p>{/if}
  {#if netmap.result}
    <p class="meta muted">
      {#if netmap.fromSaved}
        Last scan of <strong>{curNet?.name ?? "this network"}</strong>, from <strong>{netmap.resultHost === "local" ? "this computer" : (hosts.byId(netmap.resultHost ?? -1)?.name ?? "a removed host")}</strong> on {new Date(netmap.scannedAt ?? 0).toLocaleString([], { dateStyle: "medium", timeStyle: "short" })} (kept: scan from a machine on it for a fresh look)
      {:else}
        Scanned from <strong>{netmap.resultHost === "local" ? "this computer" : (hosts.byId(netmap.resultHost ?? -1)?.name ?? "?")}</strong> at {ago(netmap.scannedAt)}{#if curNet} on <strong>{curNet.name}</strong>{/if}
      {/if}
      {#if curNet && !netmap.fromSaved}{#if curNet.isNew}<span class="newc"> (a network not seen before: name it under Networks)</span>{/if}{/if}
      · {netmap.result.devices.length} devices{#if netmap.result.ranges.length} · swept {netmap.result.ranges.join(", ")}{/if}
      {#if netmap.newDevices.length}<span class="newc"> · {netmap.newDevices.length} new</span>{/if}
      {#if netmap.result.missing.length}<span class="miss"> · {netmap.result.missing.length} missing</span>{/if}
      {#if diff.hasExpected}
        {#if diff.missing.length || diff.unexpected.length}
          <span class="miss" data-guide="netmap-diff"> · differs from the expected map: {diff.missing.length} missing, {diff.unexpected.length} not in it</span>
        {:else}<span class="ok"> · matches the expected map</span>{/if}
      {/if}
      {#each netmap.result.notes as n (n)}<span class="note"> · {n}</span>{/each}
    </p>
  {:else}
    <p class="meta muted">Nothing is scanned until you click <strong>Scan</strong>. Select a node marked ◉ (a host that answers, or this computer) and the scan runs from there: it reads that machine's neighbour table, mDNS, Tailscale peers and router port forwards; it doesn't probe anything outside your network.</p>
  {/if}

  {#if netmap.networks.length}
    <section class="sec nets" data-guide="netmap-networks" aria-label="Saved networks">
      <div class="sec-row">
        <span class="lbl" title="Each scan belongs to the network it ran on, told apart by its subnet and its router's MAC address. Devices of another network are never missing from this one: they're on that one.">NETWORKS</span>
        {#each netmap.networks as n (n.id)}
          {@const cur = curNet?.id === n.id}
          <span class="netchip" class:cur class:on={netmap.overlay.includes(n.id)}>
            <button type="button" class="linkish name" title={cur ? "The network on show" : `Show the last scan of ${n.name} (no scan runs)`} onclick={() => netmap.showNetwork(n.id)} disabled={cur}>{n.name}</button>
            <span class="muted small">{n.subnet && !n.name.includes(n.subnet) ? `${n.subnet} · ` : ""}{n.devices} devices · {since(n.lastSeen)}</span>
            {#if cur}<span class="tag small">ON SHOW</span>
            {:else}
              <label class="small" title="Draw this network's last scan on the map too, linked through the Internet and the tailnet">
                <input type="checkbox" checked={netmap.overlay.includes(n.id)} disabled={!n.snapshotAt} onchange={(e) => netmap.toggleOverlay(n.id, e.currentTarget.checked)} /> on map
              </label>
            {/if}
            <button type="button" class="btn ghost tiny" aria-expanded={editNet === n.id} aria-label={`Edit ${n.name}`} onclick={() => startEdit(n.id)}>✎</button>
          </span>
        {/each}
      </div>
      {#if editNet !== null}
        {@const n = netmap.networks.find((x) => x.id === editNet)}
        {#if n}
          <form class="sec-row" onsubmit={(e) => { e.preventDefault(); void saveNet(); }}>
            <label class="small"><span class="lbl">NAME</span> <input class="input" maxlength="60" bind:value={netName} /></label>
            <button type="submit" class="btn tiny">SAVE</button>
            <span class="small muted mono">{subnetOf(n)} · router {n.gateway ?? "?"}{n.gatewayMac ? ` (${n.gatewayMac})` : ""} · first seen {since(n.firstSeen)}</span>
            <span class="grow"></span>
            {#if netmap.networks.length > 1}
              <select class="input tiny" aria-label="Merge into" bind:value={mergeInto} title="Same network, new router: fold this one into the other (its devices move over)">
                <option value="">Merge into…</option>
                {#each netmap.networks.filter((x) => x.id !== n.id) as o (o.id)}<option value={String(o.id)}>{o.name}</option>{/each}
              </select>
              {#if mergeInto}<button type="button" class="btn ghost tiny" onclick={() => { void netmap.mergeNetwork(n.id, Number(mergeInto)); editNet = null; }}>MERGE</button>{/if}
            {/if}
            <button type="button" class="btn ghost tiny danger" title="Its devices stay in the inventory, on no network, until a scan sees them again" onclick={() => { if (forgetArmed) { void netmap.forgetNetwork(n.id); editNet = null; } else forgetArmed = true; }}>{forgetArmed ? `FORGET ${n.name.toUpperCase()}?` : "FORGET"}</button>
          </form>
        {/if}
      {/if}
      {#if netmap.networksError}<p class="err small">{netmap.networksError}</p>{/if}
    </section>
  {/if}

  <section class="sec" data-guide="netmap-security" aria-label="Network security and watch">
    <div class="sec-row">
      <button type="button" class="btn ghost tiny" aria-expanded={secOpen} onclick={() => (secOpen = !secOpen)}>
        {secOpen ? "▾" : "▸"} SECURITY
      </button>
      {#if netmap.result}
        <span class={`small sum ${worst}`}>
          {netmap.findings.length ? `${netmap.findings.length} finding${netmap.findings.length === 1 ? "" : "s"}` : "nothing found in this scan"}
          {#if !netmap.security.findings && !netmap.security.running} · web check not run{/if}
        </span>
        <button type="button" class="btn ghost tiny" data-guide="netmap-webcheck" disabled={netmap.security.running} onclick={() => { secOpen = true; void netmap.checkSecurity(); }} title="Reads one page or certificate on the common web ports of each device found (at most 64). Finds expired or self-signed certificates, admin pages of gear that ships with default passwords, and logins over plain HTTP. Never tries a password.">
          {netmap.security.running ? "CHECKING…" : "CHECK WEB SECURITY"}
        </button>
      {:else}
        <span class="small muted">Scan first: the check uses what the scan found.</span>
      {/if}
      <span class="grow"></span>
      <span class="watch" data-guide="netmap-watch">
        <Toggle
          label="Watch the network"
          hint={`Scheduled scans from ${watch.enabled ? watchFrom : scannerName || "the selected scanner"}: alerts for new devices, missing expected ones and risks.`}
          checked={watch.enabled}
          onchange={(v) => setWatch(v ? { enabled: true, scanner: netmap.from === "local" || netmap.from === null ? null : netmap.from } : { enabled: false })}
        />
        {#if watch.enabled}
          <span class="small muted">from {watchFrom}, every</span>
          <select class="input tiny" aria-label="Watch interval" value={String(watch.minutes)} onchange={(e) => setWatch({ minutes: Number((e.currentTarget as HTMLSelectElement).value) })}>
            {#each [15, 30, 60, 180, 360, 1440] as m (m)}<option value={String(m)}>{m < 60 ? `${m} min` : m === 1440 ? "day" : `${m / 60} h`}</option>{/each}
          </select>
          {#if netmap.watchLast}<span class="small muted">· last {ago((netmap.watchLast?.at ?? 0) * 1000)}{#if netmap.watchLast.error}: <span class="err">{netmap.watchLast.error}</span>{/if}</span>{/if}
        {/if}
      </span>
    </div>
    {#if secOpen}
      {#if netmap.security.error}<p class="err small">{netmap.security.error}</p>{/if}
      {#if netmap.findings.length}
        <ul class="findings">
          {#each netmap.findings as f, i (i)}
            <li class={f.level}>
              <b>{f.level === "bad" ? "✕" : f.level === "warn" ? "⚠" : "·"}</b>
              <span><strong>{f.title}</strong><br /><span class="muted">{f.detail}</span></span>
              {#if f.ip}<button type="button" class="btn ghost tiny" onclick={() => nettools.open(f.port ? "http" : "ping", f.port ? `${[443, 8443, 9443].includes(f.port) ? "https" : "http"}://${f.ip}:${f.port}` : f.ip!, netmap.resultHost)}>OPEN IN TOOLS</button>{/if}
            </li>
          {/each}
        </ul>
      {:else if netmap.result}
        <p class="small muted">No forwarded risky ports, no extra DHCP server, no public addresses{netmap.security.findings ? ", and nothing wrong on the devices' web pages" : ""}.</p>
      {/if}
      <label class="small muted wopts">
        Alert on:
        <input type="checkbox" checked={watch.alertNew} onchange={(e) => setWatch({ alertNew: e.currentTarget.checked })} /> new devices
        <input type="checkbox" checked={watch.alertMissing} onchange={(e) => setWatch({ alertMissing: e.currentTarget.checked })} /> missing expected devices
        <input type="checkbox" checked={watch.alertRisks} onchange={(e) => setWatch({ alertRisks: e.currentTarget.checked })} /> risks
        <input type="checkbox" checked={watch.sweep} onchange={(e) => setWatch({ sweep: e.currentTarget.checked })} /> ping sweep each time
      </label>
    {/if}
  </section>

  <div class="body">
    <div class="canvas" class:graph={netmap.mode === "graph"} data-guide="netmap-graph">
      <div class="maptools">
        <div class="seg" role="group" aria-label="Map style" data-guide="netmap-mode">
          <button type="button" class:on={netmap.mode === "graph"} aria-pressed={netmap.mode === "graph"} onclick={() => netmap.setMode("graph")}>GRAPH</button>
          <button type="button" class:on={netmap.mode === "list"} aria-pressed={netmap.mode === "list"} onclick={() => netmap.setMode("list")}>LIST</button>
        </div>
        {#if netmap.mode === "graph"}
          <button type="button" class="btn ghost tiny" data-guide="netmap-expected" title="Remember the map as it is now (devices, positions, connections). Later scans show what's missing or new." onclick={() => netmap.saveExpected()} disabled={!netmap.result}>SAVE AS EXPECTED</button>
          {#if diff.hasExpected}<button type="button" class="btn ghost tiny" onclick={() => netmap.clearExpected()}>CLEAR EXPECTED</button>{/if}
          <button type="button" class="btn ghost tiny" data-guide="netmap-autolayout" title="Forget where you placed nodes and lay the map out again (connections you set stay)" onclick={() => netmap.autoLayout()}>AUTO LAYOUT</button>
          <span class="muted small hint">Drag to place · drop on a router, access point, switch or host to connect through it</span>
        {/if}
        {#if netmap.layoutError}<span class="err small">{netmap.layoutError}</span>{/if}
      </div>
      {#if !hosts.hosts.length && !netmap.result && !netmap.overlays.length}
        <p class="empty muted">Scan from this computer, or add hosts to see them on the map and scan from them.</p>
      {:else if netmap.mode === "graph"}
        <TopoMap onpick={(id) => pick(id)} />
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
              {#if netmap.canScan(n.id) && !exposed}
                <text class="badge scan" x={W - 10} y="21" text-anchor="end"><title>Can run a scan</title>◉</text>
              {/if}
              {#if n.kind === "device" && n.device?.online === false}
                <text class="badge off" x={W - 10} y="21" text-anchor="end">offline</text>
              {:else if n.kind === "device" && n.device?.isNew && !n.device.trusted}
                <text class="badge new" x={W - 10} y="21" text-anchor="end"><title>First seen in this scan</title>NEW</text>
              {:else if n.kind === "device" && n.device?.routed}
                <text class="badge routed" x={W - 10} y="21" text-anchor="end"><title>In a routed range</title>⇢</text>
              {:else if n.kind === "device" && n.device?.trusted}
                <text class="badge trusted" x={W - 10} y="21" text-anchor="end"><title>Trusted</title>✓</text>
              {/if}
            </g>
          {/each}
        </svg>
      {/if}
    </div>

    <aside class="detail" aria-label="Details">
      {#if !netmap.current}
        <p class="muted">Select a node for details. Hosts show their listening ports once audited; other devices can be labelled or added as hosts.</p>
        {#if netmap.newDevices.length}
          <h4 class="new-h" data-guide="netmap-new">NEW THIS SCAN</h4>
          <ul class="plain labelled">
            {#each netmap.newDevices as d (d.ip)}
              <li>
                <button type="button" class="linkish" onclick={() => (netmap.selected = `d${d.ip}`)}>{d.names[0] ?? d.ip}</button>
                <span class="muted">{[kindText(d.kind), d.vendor, d.ip].filter(Boolean).join(" · ")}</span>
                <button type="button" class="btn ghost tiny" onclick={() => netmap.trust(devKey(d), true)}>TRUST</button>
              </li>
            {/each}
          </ul>
        {/if}
        {#if netmap.result?.missing.length}
          <h4 class="miss-h" data-guide="netmap-missing">MISSING</h4>
          <p class="muted small">Seen before on this network in the swept range, didn't answer this time. Off, asleep, or gone. Devices of your other networks never show here.</p>
          <ul class="plain labelled">
            {#each netmap.result.missing as m (m.key)}
              <li>
                <span>{m.name ?? m.ip}{m.trusted ? " ✓" : ""}</span>
                <span class="muted">{[kindText(m.kind), m.vendor, m.ip, `last seen ${since(m.lastSeen)}`].filter(Boolean).join(" · ")}</span>
                <button type="button" class="btn ghost tiny" title="Remove from the inventory" onclick={() => netmap.forget(m.key)}>FORGET</button>
              </li>
            {/each}
          </ul>
        {/if}
        <h4>
          <button type="button" class="linkish" data-guide="netmap-inventory" onclick={() => ((showInventory = !showInventory), showInventory && void netmap.loadInventory())}>
            {showInventory ? "▾" : "▸"} INVENTORY ({netmap.inventory.length})
          </button>
        </h4>
        {#if showInventory}
          <p class="muted small">Every device a scan has seen, from any scanner. Trusted devices are ones you expect; the rest show as NEW the first time.</p>
          {#if netmap.inventoryError}<p class="err">{netmap.inventoryError}</p>{/if}
          <ul class="plain labelled inv">
            {#each netmap.inventory as d (d.key)}
              <li>
                <span>{d.name ?? d.ip}</span>
                <span class="muted">{[kindText(d.kind), d.vendor, d.ip, `first ${since(d.firstSeen)}`, `last ${since(d.lastSeen)}`].filter(Boolean).join(" · ")}</span>
                <button type="button" class="btn ghost tiny" onclick={() => netmap.trust(d.key, !d.trusted)}>{d.trusted ? "✓ TRUSTED" : "TRUST"}</button>
              </li>
            {:else}
              <li class="muted">Nothing yet: scan to fill it.</li>
            {/each}
          </ul>
        {/if}
        {#if labelled.length}
          <h4>LABELLED DEVICES</h4>
          <ul class="plain labelled">
            {#each labelled as x (x.l.key)}
              <li>
                {#if x.node && netmap.graph.nodes.some((n) => n.id === x.node)}
                  <button type="button" class="linkish" onclick={() => (netmap.selected = x.node)}>{x.l.name || x.ip || x.l.key}</button>
                {:else}
                  <span>{x.l.name || x.ip || x.l.key}</span>
                {/if}
                <span class="muted">{KIND_LABEL[x.l.kind] ?? x.l.kind}{x.ip ? ` · ${x.ip}` : ""}{x.l.tags.length ? " · #" + x.l.tags.join(" #") : ""}</span>
              </li>
            {/each}
          </ul>
        {/if}
      {:else}
        {@const n = netmap.current}
        <h3>{n.label}</h3>
        <p class="muted mono">{n.sub}</p>
        {#if topoSel && topoSel.node.kind !== "internet" && !topoSel.ghost}
          <label class="through" data-guide="netmap-parent">
            <span class="lbl">CONNECTED THROUGH</span>
            <select
              class="input"
              value={topoSel.manual ? (topoSel.parent ?? "") : ""}
              onchange={(e) => netmap.setParent(topoSel.id, e.currentTarget.value || null)}
            >
              <option value="">Automatic{topoSel.parent && !topoSel.manual ? ` (${netmap.topo.nodes.find((t) => t.id === topoSel.parent)?.node.label ?? "?"})` : ""}</option>
              {#each parents as p (p.id)}<option value={p.id}>{p.node.label}</option>{/each}
            </select>
          </label>
          {#if topoSel.unexpected}
            <p class="miss small">Not in the expected map. <button type="button" class="linkish" onclick={() => netmap.expect(topoSel.id, true)}>Add it</button></p>
          {/if}
        {/if}
        {#if topoSel?.ghost}
          <p>In the expected map, but this scan didn't see it. It may be off or asleep, on another network, or gone.</p>
          <div class="actions">
            <button type="button" class="btn ghost" onclick={() => (netmap.expect(topoSel.id, false), (netmap.selected = null))}>REMOVE FROM EXPECTED</button>
          </div>
        {/if}

        {#if n.kind === "internet"}
          {#if n.note}<p>{n.note}</p>{/if}
          <p>Ways in found by the last scan{netmap.overlays.length ? "s" : ""}. Nothing outside your network was contacted to find these.</p>
          <ul class="exp">
            {#each n.exposures as e, i (i)}
              <li><strong>{EXPOSURE[e.kind]}</strong><span class="mono">{e.public}</span><span class="muted">→ {e.target} ({whereTo(e)})</span>{#if e.detail}<span class="muted">{e.detail}</span>{/if}</li>
            {/each}
          </ul>
        {:else if n.kind === "gateway"}
          {#if n.device?.mac}<p class="mono muted">MAC {n.device.mac}{n.device.vendor ? ` · ${n.device.vendor}` : ""}</p>{/if}
          {#if n.ip}{@render toolRow(n.ip, n.device?.mac)}{/if}
          {#if n.device}{@render labelForm()}{/if}
          {#if n.network !== undefined}<p>{n.note}</p>
          {:else}<p>The default route of the scanned host{curNet ? ` on ${curNet.name}` : ""}. Router port forwards (UPnP) are listed under <strong>Internet</strong> and on the host they land on.</p>{/if}
        {:else if n.kind === "tailnet"}
          {#if n.note}<p class="small">{n.note}</p>{/if}
          <ul class="plain">
            {#each tsPeers as d (d.ip)}
              <li><span class:ok={d.online} class:off={d.online === false}>●</span> {d.names[0] ?? d.ip} <span class="muted mono">{d.ip}{d.os ? ` · ${d.os}` : ""}{d.link ? ` · ${d.link}` : ""}</span></li>
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
          {#if n.ip}{@render toolRow(n.ip, null)}{/if}
          {#if n.ip}{@render svcScan(`host:${n.label}`, n.ip, n.label, hid)}{/if}
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
            {#if netmap.canScan(n.id)}
              <button type="button" class="btn ghost" onclick={() => netmap.scan()} disabled={netmap.loading}>SCAN FROM HERE</button>
            {:else}
              <span class="muted small">Offline: can't scan from here now.</span>
            {/if}
          </div>
        {:else if n.kind === "device" && !n.device && (n.note || n.lldp)}
          {#if n.note}<p>{n.note}</p>{/if}
          {#if n.lldp}
            <dl>
              {#if n.lldp.name}<dt>NAME</dt><dd>{n.lldp.name}</dd>{/if}
              {#if n.lldp.port}<dt>PORT</dt><dd class="mono">{n.lldp.port}</dd>{/if}
              {#if n.lldp.mac}<dt>MAC</dt><dd class="mono">{n.lldp.mac}</dd>{/if}
              {#if n.lldp.mgmtIp}<dt>MANAGEMENT</dt><dd class="mono">{n.lldp.mgmtIp}</dd>{/if}
              {#if n.lldp.descr}<dt>SAYS</dt><dd>{n.lldp.descr}</dd>{/if}
            </dl>
          {/if}
          {#if n.ip}{@render toolRow(n.ip, n.lldp?.mac)}{/if}
          <p class="muted small">Drop devices onto it to say they connect through it.</p>
        {:else if n.kind === "device" && n.device}
          {@const d = n.device}
          <dl>
            <dt>ADDRESS</dt><dd class="mono">{d.ip}{d.routed ? " (routed range)" : ""}</dd>
            {#if kindText(d.kind)}<dt>LOOKS LIKE</dt><dd>{kindText(d.kind)} <span class="muted">(a guess from its maker, names and services; set the type in the label to override)</span></dd>{/if}
            {#if d.model}<dt>MODEL</dt><dd>{d.model}</dd>{/if}
            {#if d.firstSeen}<dt>FIRST SEEN</dt><dd>{d.isNew ? "in this scan" : since(d.firstSeen)}</dd>{/if}
            {#if d.mac}<dt>MAC</dt><dd class="mono">{d.mac}{d.vendor ? ` · ${d.vendor}` : ""}</dd>{/if}
            {#if d.also.length}<dt>ALSO</dt><dd class="mono">{d.also.join(", ")}</dd>{/if}
            {#if d.names.length}<dt>NAMES</dt><dd>{d.names.join(", ")}</dd>{/if}
            {#if d.services.length}<dt>SERVICES</dt><dd>{[...new Set(d.services.map(serviceLabel))].join(", ")}</dd>{/if}
            {#if d.os}<dt>OS</dt><dd>{d.os}{d.online === false ? " (offline)" : ""}</dd>{/if}
            {#if d.link}<dt>TAILNET</dt><dd>{d.link === "direct" ? "direct (peer to peer)" : `${d.link.replace("relay ", "through the Tailscale relay in ")}`}<span class="muted">{d.link === "direct" ? "" : " (or idle: a direct path is set up when there's traffic)"}</span></dd>{/if}
            <dt>SEEN BY</dt><dd>{seenText(d)}</dd>
          </dl>
          {@render toolRow(d.ip, d.mac)}
          {@render svcScan(devKey(d), d.ip, n.label, null)}
          {#if n.named?.note}<p class="note">{n.named.note}</p>{/if}
          {#if n.named?.tags.length}<p class="tags">{#each n.named.tags as t (t)}<span class="chip">#{t}</span>{/each}</p>{/if}
          {@render labelForm()}
          <p class="muted">Not one of your hosts. Add it to manage it over SSH; nothing is sent to it until you connect.</p>
          <div class="actions">
            <button type="button" class="btn primary" data-guide="netmap-add-host" onclick={() => addAsHost(n)}>ADD AS HOST</button>
            <button type="button" class="btn ghost" data-guide="netmap-trust" title="Trusted devices are ones you expect on the network" onclick={() => netmap.trust(devKey(d), !d.trusted)}>{d.trusted ? "✓ TRUSTED" : "TRUST"}</button>
          </div>
        {/if}
      {/if}
    </aside>
  </div>
</div>

<style>
  .canvas.graph {
    padding: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .canvas.graph :global(.topo) {
    flex: 1;
  }
  .maptools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--line);
  }
  .maptools .hint {
    margin-left: 4px;
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--line);
  }
  .seg button {
    padding: 2px 10px;
    border: 0;
    background: none;
    color: var(--text-1);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    cursor: pointer;
  }
  .seg button.on {
    background: color-mix(in srgb, var(--cyan) 18%, transparent);
    color: var(--text-0);
  }
  .risks li.bad {
    border-left-color: var(--danger);
  }
  .risks li.warn {
    border-left-color: var(--warn);
  }
  .risks li.info {
    border-left-color: var(--line);
  }
  .warnline {
    color: var(--warn);
  }
  .toolrow {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    margin: 8px 0;
  }
  .toolrow .lbl {
    margin-right: 4px;
  }
  .through {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 10px 0 4px;
  }
  .sw.routed {
    border-color: var(--warn);
    border-top-style: dotted;
  }
  .ranges {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .ranges input {
    width: 180px;
  }
  .lbl {
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-2);
  }
  .newc,
  .new-h {
    color: var(--cyan);
  }
  .miss,
  .miss-h {
    color: var(--warn);
  }
  .badge.new {
    fill: var(--cyan);
  }
  .badge.trusted {
    fill: var(--mint);
  }
  .badge.routed {
    fill: var(--text-2);
  }
  .labelled .tiny {
    align-self: flex-start;
  }
  .inv {
    max-height: 320px;
    overflow: auto;
  }
  .labelform {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 8px 0;
    padding-top: 8px;
    border-top: 1px solid var(--line);
  }
  .labelform label {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .labelform textarea {
    resize: vertical;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .note {
    white-space: pre-wrap;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .chip {
    padding: 0 6px;
    border: 1px solid var(--line);
    font-size: var(--fs-xs);
    color: var(--cyan);
  }
  .labelled li {
    display: flex;
    flex-direction: column;
    margin-bottom: 4px;
  }
  .linkish {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-0);
    text-align: left;
    cursor: pointer;
    text-decoration: underline dotted;
  }
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
  .sec {
    border: 1px solid var(--line);
    padding: 4px 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .sec-row,
  .watch {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .grow {
    flex: 1;
  }
  .sum.bad {
    color: var(--danger);
  }
  .sum.warn {
    color: var(--warn);
  }
  .sum.ok {
    color: var(--mint);
  }
  .findings {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 220px;
    overflow: auto;
    font-size: var(--fs-xs);
  }
  .findings li {
    display: grid;
    grid-template-columns: 16px 1fr auto;
    gap: 8px;
    align-items: start;
  }
  .findings li.bad b {
    color: var(--danger);
  }
  .findings li.warn b {
    color: var(--warn);
  }
  .wopts {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .nets .netchip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 1px solid var(--line);
    padding: 1px 6px;
  }
  .nets .netchip.cur {
    border-color: var(--cyan);
  }
  .nets .netchip.on {
    border-color: var(--mint);
    border-style: dashed;
  }
  .nets .name:disabled {
    color: var(--cyan);
    cursor: default;
  }
  .nets .tag {
    color: var(--cyan);
    letter-spacing: 0.08em;
  }
  .danger {
    color: var(--danger);
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
  .link.wan {
    stroke: var(--text-1);
  }
  .link.routed {
    stroke: color-mix(in srgb, var(--warn) 70%, transparent);
    stroke-dasharray: 2 4;
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
  .badge.scan {
    fill: var(--cyan);
    font-weight: 400;
  }
  .scanmark {
    color: var(--cyan);
    font-weight: 400;
    margin-left: 6px;
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
    padding: 4px 10px 4px 0;
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
