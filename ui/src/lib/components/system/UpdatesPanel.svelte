<script lang="ts">
  import { tick } from "svelte";
  import { errorText } from "../../api";
  import { holdUntilText, holds, packageHoldPlace } from "../../stores/holds.svelte";
  import { system } from "../../stores/system.svelte";

  const r = $derived(system.updates);
  const pending = $derived(r ? r.updates.filter((u) => !u.held).length : 0);
  const heldCount = $derived(r ? r.held.length : 0);
  const hostHold = $derived(system.hostId === null ? null : holds.host(system.hostId));
  const mine = $derived(system.hostId === null ? [] : holds.forHost(system.hostId));
  let busy = $state<string | null>(null);
  let holdError = $state<string | null>(null);
  $effect(() => holds.ensure());

  /** Holds or releases one package (on the host needs root, like upgrades). */
  async function setHold(name: string, hold: boolean, version: string | null) {
    const id = system.hostId;
    const rep = r;
    if (id === null || !rep) return;
    busy = name;
    holdError = null;
    try {
      await system.withSudo(hold ? `hold ${name}` : `release ${name}`, (sudo) => holds.setPackage(id, rep, name, hold, sudo, version));
      await system.checkUpdates();
    } catch (e) {
      if (errorText(e) !== "Cancelled.") holdError = errorText(e);
    } finally {
      busy = null;
    }
  }
  const run = $derived(system.run);
  const canUpgrade = $derived(!!r?.manager);
  let out: HTMLPreElement | undefined = $state();

  $effect(() => {
    void run?.lines.length;
    void tick().then(() => out && (out.scrollTop = out.scrollHeight));
  });

  const MGR: Record<string, string> = { apt: "APT", dnf: "DNF", pacman: "PACMAN", ostree: "RPM-OSTREE", windowsUpdate: "WINDOWS UPDATE" };
  const win = $derived(r?.manager === "windowsUpdate");
</script>

<div class="updates" data-guide="system-updates">
  <div class="bar">
    {#if r?.manager}<span class="chip">{MGR[r.manager]}</span>{/if}
    {#if r}
      <span class="count" class:none={!pending}>
        {pending ? `${pending} UPGRADABLE` : "UP TO DATE"}
      </span>
      {#if heldCount}<span class="held" title="Held packages: upgrades leave them alone">{heldCount} HELD</span>{/if}
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
      <button type="button" class="btn primary tiny" disabled={!pending || run?.running} onclick={() => (system.confirmUpgrade = true)}>
        Upgrade all…
      </button>
    {/if}
    {#if r?.rebootRequired}
      <button type="button" class="btn danger tiny" disabled={run?.running} onclick={() => (system.confirmPower = "reboot")}>Reboot…</button>
    {/if}
  </div>

  {#if system.errors.updates}<p class="error-text">{system.errors.updates}</p>{/if}
  {#if holdError}<p class="error-text">{holdError}</p>{/if}
  {#if hostHold}
    <p class="hint">
      This host's updates are held {holdUntilText(hostHold)}{hostHold.note ? ` (${hostHold.note})` : ""}: Fleet → UPDATES skips it and doesn't count it. Upgrade here still works.
      <button type="button" class="btn ghost tiny" onclick={() => holds.release(hostHold.id)}>Release</button>
    </p>
  {/if}
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
      {#if r}
        {@const rep = r}
        {@const quiet = rep.held.filter((n) => !rep.updates.some((u) => u.name === n))}
        {#if quiet.length}
          <p class="muted pad heldlist">
            Held, nothing new for them:
            {#each quiet as n (n)}<span class="tag">{n}</span><button type="button" class="btn ghost tiny" disabled={busy !== null || run?.running} onclick={() => setHold(n, false, null)}>Release</button>{/each}
          </p>
        {/if}
      {/if}
      {#if r?.updates.length}
        <table>
          <thead>
            {#if win}<tr><th>UPDATE</th><th></th><th>ARTICLE</th><th>CATEGORY</th><th></th></tr>{:else}<tr><th>PACKAGE</th><th>INSTALLED</th><th>AVAILABLE</th><th>FROM</th><th></th></tr>{/if}
          </thead>
          <tbody>
            {#each r.updates as p (p.name)}
              {@const place = p.held ? packageHoldPlace(r, p.name, mine) : null}
              <tr class:sec={!p.held && (p.repo ?? "").includes("security")} class:heldrow={p.held}>
                <td class="name">{p.name}{#if p.held}<span class="tag" title={place === "nightgrid" ? "Held in Nightgrid: left out of Nightgrid's upgrades" : win ? "Hidden in Windows Update" : "Held on the host: every tool leaves it alone"}>HELD</span>{/if}</td>
                <td class="ver">{p.current ?? "—"}</td>
                <td class="ver new">{p.available}</td>
                <td class="repo">{p.repo ?? ""}</td>
                <td class="acts">
                  {#if r.holdSupport !== "none"}
                    <button type="button" class="btn ghost tiny" data-guide="system-hold-package" disabled={busy !== null || run?.running}
                      title={p.held ? "Release: upgrades install it again" : win ? "Hide it in Windows Update (needs an administrator)" : r.holdSupport === "host" ? "Hold it on the host (apt-mark hold / dnf versionlock, needs root)" : "Hold it in Nightgrid: left out of Nightgrid's upgrades"}
                      onclick={() => setHold(p.name, !p.held, p.current)}>{busy === p.name ? "…" : p.held ? "Release" : "Hold"}</button>
                  {/if}
                </td>
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
  .held,
  .tag {
    color: var(--text-1);
  }
  .tag {
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
    border: 1px dashed currentColor;
    padding: 0 4px;
    margin-left: 6px;
  }
  tr.heldrow td {
    color: var(--text-1);
  }
  .acts {
    text-align: right;
    white-space: nowrap;
  }
  .heldlist {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .chip {
    padding: 0 var(--tag-px);
    border: 1px solid var(--line);
    color: var(--mint);
    display: inline-block;
    line-height: calc(var(--tag-h) - 2px);
    white-space: nowrap;
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
</style>
