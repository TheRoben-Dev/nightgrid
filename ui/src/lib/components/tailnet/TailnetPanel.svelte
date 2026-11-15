<script lang="ts">
  /** Settings → Tailnet: the built-in Tailscale client (M10j). */
  import { onMount } from "svelte";
  import { writeClipboard } from "../../clipboard";
  import { settings } from "../../stores/settings.svelte";
  import { tailnet } from "../../stores/tailnet.svelte";
  import Panel from "../Panel.svelte";
  import Toggle from "../Toggle.svelte";

  onMount(() => tailnet.start());

  const s = $derived(tailnet.status);
  const STATE: Record<string, string> = {
    off: "Off",
    unavailable: "Not included in this build",
    starting: "Starting…",
    needsLogin: "Needs a login",
    needsMachineAuth: "Waiting for approval in the tailnet's admin console",
    running: "Connected",
    stopped: "Stopped",
    failed: "Couldn't start",
  };
  const label = $derived(s ? (STATE[s.state] ?? s.state) : "…");
  const tone = $derived(s?.state === "running" ? "ok" : s?.state === "failed" || s?.state === "stopped" ? "bad" : s?.state === "off" ? "" : "warn");

  let name = $state("");
  let control = $state("");
  let advanced = $state(false);
  let saved = $state(false);
  $effect(() => {
    name = settings.value.tailnetHostname;
    control = settings.value.tailnetControlUrl;
  });
  async function saveAdvanced() {
    saved = await tailnet.configure(name, control, s?.acceptRoutes ?? settings.value.tailnetAcceptRoutes);
    if (saved) await settings.load();
  }
  let copied = $state<string | null>(null);
  async function copy(t: string) {
    await writeClipboard(t);
    copied = t;
    setTimeout(() => (copied = null), 1200);
  }
</script>

<Panel title="TAILNET" guide="settings-tailnet">
  <p class="muted intro">
    Join your tailnet from Nightgrid itself, so hosts on it (and behind a subnet router) stay reachable away from home without the Tailscale
    app. Nightgrid becomes its own device in your tailnet. Its keys are stored encrypted with a key in the OS keychain, and nothing is sent
    to Tailscale's log servers.
  </p>
  <div class="toggles">
    <Toggle
      label="Built-in Tailscale"
      hint="Off: tailnet addresses are reached through the system Tailscale app, if one is connected."
      checked={tailnet.on}
      disabled={tailnet.busy || s?.state === "unavailable"}
      onchange={(v) => tailnet.setEnabled(v)}
    />
  </div>

  <div class="state" data-guide="tailnet-status">
    <span class="dot {tone}" aria-hidden="true"></span>
    <strong>{label}</strong>
    {#if s?.state === "running" && s.selfNode}
      <span class="muted">as <span class="mono">{s.selfNode.name}</span> · {s.selfNode.ips[0] ?? ""}{s.tailnet ? ` · ${s.tailnet}` : ""}</span>
    {/if}
    <span class="spacer"></span>
    {#if s?.state === "needsLogin"}
      <button type="button" class="btn primary" data-guide="tailnet-login" disabled={tailnet.busy} onclick={() => tailnet.login()}>LOG IN</button>
    {:else if s?.state === "running"}
      <button type="button" class="btn ghost" disabled={tailnet.busy} onclick={() => tailnet.logout()}>LOG OUT</button>
    {/if}
  </div>
  {#if s?.state === "needsLogin" && s.loginUrl}
    <p class="muted small">
      Finish logging in in your browser. <button type="button" class="link" onclick={() => tailnet.login()}>Open the page again</button> ·
      <button type="button" class="link" onclick={() => s.loginUrl && copy(s.loginUrl)}>{copied === s.loginUrl ? "Copied" : "Copy link"}</button>
    </p>
  {/if}
  {#if tailnet.error || s?.error}<p class="err">{tailnet.error ?? s?.error}</p>{/if}

  {#if tailnet.on}
    <div class="toggles">
      <Toggle
        label="Use subnet routes"
        hint="Reach LAN addresses that another device in the tailnet advertises (for example a home server routing 192.168.1.0/24). Nightgrid tries the LAN first and the tailnet when the LAN doesn't answer, so the same host works at home and away."
        checked={s?.acceptRoutes ?? true}
        disabled={tailnet.busy}
        onchange={(v) => tailnet.configure(settings.value.tailnetHostname, settings.value.tailnetControlUrl, v)}
      />
    </div>
  {/if}

  {#if s?.state === "running"}
    <h3 class="sub">DEVICES <span class="muted">[{s.peers.length}]</span></h3>
    <ul class="peers">
      {#each s.peers as p (p.dnsName || p.name)}
        <li>
          <span class="dot {p.online ? 'ok' : ''}" title={p.online ? "online" : "offline"}></span>
          <span class="pn">{p.name}</span>
          <button type="button" class="ip mono" title="Copy address" onclick={() => copy(p.ips[0] ?? "")}>{copied === p.ips[0] ? "copied" : (p.ips[0] ?? "")}</button>
          <span class="muted">{p.os}</span>
          {#if p.routes.length}<span class="routes" title="Subnet routes this device serves">⇢ {p.routes.join(", ")}</span>{/if}
        </li>
      {:else}
        <li class="muted">No other devices yet.</li>
      {/each}
    </ul>
    <p class="muted small">Use a device's name ({s.peers[0]?.name ?? "lumen"}{s.magicDnsSuffix ? ` or ${s.peers[0]?.name ?? "lumen"}.${s.magicDnsSuffix}` : ""}) or its 100.x address as a host's address.</p>
  {/if}

  <button type="button" class="link adv" aria-expanded={advanced} onclick={() => (advanced = !advanced)}>{advanced ? "▾" : "▸"} Advanced</button>
  {#if advanced}
    <form class="adv-form" onsubmit={(e) => { e.preventDefault(); void saveAdvanced(); }}>
      <label><span class="lbl">DEVICE NAME</span><input class="input mono" maxlength="63" bind:value={name} oninput={() => (saved = false)} /></label>
      <label
        ><span class="lbl">CONTROL SERVER</span><input
          class="input mono"
          placeholder="Tailscale (leave empty) or https://headscale.example.com"
          bind:value={control}
          oninput={() => (saved = false)}
        /></label
      >
      <div><button type="submit" class="btn" disabled={tailnet.busy}>{saved ? "SAVED" : "SAVE"}</button></div>
      <p class="muted small">A running client restarts to use a new name or control server.</p>
    </form>
  {/if}
</Panel>

<style>
  .intro,
  .small {
    font-size: var(--fs-xs);
    margin: 0 0 8px;
  }
  .state {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 0;
    flex-wrap: wrap;
  }
  .spacer {
    flex: 1;
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
    border-color: var(--warn);
  }
  .dot.bad {
    border-color: var(--danger);
  }
  .sub {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    color: var(--text-1);
    margin: 12px 0 4px;
  }
  .peers {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .peers li {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-sm);
    flex-wrap: wrap;
  }
  .pn {
    min-width: 110px;
  }
  .ip {
    border: 0;
    background: none;
    padding: 0;
    color: var(--cyan);
    cursor: pointer;
    font-size: inherit;
  }
  .routes {
    color: var(--mint);
    font-size: var(--fs-xs);
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent);
    cursor: pointer;
    font-size: inherit;
  }
  .adv {
    margin-top: 10px;
    font-size: var(--fs-xs);
  }
  .adv-form {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 6px;
    max-width: 460px;
  }
  .adv-form label {
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
  .err {
    color: var(--danger);
    font-size: var(--fs-sm);
  }
</style>
