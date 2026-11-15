<script lang="ts">
  /**
   * System → CONFIG (M10l): config drift. Compares `/etc`, compose files in
   * the host's stack folders and the package list with a baseline saved on
   * the host, or with another host. Values that look like secrets are masked
   * in diffs until revealed (rule 6).
   */
  import { redact } from "../../helper/prompt";
  import { CHANGE_WORD, counts, filterFiles, filterPackages, groupFiles, pkgChange, reportText } from "../../confdiff";
  import { confdiff } from "../../stores/confdiff.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { system } from "../../stores/system.svelte";
  import ExplainButton from "../helper/ExplainButton.svelte";
  import Modal from "../Modal.svelte";
  import Toggle from "../Toggle.svelte";

  $effect(() => {
    if (system.hostId !== null) void confdiff.select(system.hostId);
  });

  let query = $state("");
  let reveal = $state(false);
  const r = $derived(confdiff.report);
  const c = $derived(r ? counts(r) : null);
  const files = $derived(r ? groupFiles(filterFiles(r.files, query)) : []);
  const pkgs = $derived(r ? filterPackages(r.packages, query) : []);
  const others = $derived(hosts.hosts.filter((h) => h.id !== system.hostId));
  const name = $derived(system.host?.name ?? "this host");
  const when = (t: number) => new Date(t * 1000).toLocaleString();
  const canCompare = $derived(confdiff.other !== null || !!confdiff.baseline);

  $effect(() => {
    if (!confdiff.openPath) reveal = false;
  });
</script>

<div class="config">
  <div class="bar">
    <label class="pick">
      <span class="label">COMPARE WITH</span>
      <select
        class="input"
        aria-label="Compare with"
        value={confdiff.other ?? ""}
        onchange={(e) => {
          confdiff.other = e.currentTarget.value ? Number(e.currentTarget.value) : null;
          confdiff.report = null;
          confdiff.closeDiff();
        }}
      >
        <option value="">its saved baseline</option>
        {#each others as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
      </select>
    </label>
    {#if confdiff.other !== null}
      <Toggle label="Hide host-specific files" checked={confdiff.hideHostSpecific} onchange={(v) => (confdiff.hideHostSpecific = v)} />
    {/if}
    <span class="spacer"></span>
    <button type="button" class="btn ghost tiny" data-guide="config-baseline" disabled={confdiff.saving || !confdiff.baselineLoaded} onclick={() => (confdiff.confirmSave = true)}
      >{confdiff.saving ? "Saving…" : confdiff.baseline ? "Save new baseline…" : "Save baseline"}</button
    >
    <button type="button" class="btn tiny" data-guide="config-compare" disabled={confdiff.loading || !canCompare} onclick={() => confdiff.compare()}
      >{confdiff.loading ? "Comparing…" : "Compare"}</button
    >
  </div>

  <p class="muted small info">
    {#if !confdiff.baselineLoaded}
      Looking for a saved baseline…
    {:else if confdiff.baseline}
      Baseline saved {when(confdiff.baseline.taken)} on {name}: {confdiff.baseline.files} files, {confdiff.baseline.packages} packages.
    {:else}
      No baseline on {name} yet. <strong>Save baseline</strong> records <code>/etc</code>, compose files in its stack folders and the package list, on the host itself.
    {/if}
  </p>

  {#if confdiff.error}<p class="error-text">{confdiff.error}</p>{/if}

  <div class="scroll" data-guide="system-config">
    {#if r && c}
      <div class="summary">
        <span><span class="label">{r.left.label.toUpperCase()}</span> → <span class="label">{r.right.label.toUpperCase()}</span></span>
        <span class="chg">{c.changed} CHANGED</span>
        <span class="add">{c.added} ADDED</span>
        <span class="del">{c.removed} REMOVED</span>
        <span class="pkg">{c.packages} PACKAGES</span>
        {#if r.hidden}<span class="muted small">{r.hidden} host-specific hidden</span>{/if}
        <span class="spacer"></span>
        <input class="input filter" type="search" placeholder="Filter" bind:value={query} aria-label="Filter changes" />
        {#if r.files.length || r.packages.length}
          <ExplainButton
            source="config drift: {r.left.label} → {r.right.label}"
            host={system.host?.name ?? null}
            text={() => reportText(r)}
            question="Summarize these configuration and package changes. Which look risky or unexpected (security, SSH, firewall, removed packages), and what should I check?"
          />
        {/if}
      </div>
      {#if r.mixedManagers}<p class="muted small note">The two hosts use different package managers ({r.left.manager}, {r.right.manager}), so most packages show as added or removed.</p>{/if}

      {#if !r.files.length && !r.packages.length}
        <p class="ok pad">No differences{r.hidden ? " apart from host-specific files" : ""}.</p>
      {/if}

      {#if r.files.length}
        <section>
          <h3 class="label">// FILES</h3>
          {#if !files.length}<p class="muted pad">Nothing matches.</p>{/if}
          {#each files as g (g.dir)}
            <p class="dir muted small">{g.dir}/</p>
            <ul class="list">
              {#each g.files as f (f.path)}
                <li>
                  <span class="tag {f.change}">{CHANGE_WORD[f.change]}</span>
                  {#if f.change === "changed" && !f.secret}
                    <button type="button" class="link" aria-expanded={confdiff.openPath === f.path} onclick={() => confdiff.openFile(f.path)}>{f.path.slice(g.dir.length + 1)}</button>
                  {:else}
                    <span>{f.path.slice(g.dir.length + 1)}</span>
                  {/if}
                  {#if f.secret}<span class="muted small">secret file: compared by checksum only</span>{/if}
                </li>
                {#if confdiff.openPath === f.path}
                  <li class="diffwrap">
                    {#if confdiff.diffLoading}
                      <p class="muted small">Reading both versions…</p>
                    {:else if confdiff.diffError}
                      <p class="error-text">{confdiff.diffError}</p>
                    {:else if confdiff.diff}
                      {@const d = confdiff.diff}
                      <div class="diffbar">
                        <span class="add">+{d.added}</span> <span class="del">−{d.removed}</span>
                        {#if d.truncated}<span class="muted small">only the first 512 KB compared</span>{/if}
                        <span class="spacer"></span>
                        <button type="button" class="btn ghost tiny" onclick={() => (reveal = !reveal)}>{reveal ? "Mask secrets" : "Reveal secrets"}</button>
                      </div>
                      {#if d.binary}
                        <p class="muted small">Binary file: no line diff.</p>
                      {:else if d.tooLarge}
                        <p class="muted small">Too many differences for a line diff.</p>
                      {:else}
                        <pre class="diff">{#each d.lines as l, i (i)}{#if l.tag === "skip"}<span class="skip">⋯ {l.text} unchanged lines</span>
{:else}<span class={l.tag}><span class="no">{l.old ?? ""}</span><span class="no">{l.new ?? ""}</span><span class="mk">{l.tag === "add" ? "+" : l.tag === "del" ? "−" : " "}</span>{reveal ? l.text : redact(l.text)}</span>
{/if}{/each}</pre>
                      {/if}
                    {/if}
                  </li>
                {/if}
              {/each}
            </ul>
          {/each}
        </section>
      {/if}

      {#if r.packages.length}
        <section>
          <h3 class="label">// PACKAGES</h3>
          {#if !pkgs.length}<p class="muted pad">Nothing matches.</p>{/if}
          <ul class="list">
            {#each pkgs as p (p.name)}
              {@const how = pkgChange(p)}
              <li>
                <span class="tag {how === 'added' ? 'added' : how === 'removed' ? 'removed' : 'changed'}">{how.toUpperCase()}</span>
                <code>{p.name}</code>
                <span class="muted small">{p.left ?? "—"} → {p.right ?? "—"}</span>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    {:else if !confdiff.loading}
      <p class="muted pad">
        Pick what to compare with and click <strong>Compare</strong>. Only files the login can read are included; nothing on the host changes except the baseline folder
        (<code>~/.nightgrid/baselines</code>).
      </p>
    {/if}
  </div>
</div>

<Modal open={confdiff.confirmSave} title={confdiff.baseline ? "REPLACE BASELINE" : "SAVE BASELINE"} width={520} onclose={() => (confdiff.confirmSave = false)}>
  <p>
    Save the current <code>/etc</code>, compose files and package list of <strong>{name}</strong> as its baseline{confdiff.baseline
      ? `, replacing the one from ${when(confdiff.baseline.taken)}`
      : ""}?
  </p>
  <p class="muted">
    It's stored on {name} in <code>~/.nightgrid/baselines/default</code>, readable only by this login, so file contents never leave the host. <code>.env</code>
    files are recorded by checksum only.
  </p>
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (confdiff.confirmSave = false)}>Cancel</button>
    <button type="button" class="btn primary" onclick={() => confdiff.saveBaseline()}>{confdiff.baseline ? "Replace baseline" : "Save baseline"}</button>
  {/snippet}
</Modal>

<style>
  .config {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-height: 0;
  }
  .bar,
  .summary,
  .diffbar {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
    font-size: var(--fs-xs);
  }
  .pick {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .spacer {
    flex: 1;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .info {
    margin: 0;
  }
  .scroll {
    flex: 1;
    overflow: auto;
    min-height: 0;
  }
  .summary {
    letter-spacing: 0.08em;
    padding: 4px 0 8px;
  }
  .filter {
    width: 180px;
  }
  .chg,
  .tag.changed {
    color: var(--warn);
  }
  .add,
  .tag.added {
    color: var(--mint);
  }
  .del,
  .tag.removed {
    color: var(--danger);
  }
  .pkg {
    color: var(--cyan);
  }
  .ok {
    color: var(--mint);
  }
  .pad {
    margin: 6px 10px;
  }
  .note {
    border-left: 2px solid var(--warn);
    padding-left: 8px;
  }
  section {
    margin-bottom: 12px;
  }
  .dir {
    margin: 8px 0 2px 10px;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0 10px;
  }
  .list li {
    display: flex;
    gap: 10px;
    align-items: baseline;
    font-size: var(--fs-sm);
    padding: 1px 0;
  }
  .tag {
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    width: 84px;
    flex: none;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--accent);
    cursor: pointer;
    font: inherit;
    text-decoration: underline dotted;
  }
  .list li.diffwrap {
    display: block;
    padding: 6px 0 10px 94px;
  }
  .diff {
    margin: 4px 0 0;
    padding: 6px 0;
    border: 1px solid var(--line);
    font-size: var(--fs-xs);
    line-height: 1.5;
    overflow-x: auto;
    max-height: 420px;
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
    color: var(--text-1);
    padding-left: 8px;
  }
  .no {
    display: inline-block;
    width: 4ch;
    text-align: right;
    color: var(--text-1);
    margin-right: 6px;
  }
  .mk {
    display: inline-block;
    width: 2ch;
  }
</style>
