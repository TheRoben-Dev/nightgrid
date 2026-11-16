<script lang="ts">
  /**
   * Reverse proxy route editor (M10v): Caddy sites in a Caddyfile (checked
   * and loaded by Caddy, the old file restored if it won't load) and
   * Traefik routes in Nightgrid's own file-provider file.
   */
  import type { CaddyTarget, TraefikRoute, TraefikTarget } from "../../api";
  import { redact } from "../../helper/prompt";
  import { splitDomains, targetKey } from "../../proxyedit";
  import { proxyEdit } from "../../stores/proxyedit.svelte";
  import Modal from "../Modal.svelte";

  interface Props {
    open: boolean;
    onclose: () => void;
  }
  let { open, onclose }: Props = $props();

  // Caddy
  let domains = $state("");
  let upstream = $state("");
  let changing = $state<string | null>(null);
  let newUp = $state("");
  let asText = $state(false);
  let text = $state("");
  let reveal = $state(false);
  // Traefik
  const blank = (): TraefikRoute => ({ name: "", domains: [], upstream: "", entrypoints: [], tls: false, certResolver: null });
  let route = $state<TraefikRoute>(blank());
  let routeDomains = $state("");
  let editingRoute = $state<string | null>(null);
  let confirmRemove = $state<string | null>(null);

  $effect(() => {
    if (open) {
      proxyEdit.pending = null;
      proxyEdit.notice = null;
      void proxyEdit.load();
    }
  });

  const caddy = $derived(proxyEdit.caddy);
  const traefik = $derived(proxyEdit.traefik);
  const pending = $derived(proxyEdit.pending);

  function resetForms() {
    domains = "";
    upstream = "";
    changing = null;
    asText = false;
    route = blank();
    routeDomains = "";
    editingRoute = null;
    confirmRemove = null;
  }
  function pick(key: string) {
    proxyEdit.selected = key;
    proxyEdit.pending = null;
    proxyEdit.notice = null;
    resetForms();
  }
  async function addSite(t: CaddyTarget) {
    if (await proxyEdit.propose(t, { op: "addSite", domains: splitDomains(domains), upstream: upstream.trim() })) resetForms();
  }
  async function setUp(t: CaddyTarget, site: string) {
    if (await proxyEdit.propose(t, { op: "setUpstream", site, upstream: newUp.trim() })) changing = null;
  }
  function openText(t: CaddyTarget) {
    text = t.text;
    asText = true;
  }
  async function save() {
    if (await proxyEdit.saveCaddy()) resetForms();
  }

  function editRoute(r: TraefikRoute) {
    route = { ...r, entrypoints: [...r.entrypoints] };
    routeDomains = r.domains.join(", ");
    editingRoute = r.name;
    confirmRemove = null;
  }
  function toggleEp(e: string) {
    route.entrypoints = route.entrypoints.includes(e) ? route.entrypoints.filter((x) => x !== e) : [...route.entrypoints, e];
  }
  async function saveRoute(t: TraefikTarget) {
    const r: TraefikRoute = { ...route, domains: splitDomains(routeDomains), certResolver: route.certResolver || null };
    const others = t.routes.filter((x) => x.name !== editingRoute);
    const verb = editingRoute ? `change route ${editingRoute}` : `add route ${r.name || "?"}`;
    if (await proxyEdit.saveTraefik(t, [...others, r], verb)) resetForms();
  }
  async function removeRoute(t: TraefikTarget, name: string) {
    if (await proxyEdit.saveTraefik(t, t.routes.filter((x) => x.name !== name), `remove route ${name}`)) resetForms();
  }
</script>

<Modal {open} title="EDIT ROUTES" width={860} {onclose}>
  <div class="ed" data-guide="proxy-editor">
    {#if proxyEdit.error}<p class="bad small" role="alert">{proxyEdit.error}</p>{/if}
    {#if proxyEdit.loading && !proxyEdit.targets}<p class="muted small">Reading Caddyfiles and Traefik's configuration…</p>{/if}
    {#if proxyEdit.targets && !proxyEdit.targets.caddy.length && !proxyEdit.targets.traefik.length}
      <p class="muted small">No Caddyfile or Traefik container to edit on {proxyEdit.hostName}. Nightgrid edits <code>/etc/caddy/Caddyfile</code> in a Caddy container or on the host, and Traefik routes through its file provider. Nginx Proxy Manager has its own UI.</p>
    {/if}

    {#if proxyEdit.targets}
      <div class="seg" role="group" aria-label="Proxy">
        {#each proxyEdit.targets.caddy as c (c.proxyName)}
          <button type="button" class:on={proxyEdit.selected === targetKey("caddy", c.proxyName)} onclick={() => pick(targetKey("caddy", c.proxyName))}>CADDY · {c.proxyName}</button>
        {/each}
        {#each proxyEdit.targets.traefik as t (t.proxyName)}
          <button type="button" class:on={proxyEdit.selected === targetKey("traefik", t.proxyName)} onclick={() => pick(targetKey("traefik", t.proxyName))}>TRAEFIK · {t.proxyName}</button>
        {/each}
      </div>
    {/if}

    {#if proxyEdit.notice}<p class="small {proxyEdit.notice.tone === 'ok' ? 'ok' : 'bad'}" role="status">{proxyEdit.notice.text}</p>{/if}

    {#if pending}
      <div class="review" data-guide="proxy-review">
        <p><strong>{pending.summary}</strong> <span class="muted small">in {pending.target.path} ({pending.target.proxyName === "host" ? "on the host, needs sudo" : `container ${pending.target.proxyName}`})</span></p>
        <div class="diffbar">
          <span class="ok">+{pending.preview.diff.added}</span> <span class="bad">−{pending.preview.diff.removed}</span>
          <span class="spacer"></span>
          <button type="button" class="btn ghost tiny" onclick={() => (reveal = !reveal)}>{reveal ? "Mask secrets" : "Reveal secrets"}</button>
        </div>
        {#if pending.preview.diff.tooLarge}
          <p class="muted small">Too many differences for a line diff.</p>
        {:else}
          <pre class="diff">{#each pending.preview.diff.lines as l, i (i)}{#if l.tag === "skip"}<span class="skip">⋯ {l.text} unchanged lines</span>
{:else}<span class={l.tag}><span class="no">{l.old ?? ""}</span><span class="no">{l.new ?? ""}</span><span class="mk">{l.tag === "add" ? "+" : l.tag === "del" ? "−" : " "}</span>{reveal ? l.text : redact(l.text)}</span>
{/if}{/each}</pre>
        {/if}
        <p class="muted small">Caddy checks the new file first (<code>caddy validate</code>); if it won't load, the old one is put back. A copy of the old file stays in <code>.nightgrid-backups</code> next to it.</p>
      </div>
    {:else if caddy}
      <p class="muted small">{caddy.path} · {caddy.sites.length} sites</p>
      {#if asText}
        <textarea class="input mono text" bind:value={text} spellcheck="false" aria-label="Caddyfile"></textarea>
        <div class="row">
          <span class="spacer"></span>
          <button type="button" class="btn ghost tiny" onclick={() => (asText = false)}>Back</button>
          <button type="button" class="btn primary tiny" onclick={() => void proxyEdit.propose(caddy, { op: "replace", text })}>Review changes</button>
        </div>
      {:else}
        <div class="list" aria-label="Caddy sites">
          {#each caddy.sites as s (s.firstLine)}
            {@const site = s.addresses[0]}
            <div class="item">
              <span class="dom">{s.addresses.join(", ")}</span>
              {#if changing === site}
                <input class="input mono" bind:value={newUp} aria-label="New upstream" spellcheck="false" />
                <span class="acts">
                  <button type="button" class="btn primary tiny" disabled={!newUp.trim()} onclick={() => void setUp(caddy, site)}>Review</button>
                  <button type="button" class="btn ghost tiny" onclick={() => (changing = null)}>Cancel</button>
                </span>
              {:else}
                <code class="up">{s.upstreams.join(", ") || "—"}</code>
                <span class="acts">
                  {#if s.simple}<button type="button" class="btn ghost tiny" onclick={() => ((changing = site), (newUp = s.upstreams[0] ?? ""))}>Change…</button>{/if}
                  <button type="button" class="btn ghost tiny" onclick={() => void proxyEdit.propose(caddy, { op: "removeSite", site })}>Remove…</button>
                </span>
              {/if}
            </div>
          {:else}
            <p class="muted small pad">No site blocks with braces. Use <strong>Edit as text</strong>.</p>
          {/each}
        </div>
        <form class="row" onsubmit={(e) => (e.preventDefault(), void addSite(caddy))}>
          <input class="input grow" bind:value={domains} placeholder="app.example.com, www.app.example.com" aria-label="Domains" spellcheck="false" />
          <input class="input mono" bind:value={upstream} placeholder="app:8080" aria-label="Upstream" spellcheck="false" />
          <button type="submit" class="btn primary tiny" data-guide="proxy-add-site" disabled={!domains.trim() || !upstream.trim()}>Add site…</button>
          <button type="button" class="btn ghost tiny" onclick={() => openText(caddy)}>Edit as text</button>
        </form>
        <p class="muted small">Sites with more than one <code>reverse_proxy</code>, matchers or a block are changed as text. Caddy gets certificates for new domains by itself.</p>
      {/if}
    {:else if traefik}
      {#if traefik.blocked}
        <p class="warn small">{traefik.blocked}</p>
      {:else}
        <p class="muted small">Routes Nightgrid keeps in <code>{traefik.config.directory}/nightgrid.yml</code>. Routes from labels stay with their containers.</p>
        <div class="list" aria-label="Traefik routes">
          {#each traefik.routes as r (r.name)}
            <div class="item">
              <span class="dom">{r.domains.join(", ")} <span class="muted small">· {r.name}</span></span>
              <code class="up">{r.upstream}</code>
              <span class="acts">
                <span class="muted small">{r.entrypoints.join(", ") || "all"}{r.certResolver ? ` · ${r.certResolver}` : r.tls ? " · tls" : ""}</span>
                {#if confirmRemove === r.name}
                  <button type="button" class="btn danger tiny" disabled={proxyEdit.busy} onclick={() => void removeRoute(traefik, r.name)}>Remove {r.name}</button>
                  <button type="button" class="btn ghost tiny" onclick={() => (confirmRemove = null)}>Keep</button>
                {:else}
                  <button type="button" class="btn ghost tiny" onclick={() => editRoute(r)}>Edit</button>
                  <button type="button" class="btn ghost tiny" onclick={() => (confirmRemove = r.name)}>Remove…</button>
                {/if}
              </span>
            </div>
          {:else}
            <p class="muted small pad">No Nightgrid routes yet.</p>
          {/each}
        </div>
        <form class="form" onsubmit={(e) => (e.preventDefault(), void saveRoute(traefik))}>
          <h4>{editingRoute ? `EDIT ${editingRoute}` : "ADD A ROUTE"}</h4>
          <div class="row">
            <input class="input" bind:value={route.name} placeholder="name" aria-label="Route name" disabled={!!editingRoute} spellcheck="false" />
            <input class="input grow" bind:value={routeDomains} placeholder="photos.example.com" aria-label="Route domains" spellcheck="false" />
            <input class="input mono" bind:value={route.upstream} placeholder="http://immich:2283" aria-label="Route upstream" spellcheck="false" />
          </div>
          <div class="row">
            {#if traefik.config.entrypoints.length}
              <span class="muted small">Entry points</span>
              {#each traefik.config.entrypoints as e (e)}
                <label class="check"><input type="checkbox" checked={route.entrypoints.includes(e)} onchange={() => toggleEp(e)} />{e}</label>
              {/each}
            {/if}
            <label class="check"><input type="checkbox" bind:checked={route.tls} />TLS</label>
            {#if traefik.config.resolvers.length}
              <select class="input" bind:value={route.certResolver} aria-label="Certificate resolver">
                <option value={null}>no resolver</option>
                {#each traefik.config.resolvers as r (r)}<option value={r}>{r}</option>{/each}
              </select>
            {/if}
            <span class="spacer"></span>
            {#if editingRoute}<button type="button" class="btn ghost tiny" onclick={resetForms}>Cancel</button>{/if}
            <button type="submit" class="btn primary tiny" data-guide="proxy-save-route" disabled={proxyEdit.busy || !route.name.trim() || !routeDomains.trim() || !route.upstream.trim()}>{editingRoute ? "Save" : "Add route"}</button>
          </div>
        </form>
      {/if}
    {/if}
  </div>
  {#snippet footer()}
    {#if pending}
      <button type="button" class="btn ghost" onclick={() => (proxyEdit.pending = null)}>Back</button>
      <span class="spacer"></span>
      <button type="button" class="btn {pending.edit.op === 'removeSite' ? 'danger' : 'primary'}" data-guide="proxy-save" disabled={proxyEdit.busy} onclick={save}
        >{proxyEdit.busy ? "Saving…" : pending.edit.op === "removeSite" ? `Remove ${pending.edit.site}` : "Save and reload Caddy"}</button
      >
    {:else}
      <button type="button" class="btn ghost" disabled={proxyEdit.loading} onclick={() => void proxyEdit.load()}>Refresh</button>
      <span class="spacer"></span>
      <button type="button" class="btn ghost" onclick={onclose}>Close</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .ed {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .seg {
    display: inline-flex;
    flex-wrap: wrap;
    border: 1px solid var(--line);
    align-self: flex-start;
  }
  .seg button {
    background: none;
    border: 0;
    color: var(--text-1);
    padding: 3px 10px;
    font: inherit;
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
    cursor: pointer;
  }
  .seg button.on {
    color: var(--text-0);
    box-shadow: inset 0 -2px 0 var(--magenta);
  }
  .list {
    border: 1px solid var(--line);
    max-height: 260px;
    overflow: auto;
  }
  .item {
    display: grid;
    grid-template-columns: minmax(180px, 1.2fr) minmax(140px, 1fr) auto;
    gap: 10px;
    align-items: center;
    padding: 4px 8px;
    border-bottom: 1px solid var(--line);
    font-size: var(--fs-xs);
  }
  .dom {
    overflow-wrap: anywhere;
  }
  .up {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-1);
  }
  .acts {
    display: flex;
    gap: 4px;
    align-items: center;
    justify-content: flex-end;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .grow {
    flex: 1 1 220px;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 6px;
    border: 1px solid var(--line);
    padding: 8px;
  }
  h4 {
    margin: 0;
    color: var(--cyan);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    font-weight: 500;
  }
  .check {
    display: flex;
    gap: 4px;
    align-items: center;
    font-size: var(--fs-xs);
  }
  .text {
    min-height: 320px;
    font-size: var(--fs-xs);
    resize: vertical;
    white-space: pre;
  }
  .mono {
    font-family: var(--font-mono, monospace);
  }
  .review {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .review p {
    margin: 0;
  }
  .diffbar {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: var(--fs-xs);
  }
  .diff {
    margin: 0;
    padding: 6px 0;
    border: 1px solid var(--line);
    font-size: var(--fs-xs);
    max-height: 360px;
    overflow: auto;
    font-family: var(--font-mono, monospace);
  }
  .diff > span {
    display: inline-block;
    min-width: 100%;
    padding-right: 8px;
  }
  .diff .add {
    background: color-mix(in srgb, var(--mint) 12%, transparent);
  }
  .diff .del {
    background: color-mix(in srgb, var(--danger) 12%, transparent);
  }
  .diff .skip {
    color: var(--text-2);
    padding-left: 8px;
  }
  .no {
    display: inline-block;
    width: 3.5em;
    text-align: right;
    color: var(--text-2);
    padding-right: 6px;
  }
  .mk {
    display: inline-block;
    width: 1.5em;
  }
  .spacer {
    flex: 1;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .pad {
    padding: 8px;
    margin: 0;
  }
  p {
    margin: 0;
  }
  .ok {
    color: var(--ok);
  }
  .bad {
    color: var(--danger);
  }
  .warn {
    color: var(--warn, var(--yellow));
  }
  .muted {
    color: var(--text-1);
  }
</style>
