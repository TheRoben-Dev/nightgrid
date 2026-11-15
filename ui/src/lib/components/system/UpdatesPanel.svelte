<script lang="ts">
  import { tick } from "svelte";
  import { system } from "../../stores/system.svelte";

  const r = $derived(system.updates);
  const run = $derived(system.run);
  const canUpgrade = $derived(!!r?.manager && r.manager !== "ostree");
  let out: HTMLPreElement | undefined = $state();

  $effect(() => {
    void run?.lines.length;
    void tick().then(() => out && (out.scrollTop = out.scrollHeight));
  });

  const MGR: Record<string, string> = { apt: "APT", dnf: "DNF", pacman: "PACMAN", ostree: "RPM-OSTREE" };
</script>

<div class="updates" data-guide="system-updates">
  <div class="bar">
    {#if r?.manager}<span class="chip">{MGR[r.manager]}</span>{/if}
    {#if r}
      <span class="count" class:none={!r.updates.length}>
        {r.updates.length ? `${r.updates.length} UPGRADABLE` : "UP TO DATE"}
      </span>
      {#if r.rebootRequired}<span class="reboot">REBOOT REQUIRED</span>{/if}
    {/if}
    <span class="spacer"></span>
    <button type="button" class="btn ghost tiny" onclick={() => system.checkUpdates()} disabled={system.updatesLoading || run?.running}>
      {system.updatesLoading ? "Checking…" : "Check"}
    </button>
    {#if r?.manager === "apt"}
      <button type="button" class="btn ghost tiny" onclick={() => system.runPackages("refresh")} disabled={run?.running} title="apt-get update (sudo)">
        Refresh lists
      </button>
    {/if}
    {#if canUpgrade}
      <button type="button" class="btn primary tiny" disabled={!r?.updates.length || run?.running} onclick={() => (system.confirmUpgrade = true)}>
        Upgrade all…
      </button>
    {/if}
    {#if r?.rebootRequired}
      <button type="button" class="btn danger tiny" disabled={run?.running} onclick={() => (system.confirmPower = "reboot")}>Reboot…</button>
    {/if}
  </div>

  {#if system.errors.updates}<p class="error-text">{system.errors.updates}</p>{/if}
  {#if r?.note}<p class="hint">{r.note}</p>{/if}
  {#if r?.rebootRequired && r.rebootReasons.length}
    <p class="hint warn">Reboot needed: {r.rebootReasons.join("; ")}.</p>
  {/if}

  <div class="body" class:with-run={!!run}>
    <div class="list">
      {#if system.updatesLoading && !r}
        <p class="muted pad">Checking for updates…</p>
      {:else if r && !r.updates.length}
        <p class="muted pad">{r.manager ? "Nothing to upgrade." : ""}</p>
      {/if}
      {#if r?.updates.length}
        <table>
          <thead><tr><th>PACKAGE</th><th>INSTALLED</th><th>AVAILABLE</th><th>FROM</th></tr></thead>
          <tbody>
            {#each r.updates as p (p.name)}
              <tr class:sec={(p.repo ?? "").includes("security")}>
                <td class="name">{p.name}</td>
                <td class="ver">{p.current ?? "—"}</td>
                <td class="ver new">{p.available}</td>
                <td class="repo">{p.repo ?? ""}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
    {#if run}
      <div class="run">
        <div class="runbar">
          <span class="label">{run.op === "refresh" ? "REFRESH" : "UPGRADE"}</span>
          {#if run.running}<span class="live">● RUNNING</span>
          {:else if run.error}<span class="bad">FAILED</span>
          {:else if run.code === 0}<span class="ok">DONE</span>
          {:else}<span class="bad">EXIT {run.code ?? "?"}</span>{/if}
          <span class="spacer"></span>
          {#if !run.running}<button type="button" class="btn ghost tiny" onclick={() => (system.run = null)}>Close</button>{/if}
        </div>
        <pre bind:this={out}>{run.lines.join("\n")}{run.error ? `\n${run.error}` : ""}</pre>
      </div>
    {/if}
  </div>
</div>

<style>
  .updates {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  .bar,
  .runbar {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
  }
  .chip {
    padding: 0 6px;
    border: 1px solid var(--line);
    color: var(--mint);
  }
  .count {
    color: var(--cyan);
  }
  .count.none {
    color: var(--ok);
  }
  .reboot,
  .bad {
    color: var(--danger);
  }
  .ok {
    color: var(--ok);
  }
  .live {
    color: var(--warn);
  }
  .spacer {
    flex: 1;
  }
  .hint {
    margin: 0;
    padding: 4px 8px;
    font-size: var(--fs-xs);
    border: 1px dashed var(--line);
    color: var(--text-1);
  }
  .hint.warn {
    color: var(--warn);
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    gap: 6px;
  }
  .list {
    flex: 1;
    min-width: 0;
    overflow: auto;
    border: 1px solid var(--line);
    background: var(--bg-0);
  }
  .pad {
    padding: 12px;
    margin: 0;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--fs-sm);
  }
  th {
    position: sticky;
    top: 0;
    background: var(--bg-1);
    text-align: left;
    font-family: var(--font-display);
    font-weight: 600;
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    padding: 4px 10px;
    border-bottom: 1px solid var(--line);
  }
  td {
    padding: 3px 10px;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
    white-space: nowrap;
  }
  .name {
    color: var(--text-0);
  }
  .ver {
    color: var(--text-1);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
  }
  .ver.new {
    color: var(--cyan);
  }
  .repo {
    color: var(--text-1);
    font-size: var(--fs-xs);
  }
  tr.sec .repo {
    color: var(--warn);
  }
  .run {
    flex: 0 0 50%;
    min-width: 320px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  pre {
    flex: 1;
    margin: 0;
    padding: 8px;
    overflow: auto;
    border: 1px solid var(--line);
    background: var(--bg-0);
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    white-space: pre-wrap;
    word-break: break-word;
  }
  .tiny {
    min-height: 22px;
    padding: 0 8px;
    font-size: var(--fs-xs);
  }
</style>
