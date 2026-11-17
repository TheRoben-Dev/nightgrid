<script lang="ts">
  import { api, errorText, type ContainerDetail, type ContainerView } from "../../api";
  import { openExternal } from "../../clipboard";
  import { isUp } from "../../stores/docker.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { integrations, portainerUrl } from "../../stores/integrations.svelte";
  import { targetLabel, targetUrl, webui } from "../../stores/webui.svelte";
  import ContainerLogs from "./ContainerLogs.svelte";

  interface Props {
    hostId: number;
    container: ContainerView;
    onclose: () => void;
  }
  let { hostId, container, onclose }: Props = $props();

  type Tab = "logs" | "details" | "env" | "labels" | "raw";
  let tab = $state<Tab>("logs");
  let detail = $state<ContainerDetail | null>(null);
  let error = $state<string | null>(null);
  /** Env keys whose values are shown (masked by default, security rule 6). */
  let revealed = $state<string[]>([]);

  // Reload inspect data when the container or its state changes.
  $effect(() => {
    const id = container.id;
    void container.state;
    let live = true;
    error = null;
    api
      .dockerInspect(hostId, id)
      .then((d) => live && (detail = d))
      .catch((e) => live && (error = errorText(e)));
    return () => (live = false);
  });

  // Portainer deep link, when an integration is linked to this host.
  $effect(() => {
    if (!integrations.loaded) void integrations.load();
  });
  const portainer = $derived(portainerUrl(integrations.list, hostId, container.id));

  // Hide revealed values again when switching containers.
  $effect(() => {
    void container.id;
    revealed = [];
    detail = null;
  });

  const tabs: { id: Tab; label: string }[] = [
    { id: "logs", label: "LOGS" },
    { id: "details", label: "DETAILS" },
    { id: "env", label: "ENV" },
    { id: "labels", label: "LABELS" },
    { id: "raw", label: "INSPECT JSON" },
  ];

  const allRevealed = $derived(!!detail && detail.env.length > 0 && detail.env.every((e) => revealed.includes(e.key)));
  const toggleReveal = (k: string) =>
    (revealed = revealed.includes(k) ? revealed.filter((x) => x !== k) : [...revealed, k]);

  const host = $derived(hosts.byId(hostId));
  /** Web UI addresses (D-147), best first. */
  const webs = $derived(host && container.state === "running" ? container.web.slice(0, 3).map((t) => ({ t, url: targetUrl(t, host.hostname) })) : []);

  const when = (t: string | null) => (t ? new Date(t).toLocaleString() : "—");
  const copy = (s: string) => void navigator.clipboard?.writeText(s);
</script>

<section class="panel" aria-label="Container {container.name}">
  <header>
    <span class="dot {container.state}" aria-hidden="true"></span>
    <h2>{container.name}</h2>
    <span class="muted img">{container.image}</span>
    <span class="spacer"></span>
    {#each webs as w, i (w.url)}
      {#if i === 0}
        <button
          type="button"
          class="btn tiny"
          title="Open {w.url} in a Nightgrid window"
          data-guide="docker-webui-panel"
          disabled={webui.isBusy(hostId, w.url)}
          onclick={() => void webui.open(hostId, w.url, container.service ?? container.name)}
          >{webs.length > 1 ? `Web UI ${targetLabel(w.t)}` : "Web UI"}</button
        >
      {:else}
        <button
          type="button"
          class="btn ghost tiny"
          title="Open {w.url} in a Nightgrid window"
          disabled={webui.isBusy(hostId, w.url)}
          onclick={() => void webui.open(hostId, w.url, container.service ?? container.name)}>{targetLabel(w.t)}</button
        >
      {/if}
    {/each}
    {#if portainer}
      <button type="button" class="btn ghost tiny" title={portainer} onclick={() => portainer && openExternal(portainer)}>Open in Portainer</button>
    {/if}
    <button type="button" class="x" aria-label="Close panel" title="Close" onclick={onclose}>×</button>
  </header>
  <div class="tabs" role="tablist">
    {#each tabs as t (t.id)}
      <button type="button" role="tab" aria-selected={tab === t.id} class:active={tab === t.id} onclick={() => (tab = t.id)}
        >{t.label}{#if t.id === "env" && detail}<span class="n">{detail.env.length}</span>{/if}</button
      >
    {/each}
  </div>
  <div class="body">
    {#if tab === "logs"}
      <ContainerLogs {hostId} containerId={container.id} name={container.name} running={isUp(container)} />
    {:else if error}
      <p class="error-text pad">{error}</p>
    {:else if !detail}
      <p class="muted pad">Loading…</p>
    {:else if tab === "details"}
      <div class="scroll pad">
        <dl class="kv">
          <dt>STATE</dt>
          <dd>
            {detail.state}{#if detail.health}
              · <span class="h-{detail.health}">{detail.health}</span>{/if}{#if detail.exitCode !== null && detail.state !== "running"}
              · exit {detail.exitCode}{/if}
          </dd>
          <dt>STARTED</dt>
          <dd>{when(detail.startedAt)}</dd>
          {#if detail.finishedAt && detail.state !== "running"}<dt>FINISHED</dt>
            <dd>{when(detail.finishedAt)}</dd>{/if}
          <dt>CREATED</dt>
          <dd>{when(detail.created)}</dd>
          <dt>RESTART</dt>
          <dd>{detail.restartPolicy}{detail.restartCount ? ` · restarted ${detail.restartCount}×` : ""}</dd>
          <dt>COMMAND</dt>
          <dd><code>{detail.command || "—"}</code></dd>
          <dt>ID</dt>
          <dd><code>{detail.id.slice(0, 12)}</code> <button type="button" class="link" onclick={() => copy(detail!.id)}>copy</button></dd>
        </dl>

        <h3 class="label">PORTS</h3>
        {#if detail.ports.length}
          <table>
            <tbody>
              {#each detail.ports as p, i (i)}
                <tr
                  ><td class="data">{p.public ? `${p.ip ?? "0.0.0.0"}:${p.public}` : "—"}</td><td>→</td><td
                    >{p.private}/{p.proto}</td
                  ></tr
                >
              {/each}
            </tbody>
          </table>
        {:else}<p class="muted">None.</p>{/if}

        <h3 class="label">MOUNTS</h3>
        {#if detail.mounts.length}
          <table>
            <tbody>
              {#each detail.mounts as m, i (i)}
                <tr>
                  <td class="tag">{m.kind}</td>
                  <td class="data" title={m.source}>{m.name ?? m.source}</td>
                  <td>→</td>
                  <td>{m.destination}</td>
                  <td class="muted">{m.rw ? "rw" : "ro"}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {:else}<p class="muted">None.</p>{/if}

        <h3 class="label">NETWORKS</h3>
        {#if detail.networks.length}
          <table>
            <tbody>
              {#each detail.networks as n (n.name)}
                <tr>
                  <td>{n.name}</td>
                  <td class="data">{n.ip || "—"}</td>
                  <td class="muted">gw {n.gateway || "—"}</td>
                  <td class="muted">{n.aliases.join(", ")}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {:else}<p class="muted">None.</p>{/if}
      </div>
    {:else if tab === "env"}
      <div class="scroll pad">
        <div class="envbar">
          <span class="muted">Values are hidden until you reveal them.</span>
          <span class="spacer"></span>
          {#if detail.env.length}
            <button
              type="button"
              class="btn ghost tiny"
              onclick={() => (revealed = allRevealed ? [] : detail!.env.map((e) => e.key))}
              >{allRevealed ? "Hide all" : "Reveal all"}</button
            >
          {/if}
        </div>
        {#if detail.env.length}
          <table class="env">
            <tbody>
              {#each detail.env as e, i (i)}
                {@const shown = revealed.includes(e.key)}
                <tr>
                  <td class="k">{e.key}</td>
                  <td class="v">
                    <button
                      type="button"
                      class="secret"
                      class:shown
                      aria-label={shown ? `Hide ${e.key}` : `Reveal ${e.key}`}
                      title={shown ? "Click to hide" : "Click to reveal"}
                      onclick={() => toggleReveal(e.key)}>{shown ? e.value || "(empty)" : "••••••••"}</button
                    >
                  </td>
                  <td>{#if shown}<button type="button" class="link" onclick={() => copy(e.value)}>copy</button>{/if}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {:else}<p class="muted">No environment variables.</p>{/if}
      </div>
    {:else if tab === "labels"}
      <div class="scroll pad">
        {#if detail.labels.length}
          <table class="env">
            <tbody>
              {#each detail.labels as l (l.key)}
                <tr><td class="k">{l.key}</td><td class="v data">{l.value}</td></tr>
              {/each}
            </tbody>
          </table>
        {:else}<p class="muted">No labels.</p>{/if}
      </div>
    {:else if tab === "raw"}
      <div class="rawbar">
        <span class="muted">Env values are masked here; reveal them in ENV.</span>
        <span class="spacer"></span>
        <button type="button" class="btn ghost tiny" onclick={() => copy(detail!.raw)}>Copy</button>
      </div>
      <pre class="raw">{detail.raw}</pre>
    {/if}
  </div>
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    height: 100%;
    border: 1px solid var(--line);
    background: var(--bg-1);
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--line);
    min-width: 0;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-md);
    color: var(--text-0);
    white-space: nowrap;
  }
  .img {
    font-size: var(--fs-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .spacer {
    flex: 1;
  }
  .x {
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    width: 24px;
    height: 22px;
    cursor: pointer;
  }
  .x:hover {
    color: var(--text-0);
  }
  .dot {
    width: 8px;
    height: 8px;
    flex: none;
    background: var(--text-1);
  }
  .dot.running {
    background: var(--mint);
  }
  .dot.paused,
  .dot.restarting {
    background: var(--warn);
  }
  .dot.dead {
    background: var(--danger);
  }
  .tabs {
    display: flex;
    border-bottom: 1px solid var(--line);
  }
  .tabs button {
    border: 0;
    border-right: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    padding: 5px 12px;
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    cursor: pointer;
  }
  .tabs button.active {
    color: var(--magenta);
    box-shadow: inset 0 -2px 0 var(--magenta);
  }
  .n {
    margin-left: 6px;
    color: var(--cyan);
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .scroll {
    overflow: auto;
    min-height: 0;
    flex: 1;
  }
  .pad {
    padding: 10px 12px;
  }
  .kv {
    display: grid;
    grid-template-columns: 90px 1fr;
    gap: 4px 12px;
    margin: 0 0 12px;
    font-size: var(--fs-sm);
  }
  dt {
    color: var(--text-1);
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    padding-top: 2px;
  }
  dd {
    margin: 0;
    color: var(--text-0);
    word-break: break-all;
  }
  code,
  .data {
    color: var(--cyan);
    font-family: var(--font-mono);
  }
  h3 {
    margin: 14px 0 6px;
    font-size: var(--fs-xs);
  }
  table {
    border-collapse: collapse;
    font-size: var(--fs-sm);
    width: 100%;
  }
  td {
    padding: 3px 8px 3px 0;
    vertical-align: top;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
    word-break: break-all;
  }
  .tag {
    color: var(--text-1);
    font-size: var(--fs-xs);
    text-transform: uppercase;
    width: 60px;
  }
  .env .k {
    color: var(--text-0);
    font-family: var(--font-mono);
    width: 40%;
  }
  .secret {
    border: 0;
    background: transparent;
    color: var(--text-1);
    font-family: var(--font-mono);
    font-size: var(--fs-sm);
    cursor: pointer;
    padding: 0;
    text-align: left;
    word-break: break-all;
  }
  .secret.shown {
    color: var(--cyan);
  }
  .secret:hover {
    color: var(--text-0);
  }
  .link {
    border: 0;
    background: transparent;
    color: var(--text-1);
    text-decoration: underline;
    cursor: pointer;
    font-size: var(--fs-xs);
    padding: 0;
  }
  .envbar,
  .rawbar {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-xs);
    margin-bottom: 8px;
  }
  .rawbar {
    padding: 6px 10px;
    margin: 0;
    border-bottom: 1px solid var(--line);
  }
  .raw {
    flex: 1;
    min-height: 0;
    overflow: auto;
    margin: 0;
    padding: 10px 12px;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-0);
    background: var(--bg-0);
  }
  .h-healthy {
    color: var(--mint);
  }
  .h-unhealthy {
    color: var(--danger);
  }
  .h-starting {
    color: var(--warn);
  }
  .tiny {
    padding: 2px 10px;
    font-size: var(--fs-xs);
  }
</style>
