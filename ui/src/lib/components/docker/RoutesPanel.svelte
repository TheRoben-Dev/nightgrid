<script lang="ts">
  /**
   * Reverse proxy routes (M10g): each domain Caddy, Traefik or Nginx Proxy
   * Manager serves on this host, the container behind it and certificate
   * expiry. Edit routes… changes Caddy sites and Nightgrid's Traefik routes (M10v).
   */
  import { onMount } from "svelte";
  import type { ProxyRoute } from "../../api";
  import { openExternal } from "../../clipboard";
  import { docker } from "../../stores/docker.svelte";
  import { certDays, certTone, PROXY_LABEL, proxy } from "../../stores/proxy.svelte";
  import { settings } from "../../stores/settings.svelte";
  import ExplainButton from "../helper/ExplainButton.svelte";
  import ProxyEditor from "./ProxyEditor.svelte";

  let editing = $state(false);

  onMount(() => void proxy.load());
  $effect(() => {
    if (docker.hostId !== null) void proxy.load();
  });

  const r = $derived(proxy.report);
  const now = $derived(r?.now || Date.now() / 1000);
  const warnDays = $derived(settings.value.alerts.certDays);

  function certText(x: ProxyRoute): string {
    if (!x.tls) return "HTTP";
    if (x.cert?.error) return "NO CERT";
    const d = certDays(x, now);
    if (d === null) return "—";
    if (d < 0) return "EXPIRED";
    return d === 0 ? "TODAY" : `${d}d`;
  }
  const certTitle = (x: ProxyRoute) =>
    x.cert?.error ?? (x.cert?.notAfter ? `Expires ${new Date(x.cert.notAfter * 1000).toLocaleString()}${x.cert.issuer ? ` · ${x.cert.issuer}` : ""}` : "");

  function openContainer(name: string) {
    const c = docker.containers.find((c) => c.name === name);
    if (!c) return;
    docker.tab = "containers";
    docker.detailId = c.id;
  }

  function explainText(): string {
    const lines = ["Reverse proxy routes on this host:"];
    for (const p of r?.proxies ?? []) lines.push(`Proxy ${PROXY_LABEL[p.kind]} (${p.name}): ${p.routes} routes${p.error ? `; error: ${p.error}` : ""}`);
    for (const x of r?.routes ?? []) lines.push(`- ${x.domains.join(", ")} → ${x.upstream}${x.container ? ` (container ${x.container})` : " (no container matched)"}; ${x.tls ? `TLS, cert ${certText(x)}` : "plain HTTP"}`);
    return lines.join("\n");
  }
</script>

<div class="routes">
  <div class="bar">
    {#if r}
      {#each r.proxies as p (p.kind + p.name)}
        <span class="proxy" class:err={!!p.error} title={p.error ?? ""}>{PROXY_LABEL[p.kind]} <span class="muted">· {p.name} · {p.routes}</span></span>
      {/each}
    {/if}
    <span class="spacer"></span>
    {#if r?.routes.length}<ExplainButton source="proxy routes" text={explainText} question="Look over these reverse proxy routes. Is anything misrouted, unprotected or about to expire?" />{/if}
    <button type="button" class="btn ghost tiny" data-guide="proxy-edit" onclick={() => (editing = true)}>Edit routes…</button>
    <button type="button" class="btn ghost tiny" disabled={proxy.loading} onclick={() => proxy.load(true)}>{proxy.loading ? "Reading…" : "Refresh"}</button>
  </div>
  <ProxyEditor open={editing} onclose={() => (editing = false)} />

  <div class="scroll" data-guide="docker-routes">
    {#if proxy.error}
      <p class="error-text pad">{proxy.error}</p>
    {:else if !r}
      <p class="muted pad">{proxy.loading ? "Reading proxy configs and certificates…" : ""}</p>
    {:else if !r.proxies.length}
      <div class="empty">
        <p class="label">NO REVERSE PROXY FOUND</p>
        <p class="muted">
          Nightgrid looks for Traefik router labels, a Caddy container or <code>/etc/caddy/Caddyfile</code>, caddy-docker-proxy labels and Nginx Proxy Manager's proxy hosts on this host.
        </p>
      </div>
    {:else}
      {#each r.proxies.filter((p) => p.error) as p (p.kind + p.name)}
        <p class="error-text pad">{PROXY_LABEL[p.kind]} ({p.name}): {p.error}</p>
      {/each}
      <div class="grid" role="table" aria-label="Proxy routes">
        <div class="head" role="row">
          <span role="columnheader">DOMAIN</span><span role="columnheader">FORWARDS TO</span><span role="columnheader">CONTAINER</span><span role="columnheader">CERT</span><span
            role="columnheader">VIA</span
          >
        </div>
        {#each proxy.visible as x, i (i)}
          {@const tone = certTone(x, now, warnDays)}
          <div class="row {tone}" role="row">
            <span class="dom">
              {#each x.domains as d (d)}
                {#if d.includes(".") && !d.startsWith(":")}
                  <button type="button" class="link" title="Open {x.tls ? 'https' : 'http'}://{d} in your browser" onclick={() => void openExternal(`${x.tls ? "https" : "http"}://${d}`)}>{d}</button>
                {:else}<code>{d}</code>{/if}
              {/each}
            </span>
            <code class="up" title={x.upstream}>{x.upstream}</code>
            <span>
              {#if x.container}<button type="button" class="btn ghost tiny" onclick={() => openContainer(x.container!)}>{x.container}</button>
              {:else}<span class="muted small" title="The upstream isn't a container name or a port published on this host.">—</span>{/if}
            </span>
            <span class="cert {tone}" title={certTitle(x)}>{certText(x)}</span>
            <span class="muted small via" title={x.origin}>{PROXY_LABEL[x.proxy]} · {x.origin}</span>
          </div>
        {/each}
      </div>
      <p class="muted small note">
        Certificates are read by <code>openssl</code> on the host from its own port 443, as the proxy serves them. Traefik's file provider is read only for the routes Nightgrid keeps there, and Caddy's JSON config isn't read. Yellow = expires within
        {warnDays} days.
      </p>
    {/if}
  </div>
</div>

<style>
  .routes {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
    padding: 6px 0;
  }
  .bar {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .proxy {
    border: 1px solid var(--line);
    padding: 1px 6px;
    letter-spacing: 0.06em;
    color: var(--cyan);
  }
  .proxy.err {
    border-color: var(--danger);
  }
  .spacer {
    flex: 1;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border: 1px solid var(--line);
    background: var(--bg-0);
  }
  .pad {
    padding: 8px 10px;
    margin: 0;
  }
  .empty {
    padding: 24px 16px;
    max-width: 640px;
  }
  .grid .head,
  .grid .row {
    display: grid;
    grid-template-columns: minmax(160px, 1.4fr) minmax(120px, 1fr) minmax(100px, 0.8fr) 80px minmax(120px, 0.8fr);
    gap: 10px;
    align-items: center;
    padding: 5px 10px;
  }
  .head {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-2);
    border-bottom: 1px solid var(--line);
  }
  .row {
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
    font-size: var(--fs-sm);
    border-left: 2px solid transparent;
  }
  .row:hover {
    background: var(--bg-1);
  }
  .row.bad {
    border-left-color: var(--danger);
  }
  .row.warn {
    border-left-color: var(--warn, var(--yellow));
  }
  .dom {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 8px;
    min-width: 0;
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    color: var(--text-0);
    font: inherit;
    cursor: pointer;
    text-align: left;
  }
  .link:hover {
    color: var(--cyan);
    text-decoration: underline;
  }
  .up,
  .via {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  code {
    font-family: var(--font-mono);
    color: var(--text-1);
  }
  .cert {
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    letter-spacing: 0.06em;
    color: var(--text-2);
  }
  .cert.ok {
    color: var(--mint);
  }
  .cert.warn {
    color: var(--warn, var(--yellow));
  }
  .cert.bad {
    color: var(--danger);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .note {
    padding: 10px;
    margin: 0;
  }
</style>
