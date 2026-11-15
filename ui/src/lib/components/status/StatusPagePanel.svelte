<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, type StatusPageInfo, type StatusPageSettings } from "../../api";
  import { writeClipboard } from "../../clipboard";
  import { BIND_LABEL, maskUrl } from "../../stores/statusPage";
  import Panel from "../Panel.svelte";
  import Toggle from "../Toggle.svelte";

  let info = $state<StatusPageInfo | null>(null);
  let busy = $state(false);
  let err = $state<string | null>(null);
  let revealed = $state(false);
  let copied = $state<string | null>(null);
  let port = $state("8787");

  // Dashboard export
  let withAddresses = $state(false);
  let exporting = $state(false);
  let exportMsg = $state<string | null>(null);

  onMount(() => {
    void api
      .statusPageGet()
      .then((i) => {
        info = i;
        port = String(i.settings.port);
      })
      .catch((e) => (err = errorText(e)));
  });

  async function save(patch: Partial<StatusPageSettings>) {
    if (!info) return;
    busy = true;
    err = null;
    try {
      info = await api.statusPageSet({ ...info.settings, ...patch });
      port = String(info.settings.port);
      if (!info.settings.enabled) revealed = false;
    } catch (e) {
      err = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function newToken() {
    busy = true;
    err = null;
    try {
      info = await api.statusPageNewToken();
    } catch (e) {
      err = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function copy(u: string) {
    await writeClipboard(u);
    copied = u;
    setTimeout(() => (copied = null), 1500);
  }

  async function exportPage() {
    exporting = true;
    exportMsg = null;
    try {
      const p = await api.dashboardExport(withAddresses);
      if (p) exportMsg = `Saved ${p}.`;
    } catch (e) {
      exportMsg = `Export failed: ${errorText(e)}`;
    } finally {
      exporting = false;
    }
  }
</script>

<Panel title="STATUS PAGE · SNAPSHOT" guide="settings-status-page">
  <p class="muted lead">
    A read-only page for your phone: every host's state with CPU, memory and disk, active alerts and service checks. Served by this app while it runs,
    only on the network you pick, behind a secret link. No buttons, no commands, nothing to change.
  </p>
  {#if info}
    <Toggle
      label="Serve the phone status page"
      hint="Off by default. Anyone with the link on that network can read the page."
      checked={info.settings.enabled}
      disabled={busy}
      onchange={(v) => save({ enabled: v })}
    />
    <div class="row2">
      <label class="field">
        <span class="sub">Listen on</span>
        <select class="input" value={info.settings.bind} disabled={busy} onchange={(e) => save({ bind: e.currentTarget.value })} data-guide="status-page-bind">
          {#each Object.entries(BIND_LABEL) as [k, label] (k)}<option value={k}>{label}</option>{/each}
        </select>
      </label>
      <label class="field">
        <span class="sub">Port</span>
        <input
          class="input"
          inputmode="numeric"
          bind:value={port}
          disabled={busy}
          onchange={() => {
            const n = Number(port);
            if (Number.isInteger(n) && n !== info?.settings.port) void save({ port: n });
          }}
        />
      </label>
    </div>
    {#if info.settings.bind === "all"}
      <p class="warn small">Every interface includes any public address this machine has. Prefer LAN or Tailscale.</p>
    {/if}
    {#if info.error}<p class="error-text">{info.error}</p>{/if}
    {#if info.running && info.urls.length}
      <div class="links" data-guide="status-page-links">
        {#each info.urls as u (u)}
          <div class="link">
            <code class="url">{revealed ? u : maskUrl(u)}</code>
            <button type="button" class="btn ghost tiny" onclick={() => copy(u)}>{copied === u ? "Copied" : "Copy"}</button>
          </div>
        {/each}
        <div class="actions">
          <button type="button" class="btn ghost tiny" onclick={() => (revealed = !revealed)}>{revealed ? "Hide link" : "Reveal link"}</button>
          <button type="button" class="btn ghost tiny" disabled={busy} title="Old links stop working" onclick={newToken}>New link</button>
        </div>
        <p class="muted small">Open it on your phone and add it to the home screen. It refreshes every 30 seconds. Ten wrong links from one address lock that address out for 10 minutes.</p>
      </div>
    {:else if info.settings.enabled && !info.error}
      <p class="muted small">Starting…</p>
    {/if}
  {:else if err}
    <p class="error-text">{err}</p>
  {/if}
  {#if info && err}<p class="error-text">{err}</p>{/if}

  <div class="block">
    <span class="sub">Dashboard snapshot</span>
    <p class="muted small">The same page as one HTML file, frozen at this moment: to attach to a ticket, archive, or open anywhere without the app.</p>
    <Toggle label="Include host addresses" hint="Off: only names are written." checked={withAddresses} onchange={(v) => (withAddresses = v)} />
    <div class="actions">
      <button type="button" class="btn primary" data-guide="dashboard-export" disabled={exporting} onclick={exportPage}>{exporting ? "Saving…" : "Save snapshot…"}</button>
    </div>
    {#if exportMsg}<p class="small path">{exportMsg}</p>{/if}
  </div>
</Panel>

<style>
  .lead {
    margin: 0 0 10px;
    font-size: var(--fs-sm);
  }
  .sub {
    display: block;
    margin-bottom: 6px;
    color: var(--text-1);
  }
  .row2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    margin-top: 10px;
  }
  .block {
    border-top: 1px solid var(--line);
    padding-top: 10px;
    margin-top: 10px;
  }
  .links {
    margin-top: 10px;
    display: grid;
    gap: 6px;
  }
  .link {
    display: flex;
    gap: 8px;
    align-items: center;
    min-width: 0;
  }
  .url {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
    color: var(--cyan);
    font-size: var(--fs-sm);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 8px;
  }
  .small {
    font-size: var(--fs-xs);
    margin: 4px 0 0;
  }
  .warn {
    color: var(--warn);
  }
  .path {
    overflow-wrap: anywhere;
  }
</style>
