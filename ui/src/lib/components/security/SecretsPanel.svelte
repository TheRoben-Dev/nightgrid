<script lang="ts">
  /**
   * Security → SECRETS (1.1, D-169): every secret Nightgrid knows about, with
   * its age and when it's due. Values are never shown or read here; each row
   * leads to where the secret is actually changed.
   */
  import { onMount } from "svelte";
  import type { SecretItem, SecretKind } from "../../api";
  import { KINDS, POLICIES, ageText, dueText, fingerprintOf, kindLabel, manualId, policyLabel, stateLabel, stateTone, summary } from "../../secrets";
  import { docker } from "../../stores/docker.svelte";
  import { hosts } from "../../stores/hosts.svelte";
  import { library } from "../../stores/library.svelte";
  import { security } from "../../stores/security.svelte";
  import { settings } from "../../stores/settings.svelte";
  import { ui } from "../../stores/ui.svelte";
  import type { SettingsTab } from "../../settingsTabs";
  import Toggle from "../Toggle.svelte";

  onMount(() => {
    void security.loadSecrets();
  });

  let kind = $state<SecretKind | "all">("all");
  let adding = $state(false);
  let form = $state({ name: "", place: "", date: "", note: "", hostId: null as number | null });
  const now = $derived.by(() => {
    void security.secrets;
    return Math.floor(Date.now() / 1000);
  });
  const shown = $derived(kind === "all" ? security.secrets : security.secrets.filter((s) => s.kind === kind));
  const sum = $derived(summary(security.secrets));
  const open = $derived(security.secrets.find((s) => s.key === security.secretOpen) ?? null);
  const kinds = $derived(KINDS.filter((k) => security.secrets.some((s) => s.kind === k.id)));

  const setNotices = (v: boolean) => settings.update({ security: { ...settings.value.security, rotationNotices: v } });
  const toSettings = (tab: SettingsTab) => {
    ui.settingsTab = tab;
    ui.go("settings");
  };

  /** Where this secret is changed, as a button label and an action. */
  function where(i: SecretItem): { label: string; go: () => void } | null {
    switch (i.kind) {
      case "sshKey":
        return { label: "SSH keys…", go: () => toSettings("connections") };
      case "vault":
        return {
          label: ".env vault…",
          go: () => {
            if (i.hostId != null) docker.select(i.hostId);
            docker.tab = "stacks";
            ui.go("docker");
          },
        };
      case "integration":
      case "expiry":
        return { label: "Services…", go: () => ui.go("services") };
      case "appToken":
        if (i.key === "app:mcp") return { label: "MCP settings…", go: () => toSettings("connections") };
        if (i.key === "app:status") return { label: "Status page…", go: () => toSettings("data") };
        return { label: "Notifications…", go: () => toSettings("alerts") };
      default:
        return null;
    }
  }

  function pick(i: SecretItem) {
    adding = false;
    security.secretOpen = i.key;
  }
  function startAdd(edit: SecretItem | null = null) {
    adding = true;
    form = edit
      ? { name: edit.name, place: edit.place, date: edit.rotatedAt ? new Date(edit.rotatedAt * 1000).toISOString().slice(0, 10) : "", note: edit.note, hostId: edit.hostId }
      : { name: "", place: "", date: "", note: "", hostId: null };
    if (!edit) security.secretOpen = null;
  }
  async function save() {
    const id = open ? manualId(open.key) : null;
    const rotatedAt = form.date ? Math.floor(new Date(form.date + "T12:00:00").getTime() / 1000) : null;
    await security.saveManual({ id, name: form.name, place: form.place, hostId: form.hostId, rotatedAt, note: form.note });
    if (!security.secretsError) adding = false;
  }
  function policyChange(i: SecretItem, v: string) {
    void security.setPolicy(i.key, v === "default" ? null : Number(v));
  }
  const fmt = (t: number | null) => (t ? new Date(t * 1000).toLocaleDateString() : "--");
</script>

<div class="secrets" data-guide="security-secrets">
  <div class="row">
    <button type="button" class="btn" onclick={() => security.loadSecrets()}>Refresh</button>
    <button type="button" class="btn ghost" onclick={() => startAdd()} data-guide="secrets-add">+ Add one kept elsewhere</button>
    <span class="muted small">Names and dates only: no value is read or shown here.</span>
    <span class="sp"></span>
    <Toggle label="Notify when one is due" checked={settings.value.security.rotationNotices} onchange={setNotices} />
  </div>
  {#if security.secretsError}<p class="error-text small">{security.secretsError}</p>{/if}

  <div class="cards">
    <div class="card"><span class="label">OVERDUE</span><span class="big data {sum.overdue ? 'bad' : ''}">{sum.overdue}</span><span class="muted small">past their rotation date</span></div>
    <div class="card"><span class="label">DUE IN 30 DAYS</span><span class="big data {sum.soon ? 'warn' : ''}">{sum.soon}</span><span class="muted small">plan these next</span></div>
    <div class="card"><span class="label">NO DATE</span><span class="big data">{sum.unknown}</span><span class="muted small">mark rotated to start the clock</span></div>
    <div class="card">
      <span class="label">HOW TO</span>
      <button type="button" class="link" onclick={() => library.open("howto/secret-rotation")}>Rotate secrets and certificates ▤</button>
      <span class="muted small">{sum.total} secret{sum.total === 1 ? "" : "s"} known</span>
    </div>
  </div>

  {#if kinds.length > 1}
    <div class="chips" role="group" aria-label="Kind">
      <button type="button" class:on={kind === "all"} onclick={() => (kind = "all")}>All</button>
      {#each kinds as k (k.id)}
        <button type="button" class:on={kind === k.id} onclick={() => (kind = k.id)}>{k.label}</button>
      {/each}
    </div>
  {/if}

  <div class="split">
    <div class="list">
      {#each shown as s (s.key)}
        <button type="button" class="srow" class:on={security.secretOpen === s.key} onclick={() => pick(s)}>
          <span class="st {stateTone[s.state]}">{stateLabel[s.state]}</span>
          <span class="sn">
            <span class="name">{s.name}</span>
            <span class="muted small">{kindLabel(s.kind)}{s.place ? ` · ${s.place}` : ""}</span>
          </span>
          <span class="due small {stateTone[s.state]}">{dueText(s, now)}</span>
        </button>
      {:else}
        <p class="muted small pad">{security.secretsLoaded ? "Nothing yet. Add a host, an SSH key or a vault value, or list one kept elsewhere." : "Reading…"}</p>
      {/each}
    </div>

    <div class="detail">
      {#if adding}
        <h3 class="hd">{open && manualId(open.key) != null ? "EDIT" : "ADD A SECRET KEPT ELSEWHERE"}</h3>
        <p class="muted small">A router password, a DNS API token, a recovery code. Describe it; never paste the value.</p>
        <label class="fld"><span class="label">NAME</span><input class="input" bind:value={form.name} placeholder="Router admin password" /></label>
        <label class="fld"><span class="label">WHERE IT'S USED</span><input class="input" bind:value={form.place} placeholder="router web UI" /></label>
        <label class="fld"
          ><span class="label">HOST</span>
          <select class="input" bind:value={form.hostId}>
            <option value={null}>none</option>
            {#each hosts.hosts as h (h.id)}<option value={h.id}>{h.name}</option>{/each}
          </select>
        </label>
        <label class="fld"><span class="label">LAST ROTATED</span><input class="input" type="date" bind:value={form.date} /></label>
        <label class="fld"><span class="label">NOTE</span><input class="input" bind:value={form.note} placeholder="kept in the password manager" /></label>
        <div class="row">
          <button type="button" class="btn primary" disabled={!form.name.trim()} onclick={save}>Save</button>
          <button type="button" class="btn ghost" onclick={() => (adding = false)}>Cancel</button>
        </div>
      {:else if open}
        {@const w = where(open)}
        {@const fp = fingerprintOf(open.key)}
        {@const mid = manualId(open.key)}
        {@const found = fp ? security.keyHosts[fp] : undefined}
        <div class="row">
          <span class="st {stateTone[open.state]}">{stateLabel[open.state]}</span>
          <span class="sn"><span class="name">{open.name}</span><span class="muted small">{kindLabel(open.kind)}{open.note ? ` · ${open.note}` : ""}</span></span>
        </div>
        <dl>
          <dt>Used by</dt><dd>{open.place || "--"}</dd>
          <dt>{open.kind === "expiry" ? "Expires" : "Due"}</dt><dd class={stateTone[open.state]}>{open.dueAt ? fmt(open.dueAt) : "--"} · {dueText(open, now)}</dd>
          {#if open.kind !== "expiry"}<dt>Last rotated</dt><dd>{fmt(open.rotatedAt)} · {ageText(open, now)}</dd>{/if}
        </dl>
        {#if open.kind !== "expiry"}
          <label class="fld inline"
            ><span class="label">ROTATE EVERY</span>
            <select class="input" value={open.policyDays == null ? "0" : String(open.policyDays)} onchange={(e) => policyChange(open, e.currentTarget.value)}>
              <option value="default">default</option>
              {#each POLICIES as d (d)}<option value={String(d)}>{policyLabel(d)}</option>{/each}
            </select>
          </label>
        {/if}
        <div class="row">
          {#if open.kind !== "expiry"}<button type="button" class="btn primary" onclick={() => security.markRotated(open.key)} data-guide="secrets-mark">Mark rotated today</button>{/if}
          {#if w}<button type="button" class="btn" onclick={w.go}>{w.label}</button>{/if}
          {#if mid != null}
            <button type="button" class="btn ghost" onclick={() => startAdd(open)}>Edit</button>
            <button type="button" class="btn ghost danger" onclick={() => security.deleteManual(mid)}>Remove</button>
          {/if}
        </div>
        {#if fp}
          <h3 class="hd">WHERE THIS KEY WORKS</h3>
          <p class="muted small">Before you remove an old key, push its replacement to every host that has it.</p>
          <button type="button" class="btn tiny" disabled={found === "loading"} onclick={() => security.findKey(fp)}>{found === "loading" ? "Reading authorized_keys…" : found ? "Read again" : "Find hosts that accept it"}</button>
          {#if Array.isArray(found)}
            {#each found as k (k.hostId)}
              <div class="hit small"><span class={k.error ? "muted" : k.authorized ? "ok" : "muted"}>{k.error ? "?" : k.authorized ? "✓" : "·"}</span><span>{k.name}</span><span class="muted">{k.error ?? (k.authorized ? "authorized" : "not there")}</span></div>
            {/each}
          {/if}
        {/if}
      {:else}
        <p class="muted pad">Pick a secret to see when it's due, set how often it should change, and jump to where it's changed.</p>
      {/if}
    </div>
  </div>
</div>

<style>
  .secrets {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .sp {
    flex: 1;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
    gap: 8px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 2px;
    border: 1px solid var(--line);
    padding: 8px 10px;
    align-items: flex-start;
  }
  .big {
    font-size: var(--fs-lg, 20px);
    color: var(--text-0);
  }
  .ok {
    color: var(--ok);
  }
  .warn {
    color: var(--warn);
  }
  .bad {
    color: var(--danger);
  }
  .muted {
    color: var(--text-2);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .chips button {
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-1);
    font-size: var(--fs-xs);
    padding: 2px 8px;
    cursor: pointer;
  }
  .chips button.on {
    color: var(--magenta);
    border-color: var(--magenta);
  }
  .split {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(280px, 3fr) 2fr;
    gap: 10px;
  }
  .list,
  .detail {
    overflow: auto;
    min-height: 0;
    border: 1px solid var(--line);
  }
  .detail {
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .srow {
    all: unset;
    box-sizing: border-box;
    width: 100%;
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 7px 10px;
    border-bottom: 1px solid var(--line);
    cursor: pointer;
  }
  .srow:hover,
  .srow:focus-visible,
  .srow.on {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }
  .srow.on {
    box-shadow: inset 2px 0 0 var(--magenta);
  }
  .st {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    letter-spacing: 0.08em;
    width: 76px;
    flex: none;
  }
  .sn {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .name {
    color: var(--text-0);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .due {
    flex: none;
    text-align: right;
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 3px 12px;
    margin: 0;
    font-size: var(--fs-sm, 13px);
  }
  dt {
    color: var(--text-2);
  }
  dd {
    margin: 0;
    color: var(--text-1);
  }
  .fld {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .fld.inline {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }
  .hd {
    font-family: var(--font-display);
    font-size: var(--fs-xs);
    color: var(--text-2);
    letter-spacing: 0.1em;
    margin: 8px 0 0;
  }
  .hit {
    display: grid;
    grid-template-columns: 16px 1fr auto;
    gap: 6px;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--cyan);
    font: inherit;
    cursor: pointer;
    text-align: left;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .pad {
    padding: 10px;
    margin: 0;
  }
  p {
    margin: 0;
  }
  @media (max-width: 900px) {
    .split {
      grid-template-columns: 1fr;
    }
  }
</style>
