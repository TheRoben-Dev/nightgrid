<script lang="ts">
  /**
   * Proxy route starters: pick a host with a Caddy or Traefik proxy Nightgrid
   * can edit (Docker → PROXY), type your domain, see the change, confirm.
   * Saving goes through the same calls as the proxy route editor (M10v):
   * Caddy validates before loading and keeps a backup of the old file.
   */
  import { api, errorText, type CaddyPreview, type ProxyEditTargets, type StarterPreset } from "../../api";
  import { redact } from "../../helper/prompt";
  import { routeDomain, withTraefikRoutes } from "../../presets";
  import { targetKey } from "../../proxyedit";
  import { hosts } from "../../stores/hosts.svelte";
  import { proxy } from "../../stores/proxy.svelte";
  import { system } from "../../stores/system.svelte";

  let { preset }: { preset: StarterPreset } = $props();

  let hostId = $state<number | null>(null);
  let targets = $state<ProxyEditTargets | null>(null);
  let key = $state<string | null>(null);
  let domain = $state("");
  /** Routes the user unticked. */
  let off = $state<string[]>([]);
  let caddyPreview = $state<{ preview: CaddyPreview; added: string[] } | null>(null);
  let traefikAdded = $state<string[] | null>(null);
  let confirming = $state(false);
  let busy = $state(false);
  let done = $state<string | null>(null);
  let err = $state<string | null>(null);

  const servers = $derived(hosts.hosts.filter((h) => h.role !== "client"));
  const nameOf = (id: number | null) => hosts.hosts.find((h) => h.id === id)?.name ?? "";
  const caddy = $derived(targets?.caddy.find((c) => targetKey("caddy", c.proxyName) === key) ?? null);
  const traefik = $derived(targets?.traefik.find((t) => targetKey("traefik", t.proxyName) === key) ?? null);
  const routes = $derived(preset.routes.filter((r) => !off.includes(r.name)));
  const domainOk = $derived(!!preset.routes[0] && routeDomain(preset.routes[0], domain) !== null);
  const problem = $derived(
    hostId == null ? "Pick the host running the proxy." : !targets ? null : !key ? "No Caddy or Traefik proxy found on this host (see Docker → PROXY)." : !domainOk ? "Type your domain, such as home.example.com." : !routes.length ? "Tick at least one route." : traefik?.blocked ? traefik.blocked : null,
  );

  function reset() {
    caddyPreview = null;
    traefikAdded = null;
    confirming = false;
    done = null;
    err = null;
  }

  async function pickHost(id: number | null) {
    hostId = id;
    targets = null;
    key = null;
    reset();
    if (id == null) return;
    busy = true;
    try {
      const t = await api.proxyEditTargets(id);
      if (hostId !== id) return;
      targets = t;
      key = [...t.caddy.map((c) => targetKey("caddy", c.proxyName)), ...t.traefik.map((x) => targetKey("traefik", x.proxyName))][0] ?? null;
    } catch (e) {
      err = errorText(e);
    }
    busy = false;
  }

  async function preview() {
    reset();
    if (problem) return;
    busy = true;
    try {
      if (caddy) {
        let text = caddy.text;
        const added: string[] = [];
        let last: CaddyPreview | null = null;
        for (const r of routes) {
          const host = routeDomain(r, domain)!;
          if (caddy.sites.some((s) => s.addresses.includes(host))) continue;
          last = await api.caddyEditPreview(text, { op: "addSite", domains: [host], upstream: r.upstream });
          text = last.text;
          added.push(host);
        }
        // The diff against the file as it is now, not against the last step.
        if (last && added.length) caddyPreview = { preview: await api.caddyEditPreview(caddy.text, { op: "replace", text }), added };
        else done = "These names are already in the Caddyfile.";
      } else if (traefik) {
        const { added } = withTraefikRoutes(traefik.routes, routes, domain, traefik.config.resolvers[0] ?? null);
        if (added.length) traefikAdded = added;
        else done = "These routes are already there.";
      }
    } catch (e) {
      err = errorText(e);
    }
    busy = false;
  }

  async function save() {
    if (hostId == null) return;
    const id = hostId;
    busy = true;
    confirming = false;
    err = null;
    try {
      if (caddy && caddyPreview) {
        const c = caddy;
        const p = caddyPreview;
        const summary = `add ${p.added.join(", ")}`;
        const run = (sudo: Parameters<typeof api.caddySave>[5]) => api.caddySave(id, c.proxyName, p.preview.text, c.hash, summary, sudo);
        const r = c.proxyName === "host" ? await system.withSudo(`change ${c.path}`, run, nameOf(id)) : await run(null);
        if (!r.ok) throw new Error(r.message);
        done = `Caddy loaded it: ${p.added.join(", ")}.`;
      } else if (traefik && traefikAdded) {
        const { routes: next, added } = withTraefikRoutes(traefik.routes, routes, domain, traefik.config.resolvers[0] ?? null);
        await api.traefikSave(id, traefik.proxyName, next, traefik.hash);
        done = `Saved: ${added.join(", ")}. Traefik picks the file up by itself.`;
      }
      caddyPreview = null;
      traefikAdded = null;
      void proxy.load(true);
      const t = await api.proxyEditTargets(id);
      if (hostId === id) targets = t;
    } catch (e) {
      err = errorText(e);
    }
    busy = false;
  }

  function toggle(name: string) {
    off = off.includes(name) ? off.filter((x) => x !== name) : [...off, name];
    reset();
  }
</script>

<div class="rp">
  <label class="field">Host running the proxy
    <select class="input" value={hostId} onchange={(e) => void pickHost((e.currentTarget as HTMLSelectElement).value === "" ? null : Number((e.currentTarget as HTMLSelectElement).value))}>
      <option value="">pick a host…</option>
      {#each servers as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
    </select>
  </label>
  {#if targets && (targets.caddy.length + targets.traefik.length > 1)}
    <label class="field">Proxy
      <select class="input" bind:value={key} onchange={reset}>
        {#each targets.caddy as c (c.proxyName)}<option value={targetKey("caddy", c.proxyName)}>Caddy · {c.proxyName === "host" ? "on the host" : c.proxyName}</option>{/each}
        {#each targets.traefik as t (t.proxyName)}<option value={targetKey("traefik", t.proxyName)}>Traefik · {t.proxyName}</option>{/each}
      </select>
    </label>
  {/if}
  <label class="field">Your domain
    <input class="input" bind:value={domain} oninput={reset} placeholder="home.example.com" spellcheck="false" autocomplete="off" />
    <span class="hint">Each app gets a name in front of it. The names need DNS pointing at the proxy (a wildcard record is easiest).</span>
  </label>
  <ul class="small list">
    {#each preset.routes as r (r.name)}
      <li>
        <label class="check"><input type="checkbox" checked={!off.includes(r.name)} onchange={() => toggle(r.name)} />
          <span class="mono">{routeDomain(r, domain) ?? `${r.sub}.…`}</span> <span class="muted">→ {r.upstream} · {r.what}</span></label>
      </li>
    {/each}
  </ul>
  {#if problem && hostId != null && !busy}<p class="muted small">{problem}</p>{/if}
  <div class="row">
    <button type="button" class="btn primary" disabled={!!problem || busy} onclick={() => void preview()}>{busy && !targets ? "Reading the host…" : "Preview the change"}</button>
  </div>

  {#if caddyPreview}
    <p class="small">Adds {caddyPreview.added.length} site{caddyPreview.added.length === 1 ? "" : "s"} to {caddy?.path} <span class="ok">+{caddyPreview.preview.diff.added}</span> <span class="bad">−{caddyPreview.preview.diff.removed}</span></p>
    {#if !caddyPreview.preview.diff.tooLarge}
      <pre class="diff">{#each caddyPreview.preview.diff.lines as l, i (i)}{#if l.tag === "skip"}<span class="skip">⋯ {l.text} unchanged lines</span>
{:else}<span class={l.tag}>{l.tag === "add" ? "+" : l.tag === "del" ? "−" : " "} {redact(l.text)}</span>
{/if}{/each}</pre>
    {/if}
  {/if}
  {#if traefikAdded}
    <p class="small">Adds {traefikAdded.length} route{traefikAdded.length === 1 ? "" : "s"} to Nightgrid's route file for {traefik?.proxyName}: {traefikAdded.join(", ")}{traefik?.config.resolvers[0] ? `, certificates from ${traefik.config.resolvers[0]}` : ", TLS on"}.</p>
  {/if}
  {#if caddyPreview || traefikAdded}
    {#if confirming}
      <div class="confirm" role="alertdialog" aria-label="Confirm">
        <p class="small">Save these routes to the proxy on {nameOf(hostId)}? {caddy ? "Caddy checks the new file first and keeps the old one if it won't load." : "Traefik reloads the file by itself."}</p>
        <div class="row">
          <button type="button" class="btn primary" disabled={busy} onclick={() => void save()}>Yes, save</button>
          <button type="button" class="btn ghost" onclick={() => (confirming = false)}>Cancel</button>
        </div>
      </div>
    {:else}
      <div class="row"><button type="button" class="btn primary" disabled={busy} onclick={() => (confirming = true)}>{busy ? "Saving…" : "Save to the proxy…"}</button></div>
    {/if}
  {/if}
  {#if done}<p class="ok small">{done}</p>{/if}
  {#if err}<p class="bad small" role="alert">{err}</p>{/if}
</div>

<style>
  .rp {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .hint {
    opacity: 0.85;
  }
  .small {
    font-size: var(--fs-xs);
    margin: 0;
  }
  .muted {
    color: var(--text-1);
  }
  .mono {
    font-family: var(--font-mono);
  }
  .ok {
    color: var(--ok, #4ade80);
  }
  .bad {
    color: var(--danger);
  }
  .list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: 3px;
  }
  .check {
    display: flex;
    gap: 6px;
    align-items: baseline;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .diff {
    margin: 0;
    padding: 8px 10px;
    background: var(--bg-1);
    border: 1px solid var(--line);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    overflow: auto;
    max-height: 280px;
  }
  .diff .add {
    color: var(--ok, #4ade80);
  }
  .diff .del {
    color: var(--danger);
  }
  .diff .skip {
    color: var(--text-1);
  }
  .confirm {
    border: 1px solid var(--warn);
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
</style>
