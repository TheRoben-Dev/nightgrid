<script lang="ts">
  import { errorText, type TunnelInput, type TunnelKind } from "../../api";
  import { hosts } from "../../stores/hosts.svelte";
  import { emptyTunnel, isActive, KIND_LABEL, toTunnelInput, tunnels } from "../../stores/tunnels.svelte";
  import Modal from "../Modal.svelte";
  import Toggle from "../Toggle.svelte";

  const open = $derived(tunnels.editing !== null);
  const existing = $derived(tunnels.editing ? tunnels.byId(tunnels.editing) : undefined);

  let form = $state<TunnelInput>(emptyTunnel(0));
  let startNow = $state(true);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let confirmDelete = $state(false);

  $effect(() => {
    if (tunnels.editing === null) return;
    const t = tunnels.editing ? tunnels.byId(tunnels.editing) : undefined;
    form = t ? toTunnelInput(t) : (tunnels.draft ?? emptyTunnel(hosts.preferred("tunnels")?.id ?? 0));
    startNow = true;
    error = null;
    confirmDelete = false;
  });

  const KINDS: { id: TunnelKind; hint: string }[] = [
    { id: "local", hint: "Listen on this computer; connections are made from the host. Reach a service that only listens on the host or its network." },
    { id: "remote", hint: "Listen on the host; connections come back to this computer. Show something running here to the host." },
    { id: "dynamic", hint: "A SOCKS5 proxy on this computer; every connection leaves from the host. Point a browser at it to browse the host's network." },
  ];

  function setKind(k: TunnelKind) {
    const wasDefault = !form.bindAddr || form.bindAddr === "127.0.0.1" || form.bindAddr === "localhost";
    form.kind = k;
    if (wasDefault) form.bindAddr = "";
    if (k === "dynamic" && form.bindPort === 8080) form.bindPort = 1080;
    if (k === "remote" && form.destHost === "localhost") form.destHost = "127.0.0.1";
  }

  const bindDefault = $derived(form.kind === "remote" ? "localhost" : "127.0.0.1");
  const bindAddr = $derived(form.bindAddr?.trim() || bindDefault);
  const isPublic = $derived(!["127.0.0.1", "localhost", "::1"].includes(bindAddr) && !bindAddr.startsWith("127."));
  const hostName = $derived(hosts.byId(form.hostId)?.name ?? "the host");
  const valid = $derived(
    !!form.hostId && form.bindPort >= 0 && form.bindPort <= 65535 && (form.kind === "dynamic" || (!!form.destHost?.trim() && (form.destPort ?? 0) > 0)),
  );

  const preview = $derived.by(() => {
    const b = `${bindAddr}:${form.bindPort || "auto"}`;
    const d = `${form.destHost || "?"}:${form.destPort || "?"}`;
    if (form.kind === "local") return `this computer ${b}  →  ${hostName}  →  ${d}`;
    if (form.kind === "remote") return `${hostName} ${b}  →  this computer  →  ${d}`;
    return `SOCKS5 on this computer ${b}  →  out through ${hostName}`;
  });

  const num = (v: string) => (v === "" ? 0 : Math.max(0, Math.min(65535, Math.trunc(Number(v)) || 0)));

  function close() {
    tunnels.editing = null;
    tunnels.draft = null;
  }

  async function save(e: Event) {
    e.preventDefault();
    if (!valid) return;
    busy = true;
    error = null;
    try {
      await tunnels.save(tunnels.editing ?? 0, form, startNow);
      close();
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }

  async function remove() {
    if (!existing) return;
    busy = true;
    try {
      await tunnels.remove(existing.id);
      close();
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal {open} title={existing ? `EDIT TUNNEL :: ${existing.name || existing.spec}` : "NEW TUNNEL"} width={620} onclose={close}>
  {#if confirmDelete && existing}
    <div class="confirm">
      <p>Delete tunnel <strong>{existing.name || existing.spec}</strong> (<code>{existing.spec}</code>) on {hostName}?</p>
      <p class="muted">
        {isActive(tunnels.stateOf(existing.id)) ? "It's running: it stops and its open connections are cut. " : ""}Nothing on
        the host is changed.
      </p>
    </div>
  {:else}
    <form class="grid" id="tunnel-form" onsubmit={save}>
      <fieldset class="span6">
        <legend class="field-label">Type</legend>
        <div class="seg tabstrip choice" role="radiogroup" aria-label="Tunnel type">
          {#each KINDS as k (k.id)}
            <button type="button" role="radio" aria-checked={form.kind === k.id} class:active={form.kind === k.id} onclick={() => setKind(k.id)}>
              {KIND_LABEL[k.id]}
            </button>
          {/each}
        </div>
        <p class="field-hint">{KINDS.find((k) => k.id === form.kind)?.hint}</p>
      </fieldset>

      <label class="field span3">
        <span>Host</span>
        <select class="input" value={form.hostId} onchange={(e) => (form.hostId = Number(e.currentTarget.value))}>
          {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
        </select>
      </label>
      <label class="field span3">
        <span>Name</span>
        <input class="input" bind:value={form.name} placeholder="(optional)" autocomplete="off" />
      </label>

      <label class="field span4">
        <span>{form.kind === "remote" ? "Listen on (host)" : "Listen on (this computer)"}</span>
        <input class="input" bind:value={form.bindAddr} placeholder={bindDefault} autocomplete="off" aria-invalid={isPublic} />
      </label>
      <label class="field span2">
        <span>Port</span>
        <input class="input" type="number" min="0" max="65535" value={form.bindPort} oninput={(e) => (form.bindPort = num(e.currentTarget.value))} />
      </label>

      {#if form.kind !== "dynamic"}
        <label class="field span4">
          <span>{form.kind === "local" ? "Connect to (from the host)" : "Connect to (from this computer)"}</span>
          <input class="input" bind:value={form.destHost} placeholder="localhost" required autocomplete="off" />
        </label>
        <label class="field span2">
          <span>Port</span>
          <input class="input" type="number" min="1" max="65535" value={form.destPort} oninput={(e) => (form.destPort = num(e.currentTarget.value))} />
        </label>
      {/if}

      <p class="preview span6" aria-label="Route">{preview}</p>
      {#if isPublic}
        <p class="warn span6" role="alert">
          {bindAddr} listens beyond {form.kind === "remote" ? "the host's loopback (needs GatewayPorts on the server)" : "this computer"}: anyone who can reach
          it can use the tunnel. Use 127.0.0.1 unless you mean that.
        </p>
      {/if}
      <p class="field-hint span6">Port 0 picks a free port each time it starts.</p>

      <div class="span6 toggles">
        <Toggle
          label="Auto-start"
          hint="Start when Nightgrid opens and whenever the host reconnects."
          checked={!!form.autoStart}
          onchange={(v) => (form.autoStart = v)}
        />
        {#if !existing}
          <Toggle label="Start now" checked={startNow} onchange={(v) => (startNow = v)} />
        {/if}
      </div>
    </form>
  {/if}
  {#if error}<p class="error-text" role="alert">{error}</p>{/if}

  {#snippet footer()}
    {#if confirmDelete}
      <button type="button" class="btn ghost" onclick={() => (confirmDelete = false)}>Keep</button>
      <button type="button" class="btn danger" disabled={busy} onclick={remove}>Delete {existing?.name || existing?.spec}</button>
    {:else}
      {#if existing}
        <button type="button" class="btn ghost left" onclick={() => (confirmDelete = true)}>Delete…</button>
      {/if}
      <button type="button" class="btn ghost" onclick={close}>Cancel</button>
      <button type="submit" form="tunnel-form" class="btn primary" disabled={busy || !valid}>{existing ? "Save" : "Add tunnel"}</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 12px;
  }
  .span2 {
    grid-column: span 2;
  }
  .span3 {
    grid-column: span 3;
  }
  .span4 {
    grid-column: span 4;
  }
  .span6 {
    grid-column: span 6;
  }
  fieldset {
    border: 0;
    padding: 0;
    margin: 0;
  }
  .field-label {
    font-family: var(--font-display);
    font-weight: 600;
    font-size: var(--fs-xs);
    letter-spacing: var(--tracking-label);
    text-transform: uppercase;
    color: var(--text-1);
    padding: 0;
    margin-bottom: 4px;
  }
  fieldset p {
    margin: 6px 0 0;
  }
  .preview {
    margin: 0;
    padding: 6px 8px;
    border: 1px dashed var(--line);
    color: var(--cyan);
    font-size: var(--fs-sm);
    white-space: pre-wrap;
  }
  .warn {
    margin: 0;
    color: var(--warn);
    font-size: var(--fs-sm);
  }
  .toggles {
    display: grid;
    gap: 6px;
  }
  .confirm p {
    margin: 0 0 10px;
    line-height: 1.5;
  }
  code {
    color: var(--cyan);
  }
  .error-text {
    margin: 12px 0 0;
  }
  .left {
    margin-right: auto;
  }
</style>
