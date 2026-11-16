<script lang="ts">
  import { scramble } from "../scramble";
  import { untrack } from "svelte";
  import { api, errorText, type AuthMethod, type Host, type HostInput } from "../api";
  import { hosts } from "../stores/hosts.svelte";
  import { tailnet } from "../stores/tailnet.svelte";
  import { looksTailnet } from "../tailnetAddr";
  import { ICON_PRESETS, iconProblem, normalizeIcon } from "../hosts/icons";
  import HostIcon from "./HostIcon.svelte";
  import Modal from "./Modal.svelte";
  import Toggle from "./Toggle.svelte";

  /** Swatches offered for host colours (any #RRGGBB is accepted by the backend). */
  const COLORS = ["#00E5FF", "#FF2A6D", "#F3E600", "#2BFF88", "#BDF5EC", "#FF9F1C", "#A78BFA"];
  const AUTH: { id: AuthMethod; label: string; hint: string }[] = [
    { id: "agent", label: "SSH agent", hint: "Recommended. Uses your running agent (1Password, Secretive, ssh-agent, Pageant)." },
    { id: "key", label: "Key file", hint: "A private key file. Its passphrase is asked for when needed." },
    { id: "password", label: "Password", hint: "Asked for on connect. Stored only in the OS keychain, and only if you tick remember." },
  ];

  const editingId = $derived(hosts.editing);
  const open = $derived(editingId !== null);
  const existing = $derived(editingId ? hosts.byId(editingId) : undefined);

  const blank = (): HostInput => ({
    name: "",
    hostname: "",
    port: null,
    username: null,
    groupId: null,
    color: null,
    icon: null,
    favorite: false,
    notes: null,
    authMethod: "agent",
    identityFile: null,
    proxyJump: null,
    rememberSecret: false,
    sshAlias: null,
    tailnetAddress: null,
    driftHistory: false,
    macAddress: null,
    wakeRelayId: null,
    sleepWindow: null,
    role: "server",
    tags: [],
  });

  const fromHost = (h: Host): HostInput => ({ ...h, port: h.port === 22 ? null : h.port, tags: [...h.tags] });

  let form = $state<HostInput>(blank());
  let tagsText = $state("");
  let newGroup = $state("");
  let error = $state<string | null>(null);
  let busy = $state(false);
  let confirmDelete = $state(false);

  // Reset the form each time the editor opens (only `open`/`existing` are tracked).
  $effect(() => {
    if (!open) return;
    const h = existing;
    untrack(() => {
      tailnet.start();
      form = h ? fromHost(h) : { ...blank(), ...(hosts.prefill ?? {}) };
      hosts.prefill = null;
      tagsText = form.tags.join(", ");
      newGroup = "";
      error = null;
      confirmDelete = false;
    });
  });

  const iconArt = $derived(normalizeIcon(form.icon));
  const iconError = $derived(iconProblem(form.icon));

  const nullable = (s: string | null) => (s && s.trim() ? s.trim() : null);

  function close() {
    hosts.editing = null;
  }

  async function save(e?: Event) {
    e?.preventDefault();
    busy = true;
    error = null;
    try {
      let groupId = form.groupId;
      if (newGroup.trim()) {
        const g = await api.groupCreate({ name: newGroup.trim(), color: null, icon: null, sortOrder: hosts.groups.length });
        groupId = g.id;
      }
      const input: HostInput = {
        ...form,
        groupId,
        name: form.name.trim() || form.hostname.trim(),
        username: nullable(form.username),
        identityFile: form.authMethod === "password" ? null : nullable(form.identityFile),
        proxyJump: nullable(form.proxyJump),
        tailnetAddress: nullable(form.tailnetAddress),
        macAddress: nullable(form.macAddress),
        sleepWindow: nullable(form.sleepWindow),
        notes: nullable(form.notes),
        icon: iconArt,
        tags: tagsText
          .split(",")
          .map((t) => t.trim())
          .filter(Boolean),
      };
      if (existing) await api.hostUpdate(existing.id, input);
      else await api.hostCreate(input);
      await hosts.refresh();
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
      await api.hostDelete(existing.id);
      await hosts.refresh();
      close();
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }

  async function forget() {
    if (!existing) return;
    try {
      await api.hostForgetSecrets(existing.id);
      error = null;
      form.rememberSecret = false;
    } catch (err) {
      error = errorText(err);
    }
  }
</script>

<Modal {open} title={existing ? `EDIT NODE :: ${existing.name}` : "NEW NODE"} width={640} onclose={close}>
  {#if confirmDelete && existing}
    <div class="confirm">
      <p>Delete host <strong>{existing.name}</strong> ({existing.hostname})?</p>
      <p class="muted">
        This removes it from NIGHTGRID and forgets any secrets saved for it. Your <code>~/.ssh/config</code> is not
        touched.
      </p>
    </div>
  {:else}
    <form class="grid" onsubmit={save} id="host-form">
      <label class="field span2">
        <span>Name</span>
        <input class="input" bind:value={form.name} placeholder="my-server" autocomplete="off" />
      </label>
      <label class="field span3">
        <span>Hostname / IP</span>
        <input class="input" bind:value={form.hostname} placeholder="server.lan or 192.0.2.10" required autocomplete="off" />
      </label>
      <label class="field">
        <span>Port</span>
        <input
          class="input"
          type="number"
          min="1"
          max="65535"
          placeholder="22"
          value={form.port ?? ""}
          oninput={(e) => (form.port = e.currentTarget.value ? Number(e.currentTarget.value) : null)}
        />
      </label>
      <label class="field span2">
        <span>User</span>
        <input class="input" bind:value={form.username} placeholder="(local user)" autocomplete="off" />
      </label>

      <fieldset class="span6 auth">
        <legend class="field-label">Authentication</legend>
        <div class="seg" role="radiogroup" aria-label="Authentication">
          {#each AUTH as a (a.id)}
            <button
              type="button"
              role="radio"
              aria-checked={form.authMethod === a.id}
              class:active={form.authMethod === a.id}
              onclick={() => (form.authMethod = a.id)}>{a.label}</button
            >
          {/each}
        </div>
        <p class="field-hint">{AUTH.find((a) => a.id === form.authMethod)?.hint}</p>
        {#if form.authMethod !== "password" && (looksTailnet(form.hostname) || looksTailnet(form.tailnetAddress))}
          <p class="field-hint ts" data-guide="host-tailscale-ssh">
            Tailnet address. If the host has Tailscale SSH on, this works as it is: no key needed, the tailnet's policy decides. When the policy asks for a check,
            Nightgrid shows an <b>Approve in browser</b> link while it connects.
          </p>
        {/if}
      </fieldset>

      {#if form.authMethod !== "password"}
        <label class="field span6">
          <span>{form.authMethod === "key" ? "Private key file" : "Identity file (optional)"}</span>
          <input
            class="input"
            bind:value={form.identityFile}
            placeholder="~/.ssh/id_ed25519"
            required={form.authMethod === "key"}
            autocomplete="off"
          />
          {#if form.authMethod === "agent"}
            <span class="field-hint">Picks which agent key to offer first; used directly if the agent lacks it.</span>
          {/if}
        </label>
      {/if}

      <label class="field span6">
        <span>Proxy jump</span>
        <input class="input" bind:value={form.proxyJump} placeholder="bastion, user@gw:2222" autocomplete="off" />
        <span class="field-hint">Comma-separated hops. Names of hosts saved here use their own settings.</span>
      </label>

      <label class="field span6" data-guide="host-tailnet-address">
        <span>Tailnet address</span>
        <input class="input mono" bind:value={form.tailnetAddress} placeholder="100.x.y.z or a MagicDNS name" list="ng-tailnet-peers" autocomplete="off" />
        <datalist id="ng-tailnet-peers">
          {#each tailnet.status?.peers ?? [] as p (p.dnsName || p.name)}
            <option value={p.dnsName || p.name}>{p.ips[0] ?? ""}</option>
            {#if p.ips[0]}<option value={p.ips[0]}>{p.name}</option>{/if}
          {/each}
        </datalist>
        <span class="field-hint">Optional. Tried when the address above doesn't answer (away from home), and the other way round when you're back. The host key is checked against the main address.</span>
      </label>

      <label class="field span2" data-guide="host-wake">
        <span>MAC address (Wake-on-LAN)</span>
        <input class="input mono" bind:value={form.macAddress} placeholder="aa:bb:cc:dd:ee:ff" autocomplete="off" spellcheck="false" />
      </label>
      <label class="field span2">
        <span>Wake from</span>
        <select class="input" value={form.wakeRelayId ?? ""} onchange={(e) => (form.wakeRelayId = e.currentTarget.value ? Number(e.currentTarget.value) : null)}>
          <option value="">This computer</option>
          {#each hosts.hosts.filter((x) => x.id !== existing?.id) as x (x.id)}<option value={x.id}>{x.name}</option>{/each}
        </select>
      </label>
      <label class="field span2">
        <span>Sleep window</span>
        <input class="input mono" bind:value={form.sleepWindow} placeholder="23:00-07:00" autocomplete="off" spellcheck="false" />
      </label>
      <p class="span6 field-hint">
        Wake from another host on the same LAN to wake this one when you're away. Inside the sleep window (this computer's time), a host that doesn't
        answer shows as asleep, without an offline alert.
      </p>

      <label class="field span3">
        <span>Group</span>
        <select
          class="input"
          value={form.groupId ?? ""}
          onchange={(e) => (form.groupId = e.currentTarget.value ? Number(e.currentTarget.value) : null)}
        >
          <option value="">None</option>
          {#each hosts.groups as g (g.id)}<option value={g.id}>{g.name}</option>{/each}
        </select>
      </label>
      <label class="field span3">
        <span>…or new group</span>
        <input class="input" bind:value={newGroup} placeholder="Servers" autocomplete="off" />
      </label>

      <label class="field span4">
        <span>Tags</span>
        <input class="input" bind:value={tagsText} placeholder="docker, prod" autocomplete="off" />
      </label>
      <div class="field span2">
        <span>Color</span>
        <div class="swatches" role="radiogroup" aria-label="Color">
          <button
            type="button"
            role="radio"
            aria-checked={form.color === null}
            aria-label="No color"
            class="sw none"
            class:on={form.color === null}
            onclick={() => (form.color = null)}
          ></button>
          {#each COLORS as c (c)}
            <button
              type="button"
              role="radio"
              aria-checked={form.color === c}
              aria-label={c}
              class="sw"
              class:on={form.color === c}
              style:background={c}
              onclick={() => (form.color = c)}
            ></button>
          {/each}
        </div>
      </div>

      <div class="field span6">
        <span>ASCII icon</span>
        <div class="presets" role="group" aria-label="Icon presets">
          <button type="button" class="btn ghost tiny" class:on={!iconArt} onclick={() => (form.icon = null)}>None</button>
          {#each ICON_PRESETS as p (p.id)}
            <button type="button" class="btn ghost tiny" class:on={iconArt === p.art} onclick={() => (form.icon = p.art)}>{p.label}</button>
          {/each}
        </div>
        <div class="iconedit">
          <textarea
            class="input mono"
            rows="6"
            spellcheck="false"
            autocomplete="off"
            aria-label="ASCII icon"
            placeholder="Draw your own (8 × 32 max)"
            value={form.icon ?? ""}
            oninput={(e) => (form.icon = (e.currentTarget as HTMLTextAreaElement).value)}
          ></textarea>
          <div class="preview" aria-label="Icon preview">
            {#if iconArt}<HostIcon art={iconArt} color={form.color} size="md" />{:else}<span class="muted">no icon</span>{/if}
            <span class="pname">{form.name.trim() || form.hostname.trim() || "host"}</span>
          </div>
        </div>
        {#if iconError}
          <span class="error-text">{iconError}</span>
        {:else}
          <span class="field-hint">Shown above the name on the overview and connect tiles, in the host's color.</span>
        {/if}
      </div>

      <label class="field span6">
        <span>Notes</span>
        <textarea class="input" rows="2" bind:value={form.notes}></textarea>
      </label>

      <div class="span6 toggles">
        <Toggle label="Favorite" checked={form.favorite} onchange={(v) => (form.favorite = v)} />
        <Toggle
          label="Client, not a server"
          hint="For a laptop, desktop or handheld you also manage. Views never pick it by default; you can still choose it."
          checked={form.role === "client"}
          onchange={(v) => (form.role = v ? "client" : "server")}
        />
        {#if form.authMethod !== "agent"}
          <Toggle
            label="Remember secret in OS keychain"
            hint="Off: you are asked every time (kept in memory until NIGHTGRID quits)."
            checked={form.rememberSecret}
            onchange={(v) => (form.rememberSecret = v)}
          />
        {/if}
        <Toggle
          label="DRIFT may read shell history"
          hint="When you ask DRIFT about this host, it reads the last commands from bash, zsh or fish history over SSH. Lines that look like passwords or tokens are left out. Nothing is stored."
          checked={form.driftHistory}
          onchange={(v) => (form.driftHistory = v)}
        />
      </div>
      {#if form.sshAlias}
        <p class="span6 field-hint">Imported from ~/.ssh/config alias <code>{form.sshAlias}</code>.</p>
      {/if}
    </form>
  {/if}
  {#if error}<p class="error-text" role="alert" use:scramble={error}></p>{/if}

  {#snippet footer()}
    {#if confirmDelete}
      <button type="button" class="btn ghost" onclick={() => (confirmDelete = false)}>Keep</button>
      <button type="button" class="btn danger" disabled={busy} onclick={remove}>Delete {existing?.name}</button>
    {:else}
      {#if existing}
        <button type="button" class="btn ghost left" onclick={() => (confirmDelete = true)}>Delete…</button>
        {#if existing.rememberSecret}
          <button type="button" class="btn ghost" onclick={forget}>Forget saved secret</button>
        {/if}
      {/if}
      <button type="button" class="btn ghost" onclick={close}>Cancel</button>
      <button type="submit" form="host-form" class="btn primary" disabled={busy || !form.hostname.trim() || !!iconError}>
        {existing ? "Save" : "Add host"}
      </button>
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
  .seg {
    display: flex;
    border: 1px solid var(--line);
  }
  .seg button {
    flex: 1;
    min-height: var(--hit);
    border: 0;
    border-right: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    cursor: pointer;
    font-family: var(--font-display);
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    font-size: var(--fs-sm);
  }
  .seg button:last-child {
    border-right: 0;
  }
  .seg button.active {
    color: var(--text-0);
    background: color-mix(in srgb, var(--magenta) 16%, transparent);
    box-shadow: inset 0 -2px 0 var(--magenta);
  }
  .auth p {
    margin: 6px 0 0;
  }
  .swatches {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
    align-items: center;
    min-height: var(--hit);
  }
  .sw {
    width: 18px;
    height: 18px;
    border: 1px solid var(--line);
    cursor: pointer;
    padding: 0;
  }
  .sw.none {
    background: repeating-linear-gradient(45deg, transparent 0 3px, var(--line) 3px 4px);
  }
  .sw.on {
    outline: 1px solid var(--text-0);
    outline-offset: 2px;
  }
  textarea {
    resize: vertical;
  }
  .presets {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .presets .on {
    color: var(--magenta);
    border-color: var(--magenta);
    box-shadow: var(--glow-active);
  }
  .iconedit {
    display: grid;
    grid-template-columns: 1fr 200px;
    gap: 8px;
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 1.12;
    white-space: pre;
    overflow-x: auto;
  }
  .preview {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    justify-content: center;
    gap: 6px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    background: var(--bg-0);
    overflow: hidden;
  }
  .pname {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: var(--fs-lg);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-0);
  }
  .toggles {
    display: flex;
    flex-direction: column;
  }
  code {
    color: var(--mint);
  }
  .confirm p {
    margin: 0 0 8px;
  }
  .left {
    margin-right: auto;
  }
</style>
