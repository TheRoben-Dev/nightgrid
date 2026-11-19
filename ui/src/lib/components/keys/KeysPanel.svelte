<script lang="ts">
  /** Settings → SSH KEYS (M10k): your keys, which hosts accept them, new keys and rotation. */
  import { onMount } from "svelte";
  import type { AuthorizedKey, Host, SshKey } from "../../api";
  import { writeClipboard } from "../../clipboard";
  import { hosts } from "../../stores/hosts.svelte";
  import { sshKeys } from "../../stores/sshkeys.svelte";
  import { accepted, freeKeyName, hostsAccepting, keyForHost, keyLabel, shortFp, STEP_LABEL, unknownKeys, validKeyName } from "../../sshkeys";
  import Modal from "../Modal.svelte";
  import Panel from "../Panel.svelte";
  import Toggle from "../Toggle.svelte";

  onMount(() => void sshKeys.load());

  const keys = $derived(sshKeys.keys);
  const usedBy = (k: SshKey): Host[] => hosts.hosts.filter((h) => keyForHost(h, keys, sshKeys.home)?.fingerprint === k.fingerprint);
  const checked = $derived(Object.keys(sshKeys.results).length > 0);

  let copied = $state<string | null>(null);
  async function copy(k: SshKey) {
    await writeClipboard(k.public);
    copied = k.fingerprint;
    setTimeout(() => (copied = null), 1200);
  }

  // ---- new key ----
  let showNew = $state(false);
  let name = $state("");
  let comment = $state("");
  let pass = $state("");
  let pass2 = $state("");
  function openNew() {
    showNew = !showNew;
    if (showNew) {
      name = freeKeyName(keys);
      comment = `nightgrid-${new Date().toISOString().slice(0, 10)}`;
      pass = pass2 = "";
    }
  }
  const newProblem = $derived(!validKeyName(name.trim()) ? "Use letters, digits, '.', '_' or '-'." : pass !== pass2 ? "The passphrases don't match." : null);
  async function create() {
    const k = await sshKeys.generate(name.trim(), comment.trim(), pass || null);
    pass = pass2 = "";
    if (k) {
      showNew = false;
      rotTo = k.fingerprint;
    }
  }

  // ---- single add / remove ----
  let confirmRemove = $state<{ host: Host; key: Pick<AuthorizedKey, "fingerprint" | "comment">; label: string; inUse: boolean } | null>(null);
  let typed = $state("");
  function askRemove(host: Host, fingerprint: string, comment: string) {
    const k = keys.find((x) => x.fingerprint === fingerprint);
    confirmRemove = {
      host,
      key: { fingerprint, comment },
      label: k ? keyLabel(k) : comment || shortFp(fingerprint),
      inUse: !!k && keyForHost(host, keys, sshKeys.home)?.fingerprint === fingerprint,
    };
    typed = "";
  }
  async function doRemove() {
    if (!confirmRemove) return;
    const { host, key } = confirmRemove;
    confirmRemove = null;
    await sshKeys.remove(host.id, key.fingerprint);
  }
  let openUnknown = $state<number | null>(null);

  // ---- rotate ----
  let rotFrom = $state("");
  let rotTo = $state("");
  let rotHosts = $state<number[]>([]);
  let switchHosts = $state(true);
  let removeOld = $state(true);
  let rotPass = $state("");
  let confirmRotate = $state(false);
  let rotTyped = $state("");
  const fromKey = $derived(keys.find((k) => k.fingerprint === rotFrom));
  const toKey = $derived(keys.find((k) => k.fingerprint === rotTo));
  const targets = $derived(keys.filter((k) => k.file && k.fingerprint !== rotFrom));
  function pickFrom(fp: string) {
    rotFrom = fp;
    const results = Object.values(sshKeys.results);
    const viaFile = hosts.hosts.filter((h) => keyForHost(h, keys, sshKeys.home)?.fingerprint === fp).map((h) => h.id);
    rotHosts = [...new Set([...hostsAccepting(results, fp), ...viaFile])];
    if (rotTo === fp) rotTo = "";
  }
  function toggleHost(id: number, on: boolean) {
    rotHosts = on ? [...rotHosts, id] : rotHosts.filter((x) => x !== id);
  }
  const rotProblem = $derived(
    !fromKey ? "Pick the key to replace." : !toKey ? "Pick the new key (or make one above)." : !rotHosts.length ? "Pick at least one host." : null,
  );
  const rotNames = $derived(rotHosts.map((id) => hosts.byId(id)?.name ?? `#${id}`));
  async function doRotate() {
    if (!fromKey || !toKey) return;
    confirmRotate = false;
    const pw = rotPass || null;
    rotPass = "";
    await sshKeys.rotate(fromKey, toKey, rotHosts, { switchHosts, removeOld, passphrase: pw });
  }
  const STATE_MARK: Record<string, string> = { pending: "○", running: "◐", done: "✓", failed: "✕", skipped: "–" };
</script>

<Panel title="SSH KEYS" guide="settings-keys" prefix=">" count={keys.length}>
  <p class="muted intro">
    Your key pairs in <span class="mono">{sshKeys.dir || "~/.ssh"}</span> and the SSH agent. Check which hosts accept each one, add or remove a key on a
    host, make a new ed25519 key, or rotate a key across hosts. Private keys never leave this computer, and Nightgrid doesn't edit your SSH config.
  </p>
  {#if sshKeys.agentError}<p class="muted small">No SSH agent: {sshKeys.agentError}</p>{/if}

  <ul class="keys" data-guide="keys-list">
    {#each keys as k (k.fingerprint)}
      {@const users = usedBy(k)}
      <li>
        <span class="kn">{keyLabel(k)}</span>
        <span class="muted">{k.algorithm.replace(/^ssh-/, "")}</span>
        <span class="mono fp" title={k.fingerprint}>{shortFp(k.fingerprint)}</span>
        {#if k.inAgent}<span class="badge" title="The SSH agent holds this key">AGENT</span>{/if}
        {#if k.encrypted}<span class="badge" title="The key file is passphrase protected">PASSPHRASE</span>{/if}
        {#if !k.file}<span class="badge dim" title="Only in the agent; no file on disk">AGENT ONLY</span>{/if}
        {#if users.length}<span class="muted small used" title="Hosts set to log in with this key file">used by {users.map((h) => h.name).join(", ")}</span>{/if}
        <span class="spacer"></span>
        <button type="button" class="link" onclick={() => copy(k)}>{copied === k.fingerprint ? "Copied" : "Copy public key"}</button>
      </li>
    {:else}
      <li class="muted">No keys found yet. Make one below.</li>
    {/each}
  </ul>

  <div class="row">
    <button type="button" class="btn" data-guide="keys-new" aria-expanded={showNew} onclick={openNew}>{showNew ? "CANCEL" : "NEW KEY…"}</button>
    <button type="button" class="btn" data-guide="keys-check" disabled={sshKeys.checking || !hosts.hosts.length} onclick={() => sshKeys.checkAll()}
      >{sshKeys.checking ? "CHECKING…" : checked ? "CHECK HOSTS AGAIN" : "CHECK HOSTS"}</button
    >
  </div>

  {#if showNew}
    <form class="form" onsubmit={(e) => { e.preventDefault(); void create(); }}>
      <label><span class="lbl">FILE NAME</span><input class="input mono" bind:value={name} maxlength="64" spellcheck="false" autocomplete="off" /></label>
      <label><span class="lbl">COMMENT</span><input class="input mono" bind:value={comment} maxlength="120" spellcheck="false" autocomplete="off" /></label>
      <label><span class="lbl">PASSPHRASE (OPTIONAL)</span><input class="input" type="password" bind:value={pass} autocomplete="new-password" /></label>
      <label><span class="lbl">AGAIN</span><input class="input" type="password" bind:value={pass2} autocomplete="new-password" /></label>
      <p class="muted small">
        Saved as <span class="mono">{sshKeys.dir}/{name.trim() || "…"}</span> (+ <span class="mono">.pub</span>). The passphrase only encrypts the file
        and isn't stored; Nightgrid asks for it when it connects, and can remember it in the OS keychain then.
      </p>
      {#if newProblem && name}<p class="err">{newProblem}</p>{/if}
      <div><button type="submit" class="btn primary" disabled={!!newProblem || sshKeys.busy}>{sshKeys.busy ? "MAKING…" : "MAKE ED25519 KEY"}</button></div>
    </form>
  {/if}

  {#if sshKeys.error}<p class="err">{sshKeys.error}</p>{/if}

  {#if checked}
    <h3 class="sub">WHERE THEY WORK</h3>
    <div class="matrix-wrap" data-guide="keys-matrix">
      <table class="matrix">
        <thead>
          <tr>
            <th>HOST</th>
            {#each keys as k (k.fingerprint)}<th title={k.fingerprint}>{keyLabel(k)}</th>{/each}
            <th title="Keys on the host that aren't yours">OTHER</th>
          </tr>
        </thead>
        <tbody>
          {#each hosts.hosts as h (h.id)}
            {@const r = sshKeys.results[h.id]}
            {@const ok = accepted(r)}
            {@const other = unknownKeys(r, keys)}
            {@const busy = sshKeys.working.includes(h.id)}
            {@const uses = keyForHost(h, keys, sshKeys.home)}
            <tr>
              <td class="hn">{h.name}</td>
              {#if !r}
                <td colspan={keys.length + 1} class="muted">{sshKeys.checking ? "…" : "not checked"}</td>
              {:else if r.error}
                <td colspan={keys.length + 1} class="err small" title={r.error}>couldn't read: {r.error}</td>
              {:else}
                {#each keys as k (k.fingerprint)}
                  {@const has = ok.has(k.fingerprint)}
                  <td>
                    <button
                      type="button"
                      class="cell"
                      class:on={has}
                      class:uses={uses?.fingerprint === k.fingerprint}
                      disabled={busy}
                      title={has ? `${h.name} accepts ${keyLabel(k)}. Click to remove it.` : `Add ${keyLabel(k)} to ${h.name}`}
                      onclick={() => (has ? askRemove(h, k.fingerprint, k.comment) : sshKeys.push(h.id, k.fingerprint))}>{busy ? "…" : has ? "●" : "+"}</button
                    >
                  </td>
                {/each}
                <td>
                  {#if other.length}
                    <button type="button" class="link" onclick={() => (openUnknown = openUnknown === h.id ? null : h.id)}>{other.length}</button>
                  {:else}<span class="muted">0</span>{/if}
                </td>
              {/if}
            </tr>
            {#if openUnknown === h.id}
              <tr class="unknown">
                <td></td>
                <td colspan={keys.length + 1}>
                  <ul>
                    {#each other as e (e.fingerprint)}
                      <li>
                        <span class="mono" title={e.fingerprint}>{shortFp(e.fingerprint)}</span>
                        <span>{e.comment || "(no comment)"}</span>
                        <span class="muted">{e.algorithm.replace(/^ssh-/, "")} · line {e.line}{e.options ? ` · ${e.options}` : ""}</span>
                        <button type="button" class="link danger-text" disabled={busy} onclick={() => askRemove(h, e.fingerprint, e.comment)}>Remove…</button>
                      </li>
                    {/each}
                  </ul>
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
    </div>
    <p class="muted small">● accepted · + add the key · a ringed ● is the key Nightgrid is set to use for that host.</p>
  {/if}

  <h3 class="sub">ROTATE A KEY</h3>
  <div class="form rotate" data-guide="keys-rotate">
    <label
      ><span class="lbl">REPLACE</span>
      <select class="input" value={rotFrom} onchange={(e) => pickFrom(e.currentTarget.value)}>
        <option value="">Pick a key…</option>
        {#each keys as k (k.fingerprint)}<option value={k.fingerprint}>{keyLabel(k)} · {shortFp(k.fingerprint)}</option>{/each}
      </select>
    </label>
    <label
      ><span class="lbl">WITH</span>
      <select class="input" bind:value={rotTo}>
        <option value="">Pick a key…</option>
        {#each targets as k (k.fingerprint)}<option value={k.fingerprint}>{keyLabel(k)} · {shortFp(k.fingerprint)}</option>{/each}
      </select>
    </label>
    {#if fromKey}
      <fieldset class="hosts">
        <legend class="lbl">ON</legend>
        {#if !checked}<p class="muted small">Check hosts first to pick the ones that accept {keyLabel(fromKey)}.</p>{/if}
        {#each hosts.hosts as h (h.id)}
          <label class="hostpick"><input type="checkbox" checked={rotHosts.includes(h.id)} onchange={(e) => toggleHost(h.id, e.currentTarget.checked)} /> {h.name}</label>
        {/each}
      </fieldset>
      <div class="toggles">
        <Toggle label="Use the new key in Nightgrid" hint="Point these hosts' saved settings at the new key file." checked={switchHosts} onchange={(v) => (switchHosts = v)} />
        <Toggle
          label="Remove the old key from the hosts"
          hint="Only after logging in with the new key worked on that host. Off: the old key stays, so you can remove it later."
          checked={removeOld}
          onchange={(v) => (removeOld = v)}
        />
      </div>
      {#if toKey?.encrypted}
        <label
          ><span class="lbl">NEW KEY'S PASSPHRASE</span><input class="input" type="password" bind:value={rotPass} autocomplete="off" />
          <span class="muted small">Used once to test the login; not stored.</span></label
        >
      {/if}
    {/if}
    {#if rotProblem && rotFrom}<p class="muted small">{rotProblem}</p>{/if}
    <div>
      <button type="button" class={removeOld ? "btn danger" : "btn"} disabled={!!rotProblem || sshKeys.rotation?.some((r) => r.steps.some((s) => s.state === "running"))} onclick={() => ((confirmRotate = true), (rotTyped = ""))}
        >ROTATE ON {rotHosts.length} HOST{rotHosts.length === 1 ? "" : "S"}…</button
      >
    </div>
  </div>

  {#if sshKeys.rotation}
    <ul class="progress">
      {#each sshKeys.rotation as r (r.hostId)}
        <li>
          <span class="hn">{r.name}</span>
          {#each r.steps as s (s.step)}<span class="step {s.state}">{STATE_MARK[s.state]} {STEP_LABEL[s.step]}</span>{/each}
          {#if r.error}<span class="err small">{r.error}</span>{/if}
        </li>
      {/each}
    </ul>
  {/if}
</Panel>

<Modal open={confirmRemove !== null} title="REMOVE KEY" tone="danger" width={500} onclose={() => (confirmRemove = null)}>
  {#if confirmRemove}
    <p>Remove <strong>{confirmRemove.label}</strong> from <span class="mono">~/.ssh/authorized_keys</span> on <strong>{confirmRemove.host.name}</strong>?</p>
    <p class="muted mono small">{confirmRemove.key.fingerprint}</p>
    {#if confirmRemove.inUse}
      <p class="danger-text">Nightgrid is set to log in to {confirmRemove.host.name} with this key. Unless another key or the agent works, you'll be locked out.</p>
    {/if}
    <label class="field">
      <span>Type <strong>{confirmRemove.host.name}</strong> to confirm</span>
      <input class="input" bind:value={typed} autocomplete="off" spellcheck="false" />
    </label>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (confirmRemove = null)}>Cancel</button>
    <button type="button" class="btn danger" disabled={typed.trim() !== confirmRemove?.host.name} onclick={doRemove}>Remove from {confirmRemove?.host.name}</button>
  {/snippet}
</Modal>

<Modal open={confirmRotate} title="ROTATE KEY" tone={removeOld ? "danger" : "default"} width={520} onclose={() => (confirmRotate = false)}>
  {#if fromKey && toKey}
    <p>Replace <strong>{keyLabel(fromKey)}</strong> with <strong>{keyLabel(toKey)}</strong> on:</p>
    <ul class="targets">
      {#each rotNames as n (n)}<li>{n}</li>{/each}
    </ul>
    <p class="muted">
      One host at a time: add {keyLabel(toKey)}, log in with only it{switchHosts ? ", point Nightgrid at it" : ""}{removeOld ? `, then remove ${keyLabel(fromKey)}` : ""}. A host where a step fails keeps its old key.
    </p>
    {#if removeOld}
      <label class="field">
        <span>Type <strong>{keyLabel(fromKey)}</strong> to confirm</span>
        <input class="input" bind:value={rotTyped} autocomplete="off" spellcheck="false" />
      </label>
    {/if}
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (confirmRotate = false)}>Cancel</button>
    <button type="button" class={removeOld ? "btn danger" : "btn primary"} disabled={removeOld && (!fromKey || rotTyped.trim() !== keyLabel(fromKey))} onclick={doRotate}
      >Rotate on {rotNames.length} host{rotNames.length === 1 ? "" : "s"}</button
    >
  {/snippet}
</Modal>

<style>
  .intro,
  .small {
    font-size: var(--fs-xs);
    margin: 0 0 8px;
  }
  .keys,
  .progress,
  .unknown ul,
  .targets {
    list-style: none;
    margin: 0 0 8px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .keys li,
  .progress li,
  .unknown li {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
    flex-wrap: wrap;
  }
  .kn,
  .hn {
    min-width: 120px;
  }
  .fp {
    color: var(--cyan);
    font-size: var(--fs-xs);
  }
  .used {
    margin: 0;
  }
  .badge {
    font-family: var(--font-display);
    font-size: 10px;
    letter-spacing: var(--tracking-label);
    border: 1px solid var(--mint);
    color: var(--mint);
    padding: 0 var(--tag-px);
    display: inline-block;
    line-height: calc(var(--tag-h) - 2px);
    white-space: nowrap;
  }
  .badge.dim {
    border-color: var(--text-1);
    color: var(--text-1);
  }
  .spacer {
    flex: 1;
  }
  .row {
    display: flex;
    gap: 6px;
    margin: 6px 0;
    flex-wrap: wrap;
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 6px 0;
    max-width: 520px;
  }
  .form label {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .lbl {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
  }
  .sub {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    margin: 14px 0 4px;
  }
  .matrix-wrap {
    overflow-x: auto;
  }
  .matrix {
    border-collapse: collapse;
    font-size: var(--fs-sm);
  }
  .matrix th {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    font-weight: normal;
    text-align: center;
    padding: 2px 8px;
    white-space: nowrap;
  }
  .matrix th:first-child,
  .matrix td:first-child {
    text-align: left;
  }
  .matrix td {
    padding: 2px 8px;
    text-align: center;
    border-top: 1px solid var(--line, rgba(255, 255, 255, 0.06));
  }
  .cell {
    border: 1px solid transparent;
    background: none;
    color: var(--text-1);
    cursor: pointer;
    width: 22px;
    height: 20px;
    padding: 0;
    font-size: inherit;
  }
  .cell.on {
    color: var(--ok);
  }
  .cell.uses {
    border-color: var(--ok);
  }
  .cell:hover:not(:disabled) {
    border-color: var(--accent);
  }
  .hosts {
    border: 0;
    padding: 0;
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
  }
  .hosts legend {
    width: 100%;
    margin-bottom: 2px;
  }
  .form .hostpick {
    flex-direction: row;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-sm);
  }
  .step {
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
  .step.done {
    color: var(--ok);
  }
  .step.running {
    color: var(--warn);
  }
  .step.failed {
    color: var(--danger);
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent);
    cursor: pointer;
    font-size: inherit;
  }
  .danger-text {
    color: var(--danger);
  }
  .err {
    color: var(--danger);
    font-size: var(--fs-sm);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
</style>
