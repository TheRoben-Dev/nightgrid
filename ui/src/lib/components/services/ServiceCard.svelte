<script lang="ts">
  import type { Service } from "../../api";
  import { openExternal } from "../../clipboard";
  import { hosts } from "../../stores/hosts.svelte";
  import { CERT_WARN_DAYS, certDays, healthOf, hostOfUrl, nowSecs, services, uptime } from "../../stores/services.svelte";
  import Sparkline from "../metrics/Sparkline.svelte";
  import { errorText } from "../../api";
  import { tunnels } from "../../stores/tunnels.svelte";

  interface Props {
    service: Service;
    showHost?: boolean;
  }
  let { service, showHost = true }: Props = $props();

  const st = $derived(services.status[service.id]);
  const health = $derived(healthOf(service, st));
  const points = $derived(services.history[service.id] ?? []);
  const latency = $derived(points.filter((p) => p.latencyMs != null).map((p) => p.latencyMs as number));
  const up24 = $derived(uptime(points));
  const tls = $derived(st?.result.tls ?? null);
  const days = $derived(certDays(tls));
  const host = $derived(service.hostId != null ? hosts.byId(service.hostId) : undefined);
  const busy = $derived(services.checking.includes(service.id));
  let viaBusy = $state(false);
  let viaError = $state<string | null>(null);

  async function viaTunnel() {
    if (service.hostId == null) return;
    viaBusy = true;
    viaError = null;
    try {
      await tunnels.openThrough(service.hostId, service.url);
    } catch (e) {
      viaError = errorText(e);
    } finally {
      viaBusy = false;
    }
  }
  const monogram = $derived(
    service.name
      .split(/\s+/)
      .map((w: string) => w[0] ?? "")
      .join("")
      .slice(0, 2)
      .toUpperCase(),
  );

  const STATE_WORD = { up: "ONLINE", down: "DOWN", warn: "ONLINE", pending: "CHECKING", off: "NOT CHECKED" } as const;

  const ago = (ts: number) => {
    const s = Math.max(0, nowSecs() - ts);
    return s < 60 ? `${s}s ago` : s < 3600 ? `${Math.floor(s / 60)}m ago` : `${Math.floor(s / 3600)}h ago`;
  };

  const certLabel = $derived.by(() => {
    if (!tls) return null;
    if (days !== null && days < 0) return "CERT EXPIRED";
    const d = days === null ? "" : `${days}d`;
    return tls.trusted ? `TLS ${d}` : `UNTRUSTED ${d}`.trim();
  });
  const certTone = $derived(
    !tls ? "" : (days !== null && days < 3) || (!tls.trusted && !service.allowUntrusted) ? "bad" : !tls.trusted || (days !== null && days < CERT_WARN_DAYS) ? "warn" : "ok",
  );
  const certTitle = $derived(
    tls
      ? [
          tls.trusted ? "Trusted certificate" : `Not trusted: ${tls.problem ?? "unknown reason"}`,
          tls.subject ? `Subject: ${tls.subject}` : "",
          tls.issuer ? `Issuer: ${tls.issuer}` : "",
          tls.expires ? `Expires: ${new Date(tls.expires * 1000).toLocaleString()}` : "",
        ]
          .filter(Boolean)
          .join("\n")
      : "",
  );
</script>

<article class="card chamfer-sm {health}" data-guide="service-card">
  <header>
    <span class="mono-badge" aria-hidden="true">{monogram}</span>
    <div class="title">
      <button type="button" class="name" title="Open {service.url} in your browser" onclick={() => openExternal(service.url)}>
        {service.name}
      </button>
      <span class="url">{hostOfUrl(service.url)}</span>
    </div>
    <span class="state" title={st?.result.error ?? ""}>
      <span class="dot" aria-hidden="true"></span>{STATE_WORD[health]}
    </span>
  </header>

  {#if service.description}<p class="desc">{service.description}</p>{/if}

  <div class="trend">
    {#if latency.length > 1}
      <Sparkline values={latency} capacity={144} height={22} color={health === "down" ? "var(--danger)" : "var(--cyan)"} label="Latency, last 24 hours" />
    {:else}
      <div class="flat" aria-hidden="true"></div>
    {/if}
  </div>

  <dl class="stats">
    <div>
      <dt>HTTP</dt>
      <dd class:bad={st && !st.result.up}>{st?.result.status ?? "—"}</dd>
    </div>
    <div>
      <dt>LAT</dt>
      <dd>{st?.result.latencyMs != null ? `${st.result.latencyMs}ms` : "—"}</dd>
    </div>
    <div>
      <dt>24H</dt>
      <dd>{up24 === null ? "—" : `${up24 >= 99.95 ? "100" : up24.toFixed(1)}%`}</dd>
    </div>
    {#if certLabel}
      <div>
        <dt>CERT</dt>
        <dd class="cert {certTone}" title={certTitle}>{certLabel}</dd>
      </div>
    {/if}
  </dl>

  {#if st && !st.result.up && st.result.error}
    <p class="err" role="status">{st.result.error}</p>
  {/if}
  {#if viaError}<p class="err" role="alert">{viaError}</p>{/if}

  <footer>
    {#if showHost && host}<span class="chip" title="Runs on {host.name}">{host.name}</span>{/if}
    {#if service.source === "homepage"}<span class="chip src" title="Imported from Homepage">HP</span>{/if}
    <span class="when">{st ? ago(st.ts) : ""}</span>
    <span class="spacer"></span>
    {#if service.checkEnabled}
      <button type="button" class="btn ghost tiny" disabled={busy} onclick={() => services.checkNow(service.id)}>
        {busy ? "…" : "Check"}
      </button>
    {/if}
    {#if service.hostId != null}
      <button
        type="button"
        class="btn ghost tiny"
        disabled={viaBusy}
        title="Forward {hostOfUrl(service.url)} from {host?.name ?? 'its host'} to this computer and open it"
        onclick={viaTunnel}>{viaBusy ? "…" : "Tunnel"}</button
      >
    {/if}
    <button type="button" class="btn ghost tiny" onclick={() => (services.editing = service.id)}>Edit</button>
  </footer>
</article>

<style>
  .card {
    --tone: var(--line);
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px;
    background: var(--bg-1);
    border: 1px solid var(--line);
    border-left: 2px solid var(--tone);
    min-width: 0;
  }
  .up {
    --tone: var(--ok);
  }
  .warn {
    --tone: var(--warn);
  }
  .down {
    --tone: var(--danger);
    border-color: color-mix(in srgb, var(--danger) 45%, var(--line));
  }
  .pending {
    --tone: var(--text-1);
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .mono-badge {
    flex: none;
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    font-family: var(--font-display);
    font-weight: 700;
    font-size: var(--fs-xs);
    color: var(--mint);
    border: 1px solid color-mix(in srgb, var(--mint) 35%, var(--line));
  }
  .title {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .name {
    all: unset;
    cursor: pointer;
    color: var(--text-0);
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .name:hover,
  .name:focus-visible {
    color: var(--accent);
    text-decoration: underline;
  }
  .url {
    font-size: var(--fs-xs);
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .state {
    flex: none;
    display: flex;
    align-items: center;
    gap: 5px;
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: 0.12em;
    color: var(--tone);
  }
  .off .state {
    color: var(--text-1);
  }
  .dot {
    width: 6px;
    height: 6px;
    background: var(--tone);
  }
  .up .dot {
    box-shadow: 0 0 6px var(--ok);
  }
  .down .dot {
    animation: blink 1.2s steps(2) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.2;
    }
  }
  :global(:root[data-effects="off"]) .down .dot {
    animation: none;
  }
  .desc {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .trend {
    height: 22px;
  }
  .flat {
    height: 100%;
    border-bottom: 1px dashed var(--line);
  }
  .stats {
    display: flex;
    gap: 14px;
    margin: 0;
    font-size: var(--fs-sm);
  }
  .stats div {
    display: flex;
    flex-direction: column;
  }
  dt {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
  }
  dd {
    margin: 0;
    color: var(--cyan);
  }
  dd.bad,
  .cert.bad {
    color: var(--danger);
  }
  .cert.warn {
    color: var(--warn);
  }
  .cert.ok {
    color: var(--mint);
  }
  .err {
    margin: 0;
    font-size: var(--fs-xs);
    color: var(--danger);
    overflow-wrap: anywhere;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-xs);
  }
  .chip {
    padding: 0 5px;
    border: 1px solid var(--line);
    color: var(--text-1);
    letter-spacing: 0.06em;
  }
  .chip.src {
    color: var(--mint);
  }
  .when {
    color: var(--text-1);
  }
  .spacer {
    flex: 1;
  }
  .tiny {
    min-height: 20px;
    padding: 0 8px;
    font-size: var(--fs-xs);
  }
</style>
