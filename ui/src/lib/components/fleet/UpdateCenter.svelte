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
  import { errorText, systemError } from "../../api";
  import { docker } from "../../stores/docker.svelte";
  import { fleet } from "../../stores/fleet.svelte";
  import { HOLD_FOR, holdUntilText, holds, packageHoldPlace } from "../../stores/holds.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { system } from "../../stores/system.svelte";
  import { ui } from "../../stores/ui.svelte";
  import ExplainButton from "../helper/ExplainButton.svelte";
  import Modal from "../Modal.svelte";

  $effect(() => {
    if (hosts.hosts.length) fleet.ensure();
    holds.ensure();
  });

  // ---- holds (D-195) ----
  let holdFor = $state<number | null>(7 * 86_400);
  let holdNote = $state("");
  let holdErr = $state<Record<number, string>>({});
  let pkgBusy = $state<string | null>(null);
  const dialogHost = $derived(holds.dialogHost === null ? null : (hosts.byId(holds.dialogHost) ?? null));
  const heldImages = $derived((fleet.images ?? []).filter((i) => i.update.held));
  const kindWord = { host: "HOST", package: "PACKAGE", image: "IMAGE", stack: "STACK" } as const;

  function openHold(id: number) {
    holdFor = 7 * 86_400;
    holdNote = "";
    holds.dialogHost = id;
  }

  async function confirmHold() {
    const id = holds.dialogHost;
    if (id === null) return;
    const err = await holds.holdHost(id, holdFor, holdNote.trim());
    if (err) holdErr = { ...holdErr, [id]: err };
    else {
      holds.dialogHost = null;
      fleet.selected = fleet.selected.filter((x) => x !== id);
    }
  }

  async function setPkg(hostId: number, name: string, hold: boolean, version: string | null) {
    const rep = fleet.updates[hostId]?.report;
    if (!rep) return;
    pkgBusy = `${hostId}/${name}`;
    const { [hostId]: _old, ...rest } = holdErr;
    holdErr = rest;
    try {
      await holds.setPackage(hostId, rep, name, hold, null, version);
      await fleet.checkHost(hostId);
    } catch (e) {
      const { system } = systemError(e);
      holdErr = {
        ...holdErr,
        [hostId]: system?.kind === "sudoRequired" ? "needs a sudo password: hold it from System → UPDATES (and tick remember)" : errorText(e),
      };
    } finally {
      pkgBusy = null;
    }
  }

  const holdName = (h: { kind: keyof typeof kindWord; target: string; stack: string }) =>
    h.kind === "host" ? "all updates" : h.kind === "image" ? `${h.stack} · ${h.target}` : h.target;

  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });

  const rows = $derived(updateRows(hosts.hosts, fleet.updates, (id) => holds.host(id)));
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
              {#if r.held}
                <span class="chip held" title={r.held.note || "Held: skipped by Upgrade and not counted"}>HELD {holdUntilText(r.held).toUpperCase()}</span>
                {#if r.heldCount}<span class="muted small">{r.heldCount} waiting</span>{/if}
              {:else if s?.status === "ok"}
                {#if r.count}<span class="count">{r.count}</span>{:else}<span class="ok small">UP TO DATE</span>{/if}
                {#if r.heldCount}<span class="chip held" title="Held packages: left out of upgrades">{r.heldCount} HELD</span>{/if}
                {#if r.security}<span class="chip bad">{r.security} SECURITY</span>{/if}
                {#if r.reboot}<span class="chip warn" title={s.report?.rebootReasons.join("; ")}>REBOOT</span>{/if}
              {/if}
            </td>
            <td class="status small">
              {#if holdErr[r.host.id]}
                <span class="bad" title={holdErr[r.host.id]}>HOLD: {holdErr[r.host.id]}</span>
              {:else if run?.running}
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
              {#if r.held}
                <button type="button" class="btn ghost tiny" data-guide="fleet-release-host" title="Release the hold: count and upgrade this host again" onclick={() => r.held && holds.release(r.held.id)}>Release</button>
              {:else}
                <button type="button" class="btn ghost tiny" data-guide="fleet-hold-host" title="Hold this host's updates: skip it in upgrades and silence its count" disabled={!!run?.running} onclick={() => openHold(r.host.id)}>Hold…</button>
              {/if}
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
                  {@const rep = s.report}
                  {@const mine = holds.forHost(r.host.id)}
                  <table class="pkgs">
                    <thead><tr><th>PACKAGE</th><th>INSTALLED</th><th>AVAILABLE</th><th>FROM</th><th></th></tr></thead>
                    <tbody>
                      {#each rep.updates as p (p.name)}
                        {@const place = p.held ? packageHoldPlace(rep, p.name, mine) : null}
                        <tr class:sec={!p.held && /security/i.test(p.repo ?? "")} class:heldrow={p.held}>
                          <td>{p.name}{#if p.held}<span class="chip held" title={place === "nightgrid" ? "Held in Nightgrid: left out of Nightgrid's upgrades" : "Held on the host: every tool leaves it alone"}>HELD</span>{/if}</td>
                          <td class="ver">{p.current ?? "—"}</td>
                          <td class="ver new">{p.available}</td>
                          <td class="muted">{p.repo ?? ""}</td>
                          <td class="acts">
                            {#if rep.holdSupport !== "none"}
                              <button type="button" class="btn ghost tiny" disabled={pkgBusy !== null || !!run?.running}
                                title={p.held ? "Release: upgrades install it again" : rep.holdSupport === "host" ? "Hold it on the host (needs root)" : "Hold it in Nightgrid: left out of Nightgrid's upgrades"}
                                onclick={() => setPkg(r.host.id, p.name, !p.held, p.current)}>{pkgBusy === `${r.host.id}/${p.name}` ? "…" : p.held ? "Release" : "Hold"}</button>
                            {/if}
                          </td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                {:else if isOpen && s?.status === "ok"}
                  <p class="muted small">Nothing to upgrade.</p>
                {/if}
                {#if isOpen && s?.report}
                  {@const rep = s.report}
                  {@const quiet = rep.held.filter((n) => !rep.updates.some((u) => u.name === n))}
                  {#if quiet.length}
                    <p class="muted small heldlist">
                      Also held, nothing new for them right now:
                      {#each quiet as n (n)}
                        <span class="chip held">{n}</span><button type="button" class="btn ghost tiny" disabled={pkgBusy !== null} onclick={() => setPkg(r.host.id, n, false, null)}>Release</button>
                      {/each}
                    </p>
                  {/if}
                  {#if rep.holdSupport === "none" && rep.manager}
                    <p class="muted small">This host updates as one image, so single packages can't be held: hold the whole host instead.</p>
                  {:else if rep.holdSupport === "nightgrid" && rep.manager}
                    <p class="muted small">{rep.manager === "dnf" ? "dnf's versionlock plugin isn't installed" : "pacman holds packages in pacman.conf"}, so holds here only keep Nightgrid's upgrades away from them.</p>
                  {/if}
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
              <button type="button" class="btn ghost tiny" title="Keep this image where it is: Update and Pull leave its services alone"
                onclick={async () => {
                  await holds.holdImage(i.hostId, i.stack.name, i.update.image, i.update.current.version ?? i.update.current.revision ?? null);
                  i.update.held = true;
                }}>Hold</button>
              <button type="button" class="btn ghost tiny" onclick={() => openStack(i.hostId)}>Open stack</button>
            </li>
          {/each}
        </ul>
      {/if}
      {#if heldImages.length}
        <p class="muted small pad">{heldImages.length} held image{heldImages.length === 1 ? "" : "s"} with a newer version not counted ({heldImages.filter((i) => i.update.status === "available").length} newer). See HOLDS below.</p>
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

    <section class="holds" data-guide="fleet-holds">
      <h3 class="label">// HOLDS</h3>
      {#if !holds.list.length}
        <p class="muted small pad">
          Nothing is held. <strong>Hold…</strong> on a host skips it in upgrades and silences its count for a while; <strong>Hold</strong> on a package, image or stack keeps it where it is.
        </p>
      {:else}
        <ul class="imgs">
          {#each holds.list as h (h.id)}
            <li>
              <span class="host">{nameOf(h.hostId)}</span>
              <span class="chip held">{kindWord[h.kind]}</span>
              <code>{holdName(h)}</code>
              {#if h.version}<span class="ver">at {h.version}</span>{/if}
              <span class="muted small">{holdUntilText(h)}{h.note ? ` · ${h.note}` : ""}</span>
              <span class="spacer"></span>
              <button type="button" class="btn ghost tiny" onclick={() => holds.release(h.id)}>Release</button>
            </li>
          {/each}
        </ul>
      {/if}
      <p class="muted small pad">Packages held on a host itself (apt-mark hold, dnf versionlock, hidden Windows updates) show in that host's package list.</p>
    </section>
  </div>
</div>

<Modal open={dialogHost !== null} title={dialogHost ? `HOLD UPDATES · ${dialogHost.name}` : ""} width={480} onclose={() => (holds.dialogHost = null)}>
  <p>
    While held, {dialogHost?.name} isn't counted as pending, isn't picked for <strong>Upgrade</strong>, and the Overview stays quiet about it. Checking still works, and
    nothing changes on the host.
  </p>
  <fieldset class="for">
    <legend class="label">HOLD FOR</legend>
    {#each HOLD_FOR as o (o.label)}
      <label><input type="radio" name="holdfor" checked={holdFor === o.secs} onchange={() => (holdFor = o.secs)} /> {o.label}</label>
    {/each}
  </fieldset>
  <label class="field">
    <span>Note (optional)</span>
    <input class="input" bind:value={holdNote} maxlength="500" placeholder="why: waiting for a fix, game server event, …" />
  </label>
  {#if dialogHost && holdErr[dialogHost.id]}<p class="error-text">{holdErr[dialogHost.id]}</p>{/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (holds.dialogHost = null)}>Cancel</button>
    <button type="button" class="btn primary" onclick={confirmHold}>Hold updates</button>
  {/snippet}
</Modal>

<Modal open={fleet.confirm} title="UPGRADE HOSTS" tone="danger" width={540} onclose={() => (fleet.confirm = false)}>
  <p>Install every pending update on these hosts, three at a time:</p>
  <ul class="targets">
    {#each picked as h (h.id)}
      {@const rep = fleet.updates[h.id]?.report}
      {@const n = rep?.updates.filter((u) => !u.held).length ?? 0}
      {@const kept = (rep?.updates.length ?? 0) - n}
      <li>
        <strong>{h.name}</strong>
        <span class="muted">{rep?.manager ? MANAGER_WORD[rep.manager] : ""} · {n} package{n === 1 ? "" : "s"}{kept ? ` (${kept} held, left out)` : ""}{rep?.manager === "ostree" ? " (staged for the next reboot)" : ""}</span>
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
  .chip.held {
    color: var(--text-1);
    border-style: dashed;
    margin-left: 6px;
  }
  .pkgs tr.heldrow td {
    color: var(--text-1);
  }
  .heldlist {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    margin: 6px 0 0;
  }
  .holds {
    margin-top: 16px;
  }
  .for {
    border: 1px solid var(--line);
    padding: 6px 10px 8px;
    margin: 10px 0;
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
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
