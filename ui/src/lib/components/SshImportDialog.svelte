<script lang="ts">
  import { untrack } from "svelte";
  import { api, errorText, type SshConfigCandidate } from "../api";
  import { hosts } from "../stores/hosts.svelte";
  import Modal from "./Modal.svelte";

  let candidates = $state<SshConfigCandidate[]>([]);
  let picked = $state<Record<string, boolean>>({});
  let loading = $state(false);
  let error = $state<string | null>(null);
  let result = $state<string | null>(null);

  $effect(() => {
    if (hosts.importOpen) untrack(() => void scan());
  });

  async function scan() {
    loading = true;
    error = null;
    result = null;
    try {
      candidates = await api.sshConfigScan();
      // New aliases are pre-selected; already imported ones are opt-in re-syncs.
      picked = Object.fromEntries(candidates.map((c) => [c.alias, c.existingId === null]));
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }

  const chosen = $derived(candidates.filter((c) => picked[c.alias]).map((c) => c.alias));

  async function run() {
    loading = true;
    error = null;
    try {
      const r = await api.sshConfigImport(chosen);
      await hosts.refresh();
      result = `Imported ${r.created} new, re-synced ${r.updated}.`;
      await scan();
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }

  async function importKnownHosts() {
    try {
      const n = await api.knownHostsImportSystem();
      result = `Copied ${n} known_hosts entr${n === 1 ? "y" : "ies"} into NIGHTGRID's own file.`;
    } catch (e) {
      error = errorText(e);
    }
  }

  const close = () => (hosts.importOpen = false);
</script>

<Modal open={hosts.importOpen} title="IMPORT :: ~/.SSH/CONFIG" width={680} onclose={close}>
  <p class="muted intro">
    Reads <code>~/.ssh/config</code> (including <code>Include</code> files). Nothing is ever written back to it. Re-syncing
    updates connection details and keeps your names, groups, tags and colours.
  </p>
  {#if loading && !candidates.length}
    <p class="muted">Scanning…</p>
  {:else if !candidates.length}
    <p class="muted">No concrete <code>Host</code> entries found (wildcard patterns are skipped).</p>
  {:else}
    <table>
      <thead>
        <tr>
          <th class="sel"
            ><input
              type="checkbox"
              aria-label="Select all"
              checked={chosen.length === candidates.length}
              onchange={(e) => {
                const v = e.currentTarget.checked;
                picked = Object.fromEntries(candidates.map((c) => [c.alias, v]));
              }}
            /></th
          >
          <th>Alias</th>
          <th>Target</th>
          <th>Jump</th>
          <th>Status</th>
        </tr>
      </thead>
      <tbody>
        {#each candidates as c (c.alias)}
          <tr class:on={picked[c.alias]}>
            <td class="sel"
              ><input type="checkbox" aria-label="Import {c.alias}" bind:checked={picked[c.alias]} /></td
            >
            <td class="alias">{c.alias}</td>
            <td>{c.user ? `${c.user}@` : ""}{c.hostname}{c.port && c.port !== 22 ? `:${c.port}` : ""}</td>
            <td class="muted">{c.proxyJump ?? "—"}</td>
            <td>{#if c.existingId !== null}<span class="tag sync">RE-SYNC</span>{:else}<span class="tag new">NEW</span>{/if}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
  {#if result}<p class="ok" role="status">{result}</p>{/if}
  {#if error}<p class="error-text" role="alert">{error}</p>{/if}

  {#snippet footer()}
    <button type="button" class="btn ghost left" onclick={importKnownHosts} title="Copy ~/.ssh/known_hosts into NIGHTGRID's own file">
      Import known_hosts
    </button>
    <button type="button" class="btn ghost" onclick={close}>Close</button>
    <button type="button" class="btn primary" disabled={loading || !chosen.length} onclick={run}>
      Import {chosen.length || ""}
    </button>
  {/snippet}
</Modal>

<style>
  .intro {
    margin: 0 0 12px;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  code {
    color: var(--mint);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--fs-sm);
  }
  th {
    text-align: left;
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    text-transform: uppercase;
    color: var(--text-1);
    padding: 4px 8px;
    border-bottom: 1px solid var(--line);
  }
  td {
    padding: 6px 8px;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  }
  tr.on td {
    background: color-mix(in srgb, var(--mint) 5%, transparent);
  }
  .sel {
    width: 28px;
  }
  .alias {
    color: var(--text-0);
  }
  .tag {
    font-size: 10px;
    letter-spacing: 0.12em;
    padding: 1px 6px;
    border: 1px solid currentColor;
  }
  .tag.new {
    color: var(--mint);
  }
  .tag.sync {
    color: var(--cyan);
  }
  .ok {
    color: var(--mint);
    font-size: var(--fs-sm);
  }
  .left {
    margin-right: auto;
  }
  input[type="checkbox"] {
    accent-color: var(--magenta);
  }
</style>
