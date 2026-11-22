<script lang="ts">
  /**
   * Settings → Sync (D-205): pair Nightgrid apps on your devices and keep
   * their profiles in step, by hand (push / pull) or automatically.
   */
  import { onMount } from "svelte";
  import { api, inTauri, type BackupImported, type ImportMode, type SyncPeer } from "../../api";
  import { writeClipboard } from "../../clipboard";
  import { scramble } from "../../scramble";
  import { tailnet } from "../../stores/tailnet.svelte";
  import { formatCode, MODES, peerState, SECRET_GROUPS, sync } from "../../stores/sync.svelte";
  import Modal from "../Modal.svelte";
  import Panel from "../Panel.svelte";
  import Toggle from "../Toggle.svelte";
  import FoundDevices from "./FoundDevices.svelte";
  import SnapshotsPanel from "./SnapshotsPanel.svelte";

  onMount(() => {
    sync.start();
    tailnet.start();
    const t = setInterval(() => {
      nowSecs = Date.now() / 1000;
      if (sync.info?.pairing) void sync.refresh();
    }, 1000);
    return () => clearInterval(t);
  });

  let nowSecs = $state(Date.now() / 1000);
  const info = $derived(sync.info);
  const peers = $derived(info?.peers ?? []);

  // This device
  let name = $state("");
  let port = $state(47920);
  let editing = $state(false);
  $effect(() => {
    if (!editing && info) {
      name = info.settings.deviceName;
      port = info.settings.port;
    }
  });
  async function saveDevice() {
    if (!info) return;
    if (await sync.configure({ port: Number(port), deviceName: name })) editing = false;
  }

  // Pairing
  let pairAccept = $state(true);
  let joinOpen = $state(false);
  let joinAddr = $state("");
  let joinCode = $state("");
  let joinAccept = $state(true);
  let joined = $state<SyncPeer | null>(null);
  const codeOk = $derived(formatCode(joinCode) !== null);
  const tailnetPeers = $derived(tailnet.status?.state === "running" ? tailnet.status.peers.filter((p) => p.online) : []);
  async function join() {
    const p = await sync.join(joinAddr, joinCode, joinAccept);
    if (p) {
      joined = p;
      joinCode = "";
    }
  }
  function closeJoin() {
    joinOpen = false;
    joined = null;
    joinCode = "";
  }

  let copied = $state<string | null>(null);
  async function copy(t: string) {
    await writeClipboard(t);
    copied = t;
    setTimeout(() => (copied = null), 1200);
  }

  // Push / pull
  type Transfer = { peer: SyncPeer; dir: "push" | "pull" };
  let transfer = $state<Transfer | null>(null);
  let tmode = $state<ImportMode>("merge");
  let typed = $state("");
  let tbusy = $state(false);
  let tdone = $state<string | null>(null);
  let pulled = $state<BackupImported | null>(null);
  let terr = $state<string | null>(null);
  const tok = $derived(tmode === "merge" || typed.trim().toLowerCase() === "replace");
  function startTransfer(peer: SyncPeer, dir: "push" | "pull") {
    transfer = { peer, dir };
    tmode = "merge";
    typed = "";
    tdone = terr = null;
    pulled = null;
  }
  async function runTransfer() {
    if (!transfer || !tok) return;
    tbusy = true;
    terr = null;
    try {
      if (transfer.dir === "push") {
        const r = await api.syncPush(transfer.peer.id, tmode);
        tdone = `${transfer.peer.name} took it: ${r.added} added${r.skipped ? `, ${r.skipped} already there` : ""}${r.secrets ? `, ${r.secrets} secret${r.secrets === 1 ? "" : "s"}` : ""}. It shows a restart button to load it.`;
      } else {
        pulled = await api.syncPull(transfer.peer.id, tmode);
        tdone = `Took ${transfer.peer.name}'s profile: ${pulled.added} added${pulled.skipped ? `, ${pulled.skipped} already here` : ""}${pulled.secrets ? `, ${pulled.secrets} secret${pulled.secrets === 1 ? "" : "s"}` : ""}.`;
      }
      await sync.refresh();
    } catch (e) {
      terr = e instanceof Error ? e.message : typeof e === "object" && e && "message" in e ? String(e.message) : String(e);
    } finally {
      tbusy = false;
    }
  }
  function closeTransfer() {
    if (tbusy) return;
    if (pulled) {
      void api.appRestart();
      return;
    }
    transfer = null;
  }

  // Unpair
  let unpairing = $state<SyncPeer | null>(null);

  let ran = $state<Record<string, string>>({});
  async function syncNow(p: SyncPeer) {
    const r = await sync.syncNow(p.id);
    if (r) {
      const n = r.merged.applied + r.merged.deleted;
      ran = { ...ran, [p.id]: n ? `${n} change${n === 1 ? "" : "s"} from ${p.name}` : "Already in step" };
    }
  }

  function toggleSecret(p: SyncPeer, id: (typeof SECRET_GROUPS)[number]["id"], on: boolean) {
    const s = on ? [...new Set([...p.secrets, id])] : p.secrets.filter((x) => x !== id);
    void sync.setPeer(p, { secrets: s });
  }

  let renaming = $state<string | null>(null);
  let rename = $state("");

  // Pairing by profile passphrase (D-208)
  let passOpen = $state(false);
  let pass1 = $state("");
  let pass2 = $state("");
  const passOk = $derived(pass1.length >= 8 && pass1 === pass2);
  async function savePass() {
    if (!passOk) return;
    if (await sync.setPassphrase(pass1)) closePass();
  }
  function closePass() {
    passOpen = false;
    pass1 = pass2 = "";
  }
</script>

{#if info?.receivedFrom}
  <div class="banner wide" role="status">
    <span use:scramble={`${info.receivedFrom} pushed its profile here. Restart to load it.`}></span>
    <button type="button" class="btn primary" onclick={() => void api.appRestart()}>Restart now</button>
  </div>
{/if}

<Panel title="SYNC · THIS DEVICE" guide="settings-sync">
  <p class="muted lead">
    Keep Nightgrid the same on your computers and handhelds. Devices pair once with a one-time code, then talk directly over your LAN or tailnet (the
    built-in Tailscale works too), encrypted end to end. No server or account in between.
  </p>
  <Toggle
    label="Let paired devices reach this one"
    hint={`Listens on port ${info?.settings.port ?? 47920}. Needed to pair from another device and for its changes to arrive here right away. Only paired devices get in.`}
    checked={info?.settings.enabled ?? false}
    disabled={sync.busy || !info}
    onchange={(v) => info && void sync.configure(v ? { enabled: true } : { enabled: false, discoverable: false })}
  />
  <Toggle
    label="Let my other devices find me on this network"
    hint="Answers when Nightgrid on another computer on the same LAN looks for this one, so it can import this profile and pair. Works without internet or a tailnet. Shows this device's name and host count to the LAN."
    checked={info?.settings.discoverable ?? false}
    disabled={sync.busy || !info}
    onchange={(v) => info && void sync.configure({ discoverable: v, enabled: v || info.settings.enabled })}
  />
  {#if info?.settings.discoverable}
    <div class="block">
      <span class="label sub">PAIR WITH YOUR PROFILE PASSPHRASE</span>
      {#if info.passphraseSet && info.settings.passphrasePairing}
        <p class="muted small">On. A device that finds this one can pair by typing the passphrase instead of a code. It's never sent or stored, only a key made from it.</p>
        <div class="actions">
          <button type="button" class="btn ghost" disabled={sync.busy} onclick={() => (passOpen = true)}>Change passphrase</button>
          <button type="button" class="btn ghost" disabled={sync.busy} onclick={() => void sync.setPassphrase(null)}>Turn off</button>
        </div>
      {:else}
        <p class="muted small">Optional: pair new devices with the passphrase you use for profile backups, instead of reading a code off this screen.</p>
        <div class="actions"><button type="button" class="btn" disabled={sync.busy} onclick={() => (passOpen = true)}>Set up…</button></div>
      {/if}
    </div>
  {/if}
  {#if info}
    <dl class="facts">
      <dt>NAME</dt>
      <dd>
        {#if editing}
          <form class="inline" onsubmit={(e) => (e.preventDefault(), void saveDevice())}>
            <input class="input" maxlength="64" placeholder={info.deviceName} bind:value={name} aria-label="Device name" />
            <input class="input port mono" type="number" min="1024" max="65535" bind:value={port} aria-label="Port" />
            <button type="submit" class="btn" disabled={sync.busy}>SAVE</button>
            <button type="button" class="btn ghost" onclick={() => (editing = false)}>CANCEL</button>
          </form>
        {:else}
          <span>{info.deviceName}</span>
          <button type="button" class="link" onclick={() => (editing = true)}>change name or port</button>
        {/if}
      </dd>
      <dt>KEY</dt>
      <dd class="mono" title="Compare with what the other device shows for this one">{info.fingerprint}</dd>
      {#if info.listening}
        <dt>REACH IT AT</dt>
        <dd class="addrs">
          {#each info.addresses as a (a)}
            <button type="button" class="chip mono" title="Copy" onclick={() => void copy(a)}>{copied === a ? "copied" : a}</button>
          {:else}
            <span class="muted">no network address found</span>
          {/each}
        </dd>
      {/if}
    </dl>
    {#if info.error}<p class="error" role="alert">{info.error}</p>{/if}

    <div class="block" data-guide="sync-pair">
      <span class="label sub">PAIR A DEVICE</span>
      {#if info.pairing}
        <p class="muted small">On the other device: Settings → Sync → Join, then enter this device's address and the code.</p>
        <div class="code" aria-live="polite">
          <span class="mono big" use:scramble={info.pairing.code}></span>
          <span class="muted small">valid {Math.floor(info.pairing.expiresIn / 60)}:{String(info.pairing.expiresIn % 60).padStart(2, "0")}</span>
        </div>
        <p class="muted small">{info.pairing.acceptChanges ? "The new device can change this one's profile." : "The new device can only read this one's profile."}</p>
        <div class="actions"><button type="button" class="btn ghost" onclick={() => void sync.pairCancel()}>Cancel</button></div>
      {:else}
        <Toggle
          label="It can change this device's profile"
          hint="Lets it push a profile here and send automatic changes. You can change this later per device."
          checked={pairAccept}
          onchange={(v) => (pairAccept = v)}
        />
        <div class="actions">
          <button type="button" class="btn primary" disabled={sync.busy || !info.listening} onclick={() => void sync.pairStart(pairAccept)}>Show a code</button>
          <button type="button" class="btn" data-guide="sync-join" disabled={sync.busy} onclick={() => (joinOpen = true)}>Join a device…</button>
        </div>
        {#if !info.listening}<p class="muted small">Turn on "Let paired devices reach this one" to show a code here, or join a device that shows one.</p>{/if}
      {/if}
    </div>
  {/if}
  {#if sync.error && !joinOpen && !passOpen}<p class="error" role="alert">{sync.error}</p>{/if}
  {#if !inTauri()}<p class="muted small">Browser preview: nothing leaves this page.</p>{/if}
</Panel>

<Panel title="ON THIS NETWORK" guide="sync-found" prefix=">">
  <p class="muted small">Other Nightgrid apps on this LAN that can be found. Import one's profile and keep the two in step, straight over the local network.</p>
  <FoundDevices />
</Panel>

<Panel title="PAIRED DEVICES" guide="sync-peers" prefix=">" count={peers.length}>
  {#if !peers.length}
    <p class="muted">No paired devices yet. Show a code here and join from the other device, or the other way round.</p>
  {/if}
  {#if sync.lastApplied && Date.now() - sync.lastApplied.at < 60_000}
    <p class="ok small" role="status">{sync.lastApplied.count} change{sync.lastApplied.count === 1 ? "" : "s"} arrived from {sync.lastApplied.from}.</p>
  {/if}
  <ul class="peers">
    {#each peers as p (p.id)}
      {@const st = peerState(p, nowSecs)}
      <li class="peer">
        <div class="head">
          <span class="dot {st.tone}" aria-hidden="true"></span>
          {#if renaming === p.id}
            <form class="inline" onsubmit={(e) => (e.preventDefault(), void sync.setPeer(p, { name: rename }).then(() => (renaming = null)))}>
              <input class="input" maxlength="64" bind:value={rename} aria-label="Device name" />
              <button type="submit" class="btn">SAVE</button>
            </form>
          {:else}
            <button type="button" class="pname" title="Rename" onclick={() => ((renaming = p.id), (rename = p.name))}>{p.name}</button>
          {/if}
          <span class="muted small" class:bad={st.tone === "bad"}>{st.text}{p.appVersion ? ` · ${p.appVersion}` : ""}</span>
          <span class="spacer"></span>
          <button type="button" class="btn ghost sm" disabled={sync.busy} onclick={() => void sync.check(p.id)}>Check</button>
        </div>

        <div class="modes" role="radiogroup" aria-label="How {p.name} syncs" data-guide="sync-mode">
          {#each MODES as m (m.id)}
            <button type="button" role="radio" aria-checked={p.mode === m.id} class="mode" class:active={p.mode === m.id} title={m.what} disabled={sync.busy} onclick={() => void sync.setPeer(p, { mode: m.id })}>{m.label}</button>
          {/each}
        </div>
        <p class="muted small">{MODES.find((m) => m.id === p.mode)?.what}</p>

        <div class="actions">
          {#if p.mode === "manual"}
            <button type="button" class="btn" disabled={sync.busy} onclick={() => startTransfer(p, "push")}>Push to {p.name}…</button>
            <button type="button" class="btn" disabled={sync.busy} onclick={() => startTransfer(p, "pull")}>Pull from {p.name}…</button>
          {:else}
            <button type="button" class="btn" disabled={sync.busy} onclick={() => void syncNow(p)}>Sync now</button>
            <span class="muted small">{ran[p.id] ?? (p.lastSync ? `Last synced ${new Date(p.lastSync * 1000).toLocaleTimeString()}` : "Not synced yet")}</span>
          {/if}
        </div>
        {#if p.mode !== "manual" && !p.lastSync}
          <p class="muted small hint">First time: if both devices already have a setup, push or pull once in MANUAL first so they start the same. Otherwise items made on both are merged, and where they differ the newer edit wins.</p>
        {/if}

        <details class="more">
          <summary>Secrets and permissions</summary>
          <Toggle
            label="Can change this device"
            hint={`Off: ${p.name} can still pull this profile, but its pushes and automatic changes are refused here.`}
            checked={p.acceptChanges}
            disabled={sync.busy}
            onchange={(v) => void sync.setPeer(p, { acceptChanges: v })}
          />
          <p class="muted small">Saved secrets to exchange with {p.name} (sent from here and accepted here; set it on both devices):</p>
          {#each SECRET_GROUPS as g (g.id)}
            <Toggle label={g.label} hint={g.what} checked={p.secrets.includes(g.id)} disabled={sync.busy} onchange={(v) => toggleSecret(p, g.id, v)} />
          {/each}
          <p class="muted small">Never synced: this device's name, window size and scale, handheld and controller options, tray, built-in Tailscale, status page, MCP, sync folder and the DRIFT model. SSH key files in ~/.ssh stay where they are.</p>
          <div class="actions">
            <button type="button" class="btn ghost danger" disabled={sync.busy} onclick={() => (unpairing = p)}>Unpair…</button>
            <span class="muted small mono">{p.addrs.join(" · ")}{p.port !== 47920 ? ` · port ${p.port}` : ""}</span>
          </div>
        </details>
      </li>
    {/each}
  </ul>
</Panel>

<SnapshotsPanel />

<Modal open={passOpen} title="PAIRING PASSPHRASE" width={480} onclose={closePass}>
  <form id="sync-pass" onsubmit={(e) => (e.preventDefault(), void savePass())}>
    <p class="muted small">Use the passphrase you use for profile backups (or any passphrase you'll remember). Nightgrid keeps only a key made from it, in the OS keychain.</p>
    <label class="field"><span>PASSPHRASE</span><input class="input" type="password" autocomplete="new-password" bind:value={pass1} /></label>
    <label class="field"><span>AGAIN</span><input class="input" type="password" autocomplete="new-password" bind:value={pass2} /></label>
    {#if pass1 && pass1.length < 8}<p class="muted small">At least 8 characters.</p>{:else if pass2 && pass1 !== pass2}<p class="muted small">They don't match.</p>{/if}
  </form>
  {#if sync.error && passOpen}<p class="error" role="alert">{sync.error}</p>{/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={closePass}>Cancel</button>
    <button type="submit" form="sync-pass" class="btn primary" disabled={sync.busy || !passOk}>Save</button>
  {/snippet}
</Modal>

<Modal open={joinOpen} title={joined ? "PAIRED" : "JOIN A DEVICE"} width={520} onclose={closeJoin}>
  {#if joined}
    <p use:scramble={`Paired with ${joined.name}.`}></p>
    <p class="muted">It starts in MANUAL: push or pull a profile, or pick an automatic mode in PAIRED DEVICES.</p>
  {:else}
    <form id="sync-join" onsubmit={(e) => (e.preventDefault(), void join())}>
      <p class="muted small">On the other device: Settings → Sync → turn on "Let paired devices reach this one", then Show a code.</p>
      <label class="field">
        <span>ITS ADDRESS</span>
        <input class="input mono" list="sync-tailnet-peers" placeholder="192.168.1.20, mac.tail1234.ts.net or host:port" bind:value={joinAddr} autocomplete="off" />
        {#if tailnetPeers.length}
          <datalist id="sync-tailnet-peers">{#each tailnetPeers as tp (tp.dnsName || tp.name)}<option value={tp.ips[0] ?? tp.name}>{tp.name}</option>{/each}</datalist>
        {/if}
      </label>
      <label class="field">
        <span>CODE</span>
        <input class="input mono code-in" placeholder="ABCDE-12345" bind:value={joinCode} autocomplete="off" spellcheck="false" />
      </label>
      <Toggle label="It can change this device's profile" hint="Lets it push a profile here and send automatic changes." checked={joinAccept} onchange={(v) => (joinAccept = v)} />
    </form>
  {/if}
  {#if sync.error && joinOpen}<p class="error" role="alert">{sync.error}</p>{/if}
  {#snippet footer()}
    {#if joined}
      <button type="button" class="btn primary" onclick={closeJoin}>Done</button>
    {:else}
      <button type="button" class="btn ghost" onclick={closeJoin} disabled={sync.busy}>Cancel</button>
      <button type="submit" form="sync-join" class="btn primary" disabled={sync.busy || !codeOk || !joinAddr.trim()}>{sync.busy ? "Pairing…" : "Pair"}</button>
    {/if}
  {/snippet}
</Modal>

<Modal
  open={!!transfer}
  title={transfer ? (tdone ? "DONE" : transfer.dir === "push" ? `PUSH TO ${transfer.peer.name.toUpperCase()}` : `PULL FROM ${transfer.peer.name.toUpperCase()}`) : ""}
  tone={tmode === "replace" && !tdone ? "danger" : "default"}
  width={540}
  onclose={closeTransfer}
>
  {#if transfer}
    {#if tdone}
      <p use:scramble={tdone}></p>
      {#if pulled?.secretsError}<p class="error" role="alert">The keychain step didn't finish ({pulled.secretsError}). Saved passwords and tokens may need to be entered again.</p>{/if}
      {#if pulled}<p class="muted">Nightgrid restarts to load everything{pulled.settings ? ", including the settings" : ""}.</p>{/if}
    {:else}
      {@const there = transfer.dir === "push" ? transfer.peer.name : "this device"}
      <p class="muted small">
        {transfer.dir === "push" ? `Sends this device's profile to ${transfer.peer.name}.` : `Takes ${transfer.peer.name}'s profile.`}
        {transfer.peer.secrets.length ? `Secrets included: ${SECRET_GROUPS.filter((g) => transfer?.peer.secrets.includes(g.id)).map((g) => g.label).join(", ")}.` : "No saved secrets included."}
      </p>
      <div class="tmodes" role="radiogroup" aria-label="Mode">
        <button type="button" role="radio" aria-checked={tmode === "merge"} class="tmode" class:active={tmode === "merge"} onclick={() => (tmode = "merge")}>
          <span class="name">MERGE</span>
          <span class="muted small">Keep everything on {there} and add what's new. Existing items (same name, alias or address) and settings stay as they are.</span>
        </button>
        <button type="button" role="radio" aria-checked={tmode === "replace"} class="tmode danger" class:active={tmode === "replace"} onclick={() => (tmode = "replace")}>
          <span class="name">REPLACE</span>
          <span class="muted small">Make {there} a copy: its hosts, services, notes and the rest (and their saved secrets) are deleted first. Its device settings stay.</span>
        </button>
      </div>
      {#if tmode === "replace"}
        <label class="field">
          <span>TYPE <code class="verbatim">replace</code> TO CONTINUE</span>
          <input class="input" bind:value={typed} autocomplete="off" aria-label="Type replace to confirm" />
        </label>
      {/if}
    {/if}
    {#if terr}<p class="error" role="alert">{terr}</p>{/if}
  {/if}
  {#snippet footer()}
    {#if pulled}
      <button type="button" class="btn primary" onclick={() => void api.appRestart()}>Restart now</button>
    {:else if tdone}
      <button type="button" class="btn primary" onclick={closeTransfer}>Done</button>
    {:else}
      <button type="button" class="btn ghost" onclick={closeTransfer} disabled={tbusy}>Cancel</button>
      <button type="button" class="btn {tmode === 'replace' ? 'danger' : 'primary'}" disabled={tbusy || !tok} onclick={() => void runTransfer()}>
        {tbusy ? "Working…" : transfer?.dir === "push" ? "Push" : "Pull"}
      </button>
    {/if}
  {/snippet}
</Modal>

<Modal open={!!unpairing} title="UNPAIR" tone="danger" width={460} onclose={() => (unpairing = null)}>
  {#if unpairing}
    <p>Forget {unpairing.name}? It's asked to forget this device too. Nothing is deleted from either profile; pairing again needs a new code.</p>
  {/if}
  {#snippet footer()}
    <button type="button" class="btn ghost" onclick={() => (unpairing = null)}>Cancel</button>
    <button type="button" class="btn danger" disabled={sync.busy} onclick={() => unpairing && void sync.unpair(unpairing.id).then(() => (unpairing = null))}>Unpair</button>
  {/snippet}
</Modal>

<style>
  .lead {
    margin: 0 0 10px;
    font-size: var(--fs-sm);
  }
  .wide {
    grid-column: 1 / -1;
  }
  .banner {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border: 1px solid var(--magenta);
    background: color-mix(in srgb, var(--magenta) 10%, transparent);
  }
  .banner span {
    flex: 1;
  }
  .facts {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 12px;
    margin: 10px 0 0;
    font-size: var(--fs-sm);
  }
  .facts dt {
    color: var(--text-1);
    letter-spacing: 0.1em;
    font-size: var(--fs-xs);
    padding-top: 2px;
  }
  .facts dd {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .inline {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    align-items: center;
  }
  .port {
    width: 96px;
  }
  .chip {
    border: 1px solid var(--line);
    background: transparent;
    color: var(--cyan);
    padding: 2px 8px;
    cursor: pointer;
    font-size: var(--fs-xs);
  }
  .sub {
    display: block;
    margin-bottom: 6px;
    color: var(--text-1);
  }
  .block {
    border-top: 1px solid var(--line);
    padding-top: 10px;
    margin-top: 10px;
  }
  .code {
    display: flex;
    align-items: baseline;
    gap: 12px;
    margin: 8px 0;
  }
  .big {
    font-size: 1.8em;
    letter-spacing: 0.12em;
    color: var(--magenta);
  }
  .code-in {
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
    margin-top: 8px;
  }
  .small {
    font-size: var(--fs-xs);
    margin: 4px 0 0;
  }
  .hint {
    color: var(--warn);
  }
  .error {
    color: var(--danger);
    margin: 8px 0 0;
  }
  .ok {
    color: var(--mint);
  }
  .bad {
    color: var(--danger);
  }
  .peers {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 10px;
  }
  .peer {
    border: 1px solid var(--line);
    padding: 10px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .spacer {
    flex: 1;
  }
  .pname {
    background: none;
    border: 0;
    padding: 0;
    color: var(--text-0);
    font-family: var(--font-display);
    font-weight: 700;
    letter-spacing: 0.08em;
    cursor: text;
  }
  .dot {
    width: 8px;
    height: 8px;
    border: 1px solid var(--text-1);
    flex: none;
  }
  .dot.ok {
    background: var(--ok);
    border-color: var(--ok);
  }
  .dot.warn {
    background: var(--warn);
    border-color: var(--warn);
  }
  .dot.bad {
    background: var(--danger);
    border-color: var(--danger);
  }
  .modes {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 4px;
    margin-top: 10px;
  }
  .mode {
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    padding: 6px 4px;
    min-height: var(--hit);
    font-size: var(--fs-xs);
    letter-spacing: 0.1em;
    cursor: pointer;
  }
  .mode.active {
    border-color: var(--magenta);
    color: var(--magenta);
    background: color-mix(in srgb, var(--magenta) 10%, transparent);
  }
  .more {
    margin-top: 8px;
  }
  .more summary {
    cursor: pointer;
    color: var(--text-1);
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
  }
  .tmodes {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    margin: 10px 0;
  }
  .tmode {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
    border: 1px solid var(--line);
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .tmode .name {
    font-family: var(--font-display);
    font-weight: 700;
    letter-spacing: 0.14em;
  }
  .tmode.active {
    border-color: var(--magenta);
    background: color-mix(in srgb, var(--magenta) 10%, transparent);
  }
  .tmode.active.danger {
    border-color: var(--danger);
    background: color-mix(in srgb, var(--danger) 10%, transparent);
  }
  @media (max-width: 720px) {
    .modes {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
