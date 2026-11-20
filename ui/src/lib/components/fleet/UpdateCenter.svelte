<script lang="ts">
  import { typedMatches } from "../../confirm";
  /**
   * Fleet → UPDATES (M10m): pending package updates on every host, reboot
   * flags, batch upgrade with a typed confirmation (rule 5), and container
   * images whose tag moved (registries checked only on request, rule 7).
   */
  import { tick } from "svelte";
  import { openExternal } from "../../clipboard";
  import { agoText, confirmPhrase, MANAGER_WORD, pendingImages, totals, updateRows, updatesText } from "../../fleet";
  import { docker } from "../../stores/docker.svelte";
  import { fleet } from "../../stores/fleet.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { system } from "../../stores/system.svelte";
  import { ui } from "../../stores/ui.svelte";
  import ExplainButton from "../helper/ExplainButton.svelte";
  import Modal from "../Modal.svelte";

  $effect(() => {
    if (hosts.hosts.length) fleet.ensure();
  });

  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });

  const rows = $derived(updateRows(hosts.hosts, fleet.updates));
  const sum = $derived(totals(rows));
  const images = $derived(pendingImages(fleet.images ?? []));
  const imagesChecked = $derived((fleet.images ?? []).length);
  const skippedImages = $derived(Object.keys(fleet.imageSkipped).length);
  const picked = $derived(hosts.hosts.filter((h) => fleet.selected.includes(h.id)));
  const phrase = $derived(confirmPhrase(picked, hosts.groups, hosts.hosts));
  const pickable = $derived(rows.filter((r) => r.upgradable && !fleet.runs[r.host.id]?.running));
  const allPicked = $derived(pickable.length > 0 && pickable.every((r) => fleet.selected.includes(r.host.id)));
  let typed = $state("");
  $effect(() => {
    if (!fleet.confirm) typed = "";
  });

  const nameOf = (id: number) => hosts.byId(id)?.name ?? `host ${id}`;

  function pickAll() {
    fleet.selected = allPicked ? [] : pickable.map((r) => r.host.id);
  }

  async function reboot(id: number) {
    system.select(id);
    ui.go("system");
    await system.openTab("updates");
    system.confirmPower = "reboot";
  }

  function openStack(hostId: number) {
    docker.select(hostId);
    docker.tab = "stacks";
    ui.go("docker");
  }

  let logs: Record<number, HTMLPreElement | undefined> = $state({});
  $effect(() => {
    for (const [id, r] of Object.entries(fleet.runs)) {
      void r.lines.length;
      const el = logs[Number(id)];
      void tick().then(() => el && (el.scrollTop = el.scrollHeight));
    }
  });
</script>

<div class="center" data-guide="fleet-updates">
  <div class="bar">
    <button type="button" class="btn ghost tiny" data-guide="fleet-check" disabled={fleet.checking || !hosts.hosts.length} onclick={() => fleet.checkAll()}
      >{fleet.checking ? "Checking…" : "Check all"}</button
    >
    <button
      type="button"
      class="btn ghost tiny"
      data-guide="fleet-images"
      title="Ask each image's registry whether its tag moved"
      disabled={fleet.imagesChecking || !hosts.hosts.length}
      onclick={() => fleet.checkImages()}>{fleet.imagesChecking ? "Checking images…" : "Check images"}</button
    >
    <span class="muted small">checked {agoText(fleet.checkedAt, now)}</span>
    <span class="spacer"></span>
    <span class="sum" class:none={!sum.packages}>{sum.packages ? `${sum.packages} PENDING ON ${sum.hosts} HOST${sum.hosts === 1 ? "" : "S"}` : "ALL UP TO DATE"}</span>
    {#if sum.security}<span class="sum bad">{sum.security} SECURITY</span>{/if}
    {#if sum.reboot}<span class="sum warn">{sum.reboot} NEED A REBOOT</span>{/if}
    {#if rows.some((r) => r.state?.status === "ok")}
      <ExplainButton
        source="update center"
        text={() => updatesText(rows, fleet.images ?? [])}
        question="Which of these updates matter most (security, kernel, Docker, anything that restarts services)? Which hosts should I upgrade or reboot first?"
      />
    {/if}
    <button type="button" class="btn primary tiny" data-guide="fleet-upgrade" disabled={!fleet.selected.length} onclick={() => (fleet.confirm = true)}
      >Upgrade {fleet.selected.length || ""} selected…</button
    >
  </div>

  <div class="scroll">
    <table class="hosts">
      <thead>
        <tr>
          <th class="pick"><input type="checkbox" aria-label="Select every host with updates" checked={allPicked} disabled={!pickable.length} onchange={pickAll} /></th>
          <th>HOST</th>
          <th>MANAGER</th>
          <th>PENDING</th>
          <th>STATUS</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#each rows as r (r.host.id)}
          {@const s = r.state}
          {@const run = fleet.runs[r.host.id]}
          {@const isOpen = fleet.open === r.host.id}
          <tr class="row t-{r.tone}">
            <td class="pick">
              <input
                type="checkbox"
                aria-label="Select {r.host.name}"
                checked={fleet.selected.includes(r.host.id)}
                disabled={!r.upgradable || !!run?.running}
                onchange={() => fleet.toggle(r.host.id)}
              />
            </td>
            <td>
              <button type="button" class="link" aria-expanded={isOpen} onclick={() => (fleet.open = isOpen ? null : r.host.id)}>{r.host.name}</button>
            </td>
            <td class="small">{s?.report?.manager ? MANAGER_WORD[s.report.manager] : s?.status === "ok" ? "—" : ""}</td>
            <td>
              {#if s?.status === "ok"}
                {#if r.count}<span class="count">{r.count}</span>{:else}<span class="ok small">UP TO DATE</span>{/if}
                {#if r.security}<span class="chip bad">{r.security} SECURITY</span>{/if}
                {#if r.reboot}<span class="chip warn" title={s.report?.rebootReasons.join("; ")}>REBOOT</span>{/if}
              {/if}
            </td>
            <td class="status small">
              {#if run?.running}
                <span class="live">● UPGRADING</span>
              {:else if run?.skipped}
                <span class="muted" title={run.skipped}>SKIPPED: {run.skipped}</span>
              {:else if run?.error}
                <span class="bad" title={run.error}>FAILED: {run.error}</span>
              {:else if run && run.code !== null && run.code !== 0}
                <span class="bad">UPGRADE EXIT {run.code}</span>
              {:else if run}
                <span class="ok">UPGRADED</span>
              {:else if s?.status === "checking"}
                <span class="muted">checking…</span>
              {:else if s?.status === "error"}
                <span class="bad" title={s.error}>{s.error}</span>
              {:else if s?.report?.note}
                <span class="muted" title={s.report.note}>{s.report.note}</span>
              {/if}
            </td>
            <td class="acts">
              {#if r.reboot && !run?.running}
                <button type="button" class="btn ghost tiny danger-text" title="Reboot {r.host.name} (opens System with the confirmation)" onclick={() => reboot(r.host.id)}>Reboot…</button>
              {/if}
              <button type="button" class="btn ghost tiny" disabled={s?.status === "checking" || !!run?.running} onclick={() => {
                  fleet.dismissRun(r.host.id);
                  void fleet.checkHost(r.host.id);
                }}>Check</button
              >
            </td>
          </tr>
          {#if isOpen || (run && run.lines.length)}
            <tr class="detail">
              <td></td>
              <td colspan="5">
                {#if isOpen && s?.report?.updates.length}
                  <table class="pkgs">
                    <thead><tr><th>PACKAGE</th><th>INSTALLED</th><th>AVAILABLE</th><th>FROM</th></tr></thead>
                    <tbody>
                      {#each s.report.updates as p (p.name)}
                        <tr class:sec={/security/i.test(p.repo ?? "")}>
                          <td>{p.name}</td>
                          <td class="ver">{p.current ?? "—"}</td>
                          <td class="ver new">{p.available}</td>
                          <td class="muted">{p.repo ?? ""}</td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                {:else if isOpen && s?.status === "ok"}
                  <p class="muted small">Nothing to upgrade.</p>
                {/if}
                {#if isOpen && s?.report?.rebootRequired && s.report.rebootReasons.length}
                  <p class="hint">Reboot needed: {s.report.rebootReasons.join("; ")}.</p>
                {/if}
                {#if run && run.lines.length}
                  <div class="runbar">
                    <span class="label">UPGRADE LOG</span>
                    <span class="spacer"></span>
                    {#if !run.running}<button type="button" class="btn ghost tiny" onclick={() => fleet.dismissRun(r.host.id)}>Close</button>{/if}
                  </div>
                  <pre bind:this={logs[r.host.id]}>{run.lines.join("\n")}</pre>
                {/if}
              </td>
            </tr>
          {/if}
        {/each}
      </tbody>
    </table>

    <section class="images" data-guide="fleet-image-list">
      <h3 class="label">// CONTAINER IMAGES</h3>
      {#if fleet.images === null}
        <p class="muted small pad">
          <strong>Check images</strong> asks each stack's registry whether an image tag moved (only on request). Updating a stack happens in Docker → STACKS, with a snapshot first.
        </p>
      {:else if !images.length}
        <p class="ok small pad">No newer images among {imagesChecked} checked{skippedImages ? `; ${skippedImages} host${skippedImages === 1 ? "" : "s"} skipped` : ""}.</p>
      {:else}
        <ul class="imgs">
          {#each images as i (`${i.hostId}/${i.stack.name}/${i.update.image}`)}
            <li>
              <span class="host">{nameOf(i.hostId)}</span>
              <span class="muted">{i.stack.name}</span>
              <code>{i.update.image}</code>
              <span class="ver">{i.update.current.version ?? i.update.current.revision?.slice(0, 7) ?? "current"} → <span class="new">{i.update.latest?.version ?? i.update.latest?.revision?.slice(0, 7) ?? "newer"}</span></span>
              <span class="spacer"></span>
              {#if i.update.notes}<button type="button" class="btn ghost tiny" onclick={() => i.update.notes && openExternal(i.update.notes)}>Notes</button>{/if}
              <button type="button" class="btn ghost tiny" onclick={() => openStack(i.hostId)}>Open stack</button>
            </li>
          {/each}
        </ul>
      {/if}
      {#if fleet.images !== null && skippedImages}
        <details class="skipped">
          <summary class="muted small">{skippedImages} host{skippedImages === 1 ? "" : "s"} not checked</summary>
          <ul>
            {#each Object.entries(fleet.imageSkipped) as [id, why] (id)}<li class="muted small">{nameOf(Number(id))}: {why}</li>{/each}
          </ul>
        </details>
      {/if}
    </section>
  </div>
</div>

<Modal open={fleet.confirm} title="UPGRADE HOSTS" tone="danger" width={540} onclose={() => (fleet.confirm = false)}>
  <p>Install every pending update on these hosts, three at a time:</p>
  <ul class="targets">
    {#each picked as h (h.id)}
      {@const rep = fleet.updates[h.id]?.report}
      <li>
        <strong>{h.name}</strong>
        <span class="muted">{rep?.manager ? MANAGER_WORD[rep.manager] : ""} · {rep?.updates.length ?? 0} package{rep?.updates.length === 1 ? "" : "s"}{rep?.manager === "ostree" ? " (staged for the next reboot)" : ""}</span>
      </li>
    {/each}
  </ul>
  <p class="muted">
    Services on them may restart while packages install. Config files keep your versions. Hosts whose sudo needs a password that Nightgrid doesn't remember are skipped and
    listed. Nothing reboots by itself.
  </p>
  <label class="field">
    <span>Type <strong class="verbatim">{phrase}</strong> to confirm</span>
    <input class="input" bind:value={typed} autocomplete="off" spellcheck="false" />
  </label>
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (fleet.confirm = false)}>Cancel</button>
    <button type="button" class="btn danger" disabled={!picked.length || !typedMatches(typed, phrase)} onclick={() => fleet.upgradeSelected()}
      >Upgrade {picked.length} host{picked.length === 1 ? "" : "s"}</button
    >
  {/snippet}
</Modal>

<style>
  .center {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }
  .bar,
  .runbar {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .spacer {
    flex: 1;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .sum {
    letter-spacing: 0.1em;
    color: var(--cyan);
  }
  .sum.none,
  .ok {
    color: var(--ok);
  }
  .sum.bad,
  .bad {
    color: var(--danger);
  }
  .sum.warn {
    color: var(--warn);
  }
  .scroll {
    flex: 1;
    overflow: auto;
    min-height: 0;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th {
    text-align: left;
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    color: var(--text-1);
    font-weight: normal;
    padding: 4px 8px;
    border-bottom: 1px solid var(--line);
  }
  .hosts td {
    padding: 5px 8px;
    font-size: var(--fs-sm);
    vertical-align: baseline;
  }
  .hosts .row td {
    border-bottom: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
  }
  .row.t-bad td:nth-child(2) {
    box-shadow: inset 2px 0 0 var(--danger);
  }
  .row.t-warn td:nth-child(2) {
    box-shadow: inset 2px 0 0 var(--warn);
  }
  .pick {
    width: 28px;
  }
  .count {
    color: var(--cyan);
    margin-right: 8px;
  }
  .chip {
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
    padding: 0 var(--tag-px);
    border: 1px solid currentColor;
    margin-right: 6px;
    display: inline-block;
    line-height: calc(var(--tag-h) - 2px);
    white-space: nowrap;
  }
  .chip.bad {
    color: var(--danger);
  }
  .chip.warn {
    color: var(--warn);
  }
  .status {
    max-width: 380px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .live {
    color: var(--cyan);
  }
  .acts {
    text-align: right;
    white-space: nowrap;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--text-0);
    cursor: pointer;
    font: inherit;
    text-decoration: underline dotted var(--text-1);
  }
  .detail td {
    padding: 2px 8px 12px;
  }
  .pkgs td {
    padding: 1px 8px;
    font-size: var(--fs-xs);
  }
  .pkgs tr.sec td:first-child {
    color: var(--danger);
  }
  .ver {
    font-family: var(--font-mono);
    color: var(--text-1);
  }
  .new {
    color: var(--mint);
  }
  .hint {
    font-size: var(--fs-xs);
    color: var(--warn);
    margin: 6px 0 0;
  }
  pre {
    margin: 4px 0 0;
    padding: 6px 8px;
    border: 1px solid var(--line);
    font-size: var(--fs-xs);
    max-height: 220px;
    overflow: auto;
    white-space: pre-wrap;
  }
  .images {
    margin-top: 16px;
  }
  .pad {
    margin: 6px 10px;
  }
  .imgs {
    list-style: none;
    margin: 0;
    padding: 0 10px;
  }
  .imgs li {
    display: flex;
    gap: 10px;
    align-items: baseline;
    font-size: var(--fs-sm);
    padding: 3px 0;
    border-bottom: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
  }
  .imgs .host {
    min-width: 110px;
  }
  .skipped {
    margin: 6px 10px;
  }
  .skipped ul {
    margin: 4px 0;
    padding-left: 18px;
  }
  .targets {
    margin: 6px 0 10px;
    padding-left: 18px;
  }
  .targets li {
    display: flex;
    gap: 10px;
    align-items: baseline;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
</style>
