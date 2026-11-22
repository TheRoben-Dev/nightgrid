<script lang="ts">
  /**
   * Pair with a Nightgrid app found on the LAN (D-208): prove it with a code
   * shown there or the profile passphrase, import its profile, and keep
   * syncing both ways. Everything goes straight over the local network.
   */
  import { api, type FoundDevice, type ImportMode, type SecretCategory, type SyncJoined } from "../../api";
  import { scramble } from "../../scramble";
  import { hosts } from "../../stores/hosts.svelte";
  import { formatCode, SECRET_GROUPS, sync } from "../../stores/sync.svelte";
  import Modal from "../Modal.svelte";
  import Toggle from "../Toggle.svelte";

  let { device, onclose }: { device: FoundDevice | null; onclose: () => void } = $props();

  let how = $state<"code" | "passphrase">("code");
  let code = $state("");
  let passphrase = $state("");
  let asked = $state(false);
  let doImport = $state(true);
  let replace = $state(false);
  let twoWay = $state(true);
  let secrets = $state<SecretCategory[]>([]);
  let done = $state<SyncJoined | null>(null);

  const empty = $derived(hosts.hosts.length === 0);
  // A fresh install takes the other profile as it is; one with hosts merges.
  $effect(() => {
    if (device) {
      how = device.passphrase ? "passphrase" : "code";
      code = passphrase = "";
      asked = false;
      doImport = true;
      replace = empty;
      twoWay = true;
      secrets = [];
      done = null;
      sync.error = null;
    }
  });

  const ready = $derived(how === "code" ? formatCode(code) !== null : passphrase.length > 0);
  const mode = $derived<ImportMode | null>(doImport ? (replace ? "replace" : "merge") : null);

  async function ask() {
    if (device && (await sync.askCode(device))) asked = true;
  }

  async function submit() {
    if (!device || !ready) return;
    const r = await sync.joinFound(device, how === "code" ? { code } : { passphrase }, mode, twoWay, secrets);
    passphrase = "";
    if (r) done = r;
  }

  function close() {
    if (sync.busy) return;
    if (done?.imported) {
      void api.appRestart();
      return;
    }
    onclose();
  }

  const n = (x: number, w: string) => `${x} ${w}${x === 1 ? "" : "s"}`;
</script>

<Modal open={!!device} title={done ? "PAIRED" : `IMPORT FROM ${device?.name.toUpperCase() ?? ""}`} width={560} onclose={close}>
  {#if device}
    {#if done}
      <p use:scramble={`Paired with ${done.peer.name}.`}></p>
      {#if done.imported}
        <p class="muted">
          Took its profile: {n(done.imported.added, "item")} added{done.imported.skipped ? `, ${done.imported.skipped} already here` : ""}{done.imported.secrets ? `, ${n(done.imported.secrets, "secret")}` : ""}. Restart to load it.
        </p>
        {#if done.imported.secretsError}<p class="warn small">{done.imported.secretsError}</p>{/if}
      {:else if done.importError}
        <p class="error" role="alert">Paired, but the import didn't work: {done.importError}</p>
      {/if}
      <p class="muted small">
        {done.peer.mode === "twoWay"
          ? `From now on, changes on either device reach the other within seconds while both are open on the same network. The latest edit of an item wins, and the last few versions are kept under Settings → Sync → Snapshots in case a bad change syncs over.`
          : "It's paired in MANUAL. Push, pull or turn on automatic sync in Settings → Sync."}
      </p>
    {:else}
      <form id="sync-join-found" onsubmit={(e) => (e.preventDefault(), void submit())}>
        <p class="muted small">
          <span class="mono">{device.addr}</span> · Nightgrid {device.appVersion} · {n(device.hosts, "host")}
          {#if !device.sameVersion}<br /><span class="warn">Different version from this one: importing works, automatic sync starts once both are on the same version.</span>{/if}
        </p>

        <div class="how" role="radiogroup" aria-label="How to prove it's yours">
          <button type="button" role="radio" aria-checked={how === "code"} class="mode" class:active={how === "code"} onclick={() => (how = "code")}>CODE ON {device.name.toUpperCase()}</button>
          <button
            type="button"
            role="radio"
            aria-checked={how === "passphrase"}
            class="mode"
            class:active={how === "passphrase"}
            disabled={!device.passphrase}
            title={device.passphrase ? "" : `Not set up on ${device.name} (Settings → Sync there)`}
            onclick={() => (how = "passphrase")}>PROFILE PASSPHRASE</button
          >
        </div>

        {#if how === "code"}
          <div class="row">
            <button type="button" class="btn" disabled={sync.busy} onclick={() => void ask()}>{asked ? "Show it again" : `Show a code on ${device.name}`}</button>
            {#if asked}<span class="ok small" role="status">A code is up on {device.name}'s screen. Type it here.</span>{/if}
          </div>
          <label class="field">
            <span>CODE</span>
            <input class="input mono code-in" placeholder="ABCDE-12345" bind:value={code} autocomplete="off" spellcheck="false" />
          </label>
        {:else}
          <label class="field">
            <span>PROFILE PASSPHRASE</span>
            <input class="input" type="password" bind:value={passphrase} autocomplete="off" placeholder="the one set for pairing on {device.name}" />
          </label>
          <p class="muted small">Never sent: both sides prove they know it without it crossing the network. Five wrong tries lock it there for 10 minutes.</p>
        {/if}

        <Toggle
          label={`Import ${device.name}'s profile`}
          hint="Hosts, groups, services, dashboards, notes, alerts and settings. This device keeps its own name, window size and other per-device options."
          checked={doImport}
          onchange={(v) => (doImport = v)}
        />
        {#if doImport}
          <Toggle
            label="Replace what's here"
            hint={empty ? "This device has no hosts yet, so it takes the profile as it is." : "Off: merge, so hosts already on this device stay and the newer edit of the same item wins."}
            checked={replace}
            onchange={(v) => (replace = v)}
          />
        {/if}
        <Toggle label="Keep syncing both ways" hint="Changes on either device go to the other automatically. You can change this later per device." checked={twoWay} onchange={(v) => (twoWay = v)} />
        <details class="more">
          <summary>Saved secrets ({secrets.length ? secrets.length : "none"})</summary>
          {#each SECRET_GROUPS as g (g.id)}
            <Toggle
              label={g.label}
              hint={g.what}
              checked={secrets.includes(g.id)}
              onchange={(v) => (secrets = v ? [...new Set([...secrets, g.id])] : secrets.filter((x) => x !== g.id))}
            />
          {/each}
          <p class="muted small">SSH key files in ~/.ssh don't travel; copy them yourself or use Settings → SSH keys.</p>
        </details>
      </form>
    {/if}
    {#if sync.error}<p class="error" role="alert">{sync.error}</p>{/if}
  {/if}
  {#snippet footer()}
    {#if done}
      <button type="button" class="btn primary" onclick={close}>{done.imported ? "Restart now" : "Done"}</button>
    {:else}
      <button type="button" class="btn ghost" onclick={close} disabled={sync.busy}>Cancel</button>
      <button type="submit" form="sync-join-found" class="btn primary" disabled={sync.busy || !ready}>{sync.busy ? "Pairing…" : doImport ? "Pair and import" : "Pair"}</button>
    {/if}
  {/snippet}
</Modal>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .how {
    display: flex;
    gap: 6px;
  }
  .mode {
    flex: 1;
    padding: 6px 8px;
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    cursor: pointer;
  }
  .mode.active {
    border-color: var(--cyan);
    color: var(--cyan);
  }
  .mode:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .row {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    color: var(--text-1);
  }
  .code-in {
    font-size: 1.2em;
    letter-spacing: 0.15em;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .muted {
    color: var(--text-1);
  }
  .ok {
    color: var(--mint);
  }
  .warn {
    color: var(--warn, var(--amber));
  }
  .error {
    color: var(--danger);
  }
  .more summary {
    cursor: pointer;
    font-size: var(--fs-xs);
    color: var(--text-1);
  }
</style>
