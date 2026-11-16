<script lang="ts">
  /**
   * What depends on what (M10s): alerts downstream of an outage fold under
   * the alert for its cause, and only the cause notifies.
   */
  import { onMount } from "svelte";
  import { api, errorText, type AlertDependency, type DepNode } from "../../api";
  import { hosts } from "../../stores/hosts.svelte";
  import { services } from "../../stores/services.svelte";
  import { settings } from "../../stores/settings.svelte";
  import Panel from "../Panel.svelte";
  import Toggle from "../Toggle.svelte";

  const a = $derived(settings.value.alerts);
  let deps = $state<AlertDependency[]>([]);
  let child = $state("");
  let parent = $state("");
  let error = $state<string | null>(null);

  onMount(() => {
    if (!services.loaded) void services.refresh();
    void load();
  });

  async function load() {
    try {
      deps = await api.alertsDepsList();
    } catch (e) {
      error = errorText(e);
    }
  }

  const enc = (n: DepNode) => `${n.kind}:${n.id}`;
  const dec = (s: string): DepNode | null => {
    const [k, id] = s.split(":");
    return (k === "host" || k === "service") && id ? { kind: k, id: Number(id) } : null;
  };
  function label(n: DepNode): string {
    if (n.kind === "host") return hosts.hosts.find((h) => h.id === n.id)?.name ?? `host ${n.id}`;
    return services.services.find((s) => s.id === n.id)?.name ?? `service ${n.id}`;
  }

  async function add() {
    const c = dec(child);
    const p = dec(parent);
    if (!c || !p) return;
    error = null;
    try {
      await api.alertsDepsAdd(c, p);
      child = "";
      await load();
    } catch (e) {
      error = errorText(e);
    }
  }

  async function remove(id: number) {
    error = null;
    try {
      await api.alertsDepsDelete(id);
      await load();
    } catch (e) {
      error = errorText(e);
    }
  }

  /** Grouped by what they depend on, so a router with many dependents reads as one block. */
  const groups = $derived.by(() => {
    const out: { parent: DepNode; items: AlertDependency[] }[] = [];
    for (const d of deps) {
      const g = out.find((x) => enc(x.parent) === enc(d.parent));
      if (g) g.items.push(d);
      else out.push({ parent: d.parent, items: [d] });
    }
    return out.sort((x, y) => label(x.parent).localeCompare(label(y.parent)));
  });
  const onHost = $derived(services.services.filter((s) => s.hostId != null && s.checkEnabled));
</script>

<Panel title="DEPENDENCIES" guide="settings-deps">
  <Toggle
    label="Fold alerts under their cause"
    hint="When something is down, alerts for what depends on it are listed under its alert and don't notify on their own. Everything about a host already depends on that host."
    checked={a.foldDependent}
    disabled={!a.enabled}
    onchange={(v) => settings.update({ alerts: { ...settings.value.alerts, foldDependent: v } })}
  />
  <p class="muted small">
    Say what else breaks when something goes down: a media server that reads from a NAS, or everything behind the router (add the router as a host or a service check).
    {#if onHost.length}{onHost.length} checked service{onHost.length === 1 ? "" : "s"} already depend on their host.{/if}
  </p>

  {#if groups.length}
    <ul class="groups">
      {#each groups as g (enc(g.parent))}
        <li>
          <span class="parent"><span class="kind micro">{g.parent.kind === "host" ? "HOST" : "SVC"}</span> {label(g.parent)}</span>
          <span class="muted small">takes down</span>
          <span class="kids">
            {#each g.items as d (d.id)}
              <span class="kid">
                <span class="kind micro">{d.child.kind === "host" ? "HOST" : "SVC"}</span>
                {label(d.child)}
                <button type="button" class="x" title="Remove this dependency" aria-label="Remove: {label(d.child)} depends on {label(g.parent)}" onclick={() => void remove(d.id)}>×</button>
              </span>
            {/each}
          </span>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="muted small">No dependencies yet.</p>
  {/if}

  <div class="add">
    <select class="input" bind:value={child} aria-label="What breaks">
      <option value="">What breaks…</option>
      {#if hosts.hosts.length}<optgroup label="Hosts">{#each hosts.hosts as h (h.id)}<option value="host:{h.id}">{h.name}</option>{/each}</optgroup>{/if}
      {#if services.services.length}<optgroup label="Services">{#each services.services as s (s.id)}<option value="service:{s.id}">{s.name}</option>{/each}</optgroup>{/if}
    </select>
    <span class="muted small">depends on</span>
    <select class="input" bind:value={parent} aria-label="What it depends on">
      <option value="">What it needs…</option>
      {#if hosts.hosts.length}<optgroup label="Hosts">{#each hosts.hosts as h (h.id)}<option value="host:{h.id}">{h.name}</option>{/each}</optgroup>{/if}
      {#if services.services.length}<optgroup label="Services">{#each services.services as s (s.id)}<option value="service:{s.id}">{s.name}</option>{/each}</optgroup>{/if}
    </select>
    <button type="button" class="btn tiny" disabled={!child || !parent || child === parent} onclick={() => void add()}>Add</button>
  </div>
  {#if error}<p class="error small" role="alert">{error}</p>{/if}
</Panel>

<style>
  .small {
    font-size: var(--fs-xs);
    margin: 0;
  }
  .groups {
    list-style: none;
    margin: 6px 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .groups li {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: baseline;
  }
  .parent {
    color: var(--text-0);
  }
  .kids {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .kid {
    display: inline-flex;
    align-items: baseline;
    gap: 4px;
    padding: 1px 4px 1px 6px;
    border: 1px solid var(--line);
  }
  .kind {
    color: var(--cyan);
    letter-spacing: 0.08em;
  }
  .x {
    background: none;
    border: 0;
    color: var(--text-1);
    cursor: pointer;
    padding: 0 2px;
  }
  .x:hover {
    color: var(--magenta);
  }
  .add {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    margin-top: 6px;
  }
  .add select {
    min-width: 0;
    flex: 1 1 140px;
  }
  .error {
    color: var(--danger);
  }
</style>
