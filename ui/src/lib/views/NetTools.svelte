<script lang="ts">
  import { cidrInfo, normMac } from "../netmap/cidr";
  import { hosts } from "../stores/hosts.svelte";
  import { nettools, type ToolId } from "../stores/nettools.svelte";

  const TOOLS: { id: ToolId; label: string; hint: string }[] = [
    { id: "ping", label: "PING", hint: "Does it answer, and how fast" },
    { id: "path", label: "PATH", hint: "The routers between here and there (traceroute)" },
    { id: "dns", label: "DNS", hint: "Look up a name or an address" },
    { id: "ports", label: "PORTS", hint: "Which TCP ports accept a connection" },
    { id: "http", label: "HTTP / TLS", hint: "Status, server, redirects and the certificate" },
    { id: "wol", label: "WAKE", hint: "Send a Wake-on-LAN packet" },
    { id: "neighbours", label: "NEIGHBOURS", hint: "The machine's ARP / neighbour table" },
    { id: "connections", label: "CONNECTIONS", hint: "Open connections on the machine" },
    { id: "cidr", label: "SUBNET", hint: "Subnet calculator (offline)" },
    { id: "mac", label: "MAC LOOKUP", hint: "Who made a network card (offline)" },
  ];
  const LOCAL_ONLY: ToolId[] = ["cidr", "mac"];
  const NAMES = Object.fromEntries(TOOLS.map((t) => [t.id, t.label]));
  const DNS_TYPES = ["A", "AAAA", "CNAME", "MX", "TXT", "NS", "SOA", "SRV", "PTR", "CAA"];

  const tool = $derived(nettools.tool);
  const cidr = $derived(nettools.cidr.trim() ? cidrInfo(nettools.cidr) : null);
  const macOk = $derived(normMac(nettools.mac) !== null);
  const servers = $derived(hosts.hosts.filter((h) => h.role !== "client"));
  const canRun = $derived(
    !nettools.running &&
      (tool === "http" ? nettools.url.trim().length > 0 : tool === "wol" ? macOk : tool === "neighbours" || tool === "connections" ? true : nettools.target.trim().length > 0),
  );
  const time = (t: number) => new Date(t).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
  let open = $state<Record<number, boolean>>({});

  function submit(e: Event) {
    e.preventDefault();
    if (!canRun) return;
    if (tool === "wol") void nettools.wake();
    else void nettools.run();
  }
</script>

<div class="tools">
  <nav class="list" aria-label="Tools">
    {#each TOOLS as t (t.id)}
      <button type="button" class:on={tool === t.id} aria-pressed={tool === t.id} data-guide={`nettools-${t.id}`} title={t.hint} onclick={() => (nettools.tool = t.id)}>
        <span>{t.label}</span><small>{t.hint}</small>
      </button>
    {/each}
  </nav>

  <section class="work">
    <form class="form" onsubmit={submit}>
      {#if !LOCAL_ONLY.includes(tool)}
        <label class="from" data-guide="nettools-from">
          <span class="lbl">RUN FROM</span>
          <select class="input" value={String(nettools.runFrom)} onchange={(e) => nettools.setFrom(e.currentTarget.value === "local" ? "local" : Number(e.currentTarget.value))}>
            <option value="local">This computer</option>
            {#each servers as h (h.id)}<option value={String(h.id)}>{h.name}</option>{/each}
          </select>
        </label>
      {/if}

      {#if tool === "ping" || tool === "path" || tool === "dns" || tool === "ports"}
        <label class="grow"><span class="lbl">{tool === "dns" ? "NAME OR ADDRESS" : "HOST OR ADDRESS"}</span>
          <input class="input mono" bind:value={nettools.target} placeholder={tool === "dns" ? "nas.lan or 192.168.1.20" : "192.168.1.20"} spellcheck="false" autocomplete="off" />
        </label>
      {/if}
      {#if tool === "ping"}
        <label><span class="lbl">COUNT</span>
          <select class="input" bind:value={nettools.count}>{#each ["1", "4", "10"] as c (c)}<option>{c}</option>{/each}</select>
        </label>
      {:else if tool === "dns"}
        <label><span class="lbl">TYPE</span>
          <select class="input" bind:value={nettools.dnsType}>{#each DNS_TYPES as t (t)}<option>{t}</option>{/each}</select>
        </label>
        <label><span class="lbl">SERVER</span><input class="input mono" bind:value={nettools.dnsServer} placeholder="the machine's own" spellcheck="false" /></label>
      {:else if tool === "ports"}
        <label class="grow"><span class="lbl">PORTS</span><input class="input mono" bind:value={nettools.ports} placeholder="22,80,443,8000-8010" spellcheck="false" /></label>
      {:else if tool === "http"}
        <label class="grow"><span class="lbl">ADDRESS</span><input class="input mono" bind:value={nettools.url} placeholder="https://nas.lan:5001" spellcheck="false" autocomplete="off" /></label>
      {:else if tool === "wol" || tool === "mac"}
        <label class="grow"><span class="lbl">MAC ADDRESS</span><input class="input mono" bind:value={nettools.mac} placeholder="aa:bb:cc:dd:ee:ff" spellcheck="false" oninput={() => (nettools.macResult = null)} /></label>
      {:else if tool === "cidr"}
        <label class="grow"><span class="lbl">ADDRESS / PREFIX</span><input class="input mono" bind:value={nettools.cidr} placeholder="192.168.1.77/24 or 10.0.0.0 255.255.0.0" spellcheck="false" /></label>
      {/if}

      {#if tool === "mac"}
        <button type="button" class="btn primary" disabled={!macOk} onclick={() => nettools.lookupMac()}>LOOK UP</button>
      {:else if tool !== "cidr"}
        <button type="submit" class="btn primary" data-guide="nettools-run" disabled={!canRun}>
          {nettools.running ? "RUNNING…" : tool === "wol" ? `WAKE FROM ${nettools.fromName.toUpperCase()}` : `RUN ON ${nettools.fromName.toUpperCase()}`}
        </button>
      {/if}
    </form>

    {#if tool === "wol"}
      <p class="muted small">Sends the magic packet as a broadcast on {nettools.fromName}'s network. Pick a machine on the same network as the one to wake.</p>
    {:else if tool === "http"}
      <p class="muted small">Fetches the address once (no redirects followed) and reads the certificate with that machine's curl and openssl. "Not trusted" means that machine wouldn't accept the certificate.</p>
    {:else if tool === "ports"}
      <p class="muted small">Tries a TCP connection to each port (up to 64) with nc or python3 on that machine. Nothing is sent after the connection opens.</p>
    {/if}

    {#if tool === "cidr"}
      {#if cidr}
        <dl class="facts-grid">
          <dt>NETWORK</dt><dd class="mono">{cidr.cidr}</dd>
          <dt>MASK</dt><dd class="mono">{cidr.mask} <span class="muted">(wildcard {cidr.wildcard})</span></dd>
          <dt>HOSTS</dt><dd class="mono">{cidr.first} – {cidr.last} <span class="muted">({cidr.hosts.toLocaleString()} usable)</span></dd>
          <dt>BROADCAST</dt><dd class="mono">{cidr.broadcast}</dd>
          <dt>KIND</dt><dd>{cidr.scope}</dd>
        </dl>
      {:else if nettools.cidr.trim()}<p class="err">Not an IPv4 address with a /prefix or a dotted mask.</p>{/if}
    {:else if tool === "mac"}
      {#if nettools.macResult}<p class="answer"><span class="mono">{normMac(nettools.mac)}</span> · {nettools.macResult}</p>{/if}
      <p class="muted small">From the IEEE list bundled with Nightgrid. A MAC whose second digit is 2, 6, A or E is a private random address (phones, laptops and VMs use them), so it has no maker.</p>
    {/if}

    <div class="history" aria-live="polite">
      {#each nettools.history as h (h.id)}
        <article class="run" class:failed={!!h.error}>
          <header>
            <strong>{NAMES[h.tool]}</strong>
            <span class="mono">{h.result?.ran ?? ""}</span>
            <span class="muted">from {h.from} · {time(h.at)}</span>
          </header>
          {#if h.error}
            <p class="err">{h.error}</p>
          {:else if h.result}
            {#if h.result.note}<p class="warnline">{h.result.note}</p>{/if}
            {#if h.result.facts.length}
              <ul class="facts">
                {#each h.result.facts as f (f.label)}<li class={f.tone}><span>{f.label}</span>{f.value}</li>{/each}
              </ul>
            {/if}
            {#if h.result.table && h.result.table.rows.length}
              <div class="tablewrap">
                <table>
                  <thead><tr>{#each h.result.table.columns as c (c)}<th>{c}</th>{/each}</tr></thead>
                  <tbody>
                    {#each h.result.table.rows as r, i (i)}
                      <tr class:open={h.tool === "ports" && r[1] === "open"}>{#each r as c, j (j)}<td class:mono={j < 3}>{c}</td>{/each}</tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            {/if}
            {#if h.result.output.trim()}
              <button type="button" class="linkish small" onclick={() => (open[h.id] = !open[h.id])}>{open[h.id] ? "▾" : "▸"} output</button>
              {#if open[h.id]}<pre class="out">{h.result.output}</pre>{/if}
            {/if}
          {/if}
        </article>
      {:else}
        <p class="muted empty">Pick a tool and a machine to run it from. Results stay here for this session. Devices on the Network map open these tools with their address filled in.</p>
      {/each}
    </div>
  </section>
</div>

<style>
  .tools {
    display: grid;
    grid-template-columns: 230px minmax(0, 1fr);
    gap: 12px;
    height: 100%;
    min-height: 0;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: auto;
    border: 1px solid var(--line);
    background: var(--bg-1);
    padding: 6px;
  }
  .list button {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    padding: 6px 8px;
    border: 1px solid transparent;
    background: none;
    color: var(--text-0);
    text-align: left;
    cursor: pointer;
  }
  .list button span {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
  }
  .list button small {
    color: var(--text-1);
    font-size: 11px;
  }
  .list button:hover {
    border-color: var(--line);
  }
  .list button.on {
    border-color: var(--cyan);
    background: color-mix(in srgb, var(--cyan) 10%, transparent);
  }
  .work {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
    overflow: auto;
  }
  .form {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 10px;
  }
  .form label {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .form .grow {
    flex: 1;
    min-width: 220px;
  }
  .lbl {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
  }
  .mono {
    font-family: var(--font-mono);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .muted {
    color: var(--text-1);
  }
  .err {
    color: var(--danger);
    margin: 0;
  }
  .warnline {
    color: var(--warn);
    margin: 0;
  }
  .answer {
    margin: 0;
    font-size: var(--fs-sm, 13px);
  }
  .facts-grid {
    display: grid;
    grid-template-columns: 100px 1fr;
    gap: 6px 12px;
    margin: 0;
  }
  .facts-grid dt {
    color: var(--text-1);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
  }
  .facts-grid dd {
    margin: 0;
  }
  .history {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .run {
    border: 1px solid var(--line);
    border-left: 3px solid var(--cyan);
    background: var(--bg-1);
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .run.failed {
    border-left-color: var(--danger);
  }
  .run header {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: baseline;
    font-size: var(--fs-xs);
  }
  .run header strong {
    font-family: var(--font-display);
    letter-spacing: var(--tracking-label);
  }
  .facts {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .facts li {
    display: flex;
    gap: 6px;
    align-items: baseline;
    border: 1px solid var(--line);
    padding: 2px 8px;
    font-size: var(--fs-xs);
  }
  .facts li span {
    color: var(--text-1);
    letter-spacing: var(--tracking-label);
    text-transform: uppercase;
    font-size: 10px;
  }
  .facts li.ok {
    border-color: color-mix(in srgb, var(--ok) 60%, var(--line));
  }
  .facts li.warn {
    border-color: var(--warn);
    color: var(--warn);
  }
  .facts li.bad {
    border-color: var(--danger);
    color: var(--danger);
  }
  .tablewrap {
    max-height: 320px;
    overflow: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--fs-xs);
  }
  th {
    text-align: left;
    color: var(--text-1);
    font-weight: 600;
    letter-spacing: var(--tracking-label);
    padding: 3px 6px 3px 0;
    border-bottom: 1px solid var(--line);
  }
  td {
    padding: 3px 6px 3px 0;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
    overflow-wrap: anywhere;
  }
  tr.open td {
    color: var(--ok);
  }
  .linkish {
    align-self: flex-start;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-1);
    cursor: pointer;
  }
  .out {
    margin: 0;
    max-height: 280px;
    overflow: auto;
    background: var(--bg-0);
    border: 1px solid var(--line);
    padding: 8px;
    font-family: var(--font-mono);
    font-size: 11px;
    white-space: pre-wrap;
  }
  .empty {
    padding: 8px 0;
  }
  @media (max-width: 900px) {
    .tools {
      grid-template-columns: 1fr;
    }
  }
</style>
